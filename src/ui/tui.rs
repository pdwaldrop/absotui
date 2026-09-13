use crate::App;
use crate::app::{AppView, HomeRow, LibraryRow, UpdateUninstallStage, PodcastAddStage, SETTINGS_ABOUT, SETTINGS_UPDATE_UNINSTALL};
use crate::logic::update_uninstall::Action;
use crate::api::libraries::get_library_perso_view_pod::Chapter;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Bar, BarChart, BarGroup, Block, Borders, Cell, HighlightSpacing, List, ListItem , ListState,  Paragraph, Row,
        StatefulWidget, Table, Widget, Wrap
    },
};
use crate::utils::convert_seconds::{convert_seconds, convert_seconds_for_prg, format_age, format_duration};
use crate::utils::format_size::format_sizes;
use crate::db::crud::{get_listening_session, get_is_podcast_autoplay, get_is_vlc_running, get_is_per_item_speed, get_is_auto_download};
use crate::player::integrated::player_info::{format_time, find_current_chapter};
use crate::utils::html_to_text::html_to_lines;
use crate::utils::cover_cache::{cover_cache_path, fetch_and_cache_cover, fetch_and_cache_episode_cover, fetch_and_cache_external_cover};
use crate::utils::changelog::latest_changelog_entry;
use chrono::Datelike;
use crate::ui::theme;
use crate::ui::player_tui;


const VERSION: &str = env!("CARGO_PKG_VERSION");

// Shared by both render_desc_settings (the preview shown while "Update/Uninstall" is
// merely highlighted in the Settings list) and render_update_uninstall_content (the
// Instructions stage once you've actually entered that screen) - one string so the two
// can't drift out of sync the way they did before (the preview kept describing a
// quit-and-run-a-command-yourself flow long after Update now/Uninstall started running
// things right here in-app).
const UPDATE_UNINSTALL_INSTRUCTIONS: &str = "\
Select Update now or Uninstall and press Enter (l/\u{2192}) to run it right here - no need \
to leave the app. Both may ask for your password.

You can still do either manually instead:
- If you built from source: git pull && cargo build --release
- If you installed using the script: absotui --update / absotui --uninstall
";

/// init widget for selected `AppView`
impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.view_state {
            AppView::Home => self.render_home(area, buf),
            AppView::Library => self.render_library(area, buf),
            AppView::SearchBook => self.render_search_book(area, buf),
            AppView::PodcastEpisode => self.render_pod_ep(area, buf),
            AppView::Settings => self.render_settings(area, buf),
            AppView::SettingsAccount => self.render_settings_account(area, buf),
            AppView::SettingsLibrary => self.render_settings_library(area, buf),
            AppView::SettingsAbout => {},
            AppView::SettingsUpdateUninstall => self.render_settings_update_uninstall(area, buf),
            AppView::SettingsAutoplay => self.render_settings_autoplay(area, buf),
            AppView::SettingsPerItemSpeed => self.render_settings_per_item_speed(area, buf),
            AppView::SettingsAutoDownload => self.render_settings_auto_download(area, buf),
            AppView::Keymap => self.render_keymap(area, buf),
            AppView::Collections => self.render_collections(area, buf),
            AppView::Stats => self.render_stats(area, buf),
            AppView::PodcastAdd => self.render_podcast_add(area, buf),
        }
        // Home/Library/Settings/SearchBook/PodcastEpisode render the overlay
        // themselves, anchored to their own Info box - see render_search_overlay's
        // doc comment. Everything else falls back to a generic bottom-anchored spot.
        if self.is_search_active
            && !matches!(self.view_state, AppView::Home | AppView::Library | AppView::Settings | AppView::SearchBook | AppView::PodcastEpisode) {
            self.render_search_overlay_fallback(area, buf);
        }
    }
}


/// Rendering logic
impl App {
    /// `AppView::Home` rendering
    fn render_home(&mut self, area: Rect, buf: &mut Buffer) {
        let text_render_footer = if self.is_podcast {
            let mut hints = vec![("l/→", "Play"), ("F", "Finished"), ("d", "Download"), ("/", "Search"), ("D", "Sort by age")];
            hints.push(Self::FOOTER_SCROLL_DESC);
            hints.extend(Self::footer_trailer("Library", true));
            theme::footer_text(&hints)
        } else {
            let mut hints = vec![("l/→", "Play"), ("c", "Chapters"), ("d", "Download"), ("/", "Search")];
            hints.push(Self::FOOTER_SCROLL_DESC);
            hints.extend(Self::footer_trailer("Library", true));
            theme::footer_text(&hints)
        };

        let [header_area, main_area, _player_area, _refresh_area, footer_area] = self.standard_layout(area, &text_render_footer);

        let [list_area, item_area1, item_area2] = Layout::vertical([Constraint::Fill(1), Constraint::Length(5), Constraint::Fill(1)]).areas(main_area);

        let items_number = self._titles_cnt_list.len();
        let render_list_title = if self.is_podcast {
            format!("New & Unfinished [{items_number} items]")
        } else {
            format!("Continue Listening [{items_number} items]")
        };

        App::render_header(header_area, buf, self.lib_name_type.clone(), &self.username, &self.server_address_pretty, VERSION, &self.update_msg);
        App::render_footer(footer_area, buf, &text_render_footer);

        // Pin the actively-playing item to the top. Runs on every render (not just on
        // load/refresh) so it reacts as soon as playback starts. Books match by id_item;
        // podcasts must match by episode ID (id_pod) since id_item there is the parent
        // podcast's ID, which multiple episodes in this list could share.
        //
        // Gated on is_vlc_running: the listening_session row lingers indefinitely after
        // playback ends (nothing clears it, only the next playback start overwrites it),
        // so without this check, whichever item was last ever played would get yanked
        // to the top of the list on every render forever - including right after a
        // fresh app launch with nothing playing at all.
        if get_is_vlc_running(&self.username) == "1"
            && let Ok(Some(active_session)) = get_listening_session() {
            if !self.is_podcast
                && let Some(pos) = self._ids_cnt_list.iter().position(|id| id == &active_session.id_item)
                && pos != 0 {
                    let selected_id = self.list_state_cnt_list.selected()
                        .and_then(|i| self._ids_cnt_list.get(i))
                        .cloned();

                    let mut order: Vec<usize> = (0..self._ids_cnt_list.len()).collect();
                    order.remove(pos);
                    order.insert(0, pos);

                    self._titles_cnt_list = order.iter().map(|&i| self._titles_cnt_list[i].clone()).collect();
                    self.auth_names_cnt_list = order.iter().map(|&i| self.auth_names_cnt_list[i].clone()).collect();
                    self.pub_year_cnt_list = order.iter().map(|&i| self.pub_year_cnt_list[i].clone()).collect();
                    self.duration_cnt_list = order.iter().map(|&i| self.duration_cnt_list[i]).collect();
                    self.size_cnt_list = order.iter().map(|&i| self.size_cnt_list[i]).collect();
                    self.desc_cnt_list = order.iter().map(|&i| self.desc_cnt_list[i].clone()).collect();
                    self._ids_cnt_list = order.iter().map(|&i| self._ids_cnt_list[i].clone()).collect();
                    self.book_progress_cnt_list = order.iter().map(|&i| self.book_progress_cnt_list[i].clone()).collect();
                    self.book_progress_cnt_list_cur_time = order.iter().map(|&i| self.book_progress_cnt_list_cur_time[i].clone()).collect();

                    if let Some(id) = selected_id
                        && let Some(new_pos) = self._ids_cnt_list.iter().position(|i| *i == id) {
                            self.list_state_cnt_list.select(Some(new_pos));
                    }
            }

            if self.is_podcast
                && let Some(pos) = self.ids_ep_cnt_list.iter().position(|id| id == &active_session.id_pod)
                && pos != 0 {
                    let selected_ep_id = self.list_state_cnt_list.selected()
                        .and_then(|i| self.ids_ep_cnt_list.get(i))
                        .cloned();

                    let mut order: Vec<usize> = (0..self.ids_ep_cnt_list.len()).collect();
                    order.remove(pos);
                    order.insert(0, pos);
                    self.reorder_podcast_lists(&order);

                    if let Some(id) = selected_ep_id
                        && let Some(new_pos) = self.ids_ep_cnt_list.iter().position(|i| *i == id) {
                            self.list_state_cnt_list.select(Some(new_pos));
                    }
            }
        }

        // Which item (if any) matches the actual active listening session - distinct from
        // wherever the cursor/highlight currently happens to be sitting in the list. Books
        // match by id_item; podcasts must match by episode ID (id_pod), same reasoning as
        // the reorder above.
        //
        // Gated on is_vlc_running for the same reason as the reorder above - the session
        // row lingers after playback ends, so without this the "now playing" marker would
        // sit on whatever was last ever played, forever, even with nothing playing.
        let active_session = (get_is_vlc_running(&self.username) == "1").then(|| get_listening_session().ok().flatten()).flatten();
        let now_playing_id: Option<String> = active_session.as_ref().map(|s| if self.is_podcast { s.id_pod.clone() } else { s.id_item.clone() });

        // Flattened book/chapter rows - plain Book rows 1:1 with _ids_cnt_list unless the
        // chapter list is expanded, in which case it also carries the chapter sub-rows to
        // render beneath the currently-playing book. Kept in sync with input handling
        // (app.rs) since both go through this same method.
        let home_rows = self.build_home_rows();
        let current_chapter_id: Option<i64> = if self.is_chapter_list_expanded {
            active_session.as_ref().and_then(|s| {
                let chapters: Vec<Chapter> = serde_json::from_str(&s.chapters).unwrap_or_default();
                find_current_chapter(&chapters, s.current_time as f64).and_then(|c| c.id)
            })
        } else {
            None
        };

        let progress_info: Option<Vec<(String, f32, bool)>> = if self.is_podcast {
            // Progress percent isn't shown here - it isn't as meaningful for a list
            // already filtered to "new or unfinished" episodes. Instead the time slot
            // shows the episode's age (e.g. "1Day", "2Weeks"), with percent forced to
            // 0.0 so it renders as plain text with no underline fill.
            //
            // Left-aligned within a fixed-width field (trailing spaces, not leading) so
            // the leading character (the digit, or the "T" of "Today") lands in the same
            // column on every row - right-padding instead of left-padding, since the
            // labels vary in length and right-aligning only lines up their trailing
            // edge. Width is wide enough for the longest realistic label ("12Months").
            const AGE_LABEL_WIDTH: usize = 8;
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            Some(self._titles_cnt_list.iter().enumerate().map(|(i, _)| {
                let is_now_playing = self.ids_ep_cnt_list.get(i).is_some_and(|id| Some(id) == now_playing_id.as_ref());
                let age = self.podcast_published_at_cnt_list.get(i)
                    .map(|&published_at| format_age(published_at, now_ms))
                    .unwrap_or_default();
                (format!("{age:<AGE_LABEL_WIDTH$}"), 0.0, is_now_playing)
            }).collect())
        } else {
            Some(home_rows.iter().map(|row| match row {
                HomeRow::Book(i) => {
                    let i = *i;
                    let is_now_playing = self._ids_cnt_list.get(i).is_some_and(|id| Some(id) == now_playing_id.as_ref());
                    let duration = self.duration_cnt_list.get(i).copied().unwrap_or(0.0) as f32;

                    // For the actively-playing book, use the live position from the local
                    // listening_session (updated every second while VLC plays) instead of the
                    // snapshot fetched from the server when the list last loaded - keeps this
                    // one row's progress current without any extra network calls.
                    let current_time = if is_now_playing {
                        active_session.as_ref().map(|s| s.current_time as f32).unwrap_or(0.0)
                    } else {
                        self.book_progress_cnt_list_cur_time.get(i).and_then(|v| v.first()).copied().unwrap_or(0.0) as f32
                    };
                    let percent = if is_now_playing && duration > 0.0 {
                        (current_time / duration) * 100.0
                    } else {
                        self.book_progress_cnt_list.get(i)
                            .and_then(|v| v.first())
                            .and_then(|s| s.trim().parse::<f32>().ok())
                            .unwrap_or(0.0)
                    };
                    // Gate on the raw current_time, not the rounded percent string - a book
                    // with small-but-real progress (e.g. 0.3% into an 11-hour audiobook) would
                    // round to "0" and get misreported as never started.
                    let text = if current_time > 0.0 {
                        format!("{} / {} ({}%)", format_time(current_time as u32), format_time(duration as u32), percent.round() as u32)
                    } else {
                        "Not started".to_string()
                    };
                    (text, percent, is_now_playing)
                }
                // Chapter rows render as plain indented rows (no time text/underline, no
                // now-playing marker box) - which chapter is current is shown inline in
                // the title itself instead, see display_titles below.
                HomeRow::Chapter { .. } => (String::new(), 0.0, false),
            }).collect())
        };
        // Podcasts: show "Episode Title | Podcast Title" in the list row, not just the
        // episode title alone - _titles_cnt_list is episode titles, titles_pod_cnt_list
        // is the parent podcast's own title.
        // Downloaded status is local-only state, not part of the fetched list data -
        // looked up directly here rather than threaded through as another parallel
        // array (see the parallel-arrays warning in CLAUDE.md for why that'd be worth
        // avoiding for something this orthogonal to the server-fetched row data).
        //
        // The marker is prefixed, not suffixed - a suffix on a long title gets cut off
        // entirely by the truncation/ellipsis below (see MIN_TITLE_GAP and the scroll
        // logic in render_list), since it never survives past the ellipsis on an
        // unselected long row. A prefix is always visible.
        const DOWNLOADED_MARKER: &str = "⬇ ";
        let display_titles: Vec<String> = if self.is_podcast {
            self._titles_cnt_list.iter().enumerate().map(|(i, ep_title)| {
                let title = match self.titles_pod_cnt_list.get(i) {
                    Some(pod_title) => format!("{ep_title} | {pod_title}"),
                    None => ep_title.clone(),
                };
                let is_downloaded = self.ids_ep_cnt_list.get(i)
                    .is_some_and(|id| crate::utils::download_cache::is_downloaded(&self.username, id));
                if is_downloaded {
                    format!("{DOWNLOADED_MARKER}{title}")
                } else {
                    title
                }
            }).collect()
        } else {
            home_rows.iter().map(|row| match row {
                HomeRow::Book(i) => {
                    let title = self._titles_cnt_list.get(*i).cloned().unwrap_or_default();
                    let is_downloaded = self._ids_cnt_list.get(*i)
                        .is_some_and(|id| crate::utils::download_cache::is_downloaded(&self.username, id));
                    if is_downloaded {
                        format!("{DOWNLOADED_MARKER}{title}")
                    } else {
                        title
                    }
                }
                HomeRow::Chapter { chapter, .. } => {
                    let title = chapter.title.clone().unwrap_or_default();
                    let label = if title.is_empty() {
                        format!("Chapter {}", chapter.id.unwrap_or(0) + 1)
                    } else {
                        title
                    };
                    let is_current_chapter = chapter.id.is_some() && chapter.id == current_chapter_id;
                    let marker = if is_current_chapter { "●" } else { " " };
                    format!("    {marker} {label}")
                }
            }).collect()
        };
        self.render_list(list_area, buf, &render_list_title, &display_titles, &mut self.list_state_cnt_list.clone(), progress_info.as_deref());
        if !&self._titles_cnt_list.is_empty() {
            self.render_info_home(item_area1, buf, &self.list_state_cnt_list.clone());
            self.render_desc_home(item_area2, buf, &self.list_state_cnt_list.clone());
        }
        if self.is_search_active {
            self.render_search_overlay(item_area1, buf);
        }
    }

