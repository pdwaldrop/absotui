use crate::db::crud::{get_is_vlc_launched_first_time, get_is_loop_break, update_is_loop_break, update_is_vlc_launched_first_time, try_claim_playback_slot};
use log::info;
use crate::utils::pop_up_message::{pop_message, clear_message, NEEDS_TERMINAL_CLEAR};
use std::io::stdout;
use std::sync::atomic::Ordering;

// Blocks until the previously-running playback task (if any) has finished its own
// close/sync and released the playback slot, so a new track can't start while the old
// one is still tearing down. Returns whether the slot was actually claimed - `false`
// means the caller must NOT proceed to start a track (see below).
//
// Callers deliberately do NOT follow this with a defensive close of whatever's left in
// `listening_session`. Every normal exit path (Q, natural end-of-track, VLC-quit
// detection) closes its own session before returning, so the only way that row can be
// stale-and-never-closed is an abnormal exit (crash/kill) - and `main.rs` now closes
// that case once at startup instead. Running it here as well closed the session twice
// on every track switch (confirmed live 2026-07-28: once from the previous track's own
// quit-detection loop, once from the caller) - the concrete root cause behind bug_id
// dd9a649.
pub fn wait_prev_session_finished(username: String) -> bool {

    let message = "Syncing your last listening session. Please wait...";
    let mut stdout = stdout();

        let is_vlc_first_launch = get_is_vlc_launched_first_time(&username);
        info!("[AppView::Home][is_vlc_first_launch]{is_vlc_first_launch}");
        let was_first_launch = is_vlc_first_launch == "1";

        if !was_first_launch {
            // Loops trying to atomically claim the slot (flip is_loop_break 1->0)
            // instead of a separate check-then-set - two "l" presses close enough
            // together could otherwise both observe is_loop_break=="1" before either
            // wrote "0", and both proceed to start a VLC session at once. Once this
            // call succeeds, is_loop_break is already "0" - no separate update needed.
            //
            // Bounded to ~20s (same cap `quit_app` already uses in
            // sync_session_from_database.rs for the symmetric "wait for the other
            // task's cleanup" problem) rather than looping forever. A legitimate wait
            // here is short: the caller's own player.quit() just killed the
            // previous track's player process, so that track's own poll loop should
            // notice the death and release the slot within a few seconds. If nothing
            // has released it after 20s, there's no teardown to wait for at all -
            // this attempt lost a claim race against another overlapping
            // playback-start (e.g. the play key double-tapped inside the same
            // second) whose session already started and won't naturally end for a
            // long time. Give up instead of forcing the claim: forcing it would let
            // this attempt start a second VLC process/session alongside the one that
            // legitimately won the race (confirmed live: this loop was still
            // spinning after 40+ seconds with is_loop_break stuck at "0").
            let mut claimed = false;
            for _ in 0..20 {
                if try_claim_playback_slot(&username) {
                    claimed = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
                info!("[AppView::Home][loop][is_loop_break]");
                let _ = pop_message(&mut stdout, 3, message);
            }

            if !claimed {
                info!("[AppView::Home][wait_prev_session_finished] Gave up waiting for the playback slot - it's already owned by another in-flight playback-start attempt");
                let _ = clear_message(&mut stdout, 3);
                NEEDS_TERMINAL_CLEAR.store(true, Ordering::Relaxed);
                return false;
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

        // This function runs inside a detached `tokio::spawn`ed playback task with
        // no `Terminal` in scope to call `.clear()` on directly - see
        // NEEDS_TERMINAL_CLEAR's doc comment. Set unconditionally (not just on the
        // branch that actually popped a message) since the caller's own subsequent
        // `pop_message` call (see handle_l_pod_home.rs and friends) can suffer the
        // same stale-cache fate regardless of which branch ran here.
        NEEDS_TERMINAL_CLEAR.store(true, Ordering::Relaxed);

        true
}
