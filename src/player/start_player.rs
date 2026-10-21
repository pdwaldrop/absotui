use std::io;
use std::process::Command;
use std::process::Output;
use log::error;
use crate::player::backend::{Player, Vlc};
use crate::player::mpv::mpv_command;
use crate::player::session::{Track, TrackSource};
use crate::player::vlc::exec_nc::exec_nc;
use crate::db::crud::{get_speed_rate, get_is_per_item_speed, get_item_speed_rate, set_item_speed_rate};

// Resolves the playback speed to launch the player with: per-item if Settings > Per-Item
// Speed is on, otherwise the single shared speed_rate - unchanged from before this
// setting existed. `id_item` is a book's id, or a podcast show's own id (shared across
// its episodes). A per-item item that hasn't been touched yet always starts at a fixed
// 1.0x baseline, not whatever the shared speed happens to be - turning Per-Item Speed
// on already wipes every existing per-item row for exactly this reason (see
// update_is_per_item_speed), so this only matters for an item played for the first
// time since then.
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

        let mut cmd = match self {
            Player::Mpv { socket } => mpv_command(socket, &start, &source, &speed_rate, &track.subtitle),
            Player::Vlc(vlc) => vlc_command(vlc, &start, source, speed_rate, track.author, track.subtitle, track.title),
        };

        // Called from PlayerSession's own spawned task - a panic here (the old .expect())
        // would silently kill that task with no user-visible error if the player binary
        // is missing or misconfigured. The session does notice the player never came up
        // and closes playback either way, but logging the actual cause here beats a
        // silent no-op.
        //
        // The player is a long-running process - `Command::output()` blocks the calling thread
        // until the child itself exits, ie. for the length of the whole track, not just
        // until it launches. Left inline, that blocking call can starve whichever tokio
        // worker thread happens to run it, stalling *other* tasks pinned to that same
        // thread (confirmed live: a sibling task's plain `tokio::time::sleep` never woke
        // up again while this ran directly inside a spawned async task, mid-playback,
        // when switching a multi-file book to a different chapter's file).
        // `spawn_blocking` moves it onto tokio's separate blocking-task thread pool
        // instead, which doesn't participate in polling/timers for anything else.
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

