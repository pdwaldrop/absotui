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
use crate::db::crud::{insert_listening_session, update_is_vlc_running, update_current_time, get_speed_rate, update_elapsed_time, update_is_finished, update_is_loop_break, get_is_podcast_autoplay, get_listening_session, get_download, delete_user, update_login_err};
use crate::api::server::refresh_token::{maybe_refresh_token, RefreshOutcome};
use crate::utils::convert_seconds::progress_time_diff;
use crate::api::libraries::get_library_perso_view_pod::get_new_and_unfinished_pod;
use crate::api::utils::collect_personalized_view_pod::{collect_ids_pod_cnt_list, collect_ids_ep_pod_cnt_list, collect_published_at_pod_cnt_list};
use crate::logic::handle_input::handle_l_pod_offline::handle_pod_episode_offline;


/// Starts playback of a podcast episode from `AppView::Home`'s Continue Listening list.
pub async fn handle_l_pod_home(
    token: Option<&String>,
    refresh_token: Option<&String>,
    ids_library_items: &[String],
    selected: Option<usize>,
    player: Player,
    id_pod: Vec<String>,
    server_address: String,
    username: String,
    id_selected_lib: String,
    newest_first: bool,
    initial_published_at: Option<i64>,

) {

    let Some(token) = token else { return; };
    // Owned and mutable so the proactive refresh check below (run once per loop
    // iteration) can reassign them in place - a Continue Listening episode (or an
    // autoplay chain of several) can play for well over Audiobookshelf's ~1 hour
    // default access token lifetime.
    let mut token = token.clone();
    let mut refresh_token = refresh_token.cloned().unwrap_or_default();
    let Some(current_index) = selected else { return; };
    let Some(mut id) = ids_library_items.get(current_index).cloned() else { return; };
    let Some(mut id_pod_ep) = id_pod.get(current_index).cloned() else { return; };
    // Tracks the currently-playing episode's own published_at, so autoplay can find
    // whatever's genuinely adjacent to it in the live queue's sort order - see
    // next_autoplay_episode.
    let mut current_published_at = initial_published_at;

    // Outer loop lets a finished episode advance to the next one (Podcast Autoplay,
    // toggled in Settings) without leaving this spawned task - the main render loop
    // already moved on as soon as this task was spawned, so there's no synchronous
    // caller left to hand control back to for a next episode.
    'episodes: loop {
        // Checked up front so both branches below (online, and the offline fallback if
        // the session-start call fails) know whether a local copy of this episode
        // exists - see src/utils/download_cache.rs.
        let downloaded = get_download(&username, &id_pod_ep);

        match post_start_playback_session_pod(&player.media_player_label().await, Some(&token), &id, &id_pod_ep, server_address.clone()).await {
        Err(e) => {
            if let Some(downloaded) = downloaded {
                info!("[handle_l_pod_home] Couldn't start an online playback session ({e}) - falling back to the downloaded copy of {id_pod_ep}");
                handle_pod_episode_offline(
                    id.clone(),
                    id_pod_ep.clone(),
                    downloaded,
                    player.clone(),
                    username.clone(),
                    token.clone(),
                    refresh_token.clone(),
                    server_address.clone(),
                ).await;
            } else {
                error!("[handle_l_pod_home] Failed to start playback session: {e}");
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

            info!("[handle_l_pod_home][post_start_playback_session_pod] OK");
            info!("[handle_l_pod_home][post_start_playback_session_pod] Item {id_pod_ep} started at {current_time}s");
            let _ = insert_listening_session(
                info_item[3].clone(), // id_session
                id.clone(), // (id of the podcast, not the episode)
                current_time,
                info_item[2].clone(),
                id_pod_ep.clone(), // id of the podcast episode
                0, // elapsed time start at 0 seconds
                format!("{} | {}", info_item[5], info_item[4]), // "Episode Title | Podcast Title" - info_item[5] (displayTitle) is the actual episode title, info_item[4] (mediaMetadata.title) is the podcast's own title
                String::new(), // author not shown for podcasts
                true, // is_playback
                String::new(), // chapter
                String::new(), // chapters (not wired for podcasts yet)
            );


            // downloaded episode, if any - play from the local copy instead of
            // streaming, even though the server was reachable enough to start this
            // session
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
                id_item: id.clone(),
            };
            let mut session = PlayerSession::launch(player.clone(), username.clone(), track).await;

            // clear loading message (from app.rs) once the player is up
            let mut stdout = stdout();
            let _ = clear_message(&mut stdout, 3);
            NEEDS_TERMINAL_CLEAR.store(true, Ordering::Relaxed);

            // Seeded to a value the first real position is very unlikely to land on
            // exactly, so the first poll iteration doesn't get misread as "paused" by the
            // position == last_current_time check below.
            let mut last_current_time: u32 = 3;
            let mut progress_sync: u32 = 3;

            let _ = update_is_vlc_running("1", username.as_str());

            let mut trigger = 1;

            // Position last reported by the player. Stays None if the player is closed
            // before ever reporting one, so the "closed at" sync below doesn't send the
            // session-start position back: for an episode with no prior progress that's
            // 0, which reads as progress being wiped.
            let mut last_position: Option<u32> = None;

            loop {
                // Keeps this task's own copy of the token fresh independently of the
                // main App (see CLAUDE.md's "one owner" note) - a Continue Listening
                // episode, or an autoplay chain of several, can easily outlive
                // Audiobookshelf's ~1 hour default access token.
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

                        // Manually marked finished while still playing (F in the Home
                        // list) - the key handler can't touch the player directly (this
                        // task owns it exclusively), so it just flips
                        // listening_session.is_finished as a signal. Checked first,
                        // before the normal per-second bookkeeping below, so playback
                        // actually stops within about a second instead of waiting on
                        // the natural Finished status - unlike that natural
                        // end-of-track case, the player is still actively playing here
                        // and has to be told to stop, not just detected as already
                        // having done so.
                        if let Ok(Some(listening_session)) = get_listening_session()
                            && listening_session.is_finished {
                                info!("[handle_l_pod_home][ManualFinish] Marked finished while playing, stopping");
                                let _ = session.quit();

                                // Report full duration as the current_time, not
                                // wherever playback happened to be interrupted - the
                                // server's own "New & Unfinished" view recomputes
                                // finished-status from progress%, so isFinished=true
                                // at a partial percentage doesn't stick (matches the
                                // not-currently-playing branch in app.rs's F handler,
                                // which does the same for the same reason).
                                let duration_secs = info_item[2].parse::<f64>().map(|d| d.round() as u32).unwrap_or(position);

                                let next_episode = finish_episode(
                                    &token,
                                    &id,
                                    &id_pod_ep,
                                    duration_secs,
                                    &info_item[2],
                                    info_item[3].as_str(),
                                    username.as_str(),
                                    &server_address,
                                    &id_selected_lib,
                                    newest_first,
                                    current_published_at,
                                ).await;

                                if let Some((next_id, next_id_pod, next_published_at)) = next_episode {
                                    info!("[handle_l_pod_home][ManualFinish] Autoplay is on, advancing to next episode");
                                    id = next_id;
                                    id_pod_ep = next_id_pod;
                                    current_published_at = Some(next_published_at);
                                    continue 'episodes;
                                }
                                let _ = update_is_loop_break("1", username.as_str());
                                break 'episodes;
                        }

                        let _ = update_current_time(position, info_item[3].as_str());

                        if position == last_current_time {
                            progress_sync = 0; // the track is in pause
                        } else {
                            let speed_rate_str = get_speed_rate(username.as_str());
                            let speed_rate = speed_rate_str.parse::<f64>().unwrap_or(1.0);
                            let current_time_adjusted = f64::from(current_time) / speed_rate;
                            let position_adjusted = f64::from(position) / speed_rate;
                            let diff = progress_time_diff(position_adjusted, current_time_adjusted);
                            // A diff this large means a chapter/10s jump rather than normal
                            // playback advancing - clamp the sync amount to 1s instead of
                            // reporting the raw (inaccurate) jump size.
                            if diff > 20 {
                                progress_sync += 1;
                            } else {
                                progress_sync = diff;
                            }
                        }
                        last_current_time = position;

                        // to sync progress in the server each 10 seconds
                        if trigger == 10 {
                            // /sync alone - it already updates progress server-side and triggers a
                            // websocket update other clients pick up; calling /progress too on the
                            // same tick risked a race (see known_bugs.md).
                            let _ = sync_session(Some(&token), &info_item[3],Some(position), progress_sync, server_address.clone()).await;
                            let _ = update_elapsed_time(progress_sync, info_item[3].as_str());

                            current_time = position;
                            progress_sync = 0;
                            trigger = 0;

                        } else if progress_sync != 0 {
                            trigger += 1;
                        }
                    }
                    // The track ended naturally but the player is still open - distinct
                    // from Closed below, where the user closed the player itself.
                    PlaybackStatus::Finished => {
                        info!("[handle_l_pod_home][Finished] Track finished");

                        // Podcast Autoplay: if on, and the live "New & Unfinished"
                        // queue has something adjacent to this episode to play next,
                        // finish_episode returns it to start - otherwise stop here
                        // just like before this feature existed.
                        //
                        // (finish_episode re-fetches the queue rather than just
                        // advancing to the next index of the list this task started
                        // with: that list is a one-time snapshot taken back when the
                        // user first pressed play, and can be badly stale by the time
                        // a later episode in an autoplay chain finishes - new episodes
                        // arrive, others get marked finished and drop out, entirely
                        // independent of this chain. Indexing into the stale snapshot
                        // could "autoplay" into whatever used to be next in that old
                        // ordering - including an episode already listened to earlier -
                        // rather than what's actually next in the live queue.)
                        let next_episode = finish_episode(
                            &token,
                            &id,
                            &id_pod_ep,
                            last_position.unwrap_or(current_time),
                            &info_item[2],
                            info_item[3].as_str(),
                            username.as_str(),
                            &server_address,
                            &id_selected_lib,
                            newest_first,
                            current_published_at,
                        ).await;

                        if let Some((next_id, next_id_pod, next_published_at)) = next_episode {
                            info!("[handle_l_pod_home][Finished] Autoplay is on, advancing to next episode");
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
                            // just goes idle - without explicitly quitting it here (same
                            // as every other place that starts new playback does), it
                            // keeps holding its control port/socket, and all
                            // control/sync ends up silently talking to this
                            // now-finished instance instead.
                            let _ = session.quit();
                            id = next_id;
                            id_pod_ep = next_id_pod;
                            current_published_at = Some(next_published_at);
                            continue 'episodes;
                        }
                        let _ = update_is_loop_break("1", username.as_str());
                        break 'episodes;
                    }
                    PlaybackStatus::Closed => {
                        let _ = update_is_vlc_running("0", username.as_str());
                        info!("[handle_l_pod_home][Quit]");
                        let _ = close_session_without_send_prg_data(Some(&token), &info_item[3],  server_address.clone()).await;
                        info!("[handle_l_pod_home][Quit] Session successfully closed");
                        if let Some(position) = last_position {
                            let _ = update_media_progress_pod(&id, Some(&token), Some(position), &info_item[2], &id_pod_ep, server_address.clone()).await;
                            info!("[handle_l_pod_home][Quit] Player closed");
                            info!("[handle_l_pod_home][Quit] Item {id_pod_ep} closed at {position}s");
                        } else {
                            info!("[handle_l_pod_home][Quit] Player closed - no real playback data was ever fetched, skipping progress sync");
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

/// Marks the episode finished server-side, closes its session, and updates
/// is_vlc_running - the common tail shared by the natural end-of-track path
/// (`PlaybackStatus::Finished`) and a manual "mark finished while still playing"
/// request (F in the Home list, signaled via listening_session.is_finished and caught
/// at the top of the polling loop) - the two differ only in whether the player itself
/// has already stopped or the caller had to tell it to first, which happens before
/// this runs either way.
///
/// Returns what to autoplay next (podcast id, episode id, published_at) if Podcast
/// Autoplay is on and the live "New & Unfinished" queue has something adjacent to
/// this episode - see `next_autoplay_episode` for why that's a fresh queue fetch
/// rather than just the next index of a possibly-stale snapshot.
async fn finish_episode(
    token: &String,
    id: &str,
    id_pod_ep: &str,
    position: u32,
    duration: &str,
    id_session: &str,
    username: &str,
    server_address: &str,
    id_selected_lib: &str,
    newest_first: bool,
    current_published_at: Option<i64>,
) -> Option<(String, String, i64)> {
    let _ = update_is_finished("1", id_session);

    let _ = close_session_without_send_prg_data(Some(token), id_session, server_address.to_string()).await;
    info!("[handle_l_pod_home][Finished] Session successfully closed");

    let _ = update_media_progress2_pod(id, Some(token), Some(position), duration, true, id_pod_ep, server_address.to_string()).await;
    info!("[handle_l_pod_home][Finished] Item {id_pod_ep} closed at {position}s");

    let _ = update_is_vlc_running("0", username);

    if get_is_podcast_autoplay(username) == "1" {
        next_autoplay_episode(token, server_address, id_selected_lib, newest_first, current_published_at, id_pod_ep).await
    } else {
        None
    }
}

/// Fetches the current "New & Unfinished" queue fresh and returns the (podcast id,
/// episode id, published_at) of whatever's genuinely adjacent to the episode that just
/// finished, in the same sort order the Home list itself uses (respects whichever sort
/// direction was active when this autoplay chain started - `newest_first`, captured once
/// at the top-level play same as the rest of this task's state).
///
/// This is deliberately NOT "whatever's at the front of the queue": the episode that
/// just finished isn't necessarily the newest (or oldest) unfinished one - it could sit
/// anywhere in the sort order - so "next" has to mean "the closest one on the other
/// side of it," found via `current_published_at`, not "the global top of the list."
/// Picking the global top would autoplay into whatever's chronologically first
/// regardless of where the just-finished episode actually was, which can be a
/// completely different, unrelated episode - including one already listened to earlier.
///
/// `current_published_at` is `None` only for the very first episode of a chain if its
/// publish date wasn't available; in that case this falls back to the queue's front,
/// same as before this refinement. Returns `None` if the queue is empty, the fetch
/// fails, or - when `current_published_at` is known - there's nothing further along in
/// the current sort direction (the just-finished episode was already the last one),
/// in which case autoplay stops like before this feature existed rather than wrapping
/// back around to the top.
///
/// `current_id_pod` (the just-finished episode) is explicitly excluded from candidacy -
/// its own isFinished update may not have propagated server-side by the time this fetch
/// runs, in which case it would still match its own published_at via the `<=`/`>=`
/// comparison and get picked as "next," replaying itself forever.
async fn next_autoplay_episode(token: &str, server_address: &str, id_selected_lib: &str, newest_first: bool, current_published_at: Option<i64>, current_id_pod: &str) -> Option<(String, String, i64)> {
    let roots = get_new_and_unfinished_pod(token, server_address.to_string(), &id_selected_lib.to_string()).await.ok()?;
    let ids = collect_ids_pod_cnt_list(&roots).await;
    let ids_ep = collect_ids_ep_pod_cnt_list(&roots).await;
    let published_at = collect_published_at_pod_cnt_list(&roots).await;

    let mut order: Vec<usize> = (0..published_at.len()).collect();
    if newest_first {
        order.sort_by_key(|&i| std::cmp::Reverse(published_at[i]));
    } else {
        order.sort_by_key(|&i| published_at[i]);
    }
    order.retain(|&i| ids_ep.get(i).map(String::as_str) != Some(current_id_pod));

    let chosen = match current_published_at {
        Some(cur) => order.iter().copied().find(|&i| {
            if newest_first { published_at[i] <= cur } else { published_at[i] >= cur }
        }),
        None => order.first().copied(),
    }?;

    Some((ids.get(chosen)?.clone(), ids_ep.get(chosen)?.clone(), published_at[chosen]))
}
