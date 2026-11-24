use std::io;
use std::process::Command;
use std::process::Output;
use log::error;
use crate::player::backend::{Player, Vlc};
use crate::player::mpv::{check_socket_path, mpv_command};
use crate::player::session::{Track, TrackSource};
use crate::player::vlc::exec_nc::exec_nc;
use crate::db::crud::{get_speed_rate, get_is_per_item_speed, get_item_speed_rate, set_item_speed_rate};

// Launch speed: the item's own if Per-Item Speed is on (1.0x until first adjusted), else the shared rate.
// `id_item` is a book's id or a podcast show's id.
fn resolve_speed_rate(username: &str, id_item: &str) -> String {
    if get_is_per_item_speed(username) != "1" {
        return get_speed_rate(username);
    }

    if let Some(rate) = get_item_speed_rate(username, id_item) {
        return format!("{rate:.1}");
    }

    let _ = set_item_speed_rate(username, id_item, 1.0);
    "1.0".to_string()
}

impl Player {
    pub async fn start(&self, track: Track, username: String) -> io::Result<Output> {
        let speed_rate = resolve_speed_rate(username.as_str(), track.id_item.as_str());

        // Downloaded items play straight from the local file (src/utils/download_cache.rs)
        // regardless of server reachability. Otherwise streams from the session-scoped
        // `/public/session/:id/track/:index` endpoint, which needs no token at all (unlike
        // the old `?token=...` approach - see known_bugs.md bug_id b81f4a) - confirmed
        // against the server's own source that a live session id is all it checks.
        let source = match track.source {
            TrackSource::Local(path) => path,
            TrackSource::Stream { server_address, id_session, track_index } => format!("{server_address}/public/session/{id_session}/track/{track_index}"),
        };
        let start = track.start.to_string();

        if let Player::Vlc(vlc) = self && vlc.opens_control_terminal {
            let port = vlc.port.clone();
            let address = vlc.address.clone();
            tokio::spawn(async move {
                if let Err(e) = exec_nc(&port, address).await {
                    error!("[start][exec_nc] {e}");
                }
            });
        }

        if let Player::Mpv { socket } = self {
            check_socket_path(socket)?;
        }

        let mut cmd = match self {
            Player::Mpv { socket } => mpv_command(socket, &start, &source, &speed_rate, &track.subtitle),
            Player::Vlc(vlc) => vlc_command(vlc, &start, source, speed_rate, track.author, track.subtitle, track.title),
        };

        // The player runs for the whole track, so `output()` blocks: run it on the blocking pool, not a runtime
        // worker. A launch failure is logged rather than panicking (the session notices it never came up).
        tokio::task::spawn_blocking(move || {
            cmd.output()
                .inspect_err(|e| error!("[start] Failed to execute {}: {e}", cmd.get_program().display()))
        })
        .await
        .unwrap_or_else(|e| Err(io::Error::other(format!("start blocking task panicked: {e}"))))
    }
}

fn vlc_command(
    vlc: &Vlc,
    current_time: &str,
    source: String,
    speed_rate: String,
    author: String,
    subtitle: String,
    title: String,
) -> Command {
    let mut cmd = Command::new(&vlc.program);
    // `-I dummy` suppresses VLC's normal interface so it runs headless, controlled
    // purely through the RC connection below. Gated on `is_headless` (cvlc = "1")
    // rather than `program`, since macOS has no separate cvlc binary - that's what
    // makes `cvlc = "0"` show a real VLC window there (see bug_id fe4116).
    if vlc.is_headless {
        cmd.arg("-I").arg("dummy");
    }
    cmd.arg(format!("--start-time={current_time}"))
        .arg("--extraintf")
        .arg("rc")
        .arg("--rc-host")
        .arg(vlc.endpoint())
        .arg(source)
        .arg("--rate")
        .arg(speed_rate)
        .arg("--meta-description")
        .arg(author)
        .arg("--meta-title")
        .arg(subtitle)
        .arg("--meta-artist")
        .arg(title);
    cmd
}

