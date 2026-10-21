use std::io;
use crate::player::backend::Player;
use crate::db::crud::{get_listening_session, update_is_playback, update_speed_rate, get_speed_rate, update_volume_up, update_volume_down, get_is_per_item_speed, get_item_speed_rate, set_item_speed_rate, update_item_speed_rate, update_pending_seek};
use crate::api::libraries::get_library_perso_view_pod::Chapter;

// Index of the chapter containing book-wide `time` (seconds) - same rule as
// find_current_chapter (player_info.rs), just returning a position rather than a
// reference so callers can index the previous/next chapter.
fn current_chapter_idx(chapters: &[Chapter], time: f64) -> Option<usize> {
    chapters.iter().position(|c| {
        let start = c.start.unwrap_or(0.0);
        let end = c.end.unwrap_or(f64::MAX);
        time >= start && time < end
    })
}

pub fn handle_key_player(key: &str, player: &Player, username: &str) -> io::Result<()> {
    if !player.is_reachable() {
        return Err(io::Error::new(io::ErrorKind::NotConnected, "the player is not running"));
    }

    let jump = 10;

    match key {
        // toggle playback/pause
        " " => {
            match get_listening_session() {
                Ok(Some(session)) => {
                    if session.is_playback {
                    let _ = update_is_playback("0", session.id_session.as_str());
                    } else {
                    let _ = update_is_playback("1", session.id_session.as_str());
                    }
                }
                Ok(None) => {

                }
                Err(_e) => {

                }
            }
            player.toggle_pause()?;
        }

        // jump forward
        "p" => {
            player.seek_relative(jump)?;
        }
        // jump backward
        "u" => {
            player.seek_relative(-jump)?;
        }
        // next chapter - resolved from Absotui's own chapters[] data (via pending_seek,
        // see update_pending_seek's doc comment) rather than VLC's native chapter_n,
        // since a book split across several files has no chapters embedded in any one
        // file for VLC to jump between.
        "P" => {
            if let Ok(Some(session)) = get_listening_session() {
                let chapters: Vec<Chapter> = serde_json::from_str(&session.chapters).unwrap_or_default();
                if let Some(idx) = current_chapter_idx(&chapters, f64::from(session.current_time))
                    && let Some(start) = chapters.get(idx + 1).and_then(|c| c.start) {
                        let _ = update_pending_seek(&(start.ceil() as u32).to_string(), &session.id_session);
                }
            }
        }
        // previous chapter - see "P" above.
        "U" => {
            if let Ok(Some(session)) = get_listening_session() {
                let chapters: Vec<Chapter> = serde_json::from_str(&session.chapters).unwrap_or_default();
                if let Some(idx) = current_chapter_idx(&chapters, f64::from(session.current_time)) {
                    let target = if idx == 0 { chapters.first() } else { chapters.get(idx - 1) };
                    if let Some(start) = target.and_then(|c| c.start) {
                        let _ = update_pending_seek(&(start.ceil() as u32).to_string(), &session.id_session);
                    }
                }
            }
        }
        // volume up
        "o" => {
            if let Ok(Some(session)) = get_listening_session() {
                let _ = update_volume_up(session.id_session.as_str());
            }
            player.change_volume(true)?;
        }
        // volume down
        "i" => {
            if let Ok(Some(session)) = get_listening_session() {
                let _ = update_volume_down(session.id_session.as_str());
            }
            player.change_volume(false)?;
        }
        // speed rate up
        "O" => {
            let speed_rate = adjust_speed_rate(username, true);
            player.set_speed(&speed_rate)?;
        }
        // speed rate down
        "I" => {
            let speed_rate = adjust_speed_rate(username, false);
            player.set_speed(&speed_rate)?;
        }
        // stop playback - shuts down the player itself, not the app
        "stop" => {
            player.quit()?;
        }
        _ => {}
    }

    Ok(())
}

// Adjusts and returns the effective speed rate: per-item (keyed by the active
// session's id_item) if Settings > Per-Item Speed is on, otherwise the single shared
// speed_rate - same resolution Player::start uses when launching a new session. Seeds the
// item's row at a fixed 1.0x baseline on first touch (not the current global rate -
// see update_is_per_item_speed/resolve_speed_rate), whether that's because this is the
// item's first playback under per-item mode, or because per-item mode was just turned
// on mid-session for an item that hasn't been touched yet.
fn adjust_speed_rate(username: &str, is_up: bool) -> String {
    if get_is_per_item_speed(username) == "1"
        && let Ok(Some(session)) = get_listening_session() {
            if get_item_speed_rate(username, &session.id_item).is_none() {
                let _ = set_item_speed_rate(username, &session.id_item, 1.0);
            }
            let _ = update_item_speed_rate(username, &session.id_item, is_up);
            if let Some(rate) = get_item_speed_rate(username, &session.id_item) {
                return format!("{rate:.1}");
            }
    }
    let _ = update_speed_rate(username, is_up);
    get_speed_rate(username)
}

