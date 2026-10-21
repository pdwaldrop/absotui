use std::io;
use std::net::TcpStream;
use vlc_rc::Client;
use std::process::Command;
use std::str;
use regex::Regex;
use log::{info, warn};
use crate::player::backend::Snapshot;

/// `None` once VLC's RC port is closed, ie. VLC isn't running.
pub fn poll_vlc(address: &str, port: &str) -> Option<Snapshot> {
    let endpoint = format!("{address}:{port}");
    if TcpStream::connect(&endpoint).is_err() {
        info!("[poll_vlc] VLC is not running (port {port} is closed).");
        return None;
    }

    let mut client = match Client::connect(&endpoint) {
        Ok(client) => client,
        Err(e) => {
            warn!("[poll_vlc] RC port is open but not answering yet: {e}");
            return Some(Snapshot::default());
        }
    };
    let position = client.get_time().ok().flatten().filter(|&seconds| seconds > 0);
    // `is_playing` drops to 0 once the track has ended while VLC itself stays open.
    let is_at_end = matches!(client.is_playing(), Ok(false));
    Some(Snapshot { position, is_at_end })
}

pub async fn get_vlc_version(program: &str) -> Result<String, io::Error> {
    let output = Command::new(program)
        .arg("--version")
        .output()?;

    let version_output = str::from_utf8(&output.stdout)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?; 

    let re = Regex::new(r"VLC (?:media player |version )?([\d.]+)").unwrap();

    if let Some(captures) = re.captures(version_output)
        && let Some(version) = captures.get(1) {
            return Ok(version.as_str().to_string());
        }

    Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Could not extract VLC version",
    ))
}
