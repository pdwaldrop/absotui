use crate::login_app::AppLogin;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::text::Line;
use ratatui::style::Style;
use ratatui::{Frame, Terminal};
use std::io;
use std::time::Duration;
use ratatui_textarea::TextArea;
use ratatui::layout::{Constraint, Layout, Rect};
use crate::ui::theme;
use crate::api::server::auth_process::{auth_process, finish_login};
use crate::api::server::oidc_login::{AuthMethods, SsoUpdate, base_url, fetch_auth_methods, sso_login};
use crossterm::event::{self, KeyEvent, KeyCode};
use log::{info, error};
use tokio::sync::mpsc;
use crate::utils::exit_app::clean_exit;
use crate::utils::pop_up_message::pop_message;
use crate::db::crud::{get_others, update_login_err, update_auth_in_progress};

const VERSION: &str = env!("CARGO_PKG_VERSION");

type LoginTerminal = Terminal<CrosstermBackend<io::StdoutLock<'static>>>;

enum SsoMessage {
    Update(SsoUpdate),
    Done,
}

fn input_block(title: &str, hint: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .title(title.to_string())
        .title_bottom(Line::from(format!("🦜Absotui v{VERSION} - {hint}")).right_aligned())
        .border_style(Style::new().fg(theme::ACCENT_STRUCTURE))
}

// Runs inside the render loop on a runtime worker thread, so the probe has to hand the
// thread back to the runtime while it waits instead of blocking it outright.
fn probe_auth_methods(server_address: &str) -> Option<AuthMethods> {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(fetch_auth_methods(server_address))
    })
}

fn paste_box() -> TextArea<'static> {
    let mut textarea = TextArea::default();
    textarea.set_block(input_block(
        "Browser can't reach this machine? Paste the address it ended up on, then Enter",
        "Esc to cancel.",
    ));
    textarea
}

fn draw_sso_screen(frame: &mut Frame, idp_url: &str, status: &str, paste: &TextArea) {
    let area = frame.area();
    let width = (area.width * 3 / 4).max(40).min(area.width);
    let x = area.x + (area.width - width) / 2;
    let inner_width = usize::from(width.saturating_sub(2)).max(1);
    let url_lines = idp_url.len().div_ceil(inner_width).max(1) as u16;
    let info_height = (url_lines + 7).min(area.height.saturating_sub(3));

    let [_, info_area, paste_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(info_height),
        Constraint::Length(3),
        Constraint::Fill(1),
    ])
    .areas(Rect { x, y: area.y, width, height: area.height });

    let info = Paragraph::new(vec![
        Line::from("Your browser should open to finish signing in."),
        Line::from("If it doesn't, open this link in any browser:"),
        Line::from(""),
        Line::from(idp_url.to_string()),
        Line::from(""),
        Line::from(status.to_string()),
    ])
    .wrap(Wrap { trim: false })
    .block(input_block("Sign in with SSO", "Esc to cancel."));

    frame.render_widget(info, info_area);
    frame.render_widget(paste, paste_area);
}

impl AppLogin {
    pub fn auth(&mut self) -> io::Result<()> {
        info!("[auth_input] Login");

        // init input area
        let stdout = io::stdout();
        let stdout = stdout.lock();

        let backend = CrosstermBackend::new(stdout);
        let mut term = Terminal::new(backend)?;

        let mut textarea1 = TextArea::default();
        textarea1.set_block(input_block("Server address", "Esc to quit."));

        textarea1.set_placeholder_text("http:// or https:// required");

        let mut textarea2 = TextArea::default();
        textarea2.set_block(input_block("Username", "Esc to quit."));

        let mut textarea3 = TextArea::default();
        textarea3.set_block(input_block("Password", "Esc to quit."));
        textarea3.set_mask_char('\u{2022}');

        // display
        let size = term.size()?;
        let input_area = Rect {
            x: (size.width - size.width / 2) / 2,
            y: (size.height - 3) / 2,
            width: size.width / 2,
            height: 3,
        };

        // init variables
        let mut textareas = [textarea1, textarea2, textarea3];
        let mut current_index = 0;
        let mut collected_data : Vec<String> = Vec::new();
        // The server offers SSO next to passwords (empty username = SSO) or instead of them.
        let mut sso_offered_alongside_password = false;
        let mut sso_requested = false;

        loop {
            term.draw(|f| {
                f.render_widget(&textareas[current_index], input_area);
            })?;

            // display error message (in any)
            let mut stdout = std::io::stdout();
            let error_message_login = match get_others() {
                Ok(Some(value)) => value.login_err,
                Ok(None) => {
                    String::new()
                }
                Err(e) => {
                    info!("ERROR: Failed to get login error: {e}");
                    String::new()
                }};
            let _ = pop_message(&mut stdout, 6, error_message_login.as_str());

            match crossterm::event::read()? {
                event::Event::Key(KeyEvent { code: KeyCode::Enter, .. }) => {
                    if current_index == 0 {
                        let server_address = base_url(&textareas[0].lines().join("\n")).to_string();
                        let methods = probe_auth_methods(&server_address);
                        collected_data.push(server_address);
                        match methods {
                            Some(m) if m.openid && !m.local => sso_requested = true,
                            Some(m) if m.openid => {
                                sso_offered_alongside_password = true;
                                textareas[1].set_block(input_block("Username (leave empty to sign in with SSO)", "Esc to quit."));
                            }
                            _ => {}
                        }
                        if sso_requested {
                            break;
                        }
                        current_index += 1;
                    } else if current_index < textareas.len() - 1 {
                        // will just take textarea 1 and 2, 3 will take after break loop

                        let username = textareas[current_index].lines().join("\n");
                        if sso_offered_alongside_password && username.trim().is_empty() {
                            sso_requested = true;
                            break;
                        }
                        collected_data.push(username);
                        current_index += 1;
                    } else {
                        break;
                    }
                }

                event::Event::Key(KeyEvent { code: KeyCode::Esc, .. }) => {
                    let _ = update_login_err("");
                    clean_exit();
                }

                event::Event::Key(input) => {
                    if let Some(active_textarea) = textareas.get_mut(current_index) {
                        active_textarea.input(input);
                    }
                }
                _ => {}
            }
        }

        if sso_requested {
            let server_address = collected_data[0].clone();
            return self.sso_sign_in(&mut term, server_address);
        }

        // save the last input (from textearea3)
        collected_data.push(textareas[current_index].lines().join("\n"));

        // make disappear search_area (the input bar) after the break loop
        term.draw(|f| {
            let empty_block = Block::default();
            f.render_widget(empty_block, input_area);

        })?;


        // Fetch data from api and insert them in database

        // send result
        if let Some(_active_textarea) = textareas.get(current_index) {
            let collected_data_clone = collected_data.clone();
            // Set before spawning and cleared in both branches below once auth_process
            // actually finishes - main.rs's login loop polls this instead of guessing a
            // fixed delay before re-checking whether the database now has credentials
            // (previously a blind 1s sleep, which failed - requiring the user to log in
            // twice - whenever the server took longer than that to respond).
            let _ = update_auth_in_progress("1");
            tokio::spawn(async move {
                match auth_process(
                    collected_data_clone[1].as_str(), // username
                    collected_data_clone[2].as_str(), // password
                    collected_data_clone[0].as_str(), // server_address
                ).await {
                    Ok(_response) => {
                        info!("[auth_process] Login successful");
                        let _ = update_login_err("");
                    }
                    Err(e) => {
                        error!("[auth_process] Login failed: {e}");
                        let err = format!("ERROR: {e}");
                        let _ = update_login_err(err.as_str());
                    }
                }
                let _ = update_auth_in_progress("0");
            });

            // to quit the current thread and back to login or home (if connection is successful)
            // should_exit allow to quit the terminal in login_app.rs
            print!("\x1B[2J\x1B[1;1H"); // clean all prints displayed
            self.should_exit = true;

            Ok(())
        } else {
            Err(io::Error::other("Invalid textarea"))
        }
    }

