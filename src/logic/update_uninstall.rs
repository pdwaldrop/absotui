use log::{info, error};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time::{timeout, Duration};
use pty_process::Command as PtyCommand;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Update,
    Uninstall,
}

pub enum ProgressEvent {
    Line(String),
    NeedPassword,
    AuthFailed,
    Finished(Result<(), String>),
}

enum UpdateError {
    AuthFailed,
    Other(String),
}

// Runs the same hello_absotui.sh flow `absotui --update`/`--uninstall` already runs
// (see clap.rs's run_hello_absotui), but from inside a live TUI session instead of a
// fresh CLI invocation: drive the script's own interactive prompts with scripted stdin
// answers, stream its output back line by line instead of letting it own the terminal
// directly, and detect+relay whatever sudo prompt (fingerprint reader first if the system
// has one configured, falling back to a typed password) its own internal `sudo` calls
// happen to trigger along the way. Returns a channel to push typed passwords back in on
// ProgressEvent::NeedPassword, since we don't know up front whether one will even be
// asked for.
pub fn spawn(action: Action) -> (UnboundedReceiver<ProgressEvent>, UnboundedSender<String>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let (password_tx, password_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        run(action, password_rx, tx).await;
    });
    (rx, password_tx)
}

async fn run(action: Action, mut password_rx: UnboundedReceiver<String>, tx: UnboundedSender<ProgressEvent>) {
    match run_update_or_uninstall(action, &mut password_rx, &tx).await {
        Ok(()) => {
            info!("[update_uninstall] {action:?} finished successfully");
            let _ = tx.send(ProgressEvent::Finished(Ok(())));
            crate::utils::wake::wake_main_loop();
        }
        Err(UpdateError::AuthFailed) => {
            let _ = tx.send(ProgressEvent::AuthFailed);
            crate::utils::wake::wake_main_loop();
        }
        Err(UpdateError::Other(e)) => {
            error!("[update_uninstall] {action:?} failed: {e}");
            let _ = tx.send(ProgressEvent::Finished(Err(e)));
            crate::utils::wake::wake_main_loop();
        }
    }
}

// Same one-liner clap.rs's run_hello_absotui builds (fresh copy of the script from
// `stable`, run it, clean up the tmpfile) - `2>&1` merges stderr into the same ordered
// stream `negotiate` reads below.
fn install_script(action: Action) -> String {
    let command_word = match action {
        Action::Update => "update",
        Action::Uninstall => "uninstall",
    };
    format!(
        r#"tmpfile=$(mktemp) && curl -LsSf https://github.com/pdwaldrop/absotui/raw/stable/hello_absotui.sh -o "$tmpfile" && bash "$tmpfile" {command_word} 2>&1; rc=$?; rm -f "$tmpfile"; exit $rc"#
    )
}

// No separate sudo pre-validation/keepalive here on purpose (a prior version had one,
// and it was a silent no-op: `spawn_borrowed` makes every process it spawns claim the
// pty as its controlling terminal, which fails while the script's own session already
// holds it). Just run the script - its own first internal `sudo` call triggers the
// same `NeedPassword` prompt via `negotiate` below, with nothing competing for the pty.
// Also fixes macOS, which used to need a second, separately-ticketed pty for this.
async fn run_update_or_uninstall(
    action: Action,
    password_rx: &mut UnboundedReceiver<String>,
    tx: &UnboundedSender<ProgressEvent>,
) -> Result<(), UpdateError> {
    let (mut pty, pts) = pty_process::open().map_err(|e| UpdateError::Other(format!("Couldn't open a pty: {e}")))?;
    let mut script_cmd = PtyCommand::new("bash");
    script_cmd = script_cmd.args(["-c", &install_script(action)]);
    let mut script_child = script_cmd
        .spawn(pts)
        .map_err(|e| UpdateError::Other(format!("Failed to launch installer: {e}")))?;

    finish_script(&mut pty, &mut script_child, action, password_rx, tx).await
}

