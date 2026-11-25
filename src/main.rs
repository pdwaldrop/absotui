mod login_app;
mod app;
mod config;
mod api;
mod ui;
mod player;
mod logic;
mod db;
mod utils;

use login_app::AppLogin;
use app::App;
use crate::db::database_struct::Database;
use color_eyre::Result;
use std::time::Duration;
use crossterm::event::{self, EventStream, KeyCode};
use futures::StreamExt;
use ratatui::backend::{Backend, ClearType};
use std::io::stdout;
use crate::utils::pop_up_message::{clear_message, pop_message, NEEDS_TERMINAL_CLEAR};
use std::sync::atomic::Ordering;
use crate::utils::logs::setup_logs;
use log::info;
use crate::db::crud::{update_is_vlc_launched_first_time, get_is_vlc_running, update_is_vlc_running, get_auth_in_progress};
use crate::player::integrated::player_info::{player_info, playing_item_name};
use crate::ui::player_tui::render_player;
use std::env;
use std::path::PathBuf;
use crate::utils::clap::clap;
use crate::utils::scroll_wheel::{disable_terminal_scroll_wheel, restore_terminal_scroll_wheel};
use crate::logic::server_recovery::init_app_with_retry;
use crate::logic::sync_session::sync_session_from_database::recover_session_left_playing;
use crate::app::UpdateUninstallStage;
use crate::logic::update_uninstall::{Action, ProgressEvent};
use std::os::unix::process::CommandExt;

// Where a binary-method install/update always lands (see hello_absotui.sh's
// dl_handle_compressed_binary/check_and_cleanup_binary_install) - deterministic
// because Settings > Update/Uninstall always forces the "download precompiled
// binary" method and always cleans up a stale ~/.cargo/bin/absotui first, regardless
// of how the currently-running binary was originally installed.
const INSTALLED_BINARY_PATH: &str = "/usr/local/bin/absotui";