    /// `AppView::Library` rendering
    fn render_library(&mut self, area: Rect, buf: &mut Buffer) {
        // Library's own Tab target is the only one that depends on runtime data -
        // Collections only exists in the ring once this library actually has any,
        // in which case it - not Stats - is the next stop (see toggle_view).
        let tab_target = if self.collection_names.is_empty() { "Stats" } else { "Collections" };
        let back_hint = self.active_collection.is_some().then_some(("h", "Back to collections"));

        let _text_render_footer = if self.podcast_remove_confirm {
            // Matches account_removal_confirm's own footer swap - a destructive
            // action's confirm owns the footer entirely, no scroll/nav hints mixed in.
            theme::footer_text(&[("s", "Soft"), ("h", "Hard"), ("n/Esc", "Cancel")])
        } else if self.is_podcast {
            let mut hints = vec![("l/→", "Episodes"), ("/", "Search"), ("A", "Add podcast"), ("C", "Cancel subscription")];
            hints.extend(back_hint);
            hints.push(Self::FOOTER_SCROLL_DESC);
            hints.extend(Self::footer_trailer(tab_target, true));
            theme::footer_text(&hints)
        } else {
            let mut hints = vec![("l/→", "Play"), ("/", "Search"), ("S", "Group by series")];
            hints.extend(back_hint);
            hints.push(Self::FOOTER_SCROLL_DESC);
            hints.extend(Self::footer_trailer(tab_target, true));
            theme::footer_text(&hints)
        };

        let [header_area, main_area, _player_area, _refresh_area, footer_area] = self.standard_layout(area, &_text_render_footer);

        let [list_area, item_area1, item_area2] = Layout::vertical([Constraint::Fill(1), Constraint::Length(5), Constraint::Fill(1)]).areas(main_area);

        let rows = self.build_library_rows();
        let display_titles: Vec<String> = rows.iter().map(|row| match row {
            LibraryRow::SeriesHeader(name) => format!("▸ {name}"),
            LibraryRow::Book(i) => {
                let title = &self.titles_library[*i];
                if self.is_library_grouped_by_series {
                    match self.series_sequence_library.get(*i).copied().flatten() {
                        Some(seq) => format!("  #{seq} {title}"),
                        None => format!("  {title}"),
                    }
                } else {
                    title.clone()
                }
            }
        }).collect();
        let items_number = rows.iter().filter(|row| matches!(row, LibraryRow::Book(_))).count();
        let render_list_title = match self.active_collection {
            Some(index) => format!("{} [{items_number} items]", self.collection_names[index]),
            None => format!("Library [{items_number} items]"),
        };

        App::render_header(header_area, buf, self.lib_name_type.clone(), &self.username, &self.server_address_pretty, VERSION, &self.update_msg);
        App::render_footer(footer_area, buf, &_text_render_footer);
        self.render_list(list_area, buf, &render_list_title, &display_titles, &mut self.list_state_library.clone(), None);
        // Resolved to the real book index behind the current row selection (a series
        // header has nothing to show here) - see `selected_library_book_index`'s doc
        // comment for why this can't just be `list_state_library` itself. Resolved
        // against `rows` (already built above for display) rather than calling
        // `selected_library_book_index()`, which would redo the same grouping/sort pass.
        if let Some(book_index) = App::resolve_library_book_index(self.list_state_library.selected(), &rows) {
            let mut resolved_state = ListState::default();
            resolved_state.select(Some(book_index));
            self.render_info_library(item_area1, buf, &resolved_state);
            self.render_desc_library(item_area2, buf, &resolved_state);
        }
        if self.is_search_active {
            self.render_search_overlay(item_area1, buf);
        }
        if matches!(self.podcast_add_stage, Some(PodcastAddStage::Input)) {
            self.render_podcast_add_input_overlay(item_area1, buf);
        }
        if self.podcast_remove_confirm {
            self.render_podcast_remove_confirm_overlay(item_area1, buf);
        }
    }

    /// `AppView::Collections` rendering - the index list only. Selecting one filters
    /// `AppView::Library` via `active_collection` rather than opening its own detail
    /// screen (see `render_library`).
    fn render_collections(&mut self, area: Rect, buf: &mut Buffer) {
        let mut hints = vec![("l/→ Enter", "Open collection")];
        hints.extend(Self::footer_trailer("Home", true));
        let _text_render_footer = theme::footer_text(&hints);

        let [header_area, main_area, _player_area, _refresh_area, footer_area] = self.standard_layout(area, &_text_render_footer);

        let render_list_title = "Collections";
        let rows: Vec<String> = self.collection_names.iter().enumerate()
            .map(|(index, name)| format!("{name} ({} books)", self.collection_book_indices[index].len()))
            .collect();

        App::render_header(header_area, buf, self.lib_name_type.clone(), &self.username, &self.server_address_pretty, VERSION, &self.update_msg);
        App::render_footer(footer_area, buf, &_text_render_footer);
        self.render_list(main_area, buf, render_list_title, &rows, &mut self.list_state_collections.clone(), None);
    }

    /// `AppView::Stats` rendering - a read-only dashboard built from `self.stats_summary`
    /// (see `collect_stats_summary`), itself built from Audiobookshelf's per-user
    /// `/api/me/listening-stats` - not scoped to the currently selected library.
    // Panel heights for the virtual canvas below - kept as named constants since
    // both the canvas's total height and each panel's Layout::vertical slice need
    // to agree on them.
    const STATS_OVERVIEW_H: u16 = 6;
    const STATS_CHART_H: u16 = 10;
    const STATS_DAY_OF_WEEK_H: u16 = 10;
    const STATS_HEATMAP_H: u16 = 12;
    const STATS_RANKINGS_H: u16 = 16;
    const STATS_RECENT_SESSIONS_H: u16 = 10;

    /// The Stats screen has more content than fits most terminal heights at once, so
    /// unlike every other screen it renders into a tall off-screen `Buffer` (sized to
    /// every panel's natural height) and copies a `scroll_offset`-shifted window of
    /// that into the real visible area - the same technique `render_footer` already
    /// uses for its own variable-height content, just applied to a whole screen of
    /// mixed widgets instead of one wrapped `Paragraph`.
    fn render_stats(&mut self, area: Rect, buf: &mut Buffer) {
        let mut hints = vec![("J/K/H", "Scroll stats")];
        hints.extend(Self::footer_trailer("Home", true));
        let text_render_footer = theme::footer_text(&hints);

        let [header_area, main_area, _player_area, _refresh_area, footer_area] = self.standard_layout(area, &text_render_footer);

        App::render_header(header_area, buf, self.lib_name_type.clone(), &self.username, &self.server_address_pretty, VERSION, &self.update_msg);
        App::render_footer(footer_area, buf, &text_render_footer);

        let total_h = Self::STATS_OVERVIEW_H + Self::STATS_CHART_H + Self::STATS_DAY_OF_WEEK_H
            + Self::STATS_HEATMAP_H + Self::STATS_RANKINGS_H + Self::STATS_RECENT_SESSIONS_H;
        let canvas_area = Rect::new(0, 0, main_area.width, total_h);
        let mut canvas = Buffer::empty(canvas_area);

        let [overview_area, chart_area, dow_area, heatmap_area, rankings_area, sessions_area] = Layout::vertical([
            Constraint::Length(Self::STATS_OVERVIEW_H),
            Constraint::Length(Self::STATS_CHART_H),
            Constraint::Length(Self::STATS_DAY_OF_WEEK_H),
            Constraint::Length(Self::STATS_HEATMAP_H),
            Constraint::Length(Self::STATS_RANKINGS_H),
            Constraint::Length(Self::STATS_RECENT_SESSIONS_H),
        ]).areas(canvas_area);

        self.render_stats_overview(overview_area, &mut canvas);
        self.render_stats_chart(chart_area, &mut canvas);
        self.render_stats_day_of_week(dow_area, &mut canvas);
        self.render_stats_heatmap(heatmap_area, &mut canvas);

        let [left_area, right_area] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(rankings_area);
        let [most_listened_area, genres_area] = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(left_area);
        let [authors_area, narrators_area] = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(right_area);
        Self::render_stats_ranking(most_listened_area, &mut canvas, "Most Listened", &self.stats_summary.top_items);
        Self::render_stats_ranking(genres_area, &mut canvas, "Top Genres", &self.stats_summary.top_genres);
        Self::render_stats_ranking(authors_area, &mut canvas, "Top Authors", &self.stats_summary.top_authors);
        Self::render_stats_ranking(narrators_area, &mut canvas, "Top Narrators", &self.stats_summary.top_narrators);

        self.render_stats_recent_sessions(sessions_area, &mut canvas);

        let max_scroll = total_h.saturating_sub(main_area.height);
        let scroll = self.scroll_offset.min(max_scroll);
        for row in 0..main_area.height {
            let source_y = scroll + row;
            if source_y >= total_h {
                break;
            }
            for x in 0..main_area.width {
                buf[(main_area.x + x, main_area.y + row)] = canvas[(x, source_y)].clone();
            }
        }
    }

    fn render_stats_overview(&self, area: Rect, buf: &mut Buffer) {
        let s = &self.stats_summary;
        let fmt = format_duration;
        let days_of_audio = s.total_time / 86400.0;
        let daily_average = if s.days_active > 0 { s.total_time / s.days_active as f64 } else { 0.0 };

        let lines = vec![
            Line::from(format!(
                "Total: {} (~{days_of_audio:.1} days of audio)    Today: {}    This week: {}    This month: {}",
                fmt(s.total_time), fmt(s.today), fmt(s.this_week), fmt(s.this_month),
            )),
            Line::from(format!(
                "🔥 Streak: {} day{}    🏆 Best: {} day{}    📅 Days active: {}    ⏱️ Daily avg: {}",
                s.current_streak, if s.current_streak == 1 { "" } else { "s" },
                s.best_streak, if s.best_streak == 1 { "" } else { "s" },
                s.days_active, fmt(daily_average),
            )),
            Line::from(format!("📚 Books: {}    🎧 Episodes: {}", s.books_count, s.episodes_count)),
        ];

        Paragraph::new(lines)
            .block(theme::section_block("Stats Overview"))
            .render(area, buf);
    }

