use crate::player::backend::Player;
use crate::player::session::{PlayerSession, PlaybackStatus, Track, TrackSource};
use crate::api::me::update_media_progress::{update_media_progress_pod, update_media_progress2_pod};
use crate::api::library_items::play_lib_item_or_pod::post_start_playback_session_pod;
use crate::api::sessions::sync_open_session::sync_session;
use crate::api::sessions::close_open_session::close_session_without_send_prg_data;
use crate::utils::pop_up_message::{clear_message, NEEDS_TERMINAL_CLEAR};
use std::sync::atomic::Ordering;
use std::io::stdout;
use log::{info, error};
use crate::db::crud::{insert_listening_session, update_is_vlc_running, update_current_time, get_speed_rate, update_elapsed_time, update_is_finished, update_is_loop_break, get_is_podcast_autoplay, get_download, delete_user, update_login_err};
use crate::api::server::refresh_token::{maybe_refresh_token, RefreshOutcome};
use crate::utils::convert_seconds::progress_time_diff;
use crate::logic::handle_input::handle_l_pod_offline::handle_pod_episode_offline;


/// Starts playback of a podcast episode from `AppView::PodcastEpisode`.
pub async fn handle_l_pod(
    token: Option<&String>,
    refresh_token: Option<&String>,
    ids_library_items: &[String],
    selected: Option<usize>,
    player: Player,
    id_pod: &str,
    server_address: String,
    username: String,

) {

    let Some(token) = token else { return; };
    // Owned and mutable so the proactive refresh check below (run once per loop
    // iteration) can reassign them in place - a podcast episode (or an autoplay
    // chain of several) can play for well over Audiobookshelf's ~1 hour default
    // access token lifetime.
    let mut token = token.clone();
    let mut refresh_token = refresh_token.cloned().unwrap_or_default();
    let Some(mut current_index) = selected else { return; };

    // Outer loop lets a finished episode advance to the next one in this same
    // podcast's episode list (Podcast Autoplay, toggled in Settings) without leaving
    // this spawned task - the main render loop already moved on as soon as this task
    // was spawned, so there's no synchronous caller left to hand control back to for
    // a next episode.
    'episodes: loop {
        // id is id of the podcast episode and id_pod is the id of the podcast
        let Some(id) = ids_library_items.get(current_index) else { break 'episodes; };

        // Checked up front so both branches below (online, and the offline fallback if
        // the session-start call fails) know whether a local copy of this episode
        // exists - see src/utils/download_cache.rs.
        let downloaded = get_download(&username, id);

        match post_start_playback_session_pod(&player.media_player_label().await, Some(&token), id_pod, id, server_address.clone()).await {
        Err(e) => {
            if let Some(downloaded) = downloaded {
                info!("[handle_l_pod] Couldn't start an online playback session ({e}) - falling back to the downloaded copy of {id}");
                handle_pod_episode_offline(
                    id_pod.to_string(),
                    id.clone(),
                    downloaded,
                    player.clone(),
                    username.clone(),
                    token.clone(),
                    refresh_token.clone(),
                    server_address.clone(),
                ).await;
            } else {
                error!("[handle_l_pod] Failed to start playback session: {e}");
                eprintln!("Failed to start playback session");
            }
            // Without this, wait_prev_session_finished's poll loop (blocking every
            // future play attempt until this flips back to "1") never sees it happen -
            // a single transient failure here would otherwise permanently wedge
            // playback until the app is quit cleanly with `Q`.
            let _ = update_is_loop_break("1", username.as_str());
            break 'episodes;
        }
        Ok(info_item) => {

                            let mut current_time: u32 = info_item[0].parse::<f64>().unwrap().round() as u32;

                            info!("[handle_l_pod][post_start_playback_session_pod] OK");
                            info!("[handle_l_pod][post_start_playback_session_pod] Item {id_pod} started at {current_time}s");

                            let _ = insert_listening_session(
                                info_item[3].clone(), // id_session
                                id_pod.to_string(), // id of the podcast (not the episode)
                                current_time,
                                info_item[2].clone(),
                                id.clone(), // id (the episode of the podcast)
                                0, // elapsed time start at 0 seconds
                                format!("{} | {}", info_item[5], info_item[4]), // "Episode Title | Podcast Title" - info_item[5] (displayTitle) is the actual episode title, info_item[4] (mediaMetadata.title) is the podcast's own title
                                String::new(), // author not shown for podcasts
                                true, // is_playback
                                String::new(), // chapter
                                String::new(), // chapters (not wired for podcasts yet)
                                );


                            // downloaded episode, if any - play from the local copy
                            // instead of streaming, even though the server was
                            // reachable enough to start this session
                            let source = match downloaded {
                                Some(downloaded) => TrackSource::Local(downloaded.file_path),
                                None => TrackSource::Stream {
                                    server_address: server_address.clone(),
                                    id_session: info_item[3].clone(),
                                    track_index: info_item[7].parse().unwrap_or(0),
                                },
                            };
                            let track = Track {
                                source,
                                start: current_time,
                                title: info_item[4].clone(),
                                subtitle: info_item[5].clone(),
                                author: info_item[6].clone(),
                                id_item: id_pod.to_string(),
                            };
                            let mut session = PlayerSession::launch(player.clone(), username.clone(), track).await;

                            // clear loading message (from app.rs) once the player is up
                            let mut stdout = stdout();
                            let _ = clear_message(&mut stdout, 3);
                            NEEDS_TERMINAL_CLEAR.store(true, Ordering::Relaxed);

                            // Seeded to a value the first real position is very unlikely to
                            // land on exactly, so the first poll iteration doesn't get misread
                            // as "paused" by the position == last_current_time check below.
                            let mut last_current_time: u32 = 3;
                            let mut progress_sync: u32 = 3;

                            let _ = update_is_vlc_running("1", username.as_str());

                            let mut trigger = 1;

                            // Position last reported by the player. Stays None if the player
                            // is closed before ever reporting one, so the "closed at" sync
                            // below doesn't send the session-start position back: for an
                            // episode with no prior progress that's 0, which reads as
                            // progress being wiped.
                            let mut last_position: Option<u32> = None;

                            loop {
                                // Keeps this task's own copy of the token fresh
                                // independently of the main App (see CLAUDE.md's "one
                                // owner" note) - an episode, or an autoplay chain of
                                // several, can easily outlive Audiobookshelf's ~1 hour
                                // default access token.
                                if maybe_refresh_token(&mut token, &mut refresh_token, username.as_str(), server_address.as_str()).await == RefreshOutcome::Failed {
                                    let _ = delete_user(username.as_str());
                                    let _ = update_login_err("Your session expired - restart Absotui to log in again");
                                    let _ = close_session_without_send_prg_data(Some(&token), &info_item[3], server_address.clone()).await;
                                    let _ = session.quit();
                                    let _ = update_is_vlc_running("0", username.as_str());
                                    let _ = update_is_loop_break("1", username.as_str());
                                    break 'episodes;
                                }

                                match session.next_status().await {
                                    PlaybackStatus::Playing(position) => {
                                        last_position = Some(position);

                                        let _ = update_current_time(position, info_item[3].as_str());

                                        if position == last_current_time {
                                            progress_sync = 0; // the track is in pause
                                        } else {
                                            let speed_rate_str = get_speed_rate(username.as_str());
                                            let speed_rate = speed_rate_str.parse::<f64>().unwrap_or(1.0);
                                            let current_time_adjusted = f64::from(current_time) / speed_rate;
                                            let position_adjusted = f64::from(position) / speed_rate;
                                            let diff = progress_time_diff(position_adjusted, current_time_adjusted);
                                            // A diff this large means a chapter/10s jump rather than
                                            // normal playback advancing - clamp the sync amount to
                                            // 1s instead of reporting the raw (inaccurate) jump size.
                                            if diff > 20 {
                                                progress_sync += 1;
                                            } else {
                                                progress_sync = diff;
                                            }
                                        }
                                        last_current_time = position;

                                        // to sync progress in the server each 10 seconds
                                        if trigger == 10 {
                                            // /sync alone - it already updates progress server-side and
                                            // triggers a websocket update other clients pick up; calling
                                            // /progress too on the same tick risked a race (see known_bugs.md).
                                            let _ = sync_session(Some(&token), &info_item[3],Some(position), progress_sync, server_address.clone()).await;

                                            let _ = update_elapsed_time(progress_sync, info_item[3].as_str());

                                            current_time = position;
                                            progress_sync = 0;
                                            trigger = 0;

                                        } else if progress_sync != 0 {
                                            trigger += 1;
                                        }
                                    }
                                    // The track ended naturally but the player is still open -
                                    // distinct from Closed below, where the user closed the
                                    // player itself.
                                    PlaybackStatus::Finished => {
                                        let position = last_position.unwrap_or(current_time);
                                        let is_finised = true;
                                        info!("[handle_l_pod][Finished] Track finished");

                                        let _ = update_is_finished("1", info_item[3].as_str());

                                        let _ = close_session_without_send_prg_data(Some(&token), &info_item[3],  server_address.clone()).await;
                                        info!("[handle_l_pod][Finished] Session successfully closed");

                                        let _ = update_media_progress2_pod(id_pod, Some(&token), Some(position), &info_item[2], is_finised, id, server_address.clone()).await;
                                        info!("[handle_l_pod][Finished] Player stopped");
                                        info!("[handle_l_pod][Finished] Item {id_pod} closed at {position}s");

                                        let _ = update_is_vlc_running("0", username.as_str());

                                        // Podcast Autoplay: if on, and there's a next
                                        // episode in this same podcast's episode list,
                                        // start it - otherwise stop here just like before
                                        // this feature existed.
                                        if get_is_podcast_autoplay(username.as_str()) == "1"
                                            && current_index + 1 < ids_library_items.len() {
                                                info!("[handle_l_pod][Finished] Autoplay is on, advancing to next episode");
                                                // Deliberately NOT setting is_loop_break here - this task
                                                // is about to keep running for the next episode, not
                                                // actually exit. `wait_prev_session_finished` (run before
                                                // any fresh manual play) polls this exact flag to know
                                                // this background task is done - setting it here on every
                                                // mid-chain transition let it go stale, so a manual replay
                                                // of the still-autoplaying episode would see a false "done"
                                                // and spawn a second task racing this same one over the
                                                // player and the shared listening_session row.
                                                //
                                                // The player doesn't exit on its own at end-of-track, it
                                                // just goes idle - without explicitly quitting it here
                                                // (same as every other place that starts new playback
                                                // does), it keeps holding its control port/socket, and
                                                // all control/sync ends up silently talking to this
                                                // now-finished instance instead.
                                                let _ = session.quit();
                                                current_index += 1;
                                                continue 'episodes;
                                        }
                                        let _ = update_is_loop_break("1", username.as_str());
                                        break 'episodes;
                                    }
                                    PlaybackStatus::Closed => {
                                        let _ = update_is_vlc_running("0", username.as_str());
                                        info!("[handle_l_pod][Quit]");
                                        let _ = close_session_without_send_prg_data(Some(&token), &info_item[3],  server_address.clone()).await;
                                        info!("[handle_l_pod][Quit] Session successfully closed");
                                        if let Some(position) = last_position {
                                            let _ = update_media_progress_pod(id_pod, Some(&token), Some(position), &info_item[2], id, server_address.clone()).await;
                                            info!("[handle_l_pod][Quit] Player closed");
                                            info!("[handle_l_pod][Quit] Item {id_pod} closed at {position}s");
                                        } else {
                                            info!("[handle_l_pod][Quit] Player closed - no real playback data was ever fetched, skipping progress sync");
                                        }
                                        let _ = update_is_loop_break("1", username.as_str());
                                        break 'episodes;
                                    }
                                }
                            }
                        }
        }
    }
}
