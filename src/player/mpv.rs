use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;
use std::time::Duration;
use regex::Regex;
use serde_json::{json, Value};
use log::info;
use crate::player::backend::Snapshot;

pub fn socket_path() -> PathBuf {
    dirs::runtime_dir().unwrap_or_else(std::env::temp_dir).join("absotui-mpv.sock")
}

pub fn mpv_command(socket: &Path, current_time: &str, source: &str, speed_rate: &str, title: &str) -> Command {
    let mut cmd = Command::new("mpv");
    // keep-open: a finished track (eof-reached) stays distinguishable from mpv being quit.
    cmd.arg("--no-video")
        .arg("--no-terminal")
        .arg("--idle=no")
        .arg("--keep-open=yes")
        .arg("--resume-playback=no")
        .arg("--save-position-on-quit=no")
        .arg("--ytdl=no")
        .arg("--volume-max=200")
        .arg(format!("--input-ipc-server={}", socket.display()))
        .arg(format!("--start={current_time}"))
        .arg(format!("--speed={speed_rate}"))
        .arg(format!("--force-media-title={title}"))
        .arg("--")
        .arg(source);
    cmd
}

pub struct MpvIpc {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
}

impl MpvIpc {
    pub fn connect(socket: &Path) -> io::Result<Self> {
        let writer = UnixStream::connect(socket)?;
        writer.set_read_timeout(Some(Duration::from_secs(1)))?;
        let reader = BufReader::new(writer.try_clone()?);
        Ok(Self { writer, reader })
    }

    pub fn command(&mut self, command: Value) -> io::Result<Option<Value>> {
        writeln!(self.writer, "{}", json!({ "command": command }))?;
        loop {
            let mut line = String::new();
            if self.reader.read_line(&mut line)? == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "mpv closed the IPC connection"));
            }
            let reply: Value = serde_json::from_str(&line)?;
            if reply.get("event").is_some() {
                continue;
            }
            return Ok((reply["error"] == "success").then(|| reply["data"].clone()));
        }
    }

    fn get_property(&mut self, name: &str) -> io::Result<Option<Value>> {
        self.command(json!(["get_property", name]))
    }
}

/// `None` once mpv's IPC socket is closed, ie. mpv isn't running.
pub fn poll_mpv(socket: &Path) -> Option<Snapshot> {
    let Ok(mut ipc) = MpvIpc::connect(socket) else {
        info!("[poll_mpv] mpv is not running (IPC socket is closed).");
        return None;
    };
    let position = ipc.get_property("time-pos").ok().flatten()
        .and_then(|position| position.as_f64())
        .filter(|&seconds| seconds >= 1.0)
        .map(|seconds| seconds as u32);
    let is_at_end = matches!(ipc.get_property("eof-reached"), Ok(Some(Value::Bool(true))));
    Some(Snapshot { position, is_at_end })
}

pub fn quit_mpv(socket: &Path) -> io::Result<()> {
    MpvIpc::connect(socket)?.command(json!(["quit"])).map(drop)
}

pub fn get_mpv_version() -> io::Result<String> {
    let output = Command::new("mpv").arg("--version").output()?;
    let version_output = str::from_utf8(&output.stdout)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let re = Regex::new(r"mpv v?([\d.]+)").unwrap();
    re.captures(version_output)
        .and_then(|captures| captures.get(1))
        .map(|version| version.as_str().to_string())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Could not extract mpv version"))
}
