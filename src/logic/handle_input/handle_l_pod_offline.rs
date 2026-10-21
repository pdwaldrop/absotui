use crate::player::backend::Player;
use crate::player::session::{PlayerSession, PlaybackStatus, Track, TrackSource};
use crate::utils::pop_up_message::{clear_message, NEEDS_TERMINAL_CLEAR};
use std::sync::atomic::Ordering;
use crate::api::me::update_media_progress::{update_media_progress_pod, update_media_progress2_pod};
use std::io::stdout;
use log::info;
use crate::db::crud::{insert_listening_session, update_is_vlc_running, update_current_time, update_is_finished, update_is_loop_break, get_listening_session, delete_user, update_login_err};
use crate::api::server::refresh_token::{maybe_refresh_token, RefreshOutcome};
use crate::db::database_struct::DownloadedItem;

/// Plays a downloaded podcast episode with no server session at all - used when
/// `post_start_playback_session_pod` fails (server unreachable) but a local copy of the
/// episode exists (see src/utils/download_cache.rs). Shared by both handle_l_pod_home
/// and handle_l_pod, since the offline case is identical either way - no
/// `sync_session`/`update_media_progress`/`close_session` calls happen during playback
/// (no server-issued session id to attach them to), and - unlike the online path -
/// there's no Podcast Autoplay here: advancing to a next episode needs a live fetch of
/// the current New & Unfinished/episode-list queue, which isn't available offline.
/// Mirrors handle_l_book.rs's handle_l_book_offline exactly, adapted for the extra
/// podcast/episode id split and the podcast progress-update endpoints.
pub async fn handle_pod_episode_offline(
    podcast_id: String,
    episode_id: String,
    downloaded: DownloadedItem,
    player: Player,
    username: String,
    mut token: String,
    mut refresh_token: String,
    server_address: String,
) {
    // Not a server-issued id - used purely as this local session's sqlite key/log tag.
    let id_session = format!("offline-{episode_id}");

    // Resume from wherever local playback last left off, if the last real
    // `listening_session` row happens to be this same episode - the download itself
    // doesn't track a position.
    let current_time: u32 = get_listening_session().ok().flatten()
        .filter(|s| s.id_pod == episode_id)
        .map(|s| s.current_time)
        .unwrap_or(0);

    let _ = insert_listening_session(
        id_session.clone(),
        podcast_id.clone(),
        current_time,
        downloaded.duration.clone(),
        episode_id.clone(),
        0,
        downloaded.title.clone(),
        String::new(), // author not shown for podcasts
        true,
        String::new(),
        String::new(),
    );

    let track = Track {
        source: TrackSource::Local(downloaded.file_path.clone()),
        start: current_time,
        title: downloaded.title.clone(),
        subtitle: downloaded.title.clone(),
        author: String::new(),
        id_item: podcast_id.clone(),
    };
    let mut session = PlayerSession::launch(player, username.clone(), track).await;

    let mut stdout = stdout();
    let _ = clear_message(&mut stdout, 3);
    NEEDS_TERMINAL_CLEAR.store(true, Ordering::Relaxed);

    let _ = update_is_vlc_running("1", username.as_str());

    // See handle_l_book.rs's last_position comment - same reasoning applies here.
    let mut last_position: Option<u32> = None;

    loop {
        // Same proactive-refresh reasoning as handle_l_book_offline - this path is
        // reached because the online session-start call already failed once, so the
        // token may well already be the reason, and the best-effort progress push at
        // the end of this loop still needs a usable one.
        if maybe_refresh_token(&mut token, &mut refresh_token, username.as_str(), server_address.as_str()).await == RefreshOutcome::Failed {
            let _ = delete_user(username.as_str());
            let _ = update_login_err("Your session expired - restart Absotui to log in again");
            let _ = session.quit();
            let _ = update_is_vlc_running("0", username.as_str());
            let _ = update_is_loop_break("1", username.as_str());
            break;
        }

        match session.next_status().await {
            PlaybackStatus::Playing(position) => {
                last_position = Some(position);
                let _ = update_current_time(position, id_session.as_str());
            }
            PlaybackStatus::Finished => {
                let position = last_position.unwrap_or(current_time);
                info!("[handle_pod_episode_offline][Finished] Track finished");
                let _ = update_is_finished("1", id_session.as_str());
                // Best-effort only - ignored whether the server is back or not.
                let _ = update_media_progress2_pod(&podcast_id, Some(&token), Some(position), &downloaded.duration, true, &episode_id, server_address.clone()).await;
                let _ = update_is_loop_break("1", username.as_str());
                let _ = update_is_vlc_running("0", username.as_str());
                break;
            }
            PlaybackStatus::Closed => {
                let _ = update_is_vlc_running("0", username.as_str());
                if let Some(position) = last_position {
                    info!("[handle_pod_episode_offline][Quit] Item {episode_id} closed at {position}s");
                    let _ = update_media_progress_pod(&podcast_id, Some(&token), Some(position), &downloaded.duration, &episode_id, server_address.clone()).await;
                } else {
                    info!("[handle_pod_episode_offline][Quit] Item {episode_id}: no real playback data was ever fetched, skipping progress sync");
                }
                let _ = update_is_loop_break("1", username.as_str());
                break;
            }
        }
    }
}
