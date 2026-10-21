use std::net::TcpStream;
use std::io::{self, Write};
use log::{info, error};
use std::process::Command;

pub fn quit_vlc(address: &str, port: &str) -> io::Result<()> {
    let mut stream = TcpStream::connect(format!("{address}:{port}"))?;

    writeln!(stream, "shutdown")?;

    info!("[quit_vlc.rs] VLC successfully quit");

    Ok(())
}

// need also to pkill VLC for macos - otherwise, issue:
// impossible to launch another track in the same session.
pub fn pkill_vlc() {
    if cfg!(target_os = "macos") {  
        match Command::new("pkill").arg("VLC").status() {
            Ok(status) if status.success() => info!("VLC pkill success"),
            Ok(_) => error!("VLC pkill error"),
            Err(e) => error!("[pkill_vlc] Failed to execute pkill: {e}"),
        }
    } 
}

