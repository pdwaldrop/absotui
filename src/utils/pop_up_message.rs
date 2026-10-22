use std::io::{Result, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use crossterm::{
    execute,
    style::{Color, SetBackgroundColor},
    terminal, cursor,
};

// pop_message/clear_message write straight to stdout, bypassing ratatui's own
// diff cache - the next `terminal.draw` can decide a cell already matches its
// stale cache and skip repainting it, even though clear_message just blanked it
// for real (see main.rs's 'R'/library-switch handling, which calls
// `terminal.clear()` right after clear_message for exactly this reason). Call
// sites that have a `Terminal` in scope do that directly; call sites that don't
// (background `tokio::spawn`ed playback tasks - see `wait_prev_session_finished`)
// set this flag instead, which the main render loop polls every iteration
// alongside its other cross-task signals (see `poll_pod_ep_fetch` and friends).
// `clear_message` sets it itself, so no caller can blank a row and forget the repaint.
pub static NEEDS_TERMINAL_CLEAR: AtomicBool = AtomicBool::new(false);

pub fn pop_message(stdout: &mut Stdout, lines_from_bottom: u16, message: &str) -> Result<()> {
    let (_cols, rows) = terminal::size()?;
    let target_row = rows.saturating_sub(lines_from_bottom);

    // Clear first: nothing else repaints this row, so a shorter message would otherwise
    // land on top of the tail of a longer one shown before it.
    execute!(
        stdout,
        cursor::MoveTo(0, target_row),
        SetBackgroundColor(Color::Reset),
        terminal::Clear(terminal::ClearType::CurrentLine),
    )?;

    // Raw mode doesn't turn "\n" into a carriage return too, so a multi-line message
    // would otherwise staircase to the right.
    println!("{}", message.replace('\n', "\r\n"));

    Ok(())
}



pub fn clear_message(stdout: &mut Stdout, lines_from_bottom: u16) -> Result<()> {
    NEEDS_TERMINAL_CLEAR.store(true, Ordering::Relaxed);
    crate::utils::wake::wake_main_loop();
    let (_cols, rows) = terminal::size()?;
    let target_row = rows.saturating_sub(lines_from_bottom);

    execute!(
        stdout,
        cursor::MoveTo(0, target_row),
        SetBackgroundColor(Color::Reset),
        terminal::Clear(terminal::ClearType::CurrentLine),
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clearing_a_message_requests_a_full_repaint() {
        NEEDS_TERMINAL_CLEAR.store(false, Ordering::Relaxed);
        let _ = clear_message(&mut std::io::stdout(), 3);
        assert!(NEEDS_TERMINAL_CLEAR.load(Ordering::Relaxed));
    }
}
