use crate::player::backend::Player;
use crate::db::crud::{get_listening_session, get_is_loop_break, update_is_vlc_launched_first_time};
use crate::api::sessions::close_open_session::close_session_without_send_prg_data;
use log::info;
use crate::api::me::update_media_progress::{update_media_progress_book, update_media_progress2_book, update_media_progress_pod, update_media_progress2_pod};
use crate::logic::sync_session::wait_prev_session_finished::owns_playback;
use crate::utils::exit_app::clean_exit;
use crate::utils::instance_lock;
use std::time::Duration;

// Closes and syncs the session a previous run left open by exiting mid-playback (crash,
// kill, closed terminal) and stops its orphaned player. Only for that case: after a normal
// stop the session row stays behind already synced, and re-sending its position would
// undo progress made since on another device.
pub async fn recover_session_left_playing(token: Option<String>, server_address: String, player: &Player) {
    let _ = player.quit_gracefully();

    let Ok(Some(session)) = get_listening_session() else { return };

    let _ = close_session_without_send_prg_data(token.as_ref(), session.id_session.as_str(), server_address.clone()).await;

    if session.id_pod.is_empty() {
        if session.is_finished {
            let _ = update_media_progress2_book(session.id_item.as_str(), token.as_ref(), Some(session.current_time), &session.duration, true, server_address).await;
        } else {
            let _ = update_media_progress_book(session.id_item.as_str(), token.as_ref(), Some(session.current_time), &session.duration, server_address).await;
        }
        info!("[startup] Synced book {} left playing by the previous run at {}s", session.id_item, session.current_time);
    } else {
        if session.is_finished {
            let _ = update_media_progress2_pod(session.id_item.as_str(), token.as_ref(), Some(session.current_time), &session.duration, true, session.id_pod.as_str(), server_address).await;
        } else {
            let _ = update_media_progress_pod(session.id_item.as_str(), token.as_ref(), Some(session.current_time), &session.duration, session.id_pod.as_str(), server_address).await;
        }
        info!("[startup] Synced episode {} left playing by the previous run at {}s", session.id_pod, session.current_time);
    }
}

// Handles the `Q`/`Esc` quit key, and always exits. Playback this process owns is closed
// and synced by its own task, so this only stops the player and waits (capped at ~20s)
// for that task to finish. Another running Absotui's player and session are left alone.
pub async fn quit_app(username: String, player: Player) {
    if owns_playback() {
        let _ = player.quit();
        for attempt in 0..100 {
            if get_is_loop_break(username.as_str()) == "1" {
                break;
            }
            // A task still starting up may launch its player after the quit above.
            if attempt % 5 == 4 {
                let _ = player.quit_gracefully();
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    if !instance_lock::others_running_at_exit() {
        let _ = player.quit_gracefully();
        let _ = update_is_vlc_launched_first_time("1", username.as_str());
    }
    info!("App successfully quit");
    clean_exit();
}