#[tokio::main]
async fn main() -> Result<()> {

    clap();

    setup_logs().expect("Failed to execute logger");

    // Used in `encrypt_token.rs`. home_dir() is only actually called in the
    // XDG_CONFIG_HOME-unset fallback case (matches the pattern used everywhere else in
    // the codebase, eg. cover_cache.rs/logs.rs/config.rs), since eagerly requiring $HOME
    // here would defeat the whole point of the override on a system where $HOME isn't
    // set/valid (containers, sandboxes) but XDG_CONFIG_HOME is.
    let config_path = env::var("XDG_CONFIG_HOME").map_or_else(|_| {
            let home_dir = dirs::home_dir().expect("Unable to find the user's home directory");
            if cfg!(target_os = "macos") {
            home_dir.join("Library").join("Preferences")
        } else {
            home_dir.join(".config")
        }
        }, PathBuf::from);
    let env_path = config_path.join("absotui").join(".env");
    dotenvy::from_filename(env_path.clone()).ok();
    utils::private_files::restrict_existing(&env_path);

    let mut _database = Database::new().await?;
    let mut _database_ready = false;

    loop {
        _database = Database::new().await?;
        if _database.default_usr.is_empty() {
            let app_login = AppLogin::new().await?;
            let terminal = ratatui::init();
            disable_terminal_scroll_wheel();
            let _app_result = app_login.run(terminal);
            // Wait for the login attempt just submitted to actually finish (success
            // or failure) before re-checking the database, instead of guessing a
            // fixed delay - see wait_for_auth_to_finish.
            wait_for_auth_to_finish().await;
        } else {
            // Avoid showing the previous screen's leftover print once the app launches.
            print!("\x1B[2J\x1B[1;1H");
            _database_ready = true;
            info!("Database ready");
            break;
        }
    }

    if _database_ready {
        let mut username: String = String::new();
        if let Some(var_username) = _database.default_usr.first() {
            username = var_username.clone();
        }
        // The playback flags are shared through the database, so they're only reset when
        // no other Absotui is running. Still "playing" then means the last run exited
        // mid-playback; its session gets synced once the server is reachable.
        let other_instance_running = utils::instance_lock::register();
        let previous_run_exited_mid_playback = !other_instance_running && get_is_vlc_running(username.as_str()) == "1";
        if other_instance_running {
            info!("[main] Another Absotui is running - leaving the shared playback state alone");
        } else {
            let _ = update_is_vlc_launched_first_time("1", username.as_str());
            let _ = update_is_vlc_running("0", username.as_str());
        }

        let mut terminal = ratatui::init();
        disable_terminal_scroll_wheel();
        // No existing `App` to fall back to at startup, so `allow_cancel` is
        // false and `init_app_with_retry` never returns `Ok(None)` here.
        let Some(mut app) = init_app_with_retry(&mut terminal, false).await? else {
            unreachable!("allow_cancel=false never returns Cancel");
        };

        if previous_run_exited_mid_playback {
            let token = app.token.clone();
            let server_address = app.server_address.clone();
            let player = app.player.clone();
            tokio::spawn(async move {
                recover_session_left_playing(token, server_address, &player).await;
            });
        }

        // The window title follows what's playing; empty when idle, so taskbars fall back to the app name.
        let mut last_window_title: Option<String> = None;

        // Created only now: the recovery screen (init_app_with_retry) reads the terminal
        // itself, and two readers would split the keystrokes between them.
        let mut events = EventStream::new();

        loop {

            let is_playing = get_is_vlc_running(app.username.as_str());
            let player_info = (is_playing == "1").then(|| player_info(app.username.as_str()));

            let window_title = player_info.as_ref()
                .and_then(|info| info.first())
                .map(|title| playing_item_name(title).to_string())
                .unwrap_or_default();
            if last_window_title.as_deref() != Some(window_title.as_str()) {
                let _ = crossterm::execute!(stdout(), crossterm::terminal::SetTitle(&window_title));
                last_window_title = Some(window_title);
            }

            // A playback task may have printed straight to the terminal: repaint fully right before drawing, so the
            // screen is never left blank between the clear and the draw.
            if NEEDS_TERMINAL_CLEAR.swap(false, Ordering::Relaxed) {
                clear_screen(&mut terminal);
            }

            terminal.draw(|frame| {
                // Drawn before the player box (below) rather than after - this frame's
                // `standard_layout` call, part of rendering `app`, sets
                // `app.last_footer_height` to whatever the current screen's footer
                // actually needs, which the box below reads to position itself
                // exactly, and draws on top so it's never clobbered.
                frame.render_widget(&mut app, frame.area());

                if let Some(player_info) = player_info {
                    let area = frame.area();
                    render_player(area, frame.buffer_mut(), player_info, app.username.as_str(), app.last_footer_height);
                }
            })?;

            // Keeps the podcast "New & Unfinished" list from going stale without
            // requiring a manual refresh. The fetch runs in a background task (it takes
            // a request per episode); both calls are cheap no-ops most iterations.
            app.poll_update_check();
            app.poll_stats();
            app.poll_downloads_list();
            app.poll_podcast_home_refresh();
            app.refresh_podcast_home_if_stale(is_playing == "1");

            // Keeps the access token from reaching Audiobookshelf's ~1 hour expiry while the app is open; the
            // request itself (about once every 50 minutes) runs in the background.
            app.refresh_token_if_needed();

            // Merges the background podcast-episode-list fetch spawned in App::new()
            // into the live app the moment it's ready (see bug_id 3f729c) - a no-op
            // once already consumed or while still in flight.
            app.poll_pod_ep_fetch();

            // Merges the background podcast search/feed-fetch, or the create+seed
            // pair, into the live app the moment either resolves - see
            // App::poll_podcast_add_result. A no-op unless AppView::PodcastAdd is
            // actually waiting on one.
            app.poll_podcast_add_result();

            // Same idea, for a spawned delete_library_item call (podcast remove) -
            // only reloads the library once the delete has actually finished.
            app.poll_podcast_remove_result();

            // Drain one pending Settings > Update/Uninstall progress event, if any
            // (non-blocking) - keeps that screen's log panel live without a dedicated
            // blocking sub-loop, just reusing this same draw/poll cadence.
            if let Some(event) = app.poll_update_uninstall_event() {
                let running_action = match &app.update_uninstall_stage {
                    UpdateUninstallStage::Running(a) => Some(*a),
                    _ => None,
                };
                if let Some(action) = running_action {
                    match event {
                        ProgressEvent::Line(line) => app.update_uninstall_log.push(line),
                        ProgressEvent::NeedPassword => {
                            app.update_uninstall_password = app.new_password_field();
                            app.update_uninstall_stage = UpdateUninstallStage::Password(action);
                        }
                        ProgressEvent::AuthFailed => {
                            app.update_uninstall_stage = UpdateUninstallStage::Failed(action, "Incorrect password".to_string());
                            app.update_uninstall_receiver = None;
                        }
                        ProgressEvent::Finished(Ok(())) => match action {
                            Action::Update => {
                                ratatui::restore();
                                restore_terminal_scroll_wheel();
                                // Replaces this process's image with the freshly-installed
                                // binary - never returns on success, so the app just picks
                                // up where its own `main()` starts fresh. Only reached at
                                // all if exec() itself failed to launch.
                                let exec_err = std::process::Command::new(INSTALLED_BINARY_PATH).exec();
                                eprintln!("Update installed, but couldn't relaunch automatically: {exec_err}");
                                eprintln!("Run absotui to start the new version.");
                                std::process::exit(0);
                            }
                            Action::Uninstall => {
                                ratatui::restore();
                                restore_terminal_scroll_wheel();
                                std::process::exit(0);
                            }
                        },
                        ProgressEvent::Finished(Err(message)) => {
                            app.update_uninstall_stage = UpdateUninstallStage::Failed(action, message);
                            app.update_uninstall_receiver = None;
                        }
                    }
                }
            }

            // Waits for whichever comes first: a terminal event, a background task saying it
            // has something to show (see utils::wake), or the tick - which still drives
            // the things nothing signals, like the title scroll and the periodic refresh
            // checks. A finished cover fetch or a new playback state used to sit until the
            // next tick; now the loop runs the moment it happens.
            let next_event = tokio::select! {
                // Keys first: a random pick between a ready key and a pending wake costs an extra redraw per key.
                biased;
                event = events.next() => Some(event),
                _ = crate::utils::wake::woken() => None,
                _ = tokio::time::sleep(Duration::from_millis(250)) => None,
            };
            if let Some(next) = next_event {
                match next {
                    Some(Ok(event::Event::Key(key))) => {
                        app.handle_key(key);
                        // `R` only arms the reload; the check below runs every iteration so reloads from background results
                        // happen without a keypress. Skipped while typing free text, where `R` is just a letter.
                        if let KeyCode::Char('R') = key.code
                            && !app.is_capturing_free_text() {
                            app.library_needs_reload = true;
                        }
                    }
                    // A resize or any other terminal event only needs the redraw the next
                    // iteration does anyway.
                    Some(Ok(_)) => {}
                    Some(Err(e)) => return Err(e.into()),
                    None => break,
                }
            }

            if app.library_needs_reload {
                let mut stdout = stdout();
                let _ = clear_message(&mut stdout, 3);
                let _ = pop_message(&mut stdout, 3, "Refreshing...");
                // Reload; on failure the recovery screen can cancel back to the current app. It reads the terminal
                // itself, so the event stream is dropped meanwhile.
                drop(events);
                let reloaded = init_app_with_retry(&mut terminal, true).await;
                events = EventStream::new();
                if let Some(new_app) = reloaded? {
                    app = new_app;
                } else {
                    // Cancelled - stay on the current app/library rather than
                    // immediately re-triggering this same reinit next iteration.
                    app.library_needs_reload = false;
                }
                let _ = clear_message(&mut stdout, 3);
                // The status message bypassed ratatui's diff cache, so repaint everything.
                clear_screen(&mut terminal);
            }

        }
    }

    ratatui::restore();
    restore_terminal_scroll_wheel();
    Ok(())
}

// `Terminal::clear` first asks the terminal where the cursor is, to put it back afterwards -
// a round trip that waits for the terminal's reply, and with the event stream running that
// wait can last until the next keypress. This app hides the cursor, so there is nothing to
// restore: clear the screen directly and reset the diff buffers so the next draw repaints it all.
fn clear_screen(terminal: &mut ratatui::DefaultTerminal) {
    let _ = terminal.backend_mut().clear_region(ClearType::All);
    terminal.swap_buffers();
}

// Waits for the submitted login to finish (up to 30 s) instead of guessing a delay.
async fn wait_for_auth_to_finish() {
    for _ in 0..300 {
        if get_auth_in_progress() != "1" {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
