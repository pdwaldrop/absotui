use log::info;
use crate::db::crud::{get_listening_session, get_speed_rate, get_is_speed_adjusted_time, get_is_per_item_speed, get_item_speed_rate};
use crate::api::libraries::get_library_perso_view_pod::Chapter;

// Speed in effect, for display: per-item if that setting is on (1.0x if not set yet), else the shared rate.
fn effective_speed_rate(username: &str, id_item: &str) -> f32 {
    if get_is_per_item_speed(username) == "1" {
        return get_item_speed_rate(username, id_item).unwrap_or(1.0);
    }
    get_speed_rate(username).parse().unwrap_or(1.0)
}

// Finds which chapter `current_time` (raw content-time seconds) falls within, using the
// start/end timestamps Audiobookshelf sends per chapter. Returns None if there's no chapter
// data at all (older session, or a book without embedded chapters) or the time is out of range.
pub fn find_current_chapter(chapters: &[Chapter], current_time: f64) -> Option<&Chapter> {
    chapters.iter().find(|c| {
        let start = c.start.unwrap_or(0.0);
        let end = c.end.unwrap_or(f64::MAX);
        current_time >= start && current_time < end
    })
}

// The book/podcast-show name for the currently playing item, for display outside
// the player bar itself (e.g. the terminal window title). Podcasts store
// "Episode Title | Podcast Title" directly as session.title (see
// handle_l_pod_home.rs/handle_l_pod.rs), so this takes just the show name; books
// store the plain title with nothing to split, so it's returned as-is.
pub fn playing_item_name(title: &str) -> &str {
    match title.split_once(" | ") {
        Some((_, podcast_name)) => podcast_name,
        None => title,
    }
}

pub fn player_info(username: &str) -> Vec<String> {
    let mut player_info = Vec::new();
    let is_speed_adjusted_time = get_is_speed_adjusted_time(username) == "1";
    // Default matches VLC's own unamplified normal level - only overwritten below when
    // there's an actual session to read the tracked value from (see update_volume_up/down).
    let mut volume = 100;
    let mut speed_rate: f32 = 1.0;

    match get_listening_session() {
        Ok(Some(session)) => {
            volume = session.volume;
            speed_rate = effective_speed_rate(username, &session.id_item);
            let chapters: Vec<Chapter> = serde_json::from_str(&session.chapters).unwrap_or_default();
            let current_chapter = find_current_chapter(&chapters, session.current_time as f64);

            player_info.push(session.title.clone());
            player_info.push(session.author.clone());

            // Podcast episodes are single audio files with no chapters - VLC still
            // reports a "chapter" for them (always 0, i.e. "Chapter 1"), so that field
            // is only meaningful for books. id_pod is only set for podcast sessions.
            if !session.id_pod.is_empty() {
                player_info.push(String::new());
            } else if let Some(chapter) = current_chapter {
                let title = chapter.title.clone().unwrap_or_default();
                if title.is_empty() {
                    player_info.push(format!("Chapter {}", chapter.id.unwrap_or(0) + 1));
                } else {
                    player_info.push(title);
                }
            } else if let Ok(num) = session.chapter.trim().parse::<u32>() {
                let new_chapter = format!("Chapter {}", num + 1);
                player_info.push(new_chapter);
            } else {
                player_info.push(session.chapter.clone());
            }

            player_info.push(session.is_playback.to_string());

            // `session.current_time` comes straight from VLC's real, unscaled position in
            // the content. When we know the current chapter, Current/Duration/Percent/Left
            // are shown relative to that chapter instead of the whole book - falls back to
            // whole-book numbers when there's no chapter data (older session, or a book
            // without embedded chapters).
            let original_duration = session.duration.parse::<u32>().unwrap_or(0);
            let (chapter_current, chapter_duration) = match current_chapter {
                Some(chapter) => {
                    let start = chapter.start.unwrap_or(0.0);
                    let end = chapter.end.unwrap_or(original_duration as f64);
                    let current = (session.current_time as f64 - start).max(0.0) as u32;
                    let duration = (end - start).max(0.0) as u32;
                    (current, duration)
                }
                None => (session.current_time, original_duration),
            };

            player_info.push(format_time(chapter_current));
            player_info.push(format_time(chapter_duration));

            let remaining_time = chapter_duration.saturating_sub(chapter_current);

            // Session (time so far this playback session) and Remaining are the two fields
            // that can mean either "real, speed-adjusted time" or "raw content time".
            // Current/Duration/Percent always stay in raw content-time, since your position
            // in the story doesn't change depending on how fast you're listening to it.
            if is_speed_adjusted_time {
                player_info.push(format_time(session.elapsed_time));
                let remaining_time_adjusted = (remaining_time as f32 / speed_rate) as u32;
                player_info.push(format_time(remaining_time_adjusted));
            } else {
                let elapsed_content_equivalent = (session.elapsed_time as f32 * speed_rate) as u32;
                player_info.push(format_time(elapsed_content_equivalent));
                player_info.push(format_time(remaining_time));
            }

            let percent_progress = if chapter_duration > 0 {
                (chapter_current as f32 / chapter_duration as f32) * 100.0
            } else {
                0.0
            };
            player_info.push(format!("{}", percent_progress.round() as u32));
        }
        Ok(None) => {
            player_info.push("N/A".to_string());
        }
        Err(e) => {
            player_info.push("Error".to_string());
            info!("[player_info] Error retrieving data: {e}");
        }
    }

    player_info.push(format!("{speed_rate:.1}"));
    player_info.push(if is_speed_adjusted_time { "Real".to_string() } else { "Content".to_string() });
    player_info.push(volume.to_string());

    player_info
}

pub fn format_time(seconds: u32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn podcast_title_returns_just_the_show_name() {
        assert_eq!(playing_item_name("Ep 42: Big News | The X-Files: Cold Cases"), "The X-Files: Cold Cases");
    }

    #[test]
    fn times_keep_two_digit_seconds_under_a_minute_too() {
        assert_eq!(format_time(0), "0:00");
        assert_eq!(format_time(5), "0:05");
        assert_eq!(format_time(65), "1:05");
        assert_eq!(format_time(3605), "1:00:05");
    }

    #[test]
    fn book_title_with_no_separator_is_returned_as_is() {
        assert_eq!(playing_item_name("Alien III"), "Alien III");
    }
}