async fn finish_script(
    pty: &mut pty_process::Pty,
    child: &mut tokio::process::Child,
    action: Action,
    password_rx: &mut UnboundedReceiver<String>,
    tx: &UnboundedSender<ProgressEvent>,
) -> Result<(), UpdateError> {
    // Answers the script's own interactive prompts in order: "update to latest? (Y/n)"
    // then "which install method?" for update (always "1" - download the precompiled
    // binary; see the plan's scope decision on why source-method updates stay a manual
    // `absotui --update` thing), or just "uninstall? (Y/n)" for uninstall (we already
    // got real confirmation in our own UI before getting here). Written blind and
    // up front, same as before this file started using a pty: since we already know
    // these answers regardless of what's on screen, there's no need to wait for the
    // prompts to actually appear first.
    let answers = match action {
        Action::Update => "y\n1\n",
        Action::Uninstall => "y\n",
    };
    let _ = pty.write_all(answers.as_bytes()).await;

    // hello_absotui.sh's curl calls are silent (-LsSf, no progress meter), so a real
    // binary download produces no output at all until it finishes - this timeout is a
    // hard ceiling on the whole phase, not an inactivity timeout, so it can't tell that
    // apart from a genuine hang. 180s turned out too tight for a normal download over a
    // home connection (confirmed live: killed a real update mid-download). Bumped well
    // above what a slow-but-healthy download/extract/install should ever need.
    let (status, saw_prompt) = negotiate(pty, child, password_rx, tx, Duration::from_secs(1800), "running the installer")
        .await
        .map_err(UpdateError::Other)?;
    if status.success() {
        Ok(())
    } else if saw_prompt {
        // A prompt appeared and the script still failed - sudo gave up after repeated
        // wrong-password/fingerprint attempts, same shape as the old dedicated
        // pre-validation call used to detect (see the comment on `negotiate` above).
        Err(UpdateError::AuthFailed)
    } else {
        let code = status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        Err(UpdateError::Other(format!("Installer exited with status {code}")))
    }
}

// Read/prompt/answer loop for the script phase above - loops rather than asking once,
// to cover sudo retrying after a wrong password. Also reports whether the failure looks
// like an auth failure: a prompt alone isn't enough, since a correctly-answered one sets
// it too, so `progressed_since_prompt` requires no further script output afterward
// before treating a non-zero exit as auth-related rather than an unrelated failure.
async fn negotiate(
    pty: &mut pty_process::Pty,
    child: &mut tokio::process::Child,
    password_rx: &mut UnboundedReceiver<String>,
    tx: &UnboundedSender<ProgressEvent>,
    chunk_timeout: Duration,
    phase: &str,
) -> Result<(std::process::ExitStatus, bool), String> {
    let mut partial = String::new();
    let mut buf = [0u8; 1024];
    let mut saw_prompt = false;
    let mut progressed_since_prompt = false;

    loop {
        // Only wraps the machine-paced waiting below - once we're actually waiting on
        // the user to type something, there's no clock running (see the
        // `password_rx.recv()` branch further down), so a slow typist can't get cut off.
        match timeout(chunk_timeout, read_until_idle_or_eof(pty, child, &mut buf, &mut partial, tx, &mut progressed_since_prompt)).await {
            Err(_) => {
                let _ = child.start_kill();
                return Err(format!("Timed out {phase}"));
            }
            Ok(Err(e)) => {
                let _ = child.start_kill();
                return Err(format!("Couldn't talk to sudo: {e}"));
            }
            Ok(Ok(ReadOutcome::Eof)) => {
                return child
                    .wait()
                    .await
                    .map(|status| (status, saw_prompt && !progressed_since_prompt))
                    .map_err(|e| format!("sudo process error: {e}"));
            }
            Ok(Ok(ReadOutcome::ChildExited(status))) => {
                return Ok((status, saw_prompt && !progressed_since_prompt));
            }
            Ok(Ok(ReadOutcome::Prompt)) => {
                saw_prompt = true;
                progressed_since_prompt = false;
                let _ = tx.send(ProgressEvent::NeedPassword);
                crate::utils::wake::wake_main_loop();
                match password_rx.recv().await {
                    Some(password) => {
                        partial.clear();
                        if pty.write_all(format!("{password}\n").as_bytes()).await.is_err() {
                            let _ = child.start_kill();
                            return Err("Couldn't send password to sudo".to_string());
                        }
                    }
                    None => {
                        // Sender dropped - user backed out of the password screen.
                        let _ = child.start_kill();
                        return Err("Cancelled".to_string());
                    }
                }
            }
        }
    }
}

enum ReadOutcome {
    Eof,
    ChildExited(std::process::ExitStatus),
    Prompt,
}

