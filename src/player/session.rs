use std::io;
use std::time::Duration;
use std::process::Output;
use log::{error, info, warn};
use tokio::task::JoinHandle;
use tokio::time::{sleep, sleep_until, Instant};
use crate::player::backend::Player;

const POLL_INTERVAL: Duration = Duration::from_secs(1);
const LOADING_POLL_INTERVAL: Duration = Duration::from_millis(150);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const REACHABILITY_CHECK_INTERVAL: Duration = Duration::from_millis(100);

// The player's last words, for the log - e.g. why it quit right after starting.
fn stderr_tail(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim().replace('\n', " | ");
    let start = stderr.char_indices().rev().nth(299).map_or(0, |(i, _)| i);
    if stderr.is_empty() { String::new() } else { format!(": {}", stderr[start..].trim_start()) }
}

pub enum TrackSource {
    Stream { server_address: String, id_session: String, track_index: i64 },
    Local(String),
}

pub struct Track {
    pub source: TrackSource,
    pub start: u32,
    pub title: String,
    pub subtitle: String,
    pub author: String,
    // A book's id, or a podcast show's own id - keys the per-item speed.
    pub id_item: String,
}

pub enum PlaybackStatus {
    Playing(u32),
    // No position to report yet (still opening, or paused in its first second) - lets the
    // playback loop handle chapter jumps, token refresh and Mark Finished meanwhile.
    Waiting,
    Finished,
    Closed,
}

pub struct PlayerSession {
    player: Player,
    username: String,
    is_up: bool,
    has_track_started: bool,
    last_poll: Option<Instant>,
    track_loaded_at: Instant,
}

impl PlayerSession {
    pub async fn launch(player: Player, username: String, track: Track) -> Self {
        let mut session = Self { player, username, is_up: false, has_track_started: false, last_poll: None, track_loaded_at: Instant::now() };
        session.start_track(track).await;
        session
    }

    pub async fn switch_track(&mut self, track: Track) {
        self.start_track(track).await;
    }

    pub fn seek_to(&self, seconds: u32) -> io::Result<()> {
        self.player.seek_absolute(seconds)
    }

    pub fn seek_resumes_playback(&self) -> bool {
        self.player.seek_resumes_playback()
    }

    pub fn quit(&self) -> io::Result<()> {
        self.player.quit()
    }

    pub fn quit_gracefully(&self) -> io::Result<()> {
        self.player.quit_gracefully()
    }

    // Until the current track reports a real position, a missing one means it's still
    // opening (or paused before its first second) - never passed on as Playing(0), and
    // "at end" only counts as Finished once the track has started. Polled quickly while a
    // track opens, and Waiting is returned about once a second so the caller isn't stuck.
    pub async fn next_status(&mut self) -> PlaybackStatus {
        if !self.is_up {
            return PlaybackStatus::Closed;
        }
        if let Some(last_poll) = self.last_poll {
            sleep_until(last_poll + self.poll_interval()).await;
        }
        let wait_until = Instant::now() + POLL_INTERVAL;
        loop {
            let snapshot = self.player.poll();
            self.last_poll = Some(Instant::now());
            let Some(snapshot) = snapshot else {
                return PlaybackStatus::Closed;
            };
            if snapshot.is_at_end && self.has_track_started {
                return PlaybackStatus::Finished;
            }
            if let Some(position) = snapshot.position {
                self.has_track_started = true;
                return PlaybackStatus::Playing(position);
            }
            if Instant::now() >= wait_until {
                return PlaybackStatus::Waiting;
            }
            sleep(self.poll_interval()).await;
        }
    }

    fn poll_interval(&self) -> Duration {
        let still_opening = !self.has_track_started && self.track_loaded_at.elapsed() < STARTUP_TIMEOUT;
        if still_opening { LOADING_POLL_INTERVAL } else { POLL_INTERVAL }
    }

