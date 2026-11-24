use crate::db::crud::{get_is_vlc_launched_first_time, get_is_loop_break, update_is_loop_break, update_is_vlc_launched_first_time, try_claim_playback_slot};
use log::info;
use crate::utils::pop_up_message::{pop_message, clear_message, NEEDS_TERMINAL_CLEAR};
use std::io::stdout;
use std::sync::atomic::{AtomicUsize, Ordering};

static PLAYBACK_TASKS: AtomicUsize = AtomicUsize::new(0);

/// Held by a playback task for as long as it runs, so quitting can tell this process's
/// own playback from another Absotui's.
pub struct PlaybackTask(());

impl PlaybackTask {
    fn start() -> Self {
        PLAYBACK_TASKS.fetch_add(1, Ordering::SeqCst);
        Self(())
    }
}

impl Drop for PlaybackTask {
    fn drop(&mut self) {
        PLAYBACK_TASKS.fetch_sub(1, Ordering::SeqCst);
    }
}

pub fn owns_playback() -> bool {
    PLAYBACK_TASKS.load(Ordering::SeqCst) > 0
}

// Waits until the previous playback task has closed its session and released the slot, so tracks can't
// overlap. Returns the slot to hold while playing; `None` means don't start.
pub async fn wait_prev_session_finished(username: String) -> Option<PlaybackTask> {

    let message = "Syncing your last listening session. Please wait...";
    let mut stdout = stdout();

        let is_vlc_first_launch = get_is_vlc_launched_first_time(&username);
        info!("[AppView::Home][is_vlc_first_launch]{is_vlc_first_launch}");
        let was_first_launch = is_vlc_first_launch == "1";

        if !was_first_launch {
            // Atomic claim, retried for up to ~20 s while the previous task tears down. Still taken after that means
            // another start won a race and is playing - give up rather than start a second player.
            let mut claimed = false;
            for _ in 0..20 {
                if try_claim_playback_slot(&username) {
                    claimed = true;
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                info!("[AppView::Home][loop][is_loop_break]");
                let _ = pop_message(&mut stdout, 3, message);
            }

            if !claimed {
                info!("[AppView::Home][wait_prev_session_finished] Gave up waiting for the playback slot - it's already owned by another in-flight playback-start attempt");
                let _ = clear_message(&mut stdout, 3);
                NEEDS_TERMINAL_CLEAR.store(true, Ordering::Relaxed);
                return None;
            }
        } else {
            // First launch ever for this account: is_loop_break starts at "0" (see
            // auth_process.rs), so the claim above would never succeed here - nothing
            // to wait on yet, just land on the same "0" end state directly.
            let _ = update_is_loop_break("0", &username);
        }
        let value = get_is_loop_break(username.as_str());
        info!("[AppView::Home][update_is_loop_break]{value}");
        let _ = update_is_vlc_launched_first_time("0", &username);
        let value = get_is_vlc_launched_first_time(username.as_str());
        info!("[AppView::Home][update_is_vlc_first_launch]{value}");

        let _ = clear_message(&mut stdout, 3);

        // Repaint fully: this runs in a spawned task that printed to the terminal directly.
        NEEDS_TERMINAL_CLEAR.store(true, Ordering::Relaxed);

        Some(PlaybackTask::start())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_is_owned_while_a_task_holds_it() {
        assert!(!owns_playback());
        let first = PlaybackTask::start();
        let second = PlaybackTask::start();
        assert!(owns_playback());
        drop(first);
        assert!(owns_playback());
        drop(second);
        assert!(!owns_playback());
    }
}