    fn render_stats_chart(&self, area: Rect, buf: &mut Buffer) {
        let fmt = format_duration;
        let bars: Vec<Bar> = self.stats_summary.last_7_days.iter()
            .map(|(date, seconds)| {
                // Minutes, not seconds - BarChart's value drives bar height, and a whole
                // day's worth of seconds would dwarf everything into a rounding error at
                // typical terminal heights.
                let minutes = (seconds / 60.0).round() as u64;
                Bar::default()
                    .label(Line::from(format!("{}", date.weekday())))
                    .value(minutes)
                    .text_value(if *seconds > 0.0 { fmt(*seconds) } else { String::new() })
            })
            .collect();

        BarChart::default()
            .data(BarGroup::default().bars(&bars))
            .bar_width(6)
            .bar_gap(1)
            .bar_style(Style::new().fg(theme::ACCENT_STRUCTURE))
            .value_style(Style::new().fg(theme::ACCENT_STRUCTURE).add_modifier(Modifier::REVERSED))
            .block(theme::section_block("Last 7 Days"))
            .render(area, buf);
    }

    /// `day_of_week_avg` is Monday-first already (see `collect_stats_summary`) -
    /// same `BarChart` construction as `render_stats_chart`, just fed the weekly
    /// average per weekday instead of the literal last 7 days.
    fn render_stats_day_of_week(&self, area: Rect, buf: &mut Buffer) {
        let fmt = format_duration;
        let bars: Vec<Bar> = self.stats_summary.day_of_week_avg.iter()
            .map(|(weekday, seconds)| {
                let minutes = (seconds / 60.0).round() as u64;
                Bar::default()
                    .label(Line::from(format!("{weekday}")))
                    .value(minutes)
                    .text_value(if *seconds > 0.0 { fmt(*seconds) } else { String::new() })
            })
            .collect();

        BarChart::default()
            .data(BarGroup::default().bars(&bars))
            .bar_width(6)
            .bar_gap(1)
            .bar_style(Style::new().fg(theme::ACCENT_STRUCTURE))
            .value_style(Style::new().fg(theme::ACCENT_STRUCTURE).add_modifier(Modifier::REVERSED))
            .block(theme::section_block("Average by Day of Week"))
            .render(area, buf);
    }

    /// Hand-painted GitHub-style contribution graph - no ratatui widget fits this
    /// shape. Cell intensity is bucketed relative to the *user's own* max single-day
    /// value in the visible window (not a fixed absolute threshold), and uses
    /// character density at one accent color rather than a custom RGB gradient,
    /// matching this app's terminal-native-theming principle (no palette to
    /// maintain) - the same choice the bar charts above already make.
    fn render_stats_heatmap(&self, area: Rect, buf: &mut Buffer) {
        let block = theme::section_block("Activity");
        let inner = block.inner(area);
        block.render(area, buf);

        let daily_totals = &self.stats_summary.daily_totals;
        if daily_totals.is_empty() || inner.height < 9 {
            return;
        }

        const LABEL_WIDTH: u16 = 4;
        let week_columns = ((inner.width.saturating_sub(LABEL_WIDTH)) / 2).clamp(1, 52) as usize;

        let today = daily_totals.last().map(|&(d, _)| d).unwrap_or_default();
        // Monday of *this* week, not just "7*N days back" - today isn't necessarily
        // a Sunday, so subtracting whole weeks from it doesn't land on a Monday on
        // its own (confirmed live: with today a Thursday, that math put the grid's
        // "Mon" row on an actual Friday).
        let this_week_start = today - chrono::Duration::days(i64::from(today.weekday().num_days_from_monday()));
        let window_start = this_week_start - chrono::Duration::weeks((week_columns - 1) as i64);
        let by_date = &self.stats_summary.daily_totals_by_date;
        let max_in_window = daily_totals.iter()
            .filter(|&&(d, _)| d >= window_start && d <= today)
            .map(|&(_, s)| s)
            .fold(0.0_f64, f64::max);

        let cell_char = |seconds: f64| -> char {
            if max_in_window <= 0.0 || seconds <= 0.0 { return ' '; }
            match seconds / max_in_window {
                r if r > 0.75 => '█',
                r if r > 0.50 => '▓',
                r if r > 0.25 => '▒',
                _ => '░',
            }
        };

        // Month labels first, so the grid drawn after can overwrite the cells they
        // sit above without needing to dodge them - a label only ever occupies the
        // row above the grid, never the grid itself.
        let month_row_y = inner.y;
        let mut last_labeled_month: Option<u32> = None;
        for week in 0..week_columns {
            let week_start_date = window_start + chrono::Duration::weeks(week as i64);
            if Some(week_start_date.month()) == last_labeled_month {
                continue;
            }
            last_labeled_month = Some(week_start_date.month());
            let x = inner.x + LABEL_WIDTH + (week as u16) * 2;
            if x < inner.x + inner.width {
                buf.set_string(x, month_row_y, week_start_date.format("%b").to_string(), Style::new().fg(theme::ACCENT_STRUCTURE));
            }
        }

        let grid_y = inner.y + 1;
        let weekday_labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        for (row, label) in weekday_labels.iter().enumerate() {
            buf.set_string(inner.x, grid_y + row as u16, label, Style::new().fg(theme::ACCENT_STRUCTURE));
            for week in 0..week_columns {
                // `week` columns run oldest to newest, left to right; `window_start`
                // is a real Monday, so `+ week*7 + row` lands on the matching weekday.
                let date = window_start + chrono::Duration::days((week * 7 + row) as i64);
                if date > today {
                    continue;
                }
                let seconds = by_date.get(&date).copied().unwrap_or(0.0);
                let ch = cell_char(seconds);
                let x = inner.x + LABEL_WIDTH + (week as u16) * 2;
                if x < inner.x + inner.width {
                    buf.set_string(x, grid_y + row as u16, ch.to_string(), Style::new().fg(theme::ACCENT_STRUCTURE));
                }
            }
        }

        let legend_y = grid_y + 7;
        if legend_y < inner.y + inner.height {
            buf.set_string(inner.x, legend_y, "Less ░▒▓█ More", Style::new().fg(theme::ACCENT_STRUCTURE));
        }
    }

    fn render_stats_ranking(area: Rect, buf: &mut Buffer, title: &str, entries: &[(String, f64)]) {
        let fmt = format_duration;
        let lines: Vec<Line> = if entries.is_empty() {
            vec![Line::from("Nothing yet")]
        } else {
            entries.iter().enumerate()
                .map(|(i, (name, seconds))| Line::from(format!("{}. {name} — {}", i + 1, fmt(*seconds))))
                .collect()
        };

        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(theme::section_block(title))
            .render(area, buf);
    }

    fn render_stats_recent_sessions(&self, area: Rect, buf: &mut Buffer) {
        let lines: Vec<Line> = if self.stats_summary.recent_sessions.is_empty() {
            vec![Line::from("Nothing yet")]
        } else {
            self.stats_summary.recent_sessions.iter()
                .map(|session| {
                    let title = session.display_title.as_deref().unwrap_or("Unknown");
                    let author = session.display_author.as_deref().unwrap_or("");
                    let seconds = session.time_listening.unwrap_or(0.0);
                    let duration = format_duration(seconds);
                    let date = session.date.as_deref().unwrap_or("");
                    Line::from(format!("{title} — {author} ({duration}, {date})"))
                })
                .collect()
        };

        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(theme::section_block("Recent Sessions"))
            .render(area, buf);
    }

    /// `AppView::Settings` rendering
    fn render_settings(&mut self, area: Rect, buf: &mut Buffer) {
        let selected = self.list_state_settings.selected();
        // About is the only entry with a list-level preview (render_desc_settings
        // scrolls its changelog right here without entering a separate screen) -
        // Update/Uninstall used to get its own "Scroll instructions" hint too, but
        // render_desc_settings has deliberately never shown anything for it (its
        // Instructions stage already covers that once you actually enter the
        // screen), so that hint advertised a scroll that did nothing. Every other
        // entry, including Update/Uninstall, just gets the normal "enter" hint.
        let _text_render_footer = if selected == self.settings_index(SETTINGS_ABOUT) {
            let mut hints = vec![("h", "Back"), ("J/K/H", "Scroll what's new")];
            hints.extend(Self::footer_trailer("Home", false));
            theme::footer_text(&hints)
        } else {
            let mut hints = vec![("h", "Back"), ("l/→", "See options")];
            hints.extend(Self::footer_trailer("Home", false));
            theme::footer_text(&hints)
        };

        let [header_area, main_area, _player_area, _refresh_area, footer_area] = self.standard_layout(area, &_text_render_footer);

        let [list_area, item_area1, item_area2] = Layout::vertical([Constraint::Fill(1), Constraint::Length(5), Constraint::Fill(1)]).areas(main_area);

        let render_list_title = "Settings";

        App::render_header(header_area, buf, self.lib_name_type.clone(), &self.username, &self.server_address_pretty, VERSION, &self.update_msg);
        App::render_footer(footer_area, buf, &_text_render_footer);
        self.render_list(list_area, buf, render_list_title, &self.settings.clone(), &mut self.list_state_settings.clone(), None);
        self.render_info_settings(item_area1, buf, &self.list_state_settings.clone());
        self.render_desc_settings(item_area2, buf, &self.list_state_settings.clone());
        if self.is_search_active {
            self.render_search_overlay(item_area1, buf);
        }
    }

