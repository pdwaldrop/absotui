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
use crossterm::event::{self, KeyCode};
use std::io::stdout;
use crate::utils::pop_up_message::{clear_message, pop_message, NEEDS_TERMINAL_CLEAR};
use std::sync::atomic::Ordering;
use crate::utils::logs::setup_logs;
use log::info;
use crate::db::crud::{update_is_vlc_launched_first_time, get_is_vlc_launched_first_time, get_is_vlc_running, update_is_vlc_running, get_auth_in_progress};
use crate::player::integrated::player_info::{player_info, playing_item_name};
use crate::ui::player_tui::render_player;
use std::env;
use std::path::PathBuf;
use crate::utils::clap::clap;
use crate::utils::scroll_wheel::{disable_terminal_scroll_wheel, restore_terminal_scroll_wheel};
use crate::logic::server_recovery::init_app_with_retry;
use crate::logic::sync_session::sync_session_from_database::sync_session_from_database;
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
    dotenv::from_filename(env_path.clone()).ok();

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
        let _ = update_is_vlc_launched_first_time("1", username.as_str());
        let value = get_is_vlc_launched_first_time(username.as_str());
        info!("[main][is_vlc_launched_first_time] {value}");

        // A fresh process can never have inherited a live playback task from a
        // previous run (those only exist as tokio tasks within the process that
        // spawned them - see handle_l_book.rs et al) - so if this is still "1" here,
        // it's leftover from a previous crash/kill that never got to reset it, and
        // would otherwise make the render loop show a frozen player overlay for a
        // session nothing is actually backing (confirmed live: it doesn't advance,
        // and there's no real VLC process behind it).
        let _ = update_is_vlc_running("0", username.as_str());

        let mut terminal = ratatui::init();
        disable_terminal_scroll_wheel();
        // No existing `App` to fall back to at startup, so `allow_cancel` is
        // false and `init_app_with_retry` never returns `Ok(None)` here.
        let Some(mut app) = init_app_with_retry(&mut terminal, false).await? else {
            unreachable!("allow_cancel=false never returns Cancel");
        };

        // If the app was killed or crashed mid-playback, the session it left open is
        // still open server-side (visible under /audiobookshelf/config/sessions) and
        // its last position never got synced - every normal exit path closes its own
        // session, so anything still sitting in `listening_session` here is debris from
        // an abnormal one. Closing it now rather than deferring to whenever the user
        // next presses play means the position isn't left dangling for an arbitrarily
        // long time (see known_bugs.md `bug_id: 6ac5d8`).
        //
        // Deliberately after `init_app_with_retry` rather than before: that's the point
        // the server is known reachable, so this doesn't burn its one attempt against a
        // server that's down. A no-op when there's no leftover row, and it leaves the
        // row itself in place - offline resume reads it back for the local position.
        let leftover_token = app.token.clone();
        let leftover_server = app.server_address.clone();
        let leftover_username = app.username.clone();
        let leftover_addr = app.config.player.address.clone();
        let leftover_port = app.config.player.port.clone();
        tokio::spawn(async move {
            sync_session_from_database(
                leftover_token,
                leftover_server,
                leftover_username,
                "startup",
                leftover_addr,
                leftover_port,
            ).await;
        });

        // Absotui has no window of its own - the terminal's title is whatever the
        // running program sets it to (or just "absotui", the binary name, if nothing
        // sets it). Keeping it in sync with what's actually playing makes the window
        // identifiable from a taskbar/dock without opening it. An empty title (rather
        // than repeating "Absotui") when idle avoids duplicating the app name a
        // taskbar/dock already shows next to it from the .desktop file's Name= - most
        // (this was confirmed empirically against DMS/quickshell) fall back to just
        // that name when the window title itself is blank.
        let mut last_window_title: Option<String> = None;

        loop {

            let is_playing = get_is_vlc_running(app.username.as_str());
            let player_info = player_info(app.username.as_str());

            let window_title = if is_playing == "1" {
                playing_item_name(&player_info[0]).to_string()
            } else {
                String::new()
            };
            if last_window_title.as_deref() != Some(window_title.as_str()) {
                let _ = crossterm::execute!(stdout(), crossterm::terminal::SetTitle(&window_title));
                last_window_title = Some(window_title);
            }

            // A detached playback task (wait_prev_session_finished and the
            // handle_l_* session starters) may have popped/cleared a raw stdout status
            // message with no `Terminal` in scope to reconcile ratatui's diff cache
            // itself - see NEEDS_TERMINAL_CLEAR's doc comment. Checked here, right
            // before `terminal.draw`, rather than after it, so the physical clear (a
            // real blank-the-screen write) is immediately followed by the full
            // repaint it forces, with no event-poll/sleep gap between them - doing
            // it after `draw` left the screen visibly blank for up to ~250ms on every
            // playback start (confirmed live: a full-screen flash on every "l"/Enter).
            if NEEDS_TERMINAL_CLEAR.swap(false, Ordering::Relaxed) {
                let _ = terminal.clear();
            }

            terminal.draw(|frame| {
                // Drawn before the player box (below) rather than after - this frame's
                // `standard_layout` call, part of rendering `app`, sets
                // `app.last_footer_height` to whatever the current screen's footer
                // actually needs, which the box below reads to position itself
                // exactly, and draws on top so it's never clobbered.
                frame.render_widget(&mut app, frame.area());

                if is_playing == "1" {
                    let area = frame.area();
                    render_player(area, frame.buffer_mut(), player_info, app.username.as_str(), app.last_footer_height);
                }
            })?;

            // Keeps the podcast "New & Unfinished" list from going stale without
            // requiring a manual refresh - the method itself no-ops unless enough time
            // has passed, so this is cheap to call every loop iteration.
            let _ = app.refresh_podcast_home_if_stale().await;

            // Keeps the access token from ever reaching Audiobookshelf's ~1 hour
            // default expiry while the app is open and in use - see
            // App::refresh_token_if_needed's doc comment. Cheap no-op check on every
            // tick; an actual network refresh happens roughly once every ~50 minutes
            // at most.
            app.refresh_token_if_needed().await;

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

            if crossterm::event::poll(Duration::from_millis(200))?
                && let event::Event::Key(key) = crossterm::event::read()? {
                    app.handle_key(key);
                    // 'R' just arms the same library_needs_reload flag a Settings >
                    // Library switch (or a podcast add/remove finishing - see
                    // poll_podcast_add_result/poll_podcast_remove_result) already uses,
                    // rather than reinit-ing inline here - the unconditional check below
                    // is what actually reloads, since it runs every tick regardless of
                    // whether a key was just pressed. Reloads triggered from a background
                    // poll (podcast add/remove) resolve between keystrokes, not on one -
                    // gating the reload on "a key was just pressed" left them sitting
                    // there, reload pending, until the user happened to press anything.
                    //
                    // Gated on is_capturing_free_text: this check runs on the same raw
                    // KeyEvent regardless of what handle_key already did with it, so
                    // without this guard, typing a capital R while free-typing (search,
                    // podcast-add, an update/uninstall password) would trigger a full
                    // reload mid-keystroke, discarding whatever was being typed.
                    if let KeyCode::Char('R') = key.code
                        && !app.is_capturing_free_text() {
                        app.library_needs_reload = true;
                    }
                }

            if app.library_needs_reload {
                let mut stdout = stdout();
                let _ = clear_message(&mut stdout, 3);
                let _ = pop_message(&mut stdout, 3, "Refreshing...");
                // Reinitialize app to refresh - a working `app` already exists, so on
                // failure the recovery screen offers a way to cancel back to it instead
                // of forcing a fix-or-quit loop.
                if let Some(new_app) = init_app_with_retry(&mut terminal, true).await? {
                    app = new_app;
                } else {
                    // Cancelled - stay on the current app/library rather than
                    // immediately re-triggering this same reinit next iteration.
                    app.library_needs_reload = false;
                }
                let _ = clear_message(&mut stdout, 3);
                // pop_message/clear_message write straight to `stdout`, bypassing this
                // `terminal`'s diff cache the same way search's separate Terminal
                // instance does (see the '/' comment above) - without this, the next
                // draw can decide a cell already matches its stale cache and skip
                // repainting it, even though clear_message just blanked it for real
                // (confirmed live: the player box's bottom border row sits exactly 3
                // rows from the bottom, right where this message prints, and silently
                // disappeared until something else forced a full repaint).
                let _ = terminal.clear();
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    ratatui::restore();
    restore_terminal_scroll_wheel();
    Ok(())
}

// Waits for a just-submitted login attempt's spawned auth_process call (see
// auth_input.rs::auth) to actually finish, instead of guessing a fixed delay before
// re-checking whether the database now has credentials. Guessing too short (this used
// to be a flat 1s sleep) meant a slow-but-successful login could still look like a
// failure and force re-entering credentials a second time, even though the first
// attempt was about to succeed. Capped at 30s so a hung request can't wedge the login
// loop forever - past that, the normal "still empty, show the login screen again"
// path takes over exactly like it always has for a genuine failure.
async fn wait_for_auth_to_finish() {
    for _ in 0..300 {
        if get_auth_in_progress() != "1" {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
