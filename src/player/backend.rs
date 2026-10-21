use std::io::{self, Write};
use std::net::TcpStream;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use log::error;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::config;
use crate::player::mpv::{self, MpvIpc};
use crate::player::vlc::fetch_vlc_data::{get_vlc_version, poll_vlc};
use crate::player::vlc::quit_vlc::{pkill_vlc, quit_vlc};

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    #[default]
    Vlc,
    Mpv,
}

#[derive(Debug, Clone)]
pub struct Vlc {
    pub program: String,
    pub is_headless: bool,
    pub opens_control_terminal: bool,
    pub address: String,
    pub port: String,
}

impl Vlc {
    pub fn endpoint(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}

/// What a running player reports in one poll. `position` is `None` until it has a
/// real one (above 0 s).
#[derive(Debug, Default)]
pub struct Snapshot {
    pub position: Option<u32>,
    pub is_at_end: bool,
}

#[derive(Debug, Clone)]
pub enum Player {
    Vlc(Vlc),
    Mpv { socket: PathBuf },
}

impl Player {
    pub fn from_config(config: &config::Player) -> Self {
        match config.backend {
            Backend::Mpv => Self::Mpv { socket: mpv::socket_path() },
            Backend::Vlc => Self::Vlc(Vlc {
                program: vlc_program(&config.cvlc),
                is_headless: config.cvlc == "1",
                opens_control_terminal: config.cvlc_term == "1",
                address: config.address.clone(),
                port: config.port.clone(),
            }),
        }
    }

    pub fn is_reachable(&self) -> bool {
        match self {
            Self::Vlc(vlc) => TcpStream::connect(vlc.endpoint()).is_ok(),
            Self::Mpv { socket } => UnixStream::connect(socket).is_ok(),
        }
    }

    // VLC's RC "shutdown" can silently fail to land, hence the pkill fallback.
    pub fn quit(&self) -> io::Result<()> {
        match self {
            Self::Vlc(vlc) => {
                let result = quit_vlc(&vlc.address, &vlc.port);
                pkill_vlc();
                result
            }
            Self::Mpv { socket } => mpv::quit_mpv(socket),
        }
    }

    /// `None` when the player isn't running.
    pub fn poll(&self) -> Option<Snapshot> {
        match self {
            Self::Vlc(vlc) => poll_vlc(&vlc.address, &vlc.port),
            Self::Mpv { socket } => mpv::poll_mpv(socket),
        }
    }

    pub async fn media_player_label(&self) -> String {
        let (name, version) = match self {
            Self::Vlc(vlc) => ("VLC", get_vlc_version(&vlc.program).await),
            Self::Mpv { .. } => ("mpv", mpv::get_mpv_version()),
        };
        let version = version.unwrap_or_else(|e| {
            error!("[media_player_label] {e}");
            String::new()
        });
        format!("{name} v{version}")
    }

    pub fn toggle_pause(&self) -> io::Result<()> {
        self.control(|stream| writeln!(stream, "pause"), json!(["cycle", "pause"]))
    }

    pub fn seek_relative(&self, seconds: i32) -> io::Result<()> {
        self.control(|stream| vlc_seek(stream, &format!("{seconds:+}")), json!(["seek", seconds, "relative"]))
    }

    pub fn seek_absolute(&self, seconds: u32) -> io::Result<()> {
        self.control(|stream| vlc_seek(stream, &seconds.to_string()), json!(["seek", seconds, "absolute"]))
    }

    pub fn set_speed(&self, speed_rate: &str) -> io::Result<()> {
        self.control(|stream| writeln!(stream, "rate {speed_rate}"), json!(["set", "speed", speed_rate]))
    }

    pub fn change_volume(&self, is_up: bool) -> io::Result<()> {
        self.control(
            |stream| writeln!(stream, "{}", if is_up { "volup" } else { "voldown" }),
            json!(["add", "volume", if is_up { 5 } else { -5 }]),
        )
    }

    fn control(&self, vlc_commands: impl FnOnce(&mut TcpStream) -> io::Result<()>, mpv_command: Value) -> io::Result<()> {
        match self {
            Self::Vlc(vlc) => vlc_commands(&mut TcpStream::connect(vlc.endpoint())?),
            Self::Mpv { socket } => MpvIpc::connect(socket)?.command(mpv_command).map(drop),
        }
    }
}

// On Linux, `cvlc = "1"` selects the actual `cvlc` binary. macOS has no separate cvlc
// binary (VLC.app is the only one), so whether it runs headless is decided purely by
// `is_headless` in vlc_command (see bug_id fe4116).
fn vlc_program(cvlc: &str) -> String {
    if cfg!(target_os = "macos") {
        "/Applications/VLC.app/Contents/MacOS/VLC".to_string()
    } else if cvlc == "1" {
        "cvlc".to_string()
    } else {
        "vlc".to_string()
    }
}

// Without pause => seek => play, VLC's buffer can freeze the player.
fn vlc_seek(stream: &mut TcpStream, target: &str) -> io::Result<()> {
    writeln!(stream, "pause")?;
    writeln!(stream, "seek {target}")?;
    if cfg!(target_os = "macos") {
        thread::sleep(Duration::from_millis(500));
    }
    writeln!(stream, "play")
}