    /// `AppView::SettingsAccount` rendering
    fn render_settings_account(&mut self, area: Rect, buf: &mut Buffer) {
        // See account_removal_confirm's doc comment - Yes/No while armed, same as
        // Update/Uninstall's own Confirm stage's footer.
        let hints = if self.account_removal_confirm {
            vec![("Y", "Yes"), ("N/Esc", "No")]
        } else {
            let mut h = vec![("h", "Back"), ("l/→", "Remove saved user")];
            h.extend(Self::footer_trailer("Home", false));
            h
        };
        let text_render_footer = theme::footer_text(&hints);
        let [header_area, main_area, _player_area, _refresh_area, footer_area] = self.standard_layout(area, &text_render_footer);

        let [list_area, item_area] = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1),]).areas(main_area);

        let render_list_title = "Remove account";

        App::render_header(header_area, buf, self.lib_name_type.clone(), &self.username, &self.server_address_pretty, VERSION, &self.update_msg);
        App::render_footer(footer_area, buf, &text_render_footer);
        self.render_list(list_area, buf, render_list_title, &self.all_usernames.clone(), &mut self.list_state_settings_account.clone(), None);

        if self.account_removal_confirm {
            // ACCENT_ERROR (red), matching render_list's own accent for this screen -
            // see its doc comment for why removal gets red instead of the yellow other
            // "applies a change" screens use.
            Paragraph::new("Remove this saved account?\n\nThis permanently deletes it from local storage - its server address, auth token, and every setting tied to it. You'll need to log in again on restart.\n\n[Y] Yes   [N] No")
                .wrap(Wrap { trim: true })
                .block(theme::section_block("Confirm").border_style(Style::new().fg(theme::ACCENT_ERROR)))
                .render(item_area, buf);
        } else {
            Paragraph::new("Removing an account here deletes it from local storage - the saved server address, auth token, and every setting tied to it. You'll need to log in again on restart; this can't be undone from within the app.")
                .left_aligned()
                .wrap(Wrap { trim: true })
                .block(theme::section_block("Description"))
                .render(item_area, buf);
        }
    }

    /// `AppView::SettingsLibrary` rendering
    fn render_settings_library(&mut self, area: Rect, buf: &mut Buffer) {
        let mut hints = vec![("h", "Back"), ("l/→", "Switch library")];
        hints.extend(Self::footer_trailer("Home", false));
        let text_render_footer = theme::footer_text(&hints);
        let [header_area, main_area, _player_area, _refresh_area, footer_area] = self.standard_layout(area, &text_render_footer);

        let [list_area, item_area] = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1),]).areas(main_area);

        let items_number = self.libraries_names.len();
        let render_list_title = format!("Settings Library [{items_number} items]");

        App::render_header(header_area, buf, self.lib_name_type.clone(), &self.username, &self.server_address_pretty, VERSION, &self.update_msg);
        App::render_footer(footer_area, buf, &text_render_footer);
        self.render_list(list_area, buf, &render_list_title, &self.libraries_names.clone(), &mut self.list_state_settings_library.clone(), None);
        self.render_info_settings_library(item_area, buf, &self.list_state_settings_library.clone());
    }

    /// `AppView::SettingsAutoplay` rendering
    fn render_settings_autoplay(&mut self, area: Rect, buf: &mut Buffer) {
        let mut hints = vec![("h", "Back"), ("l/→", "Apply")];
        hints.extend(Self::footer_trailer("Home", false));
        let text_render_footer = theme::footer_text(&hints);
        let [header_area, main_area, _player_area, _refresh_area, footer_area] = self.standard_layout(area, &text_render_footer);

        let [list_area, item_area] = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1),]).areas(main_area);

        let render_list_title = "Podcast Autoplay";
        let options = vec!["On".to_string(), "Off".to_string()];
        let current = if get_is_podcast_autoplay(&self.username) == "1" { "On" } else { "Off" };

        App::render_header(header_area, buf, self.lib_name_type.clone(), &self.username, &self.server_address_pretty, VERSION, &self.update_msg);
        App::render_footer(footer_area, buf, &text_render_footer);
        self.render_list(list_area, buf, render_list_title, &options, &mut self.list_state_settings_autoplay.clone(), None);
        Paragraph::new(format!("Currently: {current}\n\nWhen on, finishing a podcast episode automatically starts the next unfinished one in the list it was played from."))
            .left_aligned()
            .wrap(Wrap { trim: true })
            .block(theme::section_block("Description"))
            .render(item_area, buf);
    }

    /// `AppView::SettingsUpdateUninstall` rendering
    fn render_settings_update_uninstall(&mut self, area: Rect, buf: &mut Buffer) {
        let text_render_footer = match &self.update_uninstall_stage {
            UpdateUninstallStage::Instructions => {
                let mut hints = vec![("h", "Back"), ("l/→", "Select")];
                hints.extend(Self::footer_trailer("Home", false));
                theme::footer_text(&hints)
            }
            UpdateUninstallStage::Confirm(_) => theme::footer_text(&[("Y", "Yes"), ("N/Esc", "No")]),
            UpdateUninstallStage::Password(_) => theme::footer_text(&[("Enter", "Continue"), ("Esc", "Back")]),
            UpdateUninstallStage::Running(_) => Text::from("Working..."),
            UpdateUninstallStage::Failed(_, _) => theme::footer_text(&[("Esc", "Back")]),
        };

        let [header_area, main_area, _player_area, _refresh_area, footer_area] = self.standard_layout(area, &text_render_footer);

        let [list_area, item_area] = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1),]).areas(main_area);

        let render_list_title = SETTINGS_UPDATE_UNINSTALL;
        let options = vec!["Update now".to_string(), "Uninstall".to_string()];

        App::render_header(header_area, buf, self.lib_name_type.clone(), &self.username, &self.server_address_pretty, VERSION, &self.update_msg);
        App::render_footer(footer_area, buf, &text_render_footer);
        self.render_list(list_area, buf, render_list_title, &options, &mut self.list_state_settings_update_uninstall.clone(), None);
        self.render_update_uninstall_content(item_area, buf);
    }

    fn render_update_uninstall_content(&self, area: Rect, buf: &mut Buffer) {
        match &self.update_uninstall_stage {
            UpdateUninstallStage::Instructions => {
                // Shows what's actually in the pending update before the user commits to
                // it - only when one is available (self.update_msg), since "what's new"
                // framing doesn't make sense when already on the latest version.
                let (title, text) = if self.update_msg.is_empty() {
                    ("Instructions", UPDATE_UNINSTALL_INSTRUCTIONS.to_string())
                } else {
                    (
                        "What's New",
                        format!("{}\n\n{}", latest_changelog_entry(), UPDATE_UNINSTALL_INSTRUCTIONS),
                    )
                };
                Paragraph::new(text)
                    .wrap(Wrap { trim: true })
                    .block(theme::section_block(title))
                    .render(area, buf);
            }
            UpdateUninstallStage::Confirm(action) => {
                let message = match action {
                    Action::Update => "Update to the latest version now?\n\nThis downloads and installs it, and may ask for your password.\n\n[Y] Yes   [N] No",
                    Action::Uninstall => "Uninstall Absotui?\n\nThis deletes the binary, config, launcher, and icon. May ask for your password.\n\n[Y] Yes   [N] No",
                };
                // ACCENT_KEY (yellow), same reasoning as the search box - this stage
                // is reached by taking an action (Update/Uninstall), not a permanent
                // structural section of the screen.
                Paragraph::new(message)
                    .wrap(Wrap { trim: true })
                    .block(theme::section_block("Confirm").border_style(Style::new().fg(theme::ACCENT_KEY)))
                    .render(area, buf);
            }
            UpdateUninstallStage::Password(_) => {
                let [label_area, input_area, _rest] = Layout::vertical([
                    Constraint::Length(1),
                    Constraint::Length(3),
                    Constraint::Fill(1),
                ]).areas(area);
                Paragraph::new("Enter your password to continue:").render(label_area, buf);
                (&self.update_uninstall_password).render(input_area, buf);
            }
            UpdateUninstallStage::Running(action) => {
                let header = match action {
                    Action::Update => "Updating... this may take a minute (longer if authenticating with a fingerprint reader).",
                    Action::Uninstall => "Uninstalling... this may take a minute (longer if authenticating with a fingerprint reader).",
                };
                let mut lines = vec![header.to_string(), String::new()];
                lines.extend(self.update_uninstall_log.iter().cloned());
                // ACCENT_KEY (yellow) - see Confirm's comment above.
                Paragraph::new(lines.join("\n"))
                    .wrap(Wrap { trim: true })
                    .block(theme::section_block("Working").border_style(Style::new().fg(theme::ACCENT_KEY)))
                    .render(area, buf);
            }
            UpdateUninstallStage::Failed(_, message) => {
                let mut lines = self.update_uninstall_log.clone();
                lines.push(String::new());
                lines.push(format!("Failed: {message}"));
                Paragraph::new(lines.join("\n"))
                    .wrap(Wrap { trim: true })
                    .block(theme::section_block("Failed").border_style(Style::new().fg(theme::ACCENT_ERROR)))
                    .render(area, buf);
            }
        }
    }

    /// `AppView::SettingsPerItemSpeed` rendering
    fn render_settings_per_item_speed(&mut self, area: Rect, buf: &mut Buffer) {
        let mut hints = vec![("h", "Back"), ("l/→", "Apply")];
        hints.extend(Self::footer_trailer("Home", false));
        let text_render_footer = theme::footer_text(&hints);
        let [header_area, main_area, _player_area, _refresh_area, footer_area] = self.standard_layout(area, &text_render_footer);

        let [list_area, item_area] = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1),]).areas(main_area);

        let render_list_title = "Per-Item Speed";
        let options = vec!["On".to_string(), "Off".to_string()];
        let current = if get_is_per_item_speed(&self.username) == "1" { "On" } else { "Off" };

        App::render_header(header_area, buf, self.lib_name_type.clone(), &self.username, &self.server_address_pretty, VERSION, &self.update_msg);
        App::render_footer(footer_area, buf, &text_render_footer);
        self.render_list(list_area, buf, render_list_title, &options, &mut self.list_state_settings_per_item_speed.clone(), None);
        Paragraph::new(format!("Currently: {current}\n\nWhen on, each book or podcast show remembers its own playback speed (O/I in the player) instead of sharing one speed across everything. Turning this on resets every book/show back to 1.0x - each one then adjusts independently from there as you play it, starting fresh at 1.0x the first time. When off, O/I always adjust the single shared speed, same as before this setting existed."))
            .left_aligned()
            .wrap(Wrap { trim: true })
            .block(theme::section_block("Description"))
            .render(item_area, buf);
    }

    /// `AppView::SettingsAutoDownload` rendering
    fn render_settings_auto_download(&mut self, area: Rect, buf: &mut Buffer) {
        let mut hints = vec![("h", "Back"), ("l/→", "Apply")];
        hints.extend(Self::footer_trailer("Home", false));
        let text_render_footer = theme::footer_text(&hints);
        let [header_area, main_area, _player_area, _refresh_area, footer_area] = self.standard_layout(area, &text_render_footer);

        let [list_area, item_area] = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1),]).areas(main_area);

        let render_list_title = "Auto Download";
        let options = vec!["On".to_string(), "Off".to_string()];
        let current = if get_is_auto_download(&self.username) == "1" { "On" } else { "Off" };
        let count = self.config.downloads.auto_download_count;

        App::render_header(header_area, buf, self.lib_name_type.clone(), &self.username, &self.server_address_pretty, VERSION, &self.update_msg);
        App::render_footer(footer_area, buf, &text_render_footer);
        self.render_list(list_area, buf, render_list_title, &options, &mut self.list_state_settings_auto_download.clone(), None);
        Paragraph::new(format!("Currently: {current}\n\nWhen on, the {count} most recently played books in Continue Listening, and every episode in Podcasts' New & Unfinished, are automatically downloaded for offline playback (press 'd' on Home to do this manually). Checked each time these lists refresh (opening the app, R, switching libraries - podcasts also refresh every few seconds on their own). Books that fall out of that top-{count} window, or episodes no longer new/unfinished, have their download removed automatically, so disk usage stays bounded rather than growing forever. Change the book count via `auto_download_count` under `[downloads]` in config.toml. Files are hours long, so turning this on can mean several hundred MB to a few GB downloading in the background the moment it's enabled or a new item becomes active."))
            .left_aligned()
            .wrap(Wrap { trim: true })
            .block(theme::section_block("Description"))
            .render(item_area, buf);
    }


    /// `AppView::SearchBook` rendering
    fn render_search_book(&mut self, area: Rect, buf: &mut Buffer) {
        let _text_render_footer = if self.is_podcast {
            let mut hints = vec![("l/→", "Episodes"), ("/", "Search")];
            hints.push(Self::FOOTER_SCROLL_DESC);
            hints.extend(Self::footer_trailer("Home", true));
            theme::footer_text(&hints)
        } else {
            let mut hints = vec![("l/→", "Play"), ("/", "Search")];
            hints.push(Self::FOOTER_SCROLL_DESC);
            hints.extend(Self::footer_trailer("Home", true));
            theme::footer_text(&hints)
        };

        let [header_area, main_area, _player_area, _refresh_area, footer_area] = self.standard_layout(area, &_text_render_footer);

        let [list_area, item_area1, item_area2] = Layout::vertical([Constraint::Fill(1), Constraint::Length(5), Constraint::Fill(1)]).areas(main_area);

        let render_list_title = "Search result";

        // init variables for search result - matches on title or author, so e.g.
        // searching "sanderson" finds every book by Brandon Sanderson even if none of
        // their titles contain that word. Author comes from whichever of the two
        // author lists actually applies to the current library type (podcasts and
        // books each have their own, indexed the same way as titles_library).
        let query = self.search_query.to_lowercase();
        let idx_and_titles: Vec<(usize, String)> = self.titles_library
            .iter()
            .enumerate()
            .filter(|(index, title)| {
                let title_matches = title.to_lowercase().contains(&query);
                let author_list = if self.is_podcast { &self.auth_names_library_pod } else { &self.auth_names_library };
                let author_matches = author_list.get(*index).is_some_and(|author| author.to_lowercase().contains(&query));
                title_matches || author_matches
            })
            .map(|(index, title)| (index, title.clone()))
            .collect();

        let mut titles_search_book_or_pod: Vec<String> = Vec::new();
        let mut index_to_keep: Vec<usize> = Vec::new();
        for (index, title) in idx_and_titles {
            titles_search_book_or_pod.push(title.clone());
            index_to_keep.push(index);
        }

        let titles_search_book_or_pod: &[String] = &titles_search_book_or_pod;

        self.ids_search_book = self.ids_library
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| value.clone())
            .collect();
        self.auth_names_pod_search_book = self.auth_names_library_pod
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| value.clone())
            .collect();
        self.auth_names_search_book = self.auth_names_library
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| value.clone())
            .collect();
        self.published_year_library_search_book = self.published_year_library
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| value.clone())
            .collect();
        self.desc_library_search_book = self.desc_library
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| value.clone())
            .collect();
        self.duration_library_search_book = self.duration_library
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| *value)
            .collect();

        self.all_titles_pod_ep_search = self.all_titles_pod_ep
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| value.clone())
            .collect();
        self.all_ids_pod_ep_search = self.all_ids_pod_ep
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| value.clone())
            .collect();
        self.all_subtitles_pod_ep_search = self.all_subtitles_pod_ep
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| value.clone())
            .collect();
        self.all_seasons_pod_ep_search = self.all_seasons_pod_ep
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| value.clone())
            .collect();
        self.all_episodes_pod_ep_search = self.all_episodes_pod_ep
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| value.clone())
            .collect();
        self.all_authors_pod_ep_search = self.all_authors_pod_ep
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| value.clone())
            .collect();
        self.all_descs_pod_ep_search = self.all_descs_pod_ep
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| value.clone())
            .collect();
        self.all_titles_pod_search = self.all_titles_pod
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| value.clone())
            .collect();
        self.all_durations_pod_ep_search = self.all_durations_pod_ep
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| value.clone())
            .collect();
        self.ids_library_pod_search = self.ids_library
            .iter()
            .enumerate()
            .filter(|(index, _)| index_to_keep.contains(index))
            .map(|(_, value)| value.clone())
            .collect();

        App::render_header(header_area, buf, self.lib_name_type.clone(), &self.username, &self.server_address_pretty, VERSION, &self.update_msg);
        App::render_footer(footer_area, buf, &_text_render_footer);
        self.render_list(list_area, buf, render_list_title, titles_search_book_or_pod, &mut self.list_state_search_results.clone(), None);
        if !titles_search_book_or_pod.is_empty() {
            self.render_info_search_book(item_area1, buf, &self.list_state_search_results.clone() );
            self.render_desc_search_book(item_area2, buf, &self.list_state_search_results.clone() );
        }
        if self.is_search_active {
            self.render_search_overlay(item_area1, buf);
        }
    }

    /// `AppView::PodcastEpisode`
    fn render_pod_ep(&mut self, area: Rect, buf: &mut Buffer) {
        let mut hints = vec![("h", "Back"), ("l/→", "Play"), ("/", "Search")];
        hints.push(Self::FOOTER_SCROLL_DESC);
        hints.extend(Self::footer_trailer("Home", true));
        let text_render_footer = theme::footer_text(&hints);

        let [header_area, main_area, _player_area, _refresh_area, footer_area] = self.standard_layout(area, &text_render_footer);

        let [list_area, item_area1, item_area2] = Layout::vertical([Constraint::Fill(1), Constraint::Length(5), Constraint::Fill(1)]).areas(main_area);

        App::render_header(header_area, buf, self.lib_name_type.clone(), &self.username, &self.server_address_pretty, VERSION, &self.update_msg);
        App::render_footer(footer_area, buf, &text_render_footer);
        let no_episodes_message = "No episodes found for this podcast.\nPress 'h' to go back.";

        if self.is_from_search_pod {
            if self.titles_pod_ep_search.is_empty() {
                log::warn!("render_pod_ep (search): No episodes found.");
                Paragraph::new(no_episodes_message)
                    .centered()
                    .block(Block::new().borders(Borders::TOP).border_style(Style::new().fg(Color::DarkGray)))
                    .render(main_area, buf);
            } else {
                let items_number = self.titles_pod_ep_search.len();
                let render_list_title = format!("Episodes [{items_number} items]");
                self.render_list(list_area, buf, &render_list_title, &self.titles_pod_ep_search.clone(), &mut self.list_state_pod_ep.clone(), None);
                self.render_info_pod_ep_search(item_area1, buf, &self.list_state_pod_ep.clone() );
                self.render_desc_pod_ep_search(item_area2, buf, &self.list_state_pod_ep.clone() );
            }
        } else {
            if self.titles_pod_ep.is_empty() {
                log::warn!("render_pod_ep (library): No episodes found.");
                Paragraph::new(no_episodes_message)
                    .centered()
                    .block(Block::new().borders(Borders::TOP).border_style(Style::new().fg(Color::DarkGray)))
                    .render(main_area, buf);
            } else {
                let items_number = self.titles_pod_ep.len();
                let render_list_title = format!("Episodes [{items_number} items]");
                self.render_list(list_area, buf, &render_list_title, &self.titles_pod_ep.clone(), &mut self.list_state_pod_ep.clone(), None);
                self.render_info_pod_ep(item_area1, buf, &self.list_state_pod_ep.clone() );
                self.render_desc_pod_ep(item_area2, buf, &self.list_state_pod_ep.clone() );
            }
        }
        if self.is_search_active {
            self.render_search_overlay(item_area1, buf);
        }
    }

    /// The search box (`/`), drawn as an overlay on top of whatever's already been
    /// rendered above - part of the same single render pass as everything else (see
    /// the `render()` dispatch above), not a separate `Terminal` instance racing the
    /// main loop's own. `Clear` first, since `TextArea`'s own render only draws its
    /// border plus whatever text/cursor it actually holds - an empty box wouldn't
    /// otherwise blank the content already sitting in its area from the same buffer.
    ///
    /// Fills `target` exactly - callers with their own Info box (Home, Library,
    /// Settings, SearchBook, PodcastEpisode) pass that Rect directly, so the box
    /// replaces the low-value Info summary (author/year/duration - a quick glance
    /// away regardless) instead of sitting over the Description panel, which has the
    /// actual synopsis text and cover art worth keeping visible while typing.
    fn render_search_overlay(&mut self, target: Rect, buf: &mut Buffer) {
        ratatui::widgets::Clear.render(target, buf);
        (&self.search_textarea).render(target, buf);
    }

    /// Fallback for the handful of screens with no Info box to anchor to (Settings'
    /// own sub-screens, mostly) - `/` isn't advertised there, but it's still a global
    /// key, so this keeps it from silently vanishing if pressed anyway.
    fn render_search_overlay_fallback(&mut self, area: Rect, buf: &mut Buffer) {
        // Stays clear of the player bar's own reserved rows (6 for its box + 1 gap
        // above the footer - see player_tui.rs's `new_y` and `standard_layout`'s
        // `player_gap`/`refresh` constraints) when something's playing, rather than
        // landing on top of it.
        let player_reserved = if get_is_vlc_running(&self.username) == "1" { 7 } else { 0 };
        let target = Rect {
            x: area.x + 1,
            y: area.y + area.height.saturating_sub(5 + player_reserved),
            width: area.width.saturating_sub(2),
            height: 3,
        };
        self.render_search_overlay(target, buf);
    }

    /// The podcast-add text input (`A` on Library), drawn as an overlay the same way
    /// render_search_overlay is - Clear first, then the TextArea's own render. Rebuilds
    /// the block/title every frame (rather than only once at the `A` keypress) so a
    /// Failed outcome's error message shows up immediately without a separate render
    /// path just for that.
    fn render_podcast_add_input_overlay(&mut self, target: Rect, buf: &mut Buffer) {
        let title = match &self.podcast_add_error {
            Some(err) => format!("Add podcast (search titles or enter RSS URL) - {err}"),
            None => "Add podcast (search titles or enter RSS URL)".to_string(),
        };
        self.podcast_add_textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::new().fg(crate::ui::theme::ACCENT_KEY))
        );
        ratatui::widgets::Clear.render(target, buf);
        (&self.podcast_add_textarea).render(target, buf);
    }

    /// The podcast-removal confirm (`C` on Library), drawn as an overlay the same way
    /// render_search_overlay is - ACCENT_ERROR border, matching account_removal_confirm's
    /// own styling for the same reason (a destructive, not-undoable-from-here action).
    fn render_podcast_remove_confirm_overlay(&self, target: Rect, buf: &mut Buffer) {
        ratatui::widgets::Clear.render(target, buf);
        Paragraph::new("Remove this podcast subscription?\n\n[s] Soft - unsubscribe, keep any downloaded episodes\n[h] Hard - also delete downloaded episode files\n[n] Cancel")
            .wrap(Wrap { trim: true })
            .block(theme::section_block("Confirm").border_style(Style::new().fg(theme::ACCENT_ERROR)))
            .render(target, buf);
    }

    /// `AppView::PodcastAdd` rendering - Loading/Results/Confirm only; the Input stage
    /// renders as an overlay on top of Library itself (render_podcast_add_input_overlay)
    /// and never reaches this dispatch, since it never changes view_state away from
    /// Library in the first place.
    fn render_podcast_add(&mut self, area: Rect, buf: &mut Buffer) {
        let hints: Vec<(&str, &str)> = match &self.podcast_add_stage {
            Some(PodcastAddStage::Confirm { .. }) => vec![("←/→", "Episode count"), ("Enter", "Add"), ("h", "Back to results"), ("Esc", "Cancel")],
            Some(PodcastAddStage::Results(_)) => vec![("j/k", "Move"), ("l/→ Enter", "Select"), ("Esc", "Cancel")],
            _ => vec![("Esc", "Cancel")],
        };
        let text_render_footer = theme::footer_text(&hints);
        let [header_area, main_area, _player_area, _refresh_area, footer_area] = self.standard_layout(area, &text_render_footer);

        App::render_header(header_area, buf, self.lib_name_type.clone(), &self.username, &self.server_address_pretty, VERSION, &self.update_msg);
        App::render_footer(footer_area, buf, &text_render_footer);

        match self.podcast_add_stage.clone() {
            Some(PodcastAddStage::Results(results)) => {
                let [list_area, item_area1, item_area2] = Layout::vertical([Constraint::Fill(1), Constraint::Length(5), Constraint::Fill(1)]).areas(main_area);
                let items_number = results.len();
                let render_list_title = format!("Results [{items_number} items]");
                let titles: Vec<String> = results.iter().map(|r| {
                    let title = r.title.as_deref().unwrap_or("Unknown");
                    match &r.artist_name {
                        Some(author) if !author.is_empty() => format!("{title} — {author}"),
                        _ => title.to_string(),
                    }
                }).collect();
                self.render_list(list_area, buf, &render_list_title, &titles, &mut self.list_state_podcast_search_results.clone(), None);

                if let Some(selected) = self.list_state_podcast_search_results.selected()
                    && let Some(result) = results.get(selected) {
                        let episode_count = result.track_count.map(|n| n.to_string()).unwrap_or_else(|| "?".to_string());
                        Paragraph::new(format!("Author: {} - Episodes: {episode_count}", result.artist_name.as_deref().unwrap_or("Unknown")))
                            .left_aligned()
                            .block(theme::section_block("Info"))
                            .render(item_area1, buf);

                        self.render_podcast_result_cover_and_desc(item_area2, buf, result, "Select this podcast (l / → / Enter) to load its full description and choose subscription settings.");
                }
            }
            Some(PodcastAddStage::Confirm { chosen, episode_count }) => {
                let [item_area1, item_area2] = Layout::vertical([Constraint::Length(5), Constraint::Fill(1)]).areas(main_area);
                Paragraph::new(format!(
                        "Episodes to download now: {episode_count}   (←/→ to change)\n\nPress Enter to subscribe."
                ))
                    .left_aligned()
                    .block(theme::section_block("Confirm").border_style(Style::new().fg(theme::ACCENT_KEY)))
                    .render(item_area1, buf);
                self.render_podcast_result_cover_and_desc(item_area2, buf, &chosen, "No description available - Audiobookshelf's search doesn't always get one back from iTunes.");
            }
            _ => {
                Paragraph::new("Searching...")
                    .centered()
                    .block(Block::new().borders(Borders::TOP).border_style(Style::new().fg(Color::DarkGray)))
                    .render(main_area, buf);
            }
        }
    }

    /// Shared by Results (currently-highlighted row) and Confirm (the chosen result) -
    /// same cover-art rendering `render_desc_home` already does, just pointed at
    /// `fetch_and_cache_external_cover`/a synthetic cache key (this podcast isn't a
    /// real Audiobookshelf library item yet, so it has no item id or cover endpoint
    /// of its own - see that function's own doc comment).
    fn render_podcast_result_cover_and_desc(&mut self, area: Rect, buf: &mut Buffer, result: &crate::api::podcasts::search_podcast::PodcastSearchResult, no_description_text: &str) {
        let cache_key = result.id.map(|id| format!("itunes-{id}"));
        self.load_external_cover_for_selection(result.cover.as_deref(), cache_key.as_deref());

        // iTunes' search results very often carry an empty string (not absent) for
        // both description fields - confirmed live (e.g. "Wild Place Adventures"
        // returned `"description":"","descriptionPlain":""`). `Option::or` only falls
        // through on `None`, so `Some("")` would otherwise win and render as visibly
        // nothing rather than the fallback message - filter empties out explicitly.
        let description = result.description_plain.as_deref().filter(|s| !s.is_empty())
            .or(result.description.as_deref().filter(|s| !s.is_empty()))
            .unwrap_or(no_description_text);
        let show_cover = cache_key.is_some() && self.cover_loaded_for_id == cache_key;

        if show_cover {
            let block = theme::section_block("Description");
            let inner = block.inner(area);
            block.render(area, buf);

            let [image_area, _gap_area, text_area] = Layout::horizontal([
                Constraint::Length(30),
                Constraint::Length(3),
                Constraint::Fill(1),
            ]).areas(inner);

            if let Some(cover) = &mut self.cover_protocol {
                let image = ratatui_image::StatefulImage::default()
                    .resize(ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::Lanczos3)));
                StatefulWidget::render(image, image_area, buf, cover);
            }

            Paragraph::new(html_to_lines(description))
                .scroll((self.scroll_offset, 0))
                .wrap(Wrap { trim: true })
                .render(text_area, buf);
        } else {
            Paragraph::new(html_to_lines(description))
                .scroll((self.scroll_offset, 0))
                .wrap(Wrap { trim: true })
                .block(theme::section_block("Description"))
                .render(area, buf);
        }
    }

    /// Same as `load_cover_for_selection`, but for a podcast search result's public
    /// iTunes artwork URL rather than an Audiobookshelf item id - see
    /// `fetch_and_cache_external_cover`'s own doc comment for why this needs a
    /// separate fetch (no auth, arbitrary host) instead of reusing that function.
    fn load_external_cover_for_selection(&mut self, url: Option<&str>, cache_key: Option<&str>) {
        let (Some(url), Some(cache_key)) = (url, cache_key) else { return };
        if self.cover_loaded_for_id.as_deref() == Some(cache_key) {
            return;
        }

        let cache_path = cover_cache_path(cache_key);
        let dyn_img = if cache_path.exists() {
            std::fs::read(&cache_path).ok().and_then(|bytes| image::load_from_memory(&bytes).ok())
        } else {
            None
        };
        let protocol = dyn_img.and_then(|img| self.image_picker.as_ref().map(|picker| picker.new_resize_protocol(img)));

        if protocol.is_some() {
            self.cover_protocol = protocol;
            self.cover_loaded_for_id = Some(cache_key.to_string());
            return;
        }

        self.cover_protocol = None;
        self.cover_loaded_for_id = None;

        if self.image_picker.is_some() && !cache_path.exists() && !self.cover_fetch_requested.contains(cache_key) {
            self.cover_fetch_requested.insert(cache_key.to_string());
            let url = url.to_string();
            let cache_key = cache_key.to_string();
            tokio::spawn(async move {
                if let Err(e) = fetch_and_cache_external_cover(url, cache_key.clone()).await {
                    log::warn!("[fetch_and_cache_external_cover] {cache_key}: {e}");
                }
            });
        }
    }

    /// `AppView::Keymap` - the full keybind reference for whichever screen `?` was
    /// pressed from (`self.keymap_return_view`), matching CLIAMP/superfile's own
    /// dedicated help/keymap screens rather than the always-visible curated footer.
    fn render_keymap(&mut self, area: Rect, buf: &mut Buffer) {
        let footer_hints = [("?", "Back"), ("Tab", "Home"), ("Q/Esc", "Quit")];
        let text_render_footer = theme::footer_text(&footer_hints);
        let [header_area, main_area, _player_area, _refresh_area, footer_area] =
            self.standard_layout(area, &text_render_footer);

        App::render_header(header_area, buf, self.lib_name_type.clone(), &self.username, &self.server_address_pretty, VERSION, &self.update_msg);
        App::render_footer(footer_area, buf, &text_render_footer);

        let rows: Vec<Row> = self.keymap_entries().iter()
            .map(|(k, d)| Row::new(vec![Cell::from(*k), Cell::from(*d)]))
            .collect();
        Widget::render(
            Table::new(rows, [Constraint::Length(16), Constraint::Fill(1)])
                .block(theme::section_block("Keymap")),
            main_area, buf,
        );
    }

    /// The complete, authoritative keybind list for `self.keymap_return_view` - not
    /// just a copy of that screen's (deliberately curated) footer text. Mirrors
    /// `App::handle_key`'s own guards exactly (is_podcast/is_from_search_pod) so
    /// this can't drift the way the old footers had (T/B are global but were
    /// missing from most footers; D silently worked from any screen when
    /// is_podcast despite only Home's footer mentioning it).
    fn keymap_entries(&self) -> Vec<(&'static str, &'static str)> {
        // Global keys that work from every real screen (everywhere except the 4
        // modal Update/Uninstall sub-stages, which own input themselves and are
        // listed separately below).
        // "T" is deliberately not repeated here - it's already in PLAYER_KEYS below.
        let mut globals = vec![
            Self::FOOTER_MOVE, Self::FOOTER_LIST_JUMP, Self::FOOTER_SCROLL_DESC,
            ("/", "Search"), ("B", "Toggle player key-bindings legend"),
        ];
        globals.extend(player_tui::PLAYER_KEYS.iter().copied());

        match self.keymap_return_view {
            AppView::Home if self.is_podcast => {
                let mut hints = vec![
                    ("l/→ Enter", "Play selected episode"), ("d", "Download / remove download"),
                    ("F", "Mark finished"), ("D", "Toggle newest/oldest-first sort"),
                ];
                hints.extend(globals);
                hints.extend(Self::footer_trailer("Library", true));
                hints
            }
            AppView::Home => {
                let mut hints = vec![
                    ("l/→ Enter", "Play selected book"), ("c", "Expand/collapse chapters under now-playing book"),
                    ("d", "Download / remove download"),
                ];
                hints.extend(globals);
                hints.extend(Self::footer_trailer("Library", true));
                hints
            }
            AppView::Library => {
                let tab_target = if self.collection_names.is_empty() { "Stats" } else { "Collections" };
                let mut hints = vec![("l/→ Enter", if self.is_podcast { "Open episode list" } else { "Play selected book" })];
                hints.push(("h", "Back to collections (when viewing one)"));
                if !self.is_podcast {
                    hints.push(("S", "Group by series"));
                } else {
                    hints.push(("A", "Add podcast subscription"));
                    hints.push(("C", "Cancel subscription (remove)"));
                }
                hints.extend(globals);
                hints.extend(Self::footer_trailer(tab_target, true));
                hints
            }
            AppView::Collections => {
                let mut hints = vec![("l/→ Enter", "Open collection")];
                hints.extend(globals);
                hints.extend(Self::footer_trailer("Home", true));
                hints
            }
            // Doesn't reuse `globals` wholesale - Stats has no list/selection, so
            // FOOTER_MOVE/FOOTER_LIST_JUMP would be actively misleading here (j/k/g/G
            // are genuinely inert on this screen), and its own scroll hint is for the
            // whole page, not a description panel.
            AppView::Stats => {
                let mut hints = vec![("J/K/H", "Scroll stats"), ("/", "Search"), ("B", "Toggle player key-bindings legend")];
                hints.extend(player_tui::PLAYER_KEYS.iter().copied());
                hints.extend(Self::footer_trailer("Home", true));
                hints
            }
            AppView::SearchBook => {
                let mut hints = vec![("l/→ Enter", if self.is_podcast { "Open episode list" } else { "Play selected book" })];
                hints.extend(globals);
                hints.extend(Self::footer_trailer("Home", true));
                hints
            }
            AppView::PodcastEpisode => {
                let mut hints = vec![("h", "Back"), ("l/→ Enter", "Play selected episode")];
                hints.extend(globals);
                hints.extend(Self::footer_trailer("Home", true));
                hints
            }
            // R is added explicitly on these Settings-family arms (rather than via
            // footer_trailer, which omits it here to keep the live footer short) - it's
            // still unconditional (main.rs handles it regardless of view_state), so this
            // authoritative list has to say so regardless of what the footer shows.
            AppView::Settings => {
                let mut hints = vec![("h", "Back to Home"), ("l/→ Enter", "Open selected setting"), ("R", "Refresh")];
                hints.extend(globals);
                hints.extend(Self::footer_trailer("Home", false));
                hints
            }
            AppView::SettingsAccount => {
                let mut hints = vec![("h", "Back to Settings"), ("l/→ Enter", "Remove saved user"), ("R", "Refresh")];
                hints.extend(globals);
                hints.extend(Self::footer_trailer("Home", false));
                hints
            }
            AppView::SettingsLibrary => {
                let mut hints = vec![("h", "Back to Settings"), ("l/→ Enter", "Switch library"), ("R", "Refresh")];
                hints.extend(globals);
                hints.extend(Self::footer_trailer("Home", false));
                hints
            }
            AppView::SettingsAutoplay | AppView::SettingsPerItemSpeed | AppView::SettingsAutoDownload => {
                let mut hints = vec![("h", "Back to Settings"), ("l/→ Enter", "Apply selected option"), ("R", "Refresh")];
                hints.extend(globals);
                hints.extend(Self::footer_trailer("Home", false));
                hints
            }
            AppView::SettingsUpdateUninstall => {
                let mut hints = vec![
                    ("h", "Back to Settings"), ("l/→ Enter", "Select (Instructions stage)"), ("R", "Refresh"),
                    ("y/Y", "Confirm (Confirm stage)"), ("n/N", "Cancel (Confirm stage)"),
                    ("Enter", "Submit password (Password stage)"),
                    ("Esc", "Back/cancel (Confirm, Password, Failed stages)"),
                ];
                hints.extend(globals);
                hints.extend(Self::footer_trailer("Home", false));
                hints
            }
            // Dead/unreachable - Settings' Enter-dispatch never routes here (index 4
            // "About" is instead previewed inline while highlighted on the Settings
            // list itself).
            AppView::SettingsAbout => vec![],
            AppView::Keymap => vec![],
            AppView::PodcastAdd => vec![
                ("Enter", "Search / confirm selection (Input, Results stages)"),
                ("j/↓ k/↑", "Move (Results stage)"),
                ("←/→", "Change episode count preset (Confirm stage)"),
                ("Enter", "Add subscription (Confirm stage)"),
                ("h", "Back to results (Confirm stage)"),
                ("Esc", "Cancel, back to Library (any stage)"),
            ],
        }
    }

    fn render_header(area: Rect, buf: &mut Buffer, library_name: String, username: &str, server_address_pretty: &str, version: &str, update_msg: &str) {
        let block = theme::section_block(&library_name);
        let inner = block.inner(area);
        block.render(area, buf);
        Paragraph::new(format!("👋 Connected as {username}\n🔗 {server_address_pretty}"))
            .not_bold()
            .left_aligned()
            .render(inner, buf);
        Paragraph::new(format!("🦜 Absotui v{version}\n {update_msg}"))
            .right_aligned()
            .render(inner, buf);
    }

    /// When the footer wraps onto 2+ lines, one blank row is inserted right after
    /// the first line so the wrapped chips don't read as a single crowded block -
    /// exactly one row is added no matter how many lines wrap (see `footer_height`
    /// below for why that cap matters), so only the gap after line 1 exists; any
    /// further wrapped lines stay packed tight against each other. Rendered
    /// off-screen first (at its natural, un-spaced height) and then copied into
    /// `area`, which `standard_layout` has already sized to fit that one gap row.
    fn render_footer(area: Rect, buf: &mut Buffer, text_render_footer: &Text<'static>) {
        let content_rows = Self::footer_row_count(text_render_footer, area.width);
        let offscreen_area = Rect::new(0, 0, area.width, content_rows);
        let mut offscreen_buf = Buffer::empty(offscreen_area);
        Paragraph::new(text_render_footer.clone())
            .centered()
            .wrap(Wrap { trim: true })
            .render(offscreen_area, &mut offscreen_buf);

        for row in 0..content_rows {
            let gap = if row > 0 { 1 } else { 0 };
            let target_y = area.y + row + gap;
            if target_y >= area.y + area.height {
                break;
            }
            for x in 0..area.width {
                buf[(area.x + x, target_y)] = offscreen_buf[(x, row)].clone();
            }
        }
    }

    /// How many rows a footer's text actually needs once wrapped at `width`.
    fn footer_row_count(footer_text: &Text<'static>, width: u16) -> u16 {
        Paragraph::new(footer_text.clone())
            .wrap(Wrap { trim: true })
            .line_count(width)
            .max(1) as u16
    }

    /// The standard header/main/player/refresh/footer vertical split, with the footer
    /// sized to however many rows its (wrapped) text actually needs at this width -
    /// a fixed-height footer silently drops whichever of its lines don't fit once
    /// wrapping kicks in on a narrow terminal (confirmed live: shrinking the window
    /// until line 1 wrapped made line 2 disappear entirely, since a 2-row area has
    /// nowhere left to put it once line 1 alone consumes both rows). A wrapped (2+
    /// row) footer gets exactly one extra row on top of its content rows (see
    /// `render_footer`'s single fixed gap).
    ///
    /// Records the resulting footer height on `self.last_footer_height` - it varies
    /// per screen and per width, and `render_player`'s Now Playing box (drawn by
    /// main.rs from a separate, non-widget code path with no layout of its own)
    /// needs the real value to position itself without either overlapping the
    /// footer or leaving a gap the app's own next frame paints over (confirmed
    /// live: a fixed worst-case guess here was wrong as soon as the footer's
    /// actual height fell outside the one specific range it was tuned for).
    fn standard_layout(&mut self, area: Rect, footer_text: &Text<'static>) -> [Rect; 5] {
        let content_rows = Self::footer_row_count(footer_text, area.width);
        let footer_height = if content_rows > 1 { content_rows + 1 } else { content_rows };
        self.last_footer_height = footer_height;
        Layout::vertical([
            Constraint::Length(4),
            Constraint::Fill(1),
            Constraint::Length(6),
            Constraint::Length(1),
            Constraint::Length(footer_height),
        ]).areas(area)
    }

    // Shared footer key-hint fragments, kept in one place so wording can't drift
    // between screens the way it used to (top/bot vs top/bottom, arrows vs
    // spelled-out words, "Settings" capitalized in some footers but not others).
    const FOOTER_MOVE: (&'static str, &'static str) = ("j/↓ k/↑", "Move");
    const FOOTER_LIST_JUMP: (&'static str, &'static str) = ("g/G", "Top/bot");
    const FOOTER_SCROLL_DESC: (&'static str, &'static str) = ("J/K/H", "Scroll desc");

    // The trailing "Tab" / "R: refresh" / "[S: settings]" / "Q/Esc: quit" hints every
    // footer ends with - `tab_target` differs (Home's Tab goes to Library, everywhere
    // else's Tab goes to Home), and the Settings submenus don't mention `S` since
    // you're already there.
    //
    // `show_settings` also happens to be exactly "are we already on a Settings-family
    // screen" at every call site, so it's reused to drop `R` there too: refresh
    // re-fetches library/podcast data, which nothing on a Settings screen displays, and
    // it fully reinits the app - landing back on Home - so its only visible effect there
    // duplicates what "Tab: home" already advertises.
    fn footer_trailer(tab_target: &'static str, show_settings: bool) -> Vec<(&'static str, &'static str)> {
        let mut hints = vec![("Tab", tab_target)];
        if show_settings {
            hints.push(("R", "Refresh"));
            hints.push(("s", "Settings"));
        }
        hints.push(("?", "Keymap"));
        hints.push(("Q/Esc", "Quit"));
        hints
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer, render_list_title: &str, render_list_items: &[String], list_state: &mut ListState, progress_info: Option<&[(String, f32, bool)]>) {
        let marker_fill_style = Style::default().fg(theme::ACCENT_ACTIVE).add_modifier(Modifier::REVERSED);
        // Deliberately no fg/bg/modifiers here at all - any of those get patched across
        // every cell in the row, overriding the row's own colors (the now-playing
        // marker's background, the progress underline). Selection is shown purely via
        // the highlight_symbol (a vertical bar) below, leaving the row itself untouched.
        let selected_style: Style = Style::default();

        // ACCENT_KEY (yellow) on the handful of Settings sub-screens whose own list IS
        // the action - selecting a row there (l/→) immediately applies a real change
        // (switches library, flips a toggle, or - Account - arms the removal
        // confirmation below) rather than just navigating deeper, same reasoning as
        // the search box and the Update/Uninstall Confirm/Working/Password stages.
        // Account's own Confirm panel escalates to ACCENT_ERROR (red) once armed (see
        // render_settings_account) since l/→ there is irreversible, but the list
        // itself stays yellow like its siblings until that point. PodcastAdd's Results
        // stage is the same shape (l/→ advances the add-podcast flow, reached via the
        // `A` chip on Library) - see PodcastAdd's own Confirm stage for the equivalent
        // of Account's escalation. Every other list (Home, the main Library view,
        // Settings' own menu, SearchBook, PodcastEpisode) is just browsing/navigation,
        // so stays ACCENT_STRUCTURE.
        let list_accent = match self.view_state {
            AppView::SettingsLibrary | AppView::SettingsAutoplay | AppView::SettingsAccount
                | AppView::SettingsPerItemSpeed | AppView::SettingsAutoDownload | AppView::PodcastAdd => theme::ACCENT_KEY,
            _ => theme::ACCENT_STRUCTURE,
        };
        let block = theme::section_block(render_list_title).border_style(Style::new().fg(list_accent));

        // Approximate content width available inside each row: `block`'s own left/right
        // border columns (2 - List::block computes block.inner(area) before laying out
        // rows) plus the "▎" highlight symbol column that HighlightSpacing::Always
        // reserves on every row (1). Missing the border columns here (only the highlight
        // symbol was accounted for) made every row's computed padding 2 columns too wide,
        // pushing the actual render past the real available width and clipping the
        // rightmost couple of characters - visible as truncated progress percentages.
        let content_width = area.width.saturating_sub(3) as usize;

        // Minimum gap (in characters) always kept clear between a title and the
        // time/age label, so a long title can never push the label off the row -
        // it gets truncated (or, on the selected row, scrolled) instead. Kept small
        // since the podcast age label already reserves its own trailing space via
        // its fixed-width left-alignment (see AGE_LABEL_WIDTH in render_home) - this
        // is just a little breathing room on top of that, and the only gap at all
        // for the book list's variable-length progress text.
        const MIN_TITLE_GAP: usize = 2;
        // How many ticks the marquee scroll holds still at the start/end of a
        // truncated title before continuing - purely a readability pause.
        const SCROLL_PAUSE_TICKS: u32 = 3;

        // Advance the title-scroll tick once per render (not once per row), on a
        // timer independent of render rate, and reset it whenever the selection
        // moves to a different row.
        let selected = list_state.selected();
        if selected != self.title_scroll_selected {
            self.title_scroll_selected = selected;
            self.title_scroll_offset = 0;
            self.title_scroll_last_tick = std::time::Instant::now();
        } else if self.title_scroll_last_tick.elapsed() >= std::time::Duration::from_millis(300) {
            self.title_scroll_offset = self.title_scroll_offset.wrapping_add(1);
            self.title_scroll_last_tick = std::time::Instant::now();
        }
        let scroll_offset = self.title_scroll_offset;

        let items: Vec<ListItem> = render_list_items
            .iter()
            .enumerate()
            .map(|(i, title)| {
                match progress_info.and_then(|p| p.get(i)) {
                    Some((progress_text, percent, is_now_playing)) => {
                        // Line 1: now-playing marker (cobalt/progress-colored background) +
                        // title on the left, time/duration right-justified.
                        //
                        // The colored box itself is 3 columns wide with the ▶ glyph in the
                        // middle column, so the icon sits centered within its own box. A
                        // separate plain (uncolored) 1-column gap follows the box before
                        // the title, matching the 1-column blank the selection highlight
                        // symbol ("▎ ") already leaves before the box - so the box as a
                        // whole ends up with equal blank space on both sides of it too.
                        const MARKER_BOX_WIDTH: usize = 3;
                        const MARKER_GAP_WIDTH: usize = 1;
                        const MARKER_TOTAL_WIDTH: usize = MARKER_BOX_WIDTH + MARKER_GAP_WIDTH;
                        let marker_span = if *is_now_playing {
                            Span::styled(" ▶ ", marker_fill_style)
                        } else {
                            Span::raw("   ")
                        };
                        let time_len = progress_text.chars().count();
                        let available_for_title = content_width.saturating_sub(MARKER_TOTAL_WIDTH + time_len + MIN_TITLE_GAP);
                        let title_chars: Vec<char> = title.chars().collect();

                        let display_title: String = if title_chars.len() <= available_for_title {
                            title.clone()
                        } else if available_for_title == 0 {
                            String::new()
                        } else if selected == Some(i) {
                            // Selected + truncated: scroll a window across the title to
                            // reveal the hidden tail, pausing at both ends before looping.
                            let overflow = title_chars.len() - available_for_title;
                            let cycle_len = overflow as u32 + 2 * SCROLL_PAUSE_TICKS;
                            let pos = scroll_offset % cycle_len;
                            let window_start = if pos < SCROLL_PAUSE_TICKS {
                                0
                            } else if pos < SCROLL_PAUSE_TICKS + overflow as u32 {
                                (pos - SCROLL_PAUSE_TICKS) as usize
                            } else {
                                overflow
                            };
                            title_chars[window_start..window_start + available_for_title].iter().collect()
                        } else {
                            let cut = available_for_title.saturating_sub(1);
                            format!("{}…", title_chars[..cut].iter().collect::<String>())
                        };

                        let title_len = display_title.chars().count();
                        let padding = content_width.saturating_sub(MARKER_TOTAL_WIDTH + title_len + time_len);

                        // Progress shown as an underline beneath the time text itself -
                        // not a full-height background fill - filled up to percent complete.
                        let time_chars: Vec<char> = progress_text.chars().collect();
                        let fill_count = (((percent / 100.0) * time_chars.len() as f32).round() as usize).min(time_chars.len());
                        let time_filled: String = time_chars[..fill_count].iter().collect();
                        let time_unfilled: String = time_chars[fill_count..].iter().collect();

                        let line1 = Line::from(vec![
                            marker_span,
                            Span::raw(" ".repeat(MARKER_GAP_WIDTH)),
                            Span::raw(display_title),
                            Span::raw(" ".repeat(padding)),
                            Span::styled(time_filled, Style::default().add_modifier(Modifier::UNDERLINED)),
                            Span::raw(time_unfilled),
                        ]);

                        ListItem::new(line1)
                    }
                    None => ListItem::new(title.clone()),
                }
            })
        .collect();


        let list = List::new(items)
            .block(block)
            .highlight_style(selected_style)
            .highlight_symbol("▎")
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(list, area, buf, list_state);
    }


    fn render_info_home(&self, area: Rect, buf: &mut Buffer, list_state: &ListState) {
        let duration_cnt_list_conv = convert_seconds(self.duration_cnt_list.clone());
        let size_cnt_list_conv = format_sizes(self.size_cnt_list.clone());

        // Chapter rows don't have their own info to show - resolve back to the book they
        // belong to. Cursor position no longer maps 1:1 to a book index once chapter rows
        // are spliced in, so this has to go through the same row-building the list itself
        // used, or it reads (or panics on) the wrong entry.
        let selected = if self.is_podcast {
            list_state.selected()
        } else {
            list_state.selected().and_then(|i| self.build_home_rows().get(i).map(|row| match row {
                HomeRow::Book(book_i) => *book_i,
                HomeRow::Chapter { book_index, .. } => *book_index,
            }))
        };

        if let Some(selected) = selected {

            if self.is_podcast {
                Paragraph::new(format!("[{}] - Author: {} - Episode: {} - Duration: {}",
                        self.titles_pod_cnt_list[selected],
                        self.authors_pod_cnt_list[selected],
                        self.nums_ep_pod_cnt_list[selected],
                        self.durations_pod_cnt_list[selected],
                ))
                    .left_aligned()
                    .block(theme::section_block("Info"))
                    .render(area, buf);
                } else {
                    Paragraph::new(format!("Author: {} - Year: {} - Duration: {} - Size: {}\nProgress: {}%, {} {}",
                            self.auth_names_cnt_list[selected],
                            self.pub_year_cnt_list[selected],
                            duration_cnt_list_conv[selected],
                            size_cnt_list_conv[selected],
                            self.book_progress_cnt_list[selected][0], // percentage progression
                            convert_seconds_for_prg(self.duration_cnt_list[selected], self.book_progress_cnt_list_cur_time[selected][0]), // time left
                            self.book_progress_cnt_list[selected][1], // is finished
                    ))
                        .left_aligned()
                        .block(theme::section_block("Info"))
                        .render(area, buf);
            }
        }
    }

    fn render_desc_home(&mut self, area: Rect, buf: &mut Buffer, list_state: &ListState) {
        // See render_info_home - chapter rows resolve back to their parent book's index.
        let selected = if self.is_podcast {
            list_state.selected()
        } else {
            list_state.selected().and_then(|i| self.build_home_rows().get(i).map(|row| match row {
                HomeRow::Book(book_i) => *book_i,
                HomeRow::Chapter { book_index, .. } => *book_index,
            }))
        };

        let Some(selected) = selected else { return };

        let mut _content: String = String::new();
        if self.is_podcast {
            _content = self.subtitles_pod_cnt_list[selected].clone();
        } else {
            _content = self.desc_cnt_list[selected].clone();
        }

        let selected_id = self.cover_id_for_home_selection(selected);
        self.load_cover_for_selection(selected_id.as_deref());

        let show_cover = selected_id.is_some() && self.cover_loaded_for_id == selected_id;

        if show_cover {
            // One box around the whole panel (image + text together) rather than a
            // bare, unboxed image sitting next to a separately-boxed text panel.
            let block = theme::section_block("Description");
            let inner = block.inner(area);
            block.render(area, buf);

            let [image_area, _gap_area, text_area] = Layout::horizontal([
                Constraint::Length(30),
                Constraint::Length(3),
                Constraint::Fill(1),
            ]).areas(inner);

            if let Some(cover) = &mut self.cover_protocol {
                // Defaults to FilterType::Nearest, which drops pixels rather than blending
                // them when downscaling - looks fine on flat cover art but shreds any fine
                // text baked into it. Lanczos3 is slower but only runs once per cover, not
                // per frame, since the protocol caches its encoded output.
                let image = ratatui_image::StatefulImage::default()
                    .resize(ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::Lanczos3)));
                StatefulWidget::render(image, image_area, buf, cover);
            }

            Paragraph::new(html_to_lines(&_content))
                .scroll((self.scroll_offset, 0))
                .wrap(Wrap { trim: true })
                .render(text_area, buf);
        } else {
            Paragraph::new(html_to_lines(&_content))
                .scroll((self.scroll_offset, 0))
                .wrap(Wrap { trim: true })
                .block(theme::section_block("Description"))
                .render(area, buf);
        }
    }

    // Picks which cover cache entry to show for the current Home selection: books just
    // use their own item id. Podcast episodes prefer their own embedded cover art when
    // the episode's audio file was flagged at scan time as having one, kicking off a
    // background fetch the first time such an episode is selected; until that lands (or
    // if the episode has no embedded art at all) this falls back to the parent podcast's
    // cover, same id used before episode covers existed.
    fn cover_id_for_home_selection(&mut self, selected: usize) -> Option<String> {
        let podcast_id = self._ids_cnt_list.get(selected).cloned();
        if !self.is_podcast {
            return podcast_id;
        }

        let episode_id = self.ids_ep_cnt_list.get(selected).cloned();
        let episode_ino = self.episode_embedded_cover_ino_cnt_list.get(selected).cloned().flatten();

        if let (Some(episode_id), Some(ino)) = (episode_id, episode_ino) {
            if cover_cache_path(&episode_id).exists() {
                return Some(episode_id);
            }

            if self.image_picker.is_some() && !self.cover_fetch_requested.contains(&episode_id)
                && let (Some(token), Some(library_item_id)) = (self.token.clone(), podcast_id.clone()) {
                    self.cover_fetch_requested.insert(episode_id.clone());
                    let server_address = self.server_address.clone();
                    tokio::spawn(async move {
                        if let Err(e) = fetch_and_cache_episode_cover(token, episode_id.clone(), library_item_id, ino, server_address).await {
                            log::warn!("[fetch_and_cache_episode_cover] episode {episode_id}: {e}");
                        }
                    });
            }
        }

        podcast_id
    }

    // Loads the selected book's cover from the local disk cache if it's changed since
    // the last render, kicking off a background fetch-and-cache when nothing's cached
    // yet. Rendering just polls the cache file's existence each frame rather than
    // waiting on the fetch directly - see fetch_and_cache_cover.
    fn load_cover_for_selection(&mut self, selected_id: Option<&str>) {
        let Some(id) = selected_id else { return };
        if self.cover_loaded_for_id.as_deref() == Some(id) {
            return;
        }

        let cache_path = cover_cache_path(id);
        let dyn_img = if cache_path.exists() {
            std::fs::read(&cache_path).ok().and_then(|bytes| image::load_from_memory(&bytes).ok())
        } else {
            None
        };
        let protocol = dyn_img.and_then(|img| self.image_picker.as_ref().map(|picker| picker.new_resize_protocol(img)));

        if protocol.is_some() {
            self.cover_protocol = protocol;
            self.cover_loaded_for_id = Some(id.to_string());
            return;
        }

        self.cover_protocol = None;
        self.cover_loaded_for_id = None;

        if self.image_picker.is_some() && !cache_path.exists() && !self.cover_fetch_requested.contains(id)
            && let Some(token) = self.token.clone() {
                self.cover_fetch_requested.insert(id.to_string());
                let id_owned = id.to_string();
                let server_address = self.server_address.clone();
                tokio::spawn(async move {
                    if let Err(e) = fetch_and_cache_cover(token, id_owned.clone(), server_address).await {
                        log::warn!("[fetch_and_cache_cover] item {id_owned}: {e}");
                    }
                });
        }
    }

    fn render_info_library(&self, area: Rect, buf: &mut Buffer, list_state: &ListState) {
        let _duration_library_conv = convert_seconds(self.duration_library.clone());

        if let Some(selected) = list_state.selected() {
            if self.is_podcast {
                Paragraph::new(format!("Author: {}",
                        self.auth_names_library_pod[selected],
                ))
                    .left_aligned()
                    .block(theme::section_block("Info"))
                    .render(area, buf);
            }
            else {
                Paragraph::new(format!("Author: {} - Year: {}",
                        self.auth_names_library[selected],
                        self.published_year_library[selected],
                        ))
                    .left_aligned()
                    .block(theme::section_block("Info"))
                    .render(area, buf);
            }
        }
    }

    fn render_desc_library(&mut self, area: Rect, buf: &mut Buffer, list_state: &ListState) {
        let Some(selected) = list_state.selected() else { return };

        let content = self.desc_library[selected].clone();
        let selected_id = self.ids_library.get(selected).cloned();
        self.load_cover_for_selection(selected_id.as_deref());

        let show_cover = selected_id.is_some() && self.cover_loaded_for_id == selected_id;

        if show_cover {
            // Same split-box layout as render_desc_home - one box around image+text
            // together, not a bare image next to a separately-boxed panel.
            let block = theme::section_block("Description");
            let inner = block.inner(area);
            block.render(area, buf);

            let [image_area, _gap_area, text_area] = Layout::horizontal([
                Constraint::Length(30),
                Constraint::Length(3),
                Constraint::Fill(1),
            ]).areas(inner);

            if let Some(cover) = &mut self.cover_protocol {
                let image = ratatui_image::StatefulImage::default()
                    .resize(ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::Lanczos3)));
                StatefulWidget::render(image, image_area, buf, cover);
            }

            Paragraph::new(html_to_lines(&content))
                .scroll((self.scroll_offset, 0))
                .wrap(Wrap { trim: true })
                .render(text_area, buf);
        } else {
            Paragraph::new(html_to_lines(&content))
                .scroll((self.scroll_offset, 0))
                .wrap(Wrap { trim: true })
                .block(theme::section_block("Description"))
                .render(area, buf);
        }
    }

    fn render_info_pod_ep(&self, area: Rect, buf: &mut Buffer, list_state: &ListState) {
        if self.titles_pod.is_empty() || self.authors_pod_ep.is_empty() {
            log::error!("render_info_pod_ep: titles_pod or authors_pod_ep is empty. Cannot render episode info.");
            Paragraph::new("Error: Podcast metadata missing.")
                .left_aligned()
                .block(theme::section_block("Info"))
                .render(area, buf);
            return;
        }

        let n = self.durations_pod_ep.len();
        // Now safe to access index 0 as we've checked they are not empty
        let duplicated_titles = vec![self.titles_pod[0].clone(); n];
        let duplicated_authors = vec![self.authors_pod_ep[0].clone(); n];

        if let Some(selected) = list_state.selected() {
            log::debug!(
                "render_info_pod_ep: selected={}, titles_pod.len={}, authors_pod_ep.len={}, durations_pod_ep.len={}, episodes_pod_ep.len={}, duplicated_titles.len={}, duplicated_authors.len={}",
                selected,
                self.titles_pod.len(),
                self.authors_pod_ep.len(),
                self.durations_pod_ep.len(),
                self.episodes_pod_ep.len(),
                duplicated_titles.len(),
                duplicated_authors.len()
            );

            if selected < self.episodes_pod_ep.len() && selected < self.durations_pod_ep.len() {
                 if selected < duplicated_titles.len() && selected < duplicated_authors.len() {
                    Paragraph::new(format!("[{}] - Author: {} - Episode: {} - Duration: {} ",
                            duplicated_titles[selected].trim(),
                            duplicated_authors[selected].trim(),
                            self.episodes_pod_ep[selected].trim(),
                            self.durations_pod_ep[selected].trim(),
                    ))
                        .left_aligned()
                        .block(theme::section_block("Info"))
                        .render(area, buf);
                 } else {
                     log::error!("render_info_pod_ep: Index {} out of bounds for duplicated title/author vectors (len={})!", selected, duplicated_titles.len());
                     Paragraph::new("Error: Episode info rendering mismatch.")
                         .left_aligned()
                         .block(theme::section_block("Info"))
                         .render(area, buf);
                 }
            } else {
                log::error!("render_info_pod_ep: Index {} out of bounds for episode/duration vectors (ep_len={}, dur_len={})!", selected, self.episodes_pod_ep.len(), self.durations_pod_ep.len());
                Paragraph::new("Error: Episode data unavailable or index out of bounds.")
                    .left_aligned()
                    .block(theme::section_block("Info"))
                    .render(area, buf);
            }
        }
    }
    fn render_info_pod_ep_search(&self, area: Rect, buf: &mut Buffer, list_state: &ListState) {

        // Same guard as render_info_pod_ep: a podcast with episodes but null
        // title/author metadata (collect_titles_pod/collect_authors_pod_ep only push
        // when media.metadata is present) leaves these empty even though the episode
        // list itself isn't.
        if self.titles_pod_search.is_empty() || self.authors_pod_ep_search.is_empty() {
            log::error!("render_info_pod_ep_search: titles_pod_search or authors_pod_ep_search is empty. Cannot render episode info.");
            Paragraph::new("Error: Podcast metadata missing.")
                .left_aligned()
                .block(theme::section_block("Info"))
                .render(area, buf);
            return;
        }

        let n = self.durations_pod_ep_search.len();
        let duplicated_titles_search = vec![self.titles_pod_search[0].clone(); n];
        let duplicated_authors_search = vec![self.authors_pod_ep_search[0].clone(); n];
        if let Some(selected) = list_state.selected() {
            if selected < self.episodes_pod_ep_search.len()
                && selected < self.durations_pod_ep_search.len()
                && selected < duplicated_titles_search.len()
                && selected < duplicated_authors_search.len() {
                Paragraph::new(format!("[{}] - Author: {} - Episode: {} - Duration: {} ",
                        duplicated_titles_search[selected].trim(),
                        duplicated_authors_search[selected].trim(),
                        self.episodes_pod_ep_search[selected].trim(),
                        self.durations_pod_ep_search[selected].trim(),
                ))
                    .left_aligned()
                    .block(theme::section_block("Info"))
                    .render(area, buf);
            } else {
                log::error!("render_info_pod_ep_search: Index {selected} out of bounds for episode/duration/title/author vectors!");
                Paragraph::new("Error: Episode data unavailable or index out of bounds.")
                    .left_aligned()
                    .block(theme::section_block("Info"))
                    .render(area, buf);
            }
        }
    }

    fn render_desc_pod_ep(&self, area: Rect, buf: &mut Buffer, list_state: &ListState) {
        if let Some(selected) = list_state.selected() {
            log::debug!("render_desc_pod_ep: selected={}, subtitles_pod_ep.len={}", selected, self.subtitles_pod_ep.len());

            if selected < self.subtitles_pod_ep.len() {
                Paragraph::new(html_to_lines(&self.subtitles_pod_ep[selected]))
                    .scroll((self.scroll_offset, 0))
                    .wrap(Wrap { trim: true })
                    .block(theme::section_block("Description"))
                    .render(area, buf);
            } else {
                log::error!("render_desc_pod_ep: Index {} out of bounds for subtitles_pod_ep (len={})!", selected, self.subtitles_pod_ep.len());
                Paragraph::new("Error: Episode description unavailable.")
                    .left_aligned()
                    .block(theme::section_block("Description"))
                    .render(area, buf);
            }
        }
    }
    fn render_desc_pod_ep_search(&self, area: Rect, buf: &mut Buffer, list_state: &ListState) {

        if let Some(selected) = list_state.selected() {

            Paragraph::new(html_to_lines(&self.subtitles_pod_ep_search[selected]))
                .scroll((self.scroll_offset, 0))
                .wrap(Wrap { trim: true })
                .block(theme::section_block("Description"))
                .render(area, buf);
        }
    }

    fn render_info_search_book(&self, area: Rect, buf: &mut Buffer, list_state: &ListState) {
        let _duration_library_search_book_conv = convert_seconds(self.duration_library_search_book.clone());

        if let Some(selected) = list_state.selected() {
            if self.is_podcast {
                Paragraph::new(format!("Author: {}",
                        self.auth_names_pod_search_book[selected],
                ))
                    .left_aligned()
                    .block(theme::section_block("Info"))
                    .render(area, buf);
            }
            else {
                Paragraph::new(format!("Author: {} - Year: {}",
                        self.auth_names_search_book[selected],
                        self.published_year_library_search_book[selected],
                        ))
                    .left_aligned()
                    .block(theme::section_block("Info"))
                    .render(area, buf);
            }
        }
    }

    fn render_desc_search_book(&self, area: Rect, buf: &mut Buffer, list_state: &ListState) {

        if let Some(selected) = list_state.selected() {

            Paragraph::new(html_to_lines(&self.desc_library_search_book[selected]))
                .scroll((self.scroll_offset, 0))
                .wrap(Wrap { trim: true })
                .block(theme::section_block("Description"))
                .render(area, buf);
        }
    }

    fn render_info_settings(&self, area: Rect, buf: &mut Buffer, list_state: &ListState) {
        if list_state.selected() == self.settings_index(SETTINGS_ABOUT) {
            Paragraph::new(format!("Absotui v{} - Licence: GPL-3.0 - Issues: {}/issues\nSource code: {}\nWhat's new:",
                    VERSION,
                    "https://github.com/pdwaldrop/absotui",
                    "https://github.com/pdwaldrop/absotui",
            ))
                .left_aligned()
                .block(theme::section_block("Info"))
                .render(area, buf);
        }
    }

    fn render_desc_settings(&self, area: Rect, buf: &mut Buffer, list_state: &ListState) {
        // Update/Uninstall deliberately has no list-level preview here, same as every
        // other item besides About - its Instructions stage (rendered once you actually
        // enter the screen, see render_update_uninstall_content) already shows
        // UPDATE_UNINSTALL_INSTRUCTIONS.
        if list_state.selected() == self.settings_index(SETTINGS_ABOUT) {
            Paragraph::new(self.changelog.clone())
                .scroll((self.scroll_offset, 0))
                .wrap(Wrap { trim: true })
                .block(theme::section_block("Description"))
                .render(area, buf);
        }
    }

    fn render_info_settings_library(&self, area: Rect, buf: &mut Buffer, list_state: &ListState) {
        if let Some(selected) = list_state.selected() {
                Paragraph::new(format!("Type: {}",
                        self.media_types[selected],
                ))
                    .left_aligned()
                    .block(theme::section_block("Info"))
                    .render(area, buf);
            }
    }
}

#[cfg(test)]
mod tests {
    use super::App;

    #[test]
    fn footer_trailer_with_settings() {
        assert_eq!(App::footer_trailer("Library", true), vec![("Tab", "Library"), ("R", "Refresh"), ("s", "Settings"), ("?", "Keymap"), ("Q/Esc", "Quit")]);
    }

    #[test]
    fn footer_trailer_without_settings() {
        assert_eq!(App::footer_trailer("Home", false), vec![("Tab", "Home"), ("?", "Keymap"), ("Q/Esc", "Quit")]);
    }
}
