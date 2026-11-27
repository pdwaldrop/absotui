use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Block;

// Named ANSI colors, not RGB - a terminal remaps what each of these actually
// renders as based on its own theme, so the accent stays "in theme" instead of
// fighting it the way a fixed RGB value would.
pub const ACCENT_STRUCTURE: Color = Color::Blue;
pub const ACCENT_ACTIVE: Color = Color::Green;
pub const ACCENT_KEY: Color = Color::Yellow;
pub const ACCENT_ERROR: Color = Color::Red;

/// Standard bordered/titled box used for every section (list, info,
/// description, header panels).
pub fn section_block(title: &str) -> Block<'static> {
    Block::bordered()
        .title(title.to_string())
        .border_style(Style::new().fg(ACCENT_STRUCTURE))
}

// U+2800 (Braille blank) rather than a space: footer wrapping trims real whitespace at line ends, which
// would eat the pad of a chip that starts a line.
const CHIP_PAD: char = '\u{2800}';

/// One keybind chip: `keys` rendered as a reverse-video highlighted block,
/// followed by a space and `desc` in plain text - replaces the old
/// "key: description" convention (no colon).
pub fn footer_hint(keys: &str, desc: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(format!("{CHIP_PAD}{keys}{CHIP_PAD}"), Style::default().fg(ACCENT_KEY).add_modifier(Modifier::REVERSED)),
        Span::raw(" "),
        Span::raw(desc.to_string()),
    ]
}

/// Joins hints into one line, 2 spaces between chips.
pub fn footer_line(hints: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, (keys, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.extend(footer_hint(keys, desc));
    }
    Line::from(spans)
}

/// Builds footer text from one ordered list of hints - a single `Line`, left to
/// wrap onto as many rows as the terminal width actually needs (`Wrap{trim:true}`
/// flattens every chip's spans into one grapheme stream before wrapping, so this
/// wraps identically to hand-splitting into multiple lines, just automatically).
pub fn footer_text(hints: &[(&str, &str)]) -> Text<'static> {
    Text::from(footer_line(hints))
}

const HINT_GAP: usize = 2;

// The footer's hints, each kept whole: a hint starts at its reverse-video key chip. Text without
// chips is one piece.
fn footer_pieces(text: &Text<'static>) -> Vec<Vec<Span<'static>>> {
    let mut pieces: Vec<Vec<Span<'static>>> = Vec::new();
    for line in &text.lines {
        for span in &line.spans {
            let starts_hint = span.style.add_modifier.contains(Modifier::REVERSED);
            if starts_hint || pieces.is_empty() {
                pieces.push(Vec::new());
            }
            if let Some(piece) = pieces.last_mut() {
                piece.push(span.clone());
            }
        }
    }
    for piece in &mut pieces {
        while piece.last().is_some_and(|s| s.content.trim().is_empty() && !s.style.add_modifier.contains(Modifier::REVERSED)) {
            piece.pop();
        }
    }
    pieces.retain(|piece| !piece.is_empty());
    pieces
}

fn piece_width(piece: &[Span<'static>]) -> usize {
    piece.iter().map(|s| s.width()).sum()
}

// Greedy packing into rows no wider than `limit`; returns where each row starts.
fn pack(widths: &[usize], limit: usize) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut row_width = 0;
    for (i, &w) in widths.iter().enumerate() {
        if starts.is_empty() || row_width + HINT_GAP + w > limit {
            starts.push(i);
            row_width = w;
        } else {
            row_width += HINT_GAP + w;
        }
    }
    starts
}

/// Lays the footer out in rows of `width` columns. A hint is never split from its key, and when the
/// footer needs more than one row the rows are made as even as possible instead of leaving a
/// lone hint on the last one.
pub fn footer_rows(text: &Text<'static>, width: u16) -> Vec<Line<'static>> {
    let pieces = footer_pieces(text);
    if pieces.is_empty() {
        return vec![Line::default()];
    }
    let widths: Vec<usize> = pieces.iter().map(|p| piece_width(p)).collect();
    let width = usize::from(width).max(1);
    let rows_needed = pack(&widths, width).len();
    let widest = widths.iter().copied().max().unwrap_or(0);
    let limit = (widest..=width.max(widest))
        .find(|&limit| pack(&widths, limit).len() <= rows_needed)
        .unwrap_or(width);
    let starts = pack(&widths, limit);

    starts.iter().enumerate().map(|(row, &start)| {
        let end = starts.get(row + 1).copied().unwrap_or(pieces.len());
        let mut spans = Vec::new();
        for (i, piece) in pieces[start..end].iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw(" ".repeat(HINT_GAP)));
            }
            spans.extend(piece.iter().cloned());
        }
        Line::from(spans)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows_as_text(hints: &[(&str, &str)], width: u16) -> Vec<String> {
        footer_rows(&footer_text(hints), width).iter()
            .map(|line| line.spans.iter().map(|s| s.content.replace(CHIP_PAD, "")).collect::<String>())
            .collect()
    }

    const HINTS: &[(&str, &str)] = &[("l", "Play"), ("F", "Finished"), ("d", "Download"), ("Q/Esc", "Quit")];

    #[test]
    fn a_footer_that_fits_is_one_row_like_before() {
        assert_eq!(rows_as_text(HINTS, 80), vec!["l Play  F Finished  d Download  Q/Esc Quit"]);
    }

    #[test]
    fn a_hint_is_never_split_from_its_key_and_rows_come_out_even() {
        let rows = rows_as_text(HINTS, 40);
        assert_eq!(rows, vec!["l Play  F Finished", "d Download  Q/Esc Quit"]);
        assert!(rows.iter().all(|row| row.chars().count() <= 40));
    }

    #[test]
    fn plain_text_without_chips_is_kept() {
        let rows = footer_rows(&Text::from("Working..."), 40);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].to_string(), "Working...");
    }
}
