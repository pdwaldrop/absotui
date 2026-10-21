use std::io;
use std::time::Duration;
use std::process::Output;
use log::{error, info, warn};
use tokio::task::JoinHandle;
use tokio::time::{sleep, sleep_until, Instant};
use crate::player::backend::Player;

const POLL_INTERVAL: Duration = Duration::from_secs(1);
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
    Finished,
    Closed,
}

pub struct PlayerSession {
    player: Player,
    username: String,
    is_up: bool,
    has_track_started: bool,
    last_poll: Option<Instant>,
}

impl PlayerSession {
    pub async fn launch(player: Player, username: String, track: Track) -> Self {
        let mut session = Self { player, username, is_up: false, has_track_started: false, last_poll: None };
        session.start_track(track).await;
        session
    }

    pub async fn switch_track(&mut self, track: Track) {
        self.start_track(track).await;
    }

    pub fn seek_to(&self, seconds: u32) -> io::Result<()> {
        self.player.seek_absolute(seconds)
    }

    pub fn quit(&self) -> io::Result<()> {
        self.player.quit()
    }

    // Waits out the loading phase: until the current track reports a real position, a
    // missing one just means it's still opening - so it's never passed on as
    // Playing(0), and "at end" only counts as Finished once the track has started.
    pub async fn next_status(&mut self) -> PlaybackStatus {
        if !self.is_up {
            return PlaybackStatus::Closed;
        }
        if let Some(last_poll) = self.last_poll {
            sleep_until(last_poll + POLL_INTERVAL).await;
        }
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
            sleep(POLL_INTERVAL).await;
        }
    }

    // Whatever still holds the control port/socket (the previous track, or a player
    // left over from a crash) has to be gone first, or the new player can't take it.
    async fn start_track(&mut self, track: Track) {
        let _ = self.player.quit();
        self.wait_until_gone().await;

        self.has_track_started = false;
        self.last_poll = None;
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
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

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
