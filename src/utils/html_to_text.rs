use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// Turns description HTML into styled lines: bold/italic tags become styles, `<br>`/`<p>`/`<li>` and
/// `\n` break lines, other tags are dropped. Sloppy markup degrades (a style may run on) rather than
/// panicking.
pub fn html_to_lines(input: &str) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut text = String::new();
    let mut bold = false;
    let mut italic = false;

    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            flush_line(&mut text, &mut spans, &mut lines, bold, italic);
            i += 1;
            continue;
        }
        if c != '<' {
            text.push(c);
            i += 1;
            continue;
        }

        // Only treat `<` as the start of a tag if it's immediately followed by a
        // letter or `/` (basic tag/closing-tag grammar) and a matching `>` actually
        // follows nearby - otherwise it's a stray character (e.g. "5 < 10", "<2020
        // Edition>"), not markup, and must be kept as literal text rather than
        // swallowing everything up to the next unrelated `>` later in the string.
        let next_is_tag_start = chars.get(i + 1).is_some_and(|c| *c == '/' || c.is_ascii_alphabetic());
        let close = next_is_tag_start.then(|| chars[i + 1..].iter().position(|&c| c == '>').map(|p| i + 1 + p)).flatten();

        let Some(close) = close else {
            text.push('<');
            i += 1;
            continue;
        };

        let tag: String = chars[i + 1..close].iter().collect();
        let tag_lower = tag.to_lowercase();
        let is_closing = tag_lower.starts_with('/');
        let tag_name = tag_lower.trim_start_matches('/').trim_end_matches('/').split_whitespace().next().unwrap_or("");
        i = close + 1;

        match tag_name {
            "b" | "strong" => {
                flush_span(&mut text, &mut spans, bold, italic);
                bold = !is_closing;
            }
            "i" | "em" => {
                flush_span(&mut text, &mut spans, bold, italic);
                italic = !is_closing;
            }
            "br" => flush_line(&mut text, &mut spans, &mut lines, bold, italic),
            "p" | "div" if is_closing => {
                flush_line(&mut text, &mut spans, &mut lines, bold, italic);
                lines.push(Line::default());
            }
            "li" if !is_closing => text.push_str("- "),
            "li" => flush_line(&mut text, &mut spans, &mut lines, bold, italic),
            _ => {}
        }
    }
    flush_line(&mut text, &mut spans, &mut lines, bold, italic);

    // Adjacent closing </p>/</div> tags each add a blank separator line - collapse any
    // run of them, and drop one trailing blank line left by a final closing tag.
    lines.dedup_by(|a, b| a.width() == 0 && b.width() == 0);
    while lines.len() > 1 && lines.last().is_some_and(|l| l.width() == 0) {
        lines.pop();
    }

    lines
}

fn flush_span(text: &mut String, spans: &mut Vec<Span<'static>>, bold: bool, italic: bool) {
    if text.is_empty() {
        return;
    }
    let decoded = decode_entities(text);
    text.clear();

    let mut style = Style::default();
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    spans.push(Span::styled(decoded, style));
}

fn flush_line(text: &mut String, spans: &mut Vec<Span<'static>>, lines: &mut Vec<Line<'static>>, bold: bool, italic: bool) {
    flush_span(text, spans, bold, italic);
    lines.push(Line::from(std::mem::take(spans)));
}

// Decodes each `&name;` reference in a single left-to-right pass rather than chained
// string replacement - chaining would decode `&amp;` first and then re-scan its own
// output, so a genuinely double-encoded `&amp;lt;` (meant to display literally as
// `&lt;`) would wrongly end up as `<` instead.
fn decode_entities(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '&'
            && let Some(rel_end) = chars[i..].iter().take(10).position(|&c| c == ';') {
                let end = i + rel_end;
                let entity: String = chars[i + 1..end].iter().collect();
                let decoded = match entity.as_str() {
                    "amp" => Some('&'),
                    "quot" => Some('"'),
                    "#39" | "apos" => Some('\''),
                    "nbsp" => Some(' '),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    _ => None,
                };
                if let Some(ch) = decoded {
                    out.push(ch);
                    i = end + 1;
                    continue;
                }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{html_to_lines, decode_entities};

    #[test]
    fn stray_angle_bracket_is_kept_not_swallowed() {
        let lines = html_to_lines("Price < $10 and > $5 after that");
        assert_eq!(lines[0].to_string(), "Price < $10 and > $5 after that");
    }

    #[test]
    fn self_closing_br_still_breaks_the_line() {
        let lines = html_to_lines("one<br/>two");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].to_string(), "one");
        assert_eq!(lines[1].to_string(), "two");
    }

    #[test]
    fn literal_newline_breaks_the_line_like_br() {
        // Plenty of Audiobookshelf descriptions are plain text (no HTML) and rely on
        // real newlines to separate a numbered list of chapters/episodes.
        let lines = html_to_lines("Intro.\n1. First\n2. Second");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].to_string(), "Intro.");
        assert_eq!(lines[1].to_string(), "1. First");
        assert_eq!(lines[2].to_string(), "2. Second");
    }

    #[test]
    fn double_newline_collapses_to_one_blank_separator_line() {
        let lines = html_to_lines("Intro.\n\n1. First");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].to_string(), "Intro.");
        assert_eq!(lines[1].to_string(), "");
        assert_eq!(lines[2].to_string(), "1. First");
    }

    #[test]
    fn double_encoded_entity_is_not_decoded_twice() {
        assert_eq!(decode_entities("&amp;lt;"), "&lt;");
    }

    #[test]
    fn plain_entities_still_decode() {
        assert_eq!(decode_entities("Tom &amp; Jerry"), "Tom & Jerry");
    }
}