// Reads from the pty until either the child's side closes (Eof), the current
// unterminated line has gone quiet for a moment (Prompt) - the same way tools like
// `expect` tell an info message apart from a real prompt waiting on input - or the
// child process itself exits (ChildExited), raced against the read on every iteration.
// That last case matters because pty EOF isn't a reliable signal that the child is
// done: `sudo` privilege-separates into a monitor process, and if that monitor (or
// something PAM/fprintd forks) inherits the pty's slave fd without closing its own
// copy, the kernel never delivers EOF on the master side even after the `sudo` we're
// actually tracking has already exited - confirmed live, `sudo -k -v` sitting
// `<defunct>` in `ps` while this loop was still logging quiet spells waiting on data
// that was never coming. An info message (eg. "Place your finger on the fingerprint
// reader") is newline-terminated, so a quiet spell right after one leaves `partial`
// empty and we just keep waiting - that's what covers a fingerprint scan legitimately
// taking its time. A genuine prompt (eg. "[sudo] password for pdwaldrop: ") is never
// newline-terminated, so a quiet spell with something left in `partial` means it's
// actually waiting on us - except `partial` non-empty isn't unique to prompts:
// dpkg only shows its `\r`-updating "Progress: [ XX%] [###...]" trigger meter when
// it thinks stdout is a real terminal, which it does here since everything runs
// through a pty for sudo/fingerprint interactivity - a slow trigger (icon cache,
// man-db, etc. - installing something like vlc pulls in plenty) can leave `partial`
// non-empty with no newline for over 500ms, same shape as a real prompt. Confirmed
// live: this popped the password field mid-`apt install` on two separate VMs where
// a dependency was actually being installed, well after sudo had already been
// authenticated once - the user re-entering a password there did nothing but get
// echoed into dpkg's output, read as a false "NeedPassword" for every such lull.
// `looks_like_password_prompt` narrows "quiet + unterminated" down to text that
// actually resembles one, so idle non-prompt output like this no longer qualifies.
async fn read_until_idle_or_eof(
    pty: &mut pty_process::Pty,
    child: &mut tokio::process::Child,
    buf: &mut [u8],
    partial: &mut String,
    tx: &UnboundedSender<ProgressEvent>,
    progressed_since_prompt: &mut bool,
) -> std::io::Result<ReadOutcome> {
    loop {
        tokio::select! {
            biased;
            status = child.wait() => {
                return Ok(ReadOutcome::ChildExited(status?));
            }
            read_result = timeout(Duration::from_millis(500), pty.read(buf)) => {
                match read_result {
                    Ok(Ok(0)) => return Ok(ReadOutcome::Eof),
                    Ok(Ok(n)) => {
                        partial.push_str(&strip_ansi(&String::from_utf8_lossy(&buf[..n])));
                        while let Some(pos) = partial.find('\n') {
                            let line: String = partial.drain(..=pos).collect();
                            let line = line.trim_end_matches(['\r', '\n']);
                            if !line.trim().is_empty() {
                                *progressed_since_prompt = true;
                                let _ = tx.send(ProgressEvent::Line(line.to_string()));
                                crate::utils::wake::wake_main_loop();
                            }
                        }
                    }
                    // EIO on Linux is how a pty master read() reports "the slave side closed" -
                    // there's no clean EOF-as-Ok(0) guarantee like a pipe.
                    Ok(Err(e)) if e.raw_os_error() == Some(5) => return Ok(ReadOutcome::Eof),
                    Ok(Err(e)) => return Err(e),
                    Err(_) => {
                        if looks_like_password_prompt(partial) {
                            return Ok(ReadOutcome::Prompt);
                        }
                    }
                }
            }
        }
    }
}

// Real sudo/PAM password prompts (English or otherwise) consistently end in a
// colon, e.g. "[sudo] password for pdwaldrop: " or a bare "Password: " - unlike
// dpkg's "Progress: [ XX%] [###...]" trigger meter, which ends in a percentage/
// bracket and never contains "password". Requiring both narrows the idle-quiet
// heuristic above back down to things that are actually asking for a password.
fn looks_like_password_prompt(text: &str) -> bool {
    let trimmed = text.trim_end();
    trimmed.to_lowercase().contains("password") && trimmed.ends_with(':')
}

// hello_absotui.sh's own output is plain multi-line `echo`, except the changelog
// display which uses ANSI dim/reset codes, and one binary-download curl call that can
// emit a `\r`-driven progress meter if its own -s/-sS flags don't suppress it. Strip
// both defensively rather than trusting every code path upstream to stay silent.
fn strip_ansi(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c2 in chars.by_ref() {
                    if ('\x40'..='\x7e').contains(&c2) {
                        break;
                    }
                }
            }
        } else if c != '\r' {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_sudo_prompt_is_recognized() {
        assert!(looks_like_password_prompt("[sudo] password for pdwaldrop: "));
        assert!(looks_like_password_prompt("Password: "));
    }

    #[test]
    fn dpkg_trigger_progress_meter_is_not_mistaken_for_a_prompt() {
        // strip_ansi already removed the `\r`s by the time this reaches `partial` -
        // this is what's left buffered mid-trigger, with no "password" in sight.
        assert!(!looks_like_password_prompt("Progress: [ 45%] [#######################............]"));
    }

    #[test]
    fn unrelated_idle_output_is_not_mistaken_for_a_prompt() {
        assert!(!looks_like_password_prompt("Reading package lists... 50%"));
    }
}
