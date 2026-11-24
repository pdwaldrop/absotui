use std::io::{Result, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use crossterm::{
    execute,
    style::{Color, SetBackgroundColor},
    terminal, cursor,
};

// Status messages print straight to the terminal, bypassing ratatui's diff cache. Code without a
// `Terminal` sets this and the main loop repaints fully; `clear_message` sets it itself.
pub static NEEDS_TERMINAL_CLEAR: AtomicBool = AtomicBool::new(false);

pub const PLAYBACK_START_FAILED: &str = "Couldn't start playback - check the server connection and try again";

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
