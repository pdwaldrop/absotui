use crate::player::backend::Player;
use crate::db::crud::{get_listening_session, get_is_loop_break, get_is_vlc_running, update_is_vlc_launched_first_time, get_is_vlc_launched_first_time};
use crate::api::sessions::close_open_session::close_session_without_send_prg_data;
use log::info;
use crate::api::me::update_media_progress::{update_media_progress_book, update_media_progress2_book, update_media_progress_pod, update_media_progress2_pod};
use crate::utils::exit_app::clean_exit;
use std::time::Duration;

// close and sync listening session found in the database, if any - used both to defend
// against a leftover session from a previous crash/kill (before starting a fresh track)
// and, via `quit_app` below, to close whatever's left when nothing else is watching it
pub async fn sync_session_from_database(token: Option<String>, server_address: String, username: String, handle_key: &str, player: &Player) {

    // quit the player before close and sync session
    let _ = player.quit_gracefully();

    match get_listening_session() {
        Ok(Some(session)) => {

            let _ = close_session_without_send_prg_data(
                token.as_ref(),
                session.id_session.as_str(),
                server_address.clone()).await;

            match handle_key {
                "Q" => info!("[handle_key (Q)][Quit] Session successfully closed"),
                "l" => info!("[handle_key (l)] Session successfully closed"),
                "startup" => info!("[startup] Closed a listening session left open by a previous crash/kill"),
                _ => {}
            }

            if session.id_pod.is_empty() {
                if session.is_finished {
                    let is_finished = true;
                    let _ = update_media_progress2_book(
                        session.id_item.as_str(),
                        token.as_ref(),
                        Some(session.current_time),
                        &session.duration,
                        is_finished,
                        server_address).await;

                    match handle_key {
                        "Q" => info!("[handle_key (Q)][book][Quit] Item {} closed at {:?}s (finished)", session.id_item, session.current_time),
                        "l" => info!("[handle_key (l)] Item {} closed at {:?}s (finished)", session.id_item, session.current_time),
                        _ => {}
                    }
                } else {
                    let _ = update_media_progress_book(
                        session.id_item.as_str(),
                        token.as_ref(),
                        Some(session.current_time),
                        &session.duration,
                        server_address.clone()).await;

                    match handle_key {
                        "Q" => info!("[handle_key (Q)][book][Quit] Item {} closed at {:?}s (not finished)", session.id_item, session.current_time),
                        "l" => info!("[handle_key (l)] Item {} closed at {:?}s (not finished)", session.id_item, session.current_time),
                        "startup" => info!("[startup] Item {} synced at {:?}s (not finished)", session.id_item, session.current_time),
                        _ => {}
                    }
                }

            } else {
                if session.is_finished {
                    let is_finished = true;
                    let _ = update_media_progress2_pod(
                        session.id_item.as_str(),
                        token.as_ref(),
                        Some(session.current_time),
                        &session.duration,
                        is_finished,
                        session.id_pod.as_str(),
                        server_address.clone()).await;

                    match handle_key {
                        "Q" => info!("[handle_key (Q)][podcast][Quit] Item {} closed at {:?}s (finished)", session.id_pod, session.current_time),
                        "l" => info!("[handle_key (l)] Item {} closed at {:?}s (finished)", session.id_pod, session.current_time),
                        _ => {}
                    }
                } else {
                    let _ = update_media_progress_pod(
                        session.id_item.as_str(),
                        token.as_ref(),
                        Some(session.current_time),
                        &session.duration,
                        session.id_pod.as_str(),
                        server_address.clone()).await;


                    match handle_key {
                        "Q" => info!("[handle_key (Q)][podcast][Quit] Item {} closed at {:?}s", session.id_pod, session.current_time),
                        "l" => info!("[handle_key (l)] Item {} closed at {:?}s", session.id_pod, session.current_time),
                        "startup" => info!("[startup] Item {} synced at {:?}s", session.id_pod, session.current_time),
                        _ => {}
                    }
                }
            }
        }

        Ok(None) => {
            let value = get_is_vlc_launched_first_time(username.as_str());
            if value == "1" {
                info!("[handle_key] Quit with no listening session");
            } else {
                info!("[handle_key] First session launched");
            }
        }
        Err(e) => {
            info!("[handle_key] Error during fetching session: {e:?}");
        }
    }
}

// Handles the `Q`/`Esc` quit key. Guarantees the app always actually exits (previously,
// an sqlite read error or a couple of edge-case states in `get_listening_session` could
// leave `clean_exit` never called, hanging the app on quit) and never independently
// closes/syncs a session that a still-running playback task (handle_l_book.rs /
// handle_l_pod_home.rs / handle_l_pod.rs's polling loop) already owns - doing so raced
// that task over the same close-session API call, and this function's own hard
// `process::exit` could kill it mid-cleanup before its own final progress push
// completed. Confirmed live: pressing Q while playing produced two separate
// "Session successfully closed" log lines ~250ms apart, and the playback loop's own
// trailing "VLC closed"/"Item closed" log lines never appeared - cut off by exit.
pub async fn quit_app(token: Option<String>, server_address: String, username: String, player: Player) {

    if get_is_vlc_running(username.as_str()) == "1" {
        // A playback task is actively watching this session and will close/sync it
        // itself the moment it notices the player is gone (same arm that handles the
        // user closing the player directly) - so just make sure the player actually
        // dies and wait for that task's own is_loop_break=1 signal - the same flag
        // wait_prev_session_finished waits on before letting a new track start -
        // instead of doing the close/sync here too.
        let _ = player.quit();

        // ~20s cap so a wedged network call in the playback task's own cleanup can't
        // hang Q forever - past that, exit anyway rather than leave the app stuck.
        for _ in 0..100 {
            if get_is_loop_break(username.as_str()) == "1" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    } else {
        // Nothing actively playing in this process - either quitting without ever
        // having played anything, or a leftover session from a previous crash/kill
        // that was never closed. Nothing else is touching it, so close/sync it here.
        sync_session_from_database(token, server_address, username.clone(), "Q", &player).await;
    }

    let _ = update_is_vlc_launched_first_time("1", username.as_str());
    info!("App successfully quit");
    clean_exit();
}
