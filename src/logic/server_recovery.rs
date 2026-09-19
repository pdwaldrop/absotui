use crate::app::App;
use crate::db::crud::{select_default_usr, update_server_address};
use crate::ui::theme;
use crate::utils::exit_app::clean_exit;
use color_eyre::eyre::{Report, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use log::error;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::DefaultTerminal;
use ratatui_textarea::TextArea;

enum Action {
    Retry,
    ChangeAddress(String),
    Quit,
    Cancel,
}

/// A saved login the app can positively say is unusable, with the reason and what to
/// do about it - shown verbatim on the recovery screen instead of a bare HTTP status.
#[derive(Debug)]
pub struct LoginProblem(pub String);

impl std::fmt::Display for LoginProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LoginProblem {}

const RELOGIN_STEPS: &str = "To log in again: quit, delete db.sqlite3 (and .env) from the absotui config folder \
    (~/.config/absotui on Linux), then relaunch. This resets saved settings.";

/// Why the server refused (HTTP 401) the token Absotui had saved, worked out from what
/// we can see locally - the server's own 401 carries no reason.
pub fn diagnose_rejected_login(token: &str, had_refresh_token: bool, address: &str) -> LoginProblem {
    let why = match (crate::api::server::refresh_token::is_token_expired(token), had_refresh_token) {
        (Some(true), false) => "Your saved login has expired, and no refresh token was saved to renew it \
            (logins made before v0.5.29 can't renew themselves).".to_string(),
        (Some(true), true) => "Your saved login has expired and renewing it didn't go through. Retry; \
            if it keeps failing the refresh token is dead too.".to_string(),
        (Some(false), _) => format!("The server rejected your saved login even though it hasn't expired. \
            It was revoked on the server (logged out, password changed, or logged out everywhere), or it \
            was issued by a different server than {address}."),
        (None, _) => format!("The server rejected your saved API token. It was deleted or revoked on \
            the server, or it belongs to a different server than {address}."),
    };
    LoginProblem(format!("{why} {RELOGIN_STEPS}"))
}

/// Returns (headline, message) - the headline only claims "couldn't reach" when the
/// request genuinely never got a response.
fn classify_error(report: &Report) -> (&'static str, String) {
    if let Some(e) = report.downcast_ref::<reqwest::Error>()
        && (e.is_connect() || e.is_timeout() || e.is_request())
    {
        return ("Couldn't reach the Audiobookshelf server", format!("Could not reach the server: {e}"));
    }
    if let Some(problem) = report.downcast_ref::<LoginProblem>() {
        return ("Your saved login isn't working", problem.to_string());
    }
    ("Audiobookshelf returned an error", format!("Reached the server, but something went wrong: {report}"))
}

/// Runs `App::new()` in a retry loop, showing a recovery screen on failure
/// (Retry / change server address / Quit) instead of letting the error
/// propagate out of `main` and silently kill the process.
///
/// `allow_cancel` adds an Esc-to-cancel option, returning `Ok(None)`. Only
/// meaningful at the two mid-session reinit call sites (`R`-refresh, library
/// switch) where a working `App` already exists to fall back to - startup
/// has none, so it passes `false` and never sees `Action::Cancel`.
pub async fn init_app_with_retry(terminal: &mut DefaultTerminal, allow_cancel: bool) -> Result<Option<App>> {
    loop {
        match App::new().await {
            Ok(app) => return Ok(Some(app)),
            Err(report) => {
                error!("[init_app_with_retry] {report}");
                let default_usr = select_default_usr().unwrap_or_default();
                let username = default_usr.first().cloned().unwrap_or_default();
                let address = default_usr.get(1).cloned().unwrap_or_default();
                let (headline, message) = classify_error(&report);

                match render_error_screen(terminal, &address, headline, &message, allow_cancel)? {
                    Action::Retry => continue,
                    Action::ChangeAddress(new_address) => {
                        // The saved token was issued by the old server, so this alone
                        // won't fix auth against a different one - the next retry will
                        // likely surface as a "reached it, but something's wrong" error,
                        // which is expected (full re-login is a quit + AppLogin away).
                        let _ = update_server_address(new_address.trim(), &username);
                        continue;
                    }
                    Action::Quit => clean_exit(), // never returns
                    Action::Cancel => return Ok(None),
                }
            }
        }
    }
}

/// Blocking message/action screen, modeled on `logic::auth::auth_input::auth`'s
/// event loop. `[A]` drops into a one-field address-edit sub-view prefilled
/// with the current address; `Esc` there backs out without committing.
fn render_error_screen(
    terminal: &mut DefaultTerminal,
    address: &str,
    headline: &str,
    message: &str,
    allow_cancel: bool,
) -> Result<Action> {
    let mut editing_address = false;
    let mut textarea = TextArea::from(vec![address.to_string()]);
    textarea.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title("New server address")
            .border_style(Style::new().fg(theme::ACCENT_STRUCTURE)),
    );
    textarea.set_placeholder_text("http:// or https:// required");

    loop {
        terminal.draw(|frame| {
            let area = frame.area();

            if editing_address {
                let input_area = Rect {
                    x: (area.width / 4).max(1),
                    y: area.height.saturating_sub(3) / 2,
                    width: (area.width / 2).max(20),
                    height: 3,
                };
                frame.render_widget(&textarea, input_area);
            } else {
                let mut lines = vec![
                    Line::from(headline.to_string()),
                    Line::from(format!("Server: {address}")),
                    Line::from(""),
                    Line::from(message.to_string()),
                    Line::from(""),
                ];
                let mut hints: Vec<(&str, &str)> = vec![
                    ("R", "Retry"),
                    ("A", "Change server address"),
                    ("Q", "Quit"),
                ];
                if allow_cancel {
                    hints.push(("Esc", "Keep using current data"));
                }
                lines.push(theme::footer_line(&hints));

                let paragraph = Paragraph::new(lines)
                    .wrap(Wrap { trim: true })
                    .block(Block::default().borders(Borders::ALL).title("Connection error")
                        .border_style(Style::new().fg(theme::ACCENT_ERROR)));

                let msg_area = Rect {
                    x: (area.width / 8).max(1),
                    y: (area.height / 4).max(1),
                    width: (area.width * 3 / 4).max(20),
                    height: (area.height / 2).max(9),
                };
                frame.render_widget(paragraph, msg_area);
            }
        })?;

        let ev = event::read()?;
        if editing_address {
            match ev {
                Event::Key(KeyEvent { code: KeyCode::Enter, .. }) => {
                    let new_address = textarea.lines().join("\n");
                    return Ok(Action::ChangeAddress(new_address));
                }
                Event::Key(KeyEvent { code: KeyCode::Esc, .. }) => {
                    editing_address = false;
                }
                Event::Key(input) => {
                    textarea.input(input);
                }
                _ => {}
            }
        } else {
            match ev {
                Event::Key(KeyEvent { code: KeyCode::Char('r' | 'R'), .. }) => return Ok(Action::Retry),
                Event::Key(KeyEvent { code: KeyCode::Char('a' | 'A'), .. }) => {
                    editing_address = true;
                }
                Event::Key(KeyEvent { code: KeyCode::Char('q' | 'Q'), .. }) => return Ok(Action::Quit),
                Event::Key(KeyEvent { code: KeyCode::Esc, .. }) if allow_cancel => return Ok(Action::Cancel),
                _ => {}
            }
        }
    }
}