    /// Browser sign-in through the server's identity provider. Stays on this screen until
    /// it finishes so the user can cancel or paste an address; a failure is left in the
    /// database for the login screen that follows to show, same as a failed password login.
    fn sso_sign_in(&mut self, term: &mut LoginTerminal, server_address: String) -> io::Result<()> {
        let port = self.config.auth.sso_callback_port;
        let (message_tx, mut message_rx) = mpsc::unbounded_channel::<SsoMessage>();
        let (paste_tx, paste_rx) = mpsc::unbounded_channel::<String>();

        let _ = update_auth_in_progress("1");
        let task = tokio::spawn(async move {
            let progress_tx = message_tx.clone();
            let outcome = match sso_login(&server_address, port, paste_rx, move |update| {
                let _ = progress_tx.send(SsoMessage::Update(update));
            })
            .await
            {
                Ok(tokens) => {
                    finish_login(&tokens.username, &server_address, &tokens.access_token, tokens.refresh_token.as_deref()).await
                }
                Err(e) => Err(e),
            };
            match outcome {
                Ok(()) => {
                    info!("[sso_login] Login successful");
                    let _ = update_login_err("");
                }
                Err(e) => {
                    error!("[sso_login] Login failed: {e}");
                    let _ = update_login_err(format!("ERROR: {e}").as_str());
                }
            }
            let _ = update_auth_in_progress("0");
            let _ = message_tx.send(SsoMessage::Done);
        });

        let mut idp_url = String::new();
        let mut status = String::from("Starting sign-in...");
        let mut paste = paste_box();
        term.clear()?;

        loop {
            loop {
                match message_rx.try_recv() {
                    Ok(SsoMessage::Update(SsoUpdate::Url(url))) => idp_url = url,
                    Ok(SsoMessage::Update(SsoUpdate::Status(text))) => status = text,
                    Ok(SsoMessage::Done) => {
                        print!("\x1B[2J\x1B[1;1H");
                        self.should_exit = true;
                        return Ok(());
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    // The task went away without reporting (a panic) - don't wait forever.
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        error!("[sso_login] Sign-in task ended without a result");
                        let _ = update_auth_in_progress("0");
                        let _ = update_login_err("ERROR: Sign-in stopped unexpectedly - see the log for details.");
                        print!("\x1B[2J\x1B[1;1H");
                        self.should_exit = true;
                        return Ok(());
                    }
                }
            }

            term.draw(|f| draw_sso_screen(f, &idp_url, &status, &paste))?;

            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    event::Event::Key(KeyEvent { code: KeyCode::Esc, .. }) => {
                        task.abort();
                        let _ = update_auth_in_progress("0");
                        let _ = update_login_err("");
                        print!("\x1B[2J\x1B[1;1H");
                        self.should_exit = true;
                        return Ok(());
                    }
                    event::Event::Key(KeyEvent { code: KeyCode::Enter, .. }) => {
                        let text = paste.lines().join("");
                        if !text.trim().is_empty() {
                            let _ = paste_tx.send(text);
                            paste = paste_box();
                        }
                    }
                    event::Event::Key(input) => {
                        paste.input(input);
                    }
                    _ => {}
                }
            }
        }
    }
}