    // Whatever still holds the control port/socket (the previous track, or a player
    // left over from a crash) has to be gone first, or the new player can't take it.
    async fn start_track(&mut self, track: Track) {
        let _ = self.player.quit();
        self.wait_until_gone().await;

        self.has_track_started = false;
        self.last_poll = None;
        self.track_loaded_at = Instant::now();
        let player = self.player.clone();
        let username = self.username.clone();
        let process = tokio::spawn(async move {
            match player.start(track, username).await {
                Ok(output) if !output.status.success() => warn!("[PlayerSession] Player exited abnormally ({}){}", output.status, stderr_tail(&output)),
                Ok(_) => {}
                Err(e) => error!("[PlayerSession] {e}"),
            }
        });

        self.is_up = self.wait_until_up(&process).await;
        if self.is_up {
            info!("[PlayerSession] Player is up");
        } else {
            error!("[PlayerSession] Player didn't come up");
        }
    }

    async fn wait_until_gone(&self) {
        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        while self.player.is_reachable() && Instant::now() < deadline {
            sleep(REACHABILITY_CHECK_INTERVAL).await;
        }
    }

    async fn wait_until_up(&self, process: &JoinHandle<()>) -> bool {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        while Instant::now() < deadline {
            if self.player.is_reachable() {
                return true;
            }
            if process.is_finished() {
                return false;
            }
            sleep(REACHABILITY_CHECK_INTERVAL).await;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::process::ExitStatus;

    // Answers mpv's IPC like a player sitting at `time_pos`.
    fn fake_mpv(name: &str, time_pos: f64) -> PathBuf {
        let socket = std::env::temp_dir().join(format!("absotui-fake-mpv-{}-{name}.sock", std::process::id()));
        let _ = std::fs::remove_file(&socket);
        let listener = UnixListener::bind(&socket).unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut writer = stream.try_clone().unwrap();
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    let reply = if line.contains("time-pos") {
                        format!("{{\"data\":{time_pos},\"error\":\"success\"}}")
                    } else {
                        "{\"data\":false,\"error\":\"success\"}".to_string()
                    };
                    if writeln!(writer, "{reply}").is_err() {
                        break;
                    }
                }
            }
        });
        socket
    }

    fn session_on(socket: PathBuf, has_track_started: bool) -> PlayerSession {
        PlayerSession {
            player: Player::Mpv { socket },
            username: String::new(),
            is_up: true,
            has_track_started,
            last_poll: None,
            track_loaded_at: Instant::now(),
        }
    }

    #[tokio::test]
    async fn a_player_paused_in_its_first_second_reports_waiting_instead_of_hanging() {
        let mut session = session_on(fake_mpv("paused-early", 0.4), false);
        let status = tokio::time::timeout(Duration::from_secs(3), session.next_status()).await;
        assert!(matches!(status, Ok(PlaybackStatus::Waiting)));
    }

    #[tokio::test]
    async fn a_started_track_paused_at_zero_reports_waiting_too() {
        let mut session = session_on(fake_mpv("paused-at-zero", 0.2), true);
        let status = tokio::time::timeout(Duration::from_secs(3), session.next_status()).await;
        assert!(matches!(status, Ok(PlaybackStatus::Waiting)));
    }

    #[tokio::test]
    async fn a_real_position_is_reported_as_playing() {
        let mut session = session_on(fake_mpv("playing", 42.7), false);
        let status = tokio::time::timeout(Duration::from_secs(3), session.next_status()).await;
        assert!(matches!(status, Ok(PlaybackStatus::Playing(42))));
        assert!(session.has_track_started);
    }

    fn output_with_stderr(stderr: &str) -> Output {
        Output { status: ExitStatus::from_raw(2 << 8), stdout: Vec::new(), stderr: stderr.as_bytes().to_vec() }
    }

    #[test]
    fn stderr_tail_is_empty_when_the_player_said_nothing() {
        assert_eq!(stderr_tail(&output_with_stderr("  \n")), "");
    }

    #[test]
    fn stderr_tail_keeps_a_short_message_whole() {
        assert_eq!(stderr_tail(&output_with_stderr("could not open device\n")), ": could not open device");
    }

    #[test]
    fn stderr_tail_puts_a_multi_line_message_on_one_line() {
        assert_eq!(stderr_tail(&output_with_stderr("first\nsecond\n")), ": first | second");
    }

    #[test]
    fn stderr_tail_keeps_only_the_end_of_a_long_message() {
        let long = format!("{}END", "x".repeat(1000));
        let tail = stderr_tail(&output_with_stderr(&long));
        assert!(tail.ends_with("END"));
        assert!(tail.len() <= 303, "{}", tail.len());
    }
}
