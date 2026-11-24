use std::io::{self, Write};

/// Turns off the terminal's "alternate scroll" (`?1007`), which turns wheel and trackpad scrolling
/// into arrow keys and moves the list selection. Terminals without it ignore the sequence.
pub fn disable_terminal_scroll_wheel() {
    let mut stdout = io::stdout();
    let _ = write!(stdout, "\x1b[?1007l");
    let _ = stdout.flush();
}

/// Restores alternate scroll mode before handing the terminal back to the shell, so
/// wheel/trackpad scroll keeps working as expected outside the app (shell scrollback,
/// `less`, etc).
pub fn restore_terminal_scroll_wheel() {
    let mut stdout = io::stdout();
    let _ = write!(stdout, "\x1b[?1007h");
    let _ = stdout.flush();
}
