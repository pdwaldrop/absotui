use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};
use crate::db::crud::get_is_show_key_bindings;
use crate::ui::theme;

// Shared with tui.rs's Keymap screen (`keymap_entries()`) so this list is only
// ever spelled out once - the player's own control keys aren't tied to any
// particular AppView (the player overlay renders on every screen), so Keymap
// includes these regardless of which screen it was opened from.
pub const PLAYER_KEYS: &[(&str, &str)] = &[
    ("Spc", "Pause/play"), ("p/u", "+/−10s"), ("P/U", "Nxt/prev ch."),
    ("O/I", "Spd +/−"), ("o/i", "Vol +/−"), ("T", "Real/content time"), ("X", "Stop"),
];

/// Rows the Now Playing box takes: its title, details and (when shown) key-binding lines plus
/// the two borders, with no blank rows so the content sits evenly inside it. `standard_layout`
/// reserves exactly this much.
pub fn player_box_height(show_key_bindings: bool) -> u16 {
    if show_key_bindings { 5 } else { 4 }
}

pub fn render_player(area: Rect, buf: &mut ratatui::buffer::Buffer, player_info: Vec<String>, username: &str, footer_height: u16) {
    // player_info() only pushes the full 12 fields this function indexes into on a
    // successful `Ok(Some(session))` read (see src/player/integrated/player_info.rs) -
    // a transient sqlite read error (Ok(None)/Err path, only 4 fields) shouldn't be
    // able to happen anymore now that get_listening_session has a busy_timeout, but
    // indexing a caller-supplied Vec without checking its length first is fragile
    // regardless of how rare that's supposed to be - skip this frame instead of
    // panicking the whole render loop.
    if player_info.len() < 12 {
        log::error!("render_player: player_info has {} fields, need 12 - skipping this frame", player_info.len());
        return;
    }

    let show_key_bindings = get_is_show_key_bindings(username) == "1";
    let block_height = player_box_height(show_key_bindings);

    // This box's position is independent of the App's own layout (it's drawn via a
    // separate code path in main.rs, not as part of `Widget for &mut App`) but has
    // to land exactly on `standard_layout`'s reserved player-gap, which sits right
    // after `main_area` (`Constraint::Fill(1)`, so it grows or shrinks to absorb
    // whatever the footer doesn't need) and right before `refresh_area` (always 1
    // row) and the footer itself. `footer_height` is that frame's *actual* value
    // (`app.last_footer_height`, set by `standard_layout` when `app` was rendered a
    // moment earlier in the same frame - see main.rs), not a guessed worst case: a
    // fixed guess only tolerates the footer varying across a spread of 1 row, and
    // once wrapped-footer spacing widened that spread to 3, a guessed constant was
    // wrong for whichever end of the range it didn't happen to cover (confirmed
    // live: the box's own top border/title vanished under the app's next frame at
    // one end, and the box overlapped the footer at the other). Working back from
    // the bottom: `footer_height` rows for the footer, 1 for `refresh_area`, then
    // this box's own `block_height` rows, landing exactly where `main_area` ends.
    let new_y = area.y + area.height.saturating_sub(footer_height + 1 + block_height);

    let text_area = Rect::new(area.x, new_y, area.width, block_height);

    // Its own box in the active accent color (matching the play icon/progress fill),
    // distinct from every other panel's structural-accent box.
    let block = theme::section_block("Now Playing").border_style(Style::new().fg(theme::ACCENT_ACTIVE));
    let inner = block.inner(text_area);
    block.render(text_area, buf);

    let [title_area, rest_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
    ]).areas(inner);

    let mut key_bindings_line = Line::default();
    if show_key_bindings {
        key_bindings_line = theme::footer_line(PLAYER_KEYS);
    }

    // Volume indicator: a subtle underline beneath "Vol NN%" itself, filled up to
    // volume/200 - same underline-fill convention already used for the time/progress
    // text elsewhere, just applied to this short label instead of a separate bar/row.
    // 200 (not 100) because VLC's own volume can be amplified up to double its normal
    // unamplified level (100%) - see update_volume_up/down.
    let volume: i32 = player_info[11].parse().unwrap_or(100);
    let vol_label = format!("Vol {volume}%");
    let vol_chars: Vec<char> = vol_label.chars().collect();
    let vol_fill = (((volume as f32 / 200.0) * vol_chars.len() as f32).round() as usize).min(vol_chars.len());
    let vol_filled: String = vol_chars[..vol_fill].iter().collect();
    let vol_unfilled: String = vol_chars[vol_fill..].iter().collect();

    let details_line = Line::from(vec![
        Span::raw(format!(
            " {} {} / {} | Session: {} | Left: {} ({}%) | Speed: {}x [{}] | ",
            match player_info[3].as_str() {
                "false" => "⏸".to_string(),
                "true" => "▶".to_string(),
                _ => String::new(),

            },
            player_info[4], // Current time
            player_info[5], // Total duration
            player_info[6], // Session time so far (Real or Content, depending on the T toggle) - resets each playback session
            player_info[7], // Remaining time (Real or Content, depending on the T toggle)
            player_info[8], // Percent progress
            player_info[9], // Speed rate
            player_info[10], // "Real" or "Content" mode indicator
        )),
        Span::styled(vol_filled, Style::default().add_modifier(Modifier::UNDERLINED)),
        Span::raw(vol_unfilled),
    ]);

    let paragraph = Paragraph::new(vec![details_line, key_bindings_line])
        .centered()
        .block(Block::default());

    paragraph.render(rest_area, buf);

    // Title line: progress bar rendered as a background fill directly behind the text,
    // rather than a separate bar, to avoid spending extra vertical space in an already
    // compact player bar.
    // Podcasts store "Episode Title | Podcast Title" directly as the title with an empty
    // author/chapter (see handle_l_pod_home.rs/handle_l_pod.rs), so there's nothing to
    // append here for them - books show "Chapter #. Name | Book" instead.
    let title_text = if player_info[1].is_empty() && player_info[2].is_empty() {
        player_info[0].clone()
    } else {
        format!("{} | {}", player_info[2], player_info[0])
    };
    let percent: f32 = player_info[8].parse().unwrap_or(0.0);

    let chars: Vec<char> = title_text.chars().collect();
    let padding_left = (title_area.width as usize).saturating_sub(chars.len()) / 2;
    let fill_count = (((percent / 100.0) * chars.len() as f32).round() as usize).min(chars.len());
    let filled_text: String = chars[..fill_count].iter().collect();
    let unfilled_text: String = chars[fill_count..].iter().collect();

    let title_line = Line::from(vec![
        Span::raw(" ".repeat(padding_left)),
        Span::styled(filled_text, Style::default().fg(theme::ACCENT_ACTIVE).add_modifier(Modifier::REVERSED)),
        Span::raw(unfilled_text),
    ]);
    Paragraph::new(title_line).render(title_area, buf);
}
