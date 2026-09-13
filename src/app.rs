use crate::api::utils::collect_personalized_view::{collect_titles_cnt_list, collect_auth_names_cnt_list, collect_pub_year_cnt_list, collect_duration_cnt_list, collect_size_cnt_list, collect_desc_cnt_list, collect_ids_cnt_list};
use crate::api::utils::collect_personalized_view_pod::{collect_ids_pod_cnt_list, collect_titles_cnt_list_pod, collect_ids_ep_pod_cnt_list, collect_subtitles_pod_cnt_list, collect_nums_ep_pod_cnt_list, collect_seasons_pod_cnt_list, collect_authors_pod_cnt_list, collect_descs_pod_cnt_list, collect_titles_pod_cnt_list, collect_durations_pod_cnt_list, collect_progress_pod_cnt_list, collect_published_at_pod_cnt_list, collect_embedded_cover_ino_pod_cnt_list};
use crate::api::utils::collect_get_all_books::{collect_titles_library, collect_ids_library, collect_auth_names_library, collect_auth_names_library_pod, collect_published_year_library, collect_desc_library, collect_duration_library, collect_series_library};
use crate::api::utils::collect_get_all_collections::{collect_collection_names, collect_collection_book_indices};
use crate::api::utils::collect_get_listening_stats::{collect_stats_summary, StatsSummary};
use crate::api::utils::collect_get_pod_ep::{collect_titles_pod_ep, collect_ids_pod_ep, collect_subtitles_pod_ep, collect_seasons_pod_ep, collect_episodes_pod_ep, collect_authors_pod_ep, collect_descs_pod_ep, collect_titles_pod, collect_durations_pod_ep};
use crate::api::utils::collect_get_all_libraries::{collect_library_names, collect_media_types, collect_library_ids};
use crate::api::utils::collect_get_media_progress::{collect_progress_percentage_book, collect_is_finished_book, collect_current_time_prg};
use crate::api::me::get_media_progress::get_book_progress;
use crate::api::me::update_media_progress::update_media_progress2_pod;
use crate::api::libraries::get_library_perso_view::get_continue_listening;
use crate::api::libraries::get_library_perso_view_pod::{get_new_and_unfinished_pod, Chapter};
use crate::api::libraries::get_all_books::get_all_books;
use crate::api::libraries::get_all_collections::get_all_collections;
use crate::api::me::get_listening_stats::get_listening_stats;
use crate::api::libraries::get_all_libraries::get_all_libraries;
use crate::api::podcasts::search_podcast::{search_podcast, PodcastSearchResult};
use crate::api::podcasts::get_podcast_feed::get_podcast_feed;
use crate::api::library_items::delete_library_item::delete_library_item;
use crate::api::library_items::get_pod_ep::{get_pod_ep, Root as GetPodEpRoot};
use crate::logic::handle_input::handle_l_book::handle_l_book;
use crate::logic::handle_input::handle_l_pod::handle_l_pod;
use crate::logic::handle_input::handle_l_pod_home::handle_l_pod_home;
use crate::logic::handle_input::handle_add_podcast::create_and_seed_podcast;
use crate::config::{ConfigFile, load_config};
use crate::db::crud::{get_is_show_key_bindings, update_is_show_key_bindings, get_is_speed_adjusted_time, update_is_speed_adjusted_time, update_is_podcast_autoplay, delete_user, update_id_selected_lib, get_listening_session, get_is_vlc_running, update_is_per_item_speed, update_is_finished, get_is_auto_download, update_is_auto_download, update_pending_seek, update_login_err};
use crate::api::server::refresh_token::{maybe_refresh_token, RefreshOutcome};
use crate::db::database_struct::Database;
use crate::utils::convert_seconds::convert_seconds;
use crate::utils::download_cache::{is_downloaded, remove_download, download_book, download_episode, sync_auto_downloads, sync_auto_downloads_podcasts};
use color_eyre::Result;
use color_eyre::eyre::Report;
use futures::stream::{self, StreamExt};
use crate::utils::http_client::MAX_CONCURRENT_REQUESTS;
use log::{warn, error};
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind},
    style::Style,
    widgets::{Block, Borders, ListState},
};
use crate::utils::pop_up_message::{pop_message, clear_message};
use crate::utils::changelog::changelog;
use crate::utils::encrypt_token::decrypt_token;
use std::io::stdout;
use crate::player::vlc::quit_vlc::{quit_vlc, pkill_vlc};
use crate::logic::sync_session::sync_session_from_database::quit_app;
use crate::logic::sync_session::wait_prev_session_finished::wait_prev_session_finished;
use crate::player::integrated::handle_key_player::handle_key_player;
use crate::utils::check_update::check_update;
use crate::logic::update_uninstall::{self, Action, ProgressEvent};
use ratatui_textarea::TextArea;

// A single row of the (book-mode) Home list, once the currently-playing book's chapter
// list has been spliced in as extra rows. Kept in sync between rendering (tui.rs) and
// input handling (this file) by always going through `App::build_home_rows`.
#[derive(Clone)]
pub enum HomeRow {
    Book(usize),
    Chapter { book_index: usize, chapter: Chapter },
}

// A single row of the Library list, once series grouping has spliced in header rows.
// `Book`'s index is the ORIGINAL index into `titles_library`/`ids_library`/etc. - not
// a position within this row list - kept in sync between rendering (tui.rs) and input
// handling (this file) by always going through `App::build_library_rows`.
#[derive(Clone)]
pub enum LibraryRow {
    SeriesHeader(String),
    Book(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppView {
    Home,
    Library,
    SearchBook,
    PodcastEpisode,
    Settings,
    SettingsAccount,
    SettingsLibrary,
    SettingsAbout,
    SettingsUpdateUninstall,
    SettingsAutoplay,
    SettingsPerItemSpeed,
    SettingsAutoDownload,
    // Full keybind reference for whichever screen it was opened from (see
    // `keymap_return_view`) - bound to `?`, matching CLIAMP/superfile's own
    // dedicated help/keymap screens.
    Keymap,
    // Index of the current library's collections - only reachable via the Tab ring
    // when `collection_names` is non-empty. Selecting one filters AppView::Library
    // via `active_collection` rather than opening a separate detail screen.
    Collections,
    // Read-only listening-stats dashboard - user-level (not scoped to the current
    // library), same as the Audiobookshelf endpoint it's built from.
    Stats,
    // Add-a-podcast-subscription flow, entered from AppView::Library (podcast mode,
    // `A` key) and always exited back to it - never reachable via Tab, so it never
    // becomes a second "podcast management" destination alongside Library. Which of
    // its several stages is showing is tracked by `podcast_add_stage`, the same way
    // AppView::SettingsUpdateUninstall hosts multiple stages via `update_uninstall_stage`.
    PodcastAdd,
}

// Sub-state for the AppView::PodcastAdd screen. `Input` renders as an overlay on top
// of whatever's behind it (checked early in handle_key, same as the `/` search
// overlay's `is_search_active`) - the other three stages take over the whole screen.
#[derive(Clone)]
pub enum PodcastAddStage {
    Input,
    Loading,
    // Selection lives in the top-level `list_state_podcast_search_results`, same as
    // every other list in this app (see `select_next`/`select_previous`) - not
    // embedded here, so those generic helpers don't need a special case for this one.
    Results(Vec<crate::api::podcasts::search_podcast::PodcastSearchResult>),
    // `episode_count` cycles through the 3/5/10 presets with Left/Right before Enter
    // confirms - see `check_new_episodes`: a freshly-created podcast has zero
    // episodes until that call runs, so this is asked before create_podcast fires,
    // not after.
    Confirm { chosen: Box<crate::api::podcasts::search_podcast::PodcastSearchResult>, episode_count: u32 },
}

// Outcome of the background search_podcast/get_podcast_feed call spawned from the
// PodcastAddStage::Input -> Loading transition. An owned Vec either way (a direct-URL
// add wraps its single feed result in a one-item Vec) so Loading's poll and Results'
// render path don't need to know which of the two calls actually ran.
pub enum PodcastAddOutcome {
    Found(Vec<crate::api::podcasts::search_podcast::PodcastSearchResult>),
    // A single Results row was picked and its real feed has been re-fetched to
    // enrich it before landing on Confirm - see the `l`/Enter handler on
    // PodcastAddStage::Results. iTunes' own search API never returns a podcast
    // description at all (confirmed against its raw public API, not an
    // Audiobookshelf or client-side gap), so Results rows always show "no
    // description" - the podcast's actual RSS feed always has a real one, so this
    // re-fetches it right when the user commits to a specific show, rather than
    // for every row shown (which would mean one wasted feed fetch per result).
    ConfirmReady(Box<crate::api::podcasts::search_podcast::PodcastSearchResult>),
    // The create_podcast -> check_new_episodes pair (spawned from the Confirm stage)
    // finished successfully - carries how many episodes check_new_episodes actually
    // reported back, which may be fewer than the chosen preset if the feed itself
    // has fewer episodes than that.
    Created(usize),
    Failed(String),
}

// Sub-state for the AppView::SettingsUpdateUninstall screen. `Failed` is the only
// rendered end-state - a successful update/uninstall replaces the running process
// (exec into the new binary) or exits entirely (see main.rs), so there's nothing left
// to render in this process either way on success.
pub enum UpdateUninstallStage {
    Instructions,
    Confirm(Action),
    Password(Action),
    Running(Action),
    Failed(Action, String),
}

pub struct App {
    pub view_state: AppView,
    // The screen `?` was pressed from, so Esc/`?` can return to it from AppView::Keymap.
    pub keymap_return_view: AppView,
    // Set when the user picks a different library in Settings > Library. `App` can't
    // reinitialize itself (that's an async operation, and this struct's own methods
    // are sync), so this just signals the main loop to do the same full reload/reinit
    // it already does for the `R` key, landing back on Home in the newly selected library.
    pub library_needs_reload: bool,
    // Set by `standard_layout` every render to however many rows the current
    // screen's footer actually used - `render_player`'s Now Playing box reads
    // this (via main.rs) to position itself exactly against the real footer
    // height instead of guessing a worst case. See its own doc comment.
    pub last_footer_height: u16,
    // Whether the currently-playing book's chapter list is expanded inline under its row
    // in Continue Listening. Session-local only (not persisted) - matches the pattern used
    // by other ephemeral view toggles like `podcast_sort_newest_first`.
    pub is_chapter_list_expanded: bool,
    // Settings > Account's "Remove saved user" used to delete on a single l/→ press
    // with no warning anywhere on screen and no way back - a stray keypress while
    // browsing Settings would permanently wipe the saved login (server address,
    // token, all per-user settings) with nothing to undo it. Gates that behind a
    // Y/N confirmation, same pattern as Update/Uninstall's own Confirm stage.
    pub account_removal_confirm: bool,
    pub database: Database,
    pub id_selected_lib: String,
    pub token: Option<String>,
    pub refresh_token: Option<String>,
    pub should_exit: bool,
    pub list_state_cnt_list: ListState,
    pub list_state_library: ListState,
    pub list_state_collections: ListState,
    pub list_state_search_results: ListState,
    pub list_state_pod_ep: ListState,
    pub list_state_settings: ListState,
    pub list_state_settings_account: ListState,
    pub list_state_settings_library: ListState,
    pub list_state_settings_about: ListState,
    pub list_state_settings_update_uninstall: ListState,
    pub list_state_settings_autoplay: ListState,
    pub list_state_settings_per_item_speed: ListState,
    pub list_state_settings_auto_download: ListState,
    pub _titles_cnt_list: Vec<String>,
    pub auth_names_cnt_list: Vec<String>,
    pub pub_year_cnt_list: Vec<String>,
    pub duration_cnt_list: Vec<f64>,
    pub size_cnt_list: Vec<i64>,
    pub desc_cnt_list: Vec<String>,
    pub _ids_cnt_list: Vec<String>,
    pub titles_library: Vec<String>,
    pub ids_library: Vec<String>,
    pub auth_names_library: Vec<String>,
    pub collection_names: Vec<String>,
    pub collection_book_indices: Vec<Vec<usize>>,
    // Which collection is currently filtering AppView::Library, if any - `None` means
    // Library shows everything (today's behavior). Single source of truth for whether
    // Library is filtered right now.
    pub active_collection: Option<usize>,
    pub series_name_library: Vec<Option<String>>,
    pub series_sequence_library: Vec<Option<f64>>,
    // Whether Library is currently grouped by series - a display preference, not a
    // selection, so (unlike `active_collection`) it persists across Tab navigation
    // for the App's lifetime, same as the podcast Home `D` sort-order toggle.
    pub is_library_grouped_by_series: bool,
    // Podcast subscription management (AppView::PodcastAdd, `C` remove on Library).
    // `None` means the add flow isn't active at all - same role `is_search_active`
    // plays for search, just an `Option<Stage>` instead of a bool since this flow has
    // more than one stage. The textarea is a separate field (not embedded in the enum)
    // for the same reason `search_textarea` is separate from `is_search_active`.
    pub podcast_add_stage: Option<PodcastAddStage>,
    pub podcast_add_textarea: ratatui_textarea::TextArea<'static>,
    pub podcast_add_receiver: Option<tokio::sync::oneshot::Receiver<PodcastAddOutcome>>,
    pub list_state_podcast_search_results: ListState,
    // Kept alongside `podcast_add_stage` (not inside it) so `h` on the Confirm stage
    // can hand the same list back to Results without re-searching - Confirm only
    // carries the one chosen result, not the whole list it came from.
    pub podcast_search_results_cache: Vec<PodcastSearchResult>,
    // Set on a Failed outcome, shown on the Input stage until the next attempt (or
    // the flow is cancelled) - cleared whenever Input is (re-)entered.
    pub podcast_add_error: Option<String>,
    // Gates an inline y/n-style confirm (`s`/`h`/`n`) over the current Library screen
    // when removing a subscribed podcast - same shape as `account_removal_confirm`,
    // just a 3-way choice (soft/hard/cancel) instead of 2-way.
    pub podcast_remove_confirm: bool,
    // Set only while a spawned delete_library_item call is in flight - see
    // poll_podcast_remove_result. Waiting on this before setting
    // library_needs_reload matters: setting it immediately (before the DELETE
    // request has actually completed) would let the reload race ahead and reload
    // the library while the item is still there server-side.
    pub podcast_remove_receiver: Option<tokio::sync::oneshot::Receiver<()>>,
    // User-level listening stats (AppView::Stats) - not scoped to the current
    // library, same as the Audiobookshelf endpoint it's built from.
    pub stats_summary: StatsSummary,
    pub ids_search_book: Vec<String>,
    pub search_query: String,
    // The search box (`/`) - rendered as an overlay on top of whatever screen it was
    // opened from, through the same single Terminal/render pass as everything else
    // (see `handle_key`'s early-return guard and the overlay render in tui.rs), rather
    // than a separate `Terminal` instance racing the main loop's own. `TextArea` owns
    // its own text/cursor state, so it lives on `App` for as long as the box is open.
    pub is_search_active: bool,
    pub search_textarea: ratatui_textarea::TextArea<'static>,
    pub is_podcast: bool,
    // Set only while the background fetch spawned in `App::new()` is still in flight -
    // see `fetch_all_pod_ep` and `poll_pod_ep_fetch`. `None` both before any podcast
    // library is loaded and after the one batch this ever delivers has been consumed.
    pub(crate) pod_ep_receiver: Option<tokio::sync::oneshot::Receiver<PodEpBatch>>,
    pub all_titles_pod_ep: Vec<Vec<String>>,
    pub all_ids_pod_ep: Vec<Vec<String>>,
    pub all_subtitles_pod_ep: Vec<Vec<String>>,
    pub all_seasons_pod_ep: Vec<Vec<String>>,
    pub all_episodes_pod_ep: Vec<Vec<String>>,
    pub all_authors_pod_ep: Vec<Vec<String>>,
    pub all_descs_pod_ep: Vec<Vec<String>>,
    pub all_titles_pod: Vec<Vec<String>>,
    pub all_durations_pod_ep: Vec<Vec<String>>,
    pub titles_pod_ep: Vec<String>,
    pub ids_pod_ep: Vec<String>,
    pub ids_pod_ep_search: Vec<String>,
    pub subtitles_pod_ep: Vec<String>,
    pub seasons_pod_ep: Vec<String>,
    pub episodes_pod_ep: Vec<String>,
    pub authors_pod_ep: Vec<String>,
    pub descs_pod_ep: Vec<String>,
    pub titles_pod: Vec<String>,
    pub durations_pod_ep: Vec<String>,
    pub ids_ep_cnt_list: Vec<String>,
    pub all_titles_pod_ep_search: Vec<Vec<String>>,
    pub titles_pod_ep_search: Vec<String>,
    pub is_from_search_pod: bool,
    pub ids_library_pod_search: Vec<String>,
    pub all_ids_pod_ep_search: Vec<Vec<String>>,
    pub libraries_names: Vec<String>,
    pub media_types: Vec<String>,
    pub libraries_ids: Vec<String>,
    pub library_name: String,
    pub media_type: String,
    pub lib_name_type: String,
    pub settings: Vec<String>,
    pub all_usernames: Vec<String>,
    pub all_server_addresses: Vec<String>,
    pub username: String,
    pub server_address: String,
    pub server_address_pretty: String,
    pub scroll_offset: u16,
    pub subtitles_pod_cnt_list: Vec<String>,
    pub nums_ep_pod_cnt_list: Vec<String>,
    pub seasons_pod_cnt_list: Vec<String>,
    pub authors_pod_cnt_list: Vec<String>,
    pub descs_pod_cnt_list: Vec<String>,
    pub titles_pod_cnt_list: Vec<String>,
    pub durations_pod_cnt_list: Vec<String>,
    pub podcast_progress_cnt_list: Vec<(f64, f64, f32)>,
    pub podcast_published_at_cnt_list: Vec<i64>,
    // `ino` of the episode's audio file when it's worth checking for embedded cover art
    // (MP3 + ffprobe detected a picture stream in it) - None otherwise, same index as
    // `ids_ep_cnt_list`. See collect_embedded_cover_ino_pod_cnt_list.
    pub episode_embedded_cover_ino_cnt_list: Vec<Option<String>>,
    pub podcast_sort_newest_first: bool,
    // Marquee-scroll state for a truncated title on the currently selected list row.
    // Ticks forward on a timer (not every render) so scroll speed stays constant
    // regardless of render rate; resets whenever the selection moves to a new row.
    pub title_scroll_offset: u32,
    pub title_scroll_last_tick: std::time::Instant,
    pub title_scroll_selected: Option<usize>,
    pub published_year_library: Vec<String>,
    pub desc_library: Vec<String>,
    pub duration_library: Vec<f64>,
    pub auth_names_library_pod: Vec<String>,
    pub subtitles_pod_ep_search: Vec<String>,
    pub seasons_pod_ep_search: Vec<String>,
    pub episodes_pod_ep_search: Vec<String>,
    pub authors_pod_ep_search: Vec<String>,
    pub descs_pod_ep_search: Vec<String>,
    pub titles_pod_search: Vec<String>,
    pub durations_pod_ep_search: Vec<String>,
    pub all_subtitles_pod_ep_search: Vec<Vec<String>>,
    pub all_seasons_pod_ep_search: Vec<Vec<String>>,
    pub all_episodes_pod_ep_search: Vec<Vec<String>>,
    pub all_authors_pod_ep_search: Vec<Vec<String>>,
    pub all_descs_pod_ep_search: Vec<Vec<String>>,
    pub all_titles_pod_search: Vec<Vec<String>>,
    pub all_durations_pod_ep_search: Vec<Vec<String>>,
    pub auth_names_pod_search_book: Vec<String>,
    pub auth_names_search_book: Vec<String>,
    pub published_year_library_search_book: Vec<String>,
    pub desc_library_search_book: Vec<String>,
    pub duration_library_search_book: Vec<f64>,
    pub book_progress_cnt_list: Vec<Vec<String>>,
    pub book_progress_cnt_list_cur_time: Vec<Vec<f64>>,
    pub book_progress_search_book: Vec<Vec<String>>,
    pub book_progress_search_book_cur_time: Vec<Vec<f64>>,
    pub is_cvlc: String,
    pub is_cvlc_term: String,
    pub start_vlc_program: String,
    pub config: ConfigFile,
    pub changelog: String,
    pub update_msg: String,
    pub update_uninstall_stage: UpdateUninstallStage,
    pub update_uninstall_password: TextArea<'static>,
    pub update_uninstall_log: Vec<String>,
    pub update_uninstall_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<ProgressEvent>>,
    pub update_uninstall_password_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    pub podcast_home_last_refresh: std::time::Instant,
    // None if the terminal doesn't support any image protocol (Kitty/Sixel/iTerm2) -
    // queried once at startup via Picker::from_query_stdio().
    pub image_picker: Option<ratatui_image::picker::Picker>,
    // The decoded cover currently being shown, and which item id it belongs to - compared
    // against the selected row each render to know when to load a different cover.
    pub cover_protocol: Option<ratatui_image::protocol::StatefulProtocol>,
    pub cover_loaded_for_id: Option<String>,
    // Item ids a background fetch has already been kicked off for, so repeatedly
    // rendering the same selection while the fetch is in flight doesn't spawn duplicates.
    pub cover_fetch_requested: std::collections::HashSet<String>,
}

// Bundles what render_home needs for the podcast "New & Unfinished" list, so the same
// fetch logic can run both at initial load (App::new) and on a periodic refresh
// without duplicating it or needing to reconstruct the whole App.
struct PodcastHomeData {
    ids: Vec<String>,
    titles: Vec<String>,
    ids_ep: Vec<String>,
    subtitles: Vec<String>,
    nums_ep: Vec<String>,
    seasons: Vec<String>,
    authors: Vec<String>,
    descs: Vec<String>,
    titles_pod: Vec<String>,
    durations: Vec<String>,
    // (current_time, duration_seconds, percent) per episode - raw values, formatted for
    // display in render_home like the book progress text.
    progress: Vec<(f64, f64, f32)>,
    published_at: Vec<i64>,
    // `ino` of the episode's audio file, only when it's worth checking for embedded
    // cover art - see collect_embedded_cover_ino_pod_cnt_list.
    embedded_cover_ino: Vec<Option<String>>,
}

/// One full batch of "every podcast's episode list" data - see `App`'s `all_*_pod_ep`
/// fields and `fetch_all_pod_ep`'s doc comment for why this is fetched in the
/// background rather than as part of `App::new()` itself.
pub(crate) struct PodEpBatch {
    titles: Vec<Vec<String>>,
    ids: Vec<Vec<String>>,
    subtitles: Vec<Vec<String>>,
    seasons: Vec<Vec<String>>,
    episodes: Vec<Vec<String>>,
    authors: Vec<Vec<String>>,
    descs: Vec<Vec<String>>,
    titles_pod: Vec<Vec<String>>,
    durations: Vec<Vec<String>>,
}

/// Fetches every podcast's full episode list, one request per podcast in
/// `ids_library`, concurrently (capped at `MAX_CONCURRENT_REQUESTS`). Split out of
/// `App::new()` (see bug_id 3f729c) so it can run as a background task instead of
/// blocking the first render - it was ~5s of a ~7s startup on a 22-podcast library, and
/// is only actually needed once the user opens a specific podcast from Library or
/// searches episode titles/descriptions (`render_search_book` re-filters every render
/// frame, so it picks up results the moment `poll_pod_ep_fetch` merges them in - no
/// separate invalidation needed).
///
/// A single podcast's fetch failing does *not* abort the batch (unlike the old inline
/// version, which used `?` and would fail the whole of `App::new()` over one podcast's
/// transient error) - failures push an empty episode list for that podcast instead, so
/// every returned `Vec` stays exactly as long as `ids_library` and index-aligned with
/// it. A background task has no caller left to propagate a `Result` to by the time it
/// would fail, so silently degrading one podcast to "no episodes yet" is the only
/// option that doesn't panic downstream index lookups.
async fn fetch_all_pod_ep(token: String, server_address: String, ids_library: Vec<String>) -> PodEpBatch {
    let mut batch = PodEpBatch {
        titles: Vec::with_capacity(ids_library.len()),
        ids: Vec::with_capacity(ids_library.len()),
        subtitles: Vec::with_capacity(ids_library.len()),
        seasons: Vec::with_capacity(ids_library.len()),
        episodes: Vec::with_capacity(ids_library.len()),
        authors: Vec::with_capacity(ids_library.len()),
        descs: Vec::with_capacity(ids_library.len()),
        titles_pod: Vec::with_capacity(ids_library.len()),
        durations: Vec::with_capacity(ids_library.len()),
    };

    // Built up front as owned futures, not directly inside stream::iter(...map(closure)) -
    // the latter makes rustc infer a too-narrow higher-ranked lifetime for the borrowed
    // `token`/`server_address` here, which then fails to satisfy Send once this whole
    // function is itself awaited inside another spawned task's async block (as it is in
    // App::new()) even though nothing here actually holds a real borrow across an await
    // point. Same fix as the equivalent fan-out in get_library_perso_view_pod.rs.
    let mut episode_list_futures = Vec::with_capacity(ids_library.len());
    for id_library in &ids_library {
        let token = token.clone();
        let server_address = server_address.clone();
        let id_library = id_library.clone();
        episode_list_futures.push(async move { get_pod_ep(&token, server_address, id_library.as_str()).await });
    }
    let episode_lists: Vec<Result<GetPodEpRoot>> = stream::iter(episode_list_futures)
        .buffered(MAX_CONCURRENT_REQUESTS)
        .collect()
        .await;

    for result in episode_lists {
        let podcast_episode = match result {
            Ok(p) => p,
            Err(e) => {
                error!("[fetch_all_pod_ep] failed to fetch a podcast's episode list, treating it as empty for now: {e}");
                batch.titles.push(Vec::new());
                batch.ids.push(Vec::new());
                batch.subtitles.push(Vec::new());
                batch.seasons.push(Vec::new());
                batch.episodes.push(Vec::new());
                batch.authors.push(Vec::new());
                batch.descs.push(Vec::new());
                batch.titles_pod.push(Vec::new());
                batch.durations.push(Vec::new());
                continue;
            }
        };
        batch.titles.push(collect_titles_pod_ep(&podcast_episode).await);
        batch.ids.push(collect_ids_pod_ep(&podcast_episode).await);
        batch.subtitles.push(collect_subtitles_pod_ep(&podcast_episode).await);
        batch.seasons.push(collect_seasons_pod_ep(&podcast_episode).await);
        batch.episodes.push(collect_episodes_pod_ep(&podcast_episode).await);
        batch.authors.push(collect_authors_pod_ep(&podcast_episode).await);
        batch.descs.push(collect_descs_pod_ep(&podcast_episode).await);
        batch.titles_pod.push(collect_titles_pod(&podcast_episode).await);
        batch.durations.push(collect_durations_pod_ep(&podcast_episode).await);
    }

    batch
}

async fn fetch_podcast_home_data(token: &str, server_address: String, id_selected_lib: &String, newest_first: bool, username: &str) -> Result<PodcastHomeData> {
    let continue_listening_pod = get_new_and_unfinished_pod(token, server_address.clone(), id_selected_lib).await?;
    let mut data = PodcastHomeData {
        ids: collect_ids_pod_cnt_list(&continue_listening_pod).await,
        titles: collect_titles_cnt_list_pod(&continue_listening_pod).await,
        ids_ep: collect_ids_ep_pod_cnt_list(&continue_listening_pod).await,
        subtitles: collect_subtitles_pod_cnt_list(&continue_listening_pod).await,
        nums_ep: collect_nums_ep_pod_cnt_list(&continue_listening_pod).await,
        seasons: collect_seasons_pod_cnt_list(&continue_listening_pod).await,
        authors: collect_authors_pod_cnt_list(&continue_listening_pod).await,
        descs: collect_descs_pod_cnt_list(&continue_listening_pod).await,
        titles_pod: collect_titles_pod_cnt_list(&continue_listening_pod).await,
        durations: collect_durations_pod_cnt_list(&continue_listening_pod).await,
        progress: collect_progress_pod_cnt_list(&continue_listening_pod).await,
        published_at: collect_published_at_pod_cnt_list(&continue_listening_pod).await,
        embedded_cover_ino: collect_embedded_cover_ino_pod_cnt_list(&continue_listening_pod).await,
    };

    let mut order: Vec<usize> = (0..data.published_at.len()).collect();
    if newest_first {
        order.sort_by_key(|&i| std::cmp::Reverse(data.published_at[i]));
    } else {
        order.sort_by_key(|&i| data.published_at[i]);
    }
    data.ids = order.iter().map(|&i| data.ids[i].clone()).collect();
    data.titles = order.iter().map(|&i| data.titles[i].clone()).collect();
    data.ids_ep = order.iter().map(|&i| data.ids_ep[i].clone()).collect();
    data.subtitles = order.iter().map(|&i| data.subtitles[i].clone()).collect();
    data.nums_ep = order.iter().map(|&i| data.nums_ep[i].clone()).collect();
    data.seasons = order.iter().map(|&i| data.seasons[i].clone()).collect();
    data.authors = order.iter().map(|&i| data.authors[i].clone()).collect();
    data.descs = order.iter().map(|&i| data.descs[i].clone()).collect();
    data.titles_pod = order.iter().map(|&i| data.titles_pod[i].clone()).collect();
    data.durations = order.iter().map(|&i| data.durations[i].clone()).collect();
    data.progress = order.iter().map(|&i| data.progress[i]).collect();
    data.published_at = order.iter().map(|&i| data.published_at[i]).collect();
    data.embedded_cover_ino = order.iter().map(|&i| data.embedded_cover_ino[i].clone()).collect();

    pin_now_playing_episode(&mut data, username);

    Ok(data)
}

// If the actively-playing podcast episode isn't in the freshly-fetched "New &
// Unfinished" list, pin it at the top instead of letting it silently vanish from view.
// This happens for real: a freshly-autoplayed episode only joins the server's
// "Continue Listening" shelf once this app's own periodic progress sync reaches it
// (every ~10s of active polling), so there's a window where the episode that's
// actually playing isn't in either server shelf `get_new_and_unfinished_pod` reads from
// - during which a user who can't see it playing may reselect it, thinking it's
// unplayed, which used to restart the same session.
//
// Deliberately NOT a network fetch for full metadata: this runs on the main render
// loop's own periodic refresh (`refresh_podcast_home_if_stale`), not a background task,
// so any network call here blocks rendering and key handling for however long it takes
// - and it would fire almost every time right after an autoplay transition, exactly
// when this same propagation delay is in effect. Instead this reuses what's already
// sitting in the local session row: `title` is stored as "Episode Title | Podcast
// Title" (see the `insert_listening_session` call sites), split back apart here rather
// than re-fetched. Subtitle/author/description/season/episode-number are left blank for
// this pinned row - they only matter if the user selects it, and self-correct as soon
// as the server's shelves catch up and a normal fetch picks the episode up with full
// data.
fn pin_now_playing_episode(data: &mut PodcastHomeData, username: &str) {
    // The listening_session row lingers indefinitely after playback ends - nothing
    // clears it, it's only ever replaced by the next `insert_listening_session` call -
    // so a non-empty id_pod alone doesn't mean anything is actually still playing. Gate
    // on is_vlc_running too, or every fresh launch/refresh would pin whichever podcast
    // episode was last listened to, ever, even if that was days ago - a phantom "now
    // playing" row that pushes everything else down a line for nothing.
    if get_is_vlc_running(username) != "1" {
        return;
    }

    let Some(session) = get_listening_session().ok().flatten() else { return };
    if session.id_pod.is_empty() || data.ids_ep.contains(&session.id_pod) {
        return;
    }

    let (episode_title, podcast_title) = session.title.split_once(" | ")
        .map(|(ep, pod)| (ep.to_string(), pod.to_string()))
        .unwrap_or_else(|| (session.title.clone(), String::new()));
    let duration = session.duration.parse::<f64>().unwrap_or(0.0);

    data.ids.insert(0, session.id_item.clone());
    data.titles.insert(0, episode_title);
    data.ids_ep.insert(0, session.id_pod.clone());
    data.subtitles.insert(0, String::new());
    data.nums_ep.insert(0, String::new());
    data.seasons.insert(0, String::new());
    data.authors.insert(0, String::new());
    data.descs.insert(0, String::new());
    data.titles_pod.insert(0, podcast_title);
    data.durations.insert(0, convert_seconds(vec![duration]).into_iter().next().unwrap_or_default());
    data.progress.insert(0, (session.current_time as f64, duration, 0.0));
    // Pinned regardless of natural sort position, so its published_at doesn't matter -
    // i64::MAX just documents that it was never meant to be re-sorted.
    data.published_at.insert(0, i64::MAX);
    data.embedded_cover_ino.insert(0, None);
}

// `Picker::from_query_stdio()` asks the terminal what image protocol it supports by
// writing an escape sequence and waiting on the response - measured live at ~2s, by
// far the single largest cost in `App::new()` (bigger than every network round-trip
// combined). That capability can't change mid-session (same terminal, same protocol
// support for as long as this process runs), but `App::new()` itself re-runs on every
// `R` refresh and every library switch - querying fresh every time paid that ~2s
// repeatedly for a value that was already known. Queried once per process and
// cloned (`Picker` is cheap - just a handful of enum/small values) on every
// subsequent call instead.
static IMAGE_PICKER: std::sync::OnceLock<Option<ratatui_image::picker::Picker>> = std::sync::OnceLock::new();

// Same reasoning as IMAGE_PICKER just above: `check_update()` hits GitHub's API for
// the latest release tag, and whether this install is out of date can't change
// during the process's own lifetime - restarting is already how every other
// version-pinned check in this codebase (e.g. IMAGE_PICKER) expects to pick up a
// change. Re-querying GitHub on every `R` refresh and library switch was pure
// waste, and at high enough refresh frequency could exhaust GitHub's unauthenticated
// rate limit (60 requests/hour per IP) for no benefit - once that happens
// `check_update` degrades silently (its own Err branch just logs and returns None),
// so a heavy `R` user could lose update notifications entirely without any
// indication why. Cached the same way: computed once per process, cloned after.
static UPDATE_CHECK: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

// The Settings menu's entries, named once so the many places that need to know
// "which row is X" (the l/→ dispatch below, and the About-specific rendering in
// tui.rs) can look it up by name via `App::settings_index` instead of a raw
// position - a previous version of this list matched on hardcoded indices like
// `Some(4)` scattered across two files, which silently went stale (pointed at the
// wrong entry, or - for the one nobody happened to touch - simply never noticed)
// the moment anyone reordered `SETTINGS_ENTRIES` without hunting down every one.
pub(crate) const SETTINGS_LIBRARY: &str = "Library";
pub(crate) const SETTINGS_PER_ITEM_SPEED: &str = "Per-Item Speed";
pub(crate) const SETTINGS_PODCAST_AUTOPLAY: &str = "Podcast Autoplay";
pub(crate) const SETTINGS_AUTO_DOWNLOAD: &str = "Auto Download";
pub(crate) const SETTINGS_UPDATE_UNINSTALL: &str = "Update/Uninstall";
pub(crate) const SETTINGS_ABOUT: &str = "About";
pub(crate) const SETTINGS_ACCOUNT: &str = "Account";
const SETTINGS_ENTRIES: [&str; 7] = [
    SETTINGS_LIBRARY,
    SETTINGS_PER_ITEM_SPEED,
    SETTINGS_PODCAST_AUTOPLAY,
    SETTINGS_AUTO_DOWNLOAD,
    SETTINGS_UPDATE_UNINSTALL,
    SETTINGS_ABOUT,
    SETTINGS_ACCOUNT,
];

/// Init app
impl App {
    /// Index of a named Settings menu entry in `self.settings`, if present - lets
    /// callers match by name (a compile-time-checked `SETTINGS_*` constant) instead
    /// of a raw position. See `SETTINGS_ENTRIES`'s doc comment for why.
    pub(crate) fn settings_index(&self, name: &str) -> Option<usize> {
        self.settings.iter().position(|s| s == name)
    }

    pub async fn new() -> Result<Self> {

        let config = load_config()?;
        let database = Database::new().await?;
        let changelog = changelog();

        let mut token: String = String::new();
        if let Some(var_token) = database.default_usr.get(2) {
            token = var_token.clone();
        }
        match decrypt_token(token.as_str()) {
            Ok(decrypted_token) => {
                token = decrypted_token;
                //info!("Token successfully decrypted")
            }
            Err(e) => {
                println!("Error: {e}");
            }
        }

        // retrieve crypted refresh token from database - empty for a legacy/non-JWT
        // login or an install that hasn't re-authenticated since this feature was
        // added (see refresh_token.rs's `maybe_refresh_token`, which no-ops on empty)
        let mut refresh_token: String = String::new();
        if let Some(var_refresh_token) = database.default_usr.get(15) {
            refresh_token = var_refresh_token.clone();
        }
        if !refresh_token.is_empty() {
            match decrypt_token(refresh_token.as_str()) {
                Ok(decrypted_refresh_token) => refresh_token = decrypted_refresh_token,
                Err(e) => println!("Error: {e}"),
            }
        }


        let mut _server_address: String = String::new();
        if let Some(var_server_address) = database.default_usr.get(1) {
            _server_address = var_server_address.clone();
        }

        let mut id_selected_lib: String = String::new();
        if let Some(var_id_selected_lib) = database.default_usr.get(5) {
            id_selected_lib = var_id_selected_lib.clone();
        }

        let mut username: String = String::new();
        if let Some(var_username) = database.default_usr.first() {
            username = var_username.clone();
        }

        // Without the "http(s)://" prefix, for display.
        let mut server_address: String = String::new();
        let mut server_address_pretty: String = String::new();
        if let Some(var_server_address) = database.default_usr.get(1) {
            server_address = var_server_address.clone();

            // Remove "http://" or "https://"
            if let Some(stripped) = server_address.strip_prefix("http://") {
                server_address_pretty = stripped.to_string();
            } else if let Some(stripped) = server_address.strip_prefix("https://") {
                server_address_pretty = stripped.to_string();
            }
        }

        // Proactively refresh before the very first API call below - otherwise any
        // access token that already expired while the app wasn't running (they last
        // ~1 hour) makes this fail with a 401 before `refresh_token_if_needed` ever
        // gets a chance to run (that only fires once an `App` already exists).
        if maybe_refresh_token(&mut token, &mut refresh_token, &username, &server_address).await == RefreshOutcome::Failed {
            // Refresh token itself is dead (30 days idle, or server-revoked) - same
            // handling as refresh_token_if_needed's mid-session case (see below):
            // delete the account and fail clearly here instead of proceeding to the
            // API call below with a token guaranteed to 401, which would otherwise
            // surface as a cryptic "reached the server, but HTTP 401" error with no
            // indication that a fresh login (not a retry) is what's actually needed.
            let _ = delete_user(&username);
            let _ = update_login_err("Your session expired - restart Absotui to log in again");
            return Err(Report::new(std::io::Error::other("Your session expired - restart Absotui to log in again")));
        }

        let all_libraries = get_all_libraries(&token, server_address.clone()).await?;
    let libraries_names = collect_library_names(&all_libraries).await;
    let media_types = collect_media_types(&all_libraries).await;
    let libraries_ids = collect_library_ids(&all_libraries).await;
    let mut library_name = String::new();
    let mut media_type = String::new();

    let target = id_selected_lib.clone();

    if let Some(index) = libraries_ids.iter().position(|x| x == &target) {
        library_name = libraries_names[index].clone();
        media_type = media_types[index].clone();
    }
    let is_podcast = media_type == "podcast";
    let lib_icon = if is_podcast { "🎙️" } else { "📚" };
    let lib_name_type = format!("{lib_icon} {library_name} ({media_type})");

    let mut _titles_cnt_list: Vec<String> = Vec::new();
    let mut auth_names_cnt_list: Vec<String> = Vec::new();
    let mut pub_year_cnt_list: Vec<String> = Vec::new();
    let mut duration_cnt_list: Vec<f64> = Vec::new();
    let mut size_cnt_list: Vec<i64> = Vec::new();
    let mut desc_cnt_list: Vec<String> = Vec::new();
    let mut _ids_cnt_list: Vec<String> = Vec::new();
    let mut ids_ep_cnt_list: Vec<String> = Vec::new();
    let mut subtitles_pod_cnt_list: Vec<String> = Vec::new();
    let mut nums_ep_pod_cnt_list: Vec<String> = Vec::new();
    let mut seasons_pod_cnt_list: Vec<String> = Vec::new();
    let mut authors_pod_cnt_list: Vec<String> = Vec::new();
    let mut descs_pod_cnt_list: Vec<String> = Vec::new();
    let mut titles_pod_cnt_list: Vec<String> = Vec::new();
    let mut durations_pod_cnt_list: Vec<String> = Vec::new();
    let mut podcast_progress_cnt_list: Vec<(f64, f64, f32)> = Vec::new();
    let mut podcast_published_at_cnt_list: Vec<i64> = Vec::new();
    let mut episode_embedded_cover_ino_cnt_list: Vec<Option<String>> = Vec::new();
    let podcast_sort_newest_first = true;
    let mut book_progress_cnt_list: Vec<Vec<String>> = Vec::new();
    let mut book_progress_cnt_list_cur_time: Vec<Vec<f64>> = Vec::new();

    // The Home data (Continue Listening / New & Unfinished), the full library listing,
    // and the update check are three independent round-trips that only ever needed
    // `id_selected_lib` - running them one after another just added their latencies
    // together (measured ~1.8s + ~2.4s + ~0.5s of an 8.4s startup). Driven concurrently
    // here so startup costs about the slowest of the three instead of their sum.
    //
    // `tokio::join!` rather than building the futures and awaiting them later: Rust
    // futures are lazy, so a future that isn't being polled isn't running, and awaiting
    // them in sequence would be exactly as slow as the original code.
    let home_fut = async {
    if is_podcast {
        let data = fetch_podcast_home_data(&token, server_address.clone(), &id_selected_lib, podcast_sort_newest_first, &username).await?;
        _ids_cnt_list = data.ids;
        _titles_cnt_list = data.titles;
        ids_ep_cnt_list = data.ids_ep;
        subtitles_pod_cnt_list = data.subtitles;
        nums_ep_pod_cnt_list = data.nums_ep;
        seasons_pod_cnt_list = data.seasons;
        authors_pod_cnt_list = data.authors;
        descs_pod_cnt_list = data.descs;
        titles_pod_cnt_list = data.titles_pod;
        durations_pod_cnt_list = data.durations;
        podcast_progress_cnt_list = data.progress;
        podcast_published_at_cnt_list = data.published_at;
        episode_embedded_cover_ino_cnt_list = data.embedded_cover_ino;
    }
    else {
        let continue_listening = get_continue_listening(&token, server_address.clone(), &id_selected_lib.clone()).await?;
        _titles_cnt_list = collect_titles_cnt_list(&continue_listening).await;
        auth_names_cnt_list = collect_auth_names_cnt_list(&continue_listening).await;
        pub_year_cnt_list = collect_pub_year_cnt_list(&continue_listening).await;
        duration_cnt_list = collect_duration_cnt_list(&continue_listening).await;
        size_cnt_list = collect_size_cnt_list(&continue_listening).await;
        desc_cnt_list = collect_desc_cnt_list(&continue_listening).await;
        _ids_cnt_list = collect_ids_cnt_list(&continue_listening).await;
        // Same per-item fan-out as the podcast paths (see bug_id 3f729c): one progress
        // lookup per Continue Listening book, concurrent instead of sequential.
        // `buffered` keeps request order, so each result still lands at the index its
        // book occupies in `_ids_cnt_list` / the two `book_progress_*` arrays beside it.
        let mut book_progress_futures = Vec::with_capacity(_ids_cnt_list.len());
        for id in _ids_cnt_list.clone() {
            let token = token.clone();
            let server_address = server_address.clone();
            book_progress_futures.push(async move {
                let res = get_book_progress(&token, &id, server_address).await;
                (id, res)
            });
        }
        let book_progress_results: Vec<_> = stream::iter(book_progress_futures)
            .buffered(MAX_CONCURRENT_REQUESTS)
            .collect()
            .await;

        for (id, result) in book_progress_results {
            match result {
                Ok(val) => {
                    let mut values: Vec<String> = Vec::new();
                    let mut values_f64: Vec<f64> = Vec::new();
                    values.push(collect_progress_percentage_book(&val).await);
                    values.push(collect_is_finished_book(&val).await);
                    values_f64.push(collect_current_time_prg(&val).await);
                    book_progress_cnt_list.push(values);
                    book_progress_cnt_list_cur_time.push(values_f64);
                }
                Err(e) => {
                    // This can genuinely mean "never started" (server 404s with no
                    // progress record), but it could also be a real request failure -
                    // logged so the two cases can be told apart.
                    warn!("[get_book_progress] item {id} - treating as not started: {e}");
                    let mut values: Vec<String> = Vec::new();
                    let mut values_f64: Vec<f64> = Vec::new();
                    values.push(" N/A".to_string());
                    values.push(" N/A".to_string());
                    values_f64.push(0.0);
                    book_progress_cnt_list.push(values);
                    book_progress_cnt_list_cur_time.push(values_f64);
                }
            }}}
    Ok::<(), color_eyre::Report>(())
    };

    // See UPDATE_CHECK's doc comment - only actually hits GitHub the first time this
    // process calls it, cloning the cached result every time after.
    let update_fut = async {
        match UPDATE_CHECK.get() {
            Some(cached) => cached.clone(),
            None => {
                let result = check_update().await;
                let _ = UPDATE_CHECK.set(result.clone());
                result
            }
        }
    };

    // Collections are book-only per Audiobookshelf's own schema - skipped entirely
    // for podcast libraries rather than firing a request that's never useful.
    let collections_fut = async {
        if is_podcast {
            Ok(crate::api::libraries::get_all_collections::Root::default())
        } else {
            get_all_collections(&token, &id_selected_lib, server_address.clone()).await
        }
    };

    let (home_res, all_books_res, update_res, collections_res, listening_stats_res) = tokio::join!(
        home_fut,
        get_all_books(&token, &id_selected_lib, server_address.clone()),
        update_fut,
        collections_fut,
        get_listening_stats(&token, server_address.clone()),
    );
    home_res?;
    let all_books = all_books_res?;
    let update_msg = update_res.unwrap_or_default();
    // A failed collections fetch just means the Collections ring stop doesn't
    // appear this session, not a hard startup failure like `all_books` above.
    let all_collections = collections_res.unwrap_or_default();
    // Same tolerance as collections above - a failed stats fetch just means an
    // all-zero AppView::Stats screen this session, not a startup failure.
    let stats_summary = match listening_stats_res {
        Ok(stats) => collect_stats_summary(&stats, chrono::Local::now().date_naive()).await,
        Err(_) => StatsSummary::default(),
    };

    // Settings > Auto Download: keep the local download set mirroring Continue
    // Listening (books) or New & Unfinished (podcasts). Hooked in here rather than a
    // separate periodic task since `R` and every library switch already fully
    // reconstruct `App` via this same function - see the CLAUDE.md note on App::new()
    // being the one place cross-cutting state gets refreshed. Podcasts get a second
    // hook in refresh_podcast_home_if_stale, since that list can change every few
    // seconds on its own (new episode arrives, one finishes and drops out) without a
    // full App::new() - books don't need that, Continue Listening only changes on the
    // refreshes already covered here.
    if get_is_auto_download(&username) == "1" {
        if is_podcast {
            sync_auto_downloads_podcasts(username.clone(), token.clone(), server_address.clone(), _ids_cnt_list.clone(), ids_ep_cnt_list.clone(), _titles_cnt_list.clone(), titles_pod_cnt_list.clone());
        } else {
            sync_auto_downloads(username.clone(), token.clone(), server_address.clone(), _ids_cnt_list.clone(), _titles_cnt_list.clone(), auth_names_cnt_list.clone(), config.downloads.auto_download_count);
        }
    }

    // None if the terminal doesn't support any image protocol - cover images just won't
    // be shown, falling back to text-only description panels everywhere. See
    // IMAGE_PICKER's doc comment for why this is cached rather than queried fresh
    // on every `App::new()` call.
    let image_picker = IMAGE_PICKER.get_or_init(|| ratatui_image::picker::Picker::from_query_stdio().ok()).clone();

    let titles_library = collect_titles_library(&all_books).await;
    let ids_library = collect_ids_library(&all_books).await;
    let auth_names_library = collect_auth_names_library(&all_books).await;
    let auth_names_library_pod = collect_auth_names_library_pod(&all_books).await;
    let published_year_library = collect_published_year_library(&all_books).await;
    let desc_library = collect_desc_library(&all_books).await;
    let duration_library = collect_duration_library(&all_books).await;
    let collection_names = collect_collection_names(&all_collections).await;
    let collection_book_indices = collect_collection_book_indices(&all_collections, &ids_library).await;
    let active_collection: Option<usize> = None;
    let (series_name_library, series_sequence_library) = collect_series_library(&all_books).await;
    let is_library_grouped_by_series = false;

    let ids_search_book: Vec<String> = Vec::new();
    let _auth_names_pod_search_book: Vec<String> = Vec::new();
    let _auth_names_search_book: Vec<String> = Vec::new();
    let _published_year_library_search_book: Vec<String> = Vec::new();
    let _desc_library_search_book: Vec<String> = Vec::new();
    let auth_names_search_book: Vec<String> = Vec::new();
    let auth_names_pod_search_book: Vec<String> = Vec::new();
    let published_year_library_search_book: Vec<String> = Vec::new();
    let desc_library_search_book: Vec<String> = Vec::new();
    let duration_library_search_book: Vec<f64> = Vec::new();
    let book_progress_search_book: Vec<Vec<String>> = Vec::new(); 
    let book_progress_search_book_cur_time: Vec<Vec<f64>> = Vec::new(); 
    let is_search_active = false;
    let search_query = "  ".to_string();
    // Populated in tui.rs's render_search_book, not here.
    let all_titles_pod_ep_search: Vec<Vec<String>> = Vec::new();
    let all_ids_pod_ep_search: Vec<Vec<String>> = Vec::new(); 
    let all_subtitles_pod_ep_search: Vec<Vec<String>> = Vec::new(); 
    let all_seasons_pod_ep_search: Vec<Vec<String>> = Vec::new(); 
    let all_episodes_pod_ep_search: Vec<Vec<String>> = Vec::new(); 
    let all_authors_pod_ep_search: Vec<Vec<String>> = Vec::new(); 
    let all_descs_pod_ep_search: Vec<Vec<String>> = Vec::new(); 
    let all_titles_pod_search: Vec<Vec<String>> = Vec::new(); 
    let all_durations_pod_ep_search: Vec<Vec<String>> = Vec::new(); 
    let titles_pod_ep_search: Vec<String> = Vec::new();
    let ids_library_pod_search: Vec<String> = Vec::new(); // library because we take index of library
    let subtitles_pod_ep_search: Vec<String> = Vec::new();
    let seasons_pod_ep_search: Vec<String> = Vec::new();
    let episodes_pod_ep_search: Vec<String> = Vec::new();
    let authors_pod_ep_search: Vec<String> = Vec::new();
    let descs_pod_ep_search: Vec<String> = Vec::new();
    let titles_pod_search: Vec<String> = Vec::new();
    let durations_pod_ep_search: Vec<String> = Vec::new();
    let is_from_search_pod = false;



    // One inner Vec per podcast, e.g. all_titles_pod_ep[i] is podcast i's episode titles.
    let all_titles_pod_ep: Vec<Vec<String>> = Vec::new();
    let all_ids_pod_ep: Vec<Vec<String>> = Vec::new();
    let all_subtitles_pod_ep: Vec<Vec<String>> = Vec::new();
    let all_seasons_pod_ep: Vec<Vec<String>> = Vec::new();
    let all_episodes_pod_ep: Vec<Vec<String>> = Vec::new();
    let all_authors_pod_ep: Vec<Vec<String>> = Vec::new();
    let all_descs_pod_ep: Vec<Vec<String>> = Vec::new();
    let all_titles_pod: Vec<Vec<String>> = Vec::new(); // the podcast's own title, not an episode's
    let all_durations_pod_ep: Vec<Vec<String>> = Vec::new();
    // Flat - one podcast's episode titles, unlike all_titles_pod_ep above.
    let titles_pod_ep: Vec<String> = Vec::new();
    let ids_pod_ep: Vec<String> = Vec::new();
    let ids_pod_ep_search: Vec<String> = Vec::new();
    let subtitles_pod_ep: Vec<String> = Vec::new();
    let seasons_pod_ep: Vec<String> = Vec::new();
    let episodes_pod_ep: Vec<String> = Vec::new();
    let authors_pod_ep: Vec<String> = Vec::new();
    let descs_pod_ep: Vec<String> = Vec::new();
    let titles_pod: Vec<String> = Vec::new();
    let durations_pod_ep: Vec<String> = Vec::new();

    // Every podcast's full episode list (the `all_*_pod_ep` arrays) is fetched in the
    // background rather than here - see `fetch_all_pod_ep`'s doc comment for why (bug_id
    // 3f729c: this was ~5s of a ~7s startup on a 22-podcast library). `poll_pod_ep_fetch`
    // merges the result into `self` once it arrives, called every render-loop iteration
    // from main.rs the same way `poll_update_uninstall_event` already is. Until then the
    // arrays are simply empty - `render_search_book` and the Library->PodcastEpisode
    // transition both already guard against an index not being populated yet.
    let pod_ep_receiver = if is_podcast {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let token = token.clone();
        let server_address = server_address.clone();
        let ids_library = ids_library.clone();
        tokio::spawn(async move {
            let _ = tx.send(fetch_all_pod_ep(token, server_address, ids_library).await);
        });
        Some(rx)
    } else {
        None
    };
    // init for `Settings` - order is: the settings you routinely adjust, the
    // Update/Uninstall maintenance action, then About/Account last since Account's
    // own action is destructive (see its own screen) and shouldn't sit in the
    // middle of routine navigation.
    let settings: Vec<String> = SETTINGS_ENTRIES.iter().map(|s| s.to_string()).collect();

    let mut all_usernames: Vec<String> = Vec::new();
    let mut all_server_addresses: Vec<String> = Vec::new();
    if let Some(var_username) = database.default_usr.first() {
        all_usernames.push(var_username.clone());
    }
    if let Some(var_server_address) = database.default_usr.get(1) {
        all_server_addresses.push(var_server_address.clone());
    }

    let scroll_offset = 0;

    let mut view_state = AppView::Home;
    // Nothing to continue - land on Library instead of an empty Home.
    if _ids_cnt_list.is_empty() {
        view_state = AppView::Library;
    }

    // Which screen to return to when leaving AppView::Keymap - set for real when `?`
    // is pressed, this initial value is never actually shown since Keymap can't be
    // entered before that.
    let keymap_return_view = AppView::Home;

    let is_cvlc = config.player.cvlc.clone();
    let is_cvlc_term = config.player.cvlc_term.clone();
    // On Linux, `cvlc = "1"` selects the actual `cvlc` binary; macOS has no separate
    // cvlc binary to select (VLC.app is the only one), so this always launches that
    // one regardless. Whether playback runs headless is a *separate* question, decided
    // in start_vlc.rs purely from `is_cvlc` (see bug_id fe4116) - previously this
    // override discarded `is_cvlc` outright on macOS, which is what made the setting
    // do nothing there.
    let mut start_vlc_program = match is_cvlc.as_str() {
        "1" => "cvlc".to_string(),
        _ => "vlc".to_string(),
    };
    if cfg!(target_os = "macos") {
        start_vlc_program = "/Applications/VLC.app/Contents/MacOS/VLC".to_string();
    }

    // Every list starts with its first row selected.
    let mut list_state_cnt_list = ListState::default();
    list_state_cnt_list.select(Some(0));
    let mut list_state_library = ListState::default();
    list_state_library.select(Some(0));
    let mut list_state_collections = ListState::default();
    list_state_collections.select(Some(0));
    let mut list_state_search_results = ListState::default();
    list_state_search_results.select(Some(0));
    let mut list_state_pod_ep = ListState::default();
    list_state_pod_ep.select(Some(0));
    let mut list_state_settings = ListState::default();
    list_state_settings.select(Some(0));
    let mut list_state_settings_account = ListState::default();
    list_state_settings_account.select(Some(0));
    let mut list_state_settings_library = ListState::default();
    list_state_settings_library.select(Some(0));
    let mut list_state_settings_about = ListState::default();
    list_state_settings_about.select(Some(0));
    let mut list_state_settings_update_uninstall = ListState::default();
    list_state_settings_update_uninstall.select(Some(0));
    let mut list_state_settings_autoplay = ListState::default();
    list_state_settings_autoplay.select(Some(0));
    let mut list_state_settings_per_item_speed = ListState::default();
    list_state_settings_per_item_speed.select(Some(0));
    let mut list_state_settings_auto_download = ListState::default();
    list_state_settings_auto_download.select(Some(0));

    Ok(Self {
        database,
        id_selected_lib,
        token: Some(token),
        refresh_token: Some(refresh_token),
        should_exit: false,
        list_state_cnt_list,
        list_state_library,
        list_state_collections,
        list_state_search_results,
        list_state_pod_ep,
        list_state_settings,
        list_state_settings_account,
        list_state_settings_library,
        list_state_settings_about,
        list_state_settings_update_uninstall,
        list_state_settings_autoplay,
        list_state_settings_per_item_speed,
        list_state_settings_auto_download,
        _titles_cnt_list,
        auth_names_cnt_list,
        pub_year_cnt_list,
        duration_cnt_list,
        size_cnt_list,
        desc_cnt_list,
        _ids_cnt_list,
        view_state,
        keymap_return_view,
        library_needs_reload: false,
        last_footer_height: 1,
        is_chapter_list_expanded: false,
        account_removal_confirm: false,
        titles_library,
        ids_library,
        auth_names_library,
        collection_names,
        collection_book_indices,
        active_collection,
        series_name_library,
        series_sequence_library,
        is_library_grouped_by_series,
        podcast_add_stage: None,
        podcast_add_textarea: ratatui_textarea::TextArea::default(),
        podcast_add_receiver: None,
        list_state_podcast_search_results: ListState::default(),
        podcast_search_results_cache: Vec::new(),
        podcast_add_error: None,
        podcast_remove_confirm: false,
        podcast_remove_receiver: None,
        stats_summary,
        ids_search_book,
        is_search_active,
        search_textarea: ratatui_textarea::TextArea::default(),
        search_query,
        is_podcast,
        pod_ep_receiver,
        all_titles_pod_ep,
        all_ids_pod_ep,
        titles_pod_ep,
        ids_pod_ep,
        ids_pod_ep_search,
        ids_ep_cnt_list, 
        all_titles_pod_ep_search,
        titles_pod_ep_search,
        is_from_search_pod,
        ids_library_pod_search,
        all_ids_pod_ep_search,
        libraries_names,
        libraries_ids,
        media_types,
        library_name,
        media_type,
        lib_name_type,
        settings,
        all_usernames,
        all_server_addresses,
        username,
        server_address,
        server_address_pretty,
        scroll_offset,
        subtitles_pod_cnt_list,
        nums_ep_pod_cnt_list,
        seasons_pod_cnt_list,
        authors_pod_cnt_list,
        descs_pod_cnt_list,
        titles_pod_cnt_list,
        durations_pod_cnt_list,
        podcast_progress_cnt_list,
        podcast_published_at_cnt_list,
        episode_embedded_cover_ino_cnt_list,
        podcast_sort_newest_first,
        title_scroll_offset: 0,
        title_scroll_last_tick: std::time::Instant::now(),
        title_scroll_selected: None,
        published_year_library,
        desc_library,
        duration_library,
        auth_names_library_pod,
        all_subtitles_pod_ep,
        all_seasons_pod_ep,
        all_episodes_pod_ep,
        all_authors_pod_ep,
        all_descs_pod_ep,
        all_titles_pod,
        all_durations_pod_ep,
        subtitles_pod_ep,
        seasons_pod_ep,
        episodes_pod_ep,
        authors_pod_ep,
        descs_pod_ep,
        titles_pod,
        durations_pod_ep,
        subtitles_pod_ep_search,
        seasons_pod_ep_search,
        episodes_pod_ep_search,
        authors_pod_ep_search,
        descs_pod_ep_search,
        titles_pod_search,
        durations_pod_ep_search,
        all_subtitles_pod_ep_search,
        all_seasons_pod_ep_search,
        all_episodes_pod_ep_search,
        all_authors_pod_ep_search,
        all_descs_pod_ep_search,
        all_titles_pod_search,
        all_durations_pod_ep_search,
        auth_names_pod_search_book,
        auth_names_search_book,
        published_year_library_search_book,
        desc_library_search_book,
        duration_library_search_book,
        book_progress_cnt_list,
        book_progress_cnt_list_cur_time,
        book_progress_search_book,
        book_progress_search_book_cur_time,
        is_cvlc,
        is_cvlc_term,
        start_vlc_program,
        config,
        changelog,
        update_msg,
        update_uninstall_stage: UpdateUninstallStage::Instructions,
        update_uninstall_password: TextArea::default(),
        update_uninstall_log: Vec::new(),
        update_uninstall_receiver: None,
        update_uninstall_password_tx: None,
        podcast_home_last_refresh: std::time::Instant::now(),
        image_picker,
        cover_protocol: None,
        cover_loaded_for_id: None,
        cover_fetch_requested: std::collections::HashSet::new(),
    })
    }

    // Re-fetches just the podcast "New & Unfinished" list if it's gotten stale, without
    // touching cursor position/selection or anything else - so an episode that just
    // finished (or a newly-published one) shows up without needing a manual refresh,
    // and without disrupting whatever the user is doing in the list.
    /// Called every main-loop tick (see main.rs) - proactively renews the access token
    /// well before Audiobookshelf's ~1 hour default expiry, and updates it in place so
    /// every subsequent call this same running `App` makes uses the fresh value right
    /// away, without waiting for a full `App::new()` reinit. A no-op whenever there's
    /// no refresh token to use (see `maybe_refresh_token`'s doc comment).
    ///
    /// A long-running playback session doesn't go through this `App` at all (it's a
    /// detached task talking only to sqlite - see CLAUDE.md's "one owner" note), so it
    /// carries out this exact same check independently every poll tick - see
    /// `handle_l_book`/`handle_l_pod`/`handle_l_pod_home`.
    pub async fn refresh_token_if_needed(&mut self) {
        let Some(mut token) = self.token.clone() else { return; };
        let Some(mut refresh_token) = self.refresh_token.clone() else { return; };

        match maybe_refresh_token(&mut token, &mut refresh_token, &self.username, &self.server_address).await {
            RefreshOutcome::Refreshed => {
                self.token = Some(token);
                self.refresh_token = Some(refresh_token);
            }
            RefreshOutcome::Failed => {
                // Refresh token itself is dead (30 days idle, or server-revoked) -
                // matches the existing Settings > Account "remove account" precedent:
                // delete the row and rely on the user restarting the app, since there's
                // no existing live in-process path back to the login screen.
                let _ = delete_user(&self.username);
                let _ = update_login_err("Your session expired - restart Absotui to log in again");
            }
            RefreshOutcome::NotNeeded | RefreshOutcome::TransientError => {}
        }
    }

    pub async fn refresh_podcast_home_if_stale(&mut self) -> Result<()> {
        // Finishing an episode doesn't push a signal to the main render loop (the
        // playback handler runs in a separate spawned task) - it just relies on this
        // periodic refresh to eventually notice the server-side "finished" flag and
        // drop it from the list. Kept short so that removal feels prompt.
        const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(8);

        if !self.is_podcast || self.podcast_home_last_refresh.elapsed() < STALE_AFTER {
            return Ok(());
        }

        let Some(token) = self.token.clone() else { return Ok(()) };

        // The refreshed list's composition/order can shift (an episode finishes and
        // drops out, a new one appears, published_at ties break differently) - remember
        // which episode the cursor was on so it can be re-found below, otherwise the
        // still-valid numeric index silently ends up pointing at a different episode and
        // the selection bar appears to jump around on its own every refresh.
        let selected_ep_id = self.list_state_cnt_list.selected()
            .and_then(|i| self.ids_ep_cnt_list.get(i))
            .cloned();

        let data = fetch_podcast_home_data(&token, self.server_address.clone(), &self.id_selected_lib, self.podcast_sort_newest_first, &self.username).await?;
        self._ids_cnt_list = data.ids;
        self._titles_cnt_list = data.titles;
        self.ids_ep_cnt_list = data.ids_ep;
        self.subtitles_pod_cnt_list = data.subtitles;
        self.nums_ep_pod_cnt_list = data.nums_ep;
        self.seasons_pod_cnt_list = data.seasons;
        self.authors_pod_cnt_list = data.authors;
        self.descs_pod_cnt_list = data.descs;
        self.titles_pod_cnt_list = data.titles_pod;
        self.durations_pod_cnt_list = data.durations;
        self.podcast_progress_cnt_list = data.progress;
        self.podcast_published_at_cnt_list = data.published_at;
        self.episode_embedded_cover_ino_cnt_list = data.embedded_cover_ino;
        self.podcast_home_last_refresh = std::time::Instant::now();

        if get_is_auto_download(&self.username) == "1" {
            sync_auto_downloads_podcasts(self.username.clone(), token, self.server_address.clone(), self._ids_cnt_list.clone(), self.ids_ep_cnt_list.clone(), self._titles_cnt_list.clone(), self.titles_pod_cnt_list.clone());
        }

        if let Some(id) = selected_ep_id
            && let Some(new_pos) = self.ids_ep_cnt_list.iter().position(|i| *i == id) {
                self.list_state_cnt_list.select(Some(new_pos));
        } else if let Some(session) = get_listening_session().ok().flatten()
            && let Some(new_pos) = self.ids_ep_cnt_list.iter().position(|i| *i == session.id_pod) {
                // The previously selected episode is gone - most commonly because it just
                // finished and Podcast Autoplay moved on to the next one, dropping it out of
                // this "New & Unfinished" list. Follow the now-playing episode instead of
                // leaving the cursor on whatever numeric index it used to be, which would
                // otherwise silently point at a different episode once the list shifts.
                self.list_state_cnt_list.select(Some(new_pos));
        }

        Ok(())
    }

    // Applies a permutation to every one of the podcast Home list's parallel arrays at
    // once, so they can never end up desynced from each other - used both when pinning
    // the now-playing episode to the top and when toggling sort order.
    pub fn reorder_podcast_lists(&mut self, order: &[usize]) {
        self._titles_cnt_list = order.iter().map(|&i| self._titles_cnt_list[i].clone()).collect();
        self._ids_cnt_list = order.iter().map(|&i| self._ids_cnt_list[i].clone()).collect();
        self.ids_ep_cnt_list = order.iter().map(|&i| self.ids_ep_cnt_list[i].clone()).collect();
        self.subtitles_pod_cnt_list = order.iter().map(|&i| self.subtitles_pod_cnt_list[i].clone()).collect();
        self.nums_ep_pod_cnt_list = order.iter().map(|&i| self.nums_ep_pod_cnt_list[i].clone()).collect();
        self.seasons_pod_cnt_list = order.iter().map(|&i| self.seasons_pod_cnt_list[i].clone()).collect();
        self.authors_pod_cnt_list = order.iter().map(|&i| self.authors_pod_cnt_list[i].clone()).collect();
        self.descs_pod_cnt_list = order.iter().map(|&i| self.descs_pod_cnt_list[i].clone()).collect();
        self.titles_pod_cnt_list = order.iter().map(|&i| self.titles_pod_cnt_list[i].clone()).collect();
        self.durations_pod_cnt_list = order.iter().map(|&i| self.durations_pod_cnt_list[i].clone()).collect();
        self.podcast_progress_cnt_list = order.iter().map(|&i| self.podcast_progress_cnt_list[i]).collect();
        self.podcast_published_at_cnt_list = order.iter().map(|&i| self.podcast_published_at_cnt_list[i]).collect();
        self.episode_embedded_cover_ino_cnt_list = order.iter().map(|&i| self.episode_embedded_cover_ino_cnt_list[i].clone()).collect();
    }


pub fn handle_key(&mut self, key: KeyEvent) {
    let mut is_playback = true;

    if key.kind != KeyEventKind::Press {
        return;
    }

    // The search box (`/`) owns every key itself while open, the same way the
    // Update/Uninstall sub-stages and Keymap below do - checked first since it's a
    // modal overlay on top of whatever screen it was opened from, not tied to any
    // particular view_state.
    if self.is_search_active {
        match key.code {
            KeyCode::Enter => {
                self.is_search_active = false;
                self.search_query = self.search_textarea.lines().join("\n");
                self.view_state = AppView::SearchBook;
                self.list_state_search_results.select(Some(0));
            }
            KeyCode::Esc => {
                self.is_search_active = false;
            }
            _ => {
                self.search_textarea.input(key);
            }
        }
        return;
    }

    // Settings > Account's removal confirmation owns every key itself while armed,
    // same reasoning as Update/Uninstall's Confirm stage just below - `l/→` (also
    // used for plain navigation on the rest of the app's screens) must not be able
    // to reach the actual delete a second time by accident.
    if matches!(self.view_state, AppView::SettingsAccount) && self.account_removal_confirm {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(index) = self.list_state_settings_account.selected()
                    && let Some(username) = self.all_usernames.get(index) {
                        let _ = delete_user(username.as_str());
                }
                self.account_removal_confirm = false;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.account_removal_confirm = false;
            }
            _ => {}
        }
        return;
    }

    // The Confirm/Password/Running/Failed stages of Settings > Update/Uninstall
    // own every key themselves (most importantly so free-text password typing can't
    // be swallowed by the global single-letter bindings below, eg. player controls) -
    // handle them here and return early. Only the passive `Instructions` stage (just
    // a 2-item list + Enter, like every other Settings sub-screen) falls through to
    // the normal dispatch further down.
    if matches!(self.view_state, AppView::SettingsUpdateUninstall) {
        match &self.update_uninstall_stage {
            UpdateUninstallStage::Confirm(action) => {
                let action = *action;
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        self.update_uninstall_log.clear();
                        let (rx, password_tx) = update_uninstall::spawn(action);
                        self.update_uninstall_receiver = Some(rx);
                        self.update_uninstall_password_tx = Some(password_tx);
                        self.update_uninstall_stage = UpdateUninstallStage::Running(action);
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        self.update_uninstall_stage = UpdateUninstallStage::Instructions;
                    }
                    _ => {}
                }
                return;
            }
            UpdateUninstallStage::Password(action) => {
                let action = *action;
                match key.code {
                    KeyCode::Enter => {
                        let password = self.update_uninstall_password.lines().join("\n");
                        self.update_uninstall_password = TextArea::default();
                        if let Some(tx) = &self.update_uninstall_password_tx {
                            let _ = tx.send(password);
                        }
                        self.update_uninstall_stage = UpdateUninstallStage::Running(action);
                    }
                    KeyCode::Esc => {
                        // Drop the sender - update_uninstall::negotiate's password_rx.recv()
                        // sees the channel close, kills the in-flight sudo/pty, and returns.
                        self.update_uninstall_password_tx = None;
                        self.update_uninstall_stage = UpdateUninstallStage::Confirm(action);
                    }
                    _ => {
                        self.update_uninstall_password.input(key);
                    }
                }
                return;
            }
            UpdateUninstallStage::Running(_) => {
                // No interaction in v1 - the log keeps streaming in via main.rs's
                // channel drain every loop iteration regardless of key input.
                return;
            }
            UpdateUninstallStage::Failed(_, _) => {
                if let KeyCode::Esc = key.code {
                    self.update_uninstall_stage = UpdateUninstallStage::Instructions;
                    self.update_uninstall_receiver = None;
                    self.update_uninstall_password_tx = None;
                    self.update_uninstall_log.clear();
                }
                return;
            }
            UpdateUninstallStage::Instructions => {}
        }
    }

    // AppView::Keymap owns most keys itself, the same way the Update/Uninstall
    // sub-stages above do - but Esc and Tab deliberately fall through to their
    // normal global meaning below instead of being intercepted here, so Esc still
    // quits (consistent with every other screen) and Tab still goes to Home via
    // the ordinary toggle_view() path.
    if matches!(self.view_state, AppView::Keymap) {
        match key.code {
            KeyCode::Char('?') => {
                self.view_state = self.keymap_return_view;
                return;
            }
            KeyCode::Esc | KeyCode::Tab => {}
            _ => return,
        }
    }

    // Podcast subscription add flow (`A` on Library, podcast mode) owns every key
    // itself while active, same reasoning as the blocks above - Input is checked
    // here (like the `/` search overlay's is_search_active) before view_state has
    // even changed to PodcastAdd yet; Loading/Results/Confirm all run with
    // view_state already PodcastAdd, so the big match below never sees this view
    // (see its own no-op arm).
    if let Some(stage) = self.podcast_add_stage.take() {
        match stage {
            PodcastAddStage::Input => {
                match key.code {
                    KeyCode::Enter => {
                        let query = self.podcast_add_textarea.lines().join("\n").trim().to_string();
                        if query.is_empty() {
                            self.podcast_add_stage = Some(PodcastAddStage::Input);
                            return;
                        }
                        self.podcast_add_error = None;
                        self.podcast_add_stage = Some(PodcastAddStage::Loading);
                        self.view_state = AppView::PodcastAdd;
                        let token = self.token.clone();
                        let server_address = self.server_address.clone();
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        self.podcast_add_receiver = Some(rx);
                        tokio::spawn(async move {
                            let Some(token) = token else {
                                let _ = tx.send(PodcastAddOutcome::Failed("Not logged in".to_string()));
                                return;
                            };
                            let outcome = if query.starts_with("http://") || query.starts_with("https://") {
                                match get_podcast_feed(&query, &token, server_address).await {
                                    Ok(podcast) => {
                                        let result = PodcastSearchResult {
                                            title: podcast.metadata.title,
                                            artist_name: podcast.metadata.author,
                                            description: podcast.metadata.description,
                                            description_plain: podcast.metadata.description_plain,
                                            cover: podcast.metadata.image,
                                            feed_url: podcast.metadata.feed_url.or(Some(query.clone())),
                                            track_count: Some(podcast.num_episodes),
                                            genres: podcast.metadata.categories,
                                            ..Default::default()
                                        };
                                        PodcastAddOutcome::Found(vec![result])
                                    }
                                    Err(e) => PodcastAddOutcome::Failed(e.to_string()),
                                }
                            } else {
                                match search_podcast(&query, &token, server_address).await {
                                    Ok(results) => PodcastAddOutcome::Found(results),
                                    Err(e) => PodcastAddOutcome::Failed(e.to_string()),
                                }
                            };
                            let _ = tx.send(outcome);
                        });
                    }
                    KeyCode::Esc => {
                        self.podcast_add_error = None;
                    }
                    _ => {
                        self.podcast_add_textarea.input(key);
                        self.podcast_add_stage = Some(PodcastAddStage::Input);
                    }
                }
                return;
            }
            PodcastAddStage::Loading => {
                if !matches!(key.code, KeyCode::Esc) {
                    self.podcast_add_stage = Some(PodcastAddStage::Loading);
                    return;
                }
                self.podcast_add_receiver = None;
                self.view_state = AppView::Library;
                return;
            }
            PodcastAddStage::Results(results) => {
                match key.code {
                    KeyCode::Esc => {
                        self.view_state = AppView::Library;
                    }
                    KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                        if let Some(chosen) = self.list_state_podcast_search_results.selected().and_then(|i| results.get(i)).cloned() {
                            self.podcast_add_stage = Some(PodcastAddStage::Loading);
                            let token = self.token.clone();
                            let server_address = self.server_address.clone();
                            let (tx, rx) = tokio::sync::oneshot::channel();
                            self.podcast_add_receiver = Some(rx);
                            tokio::spawn(async move {
                                let Some(token) = token else {
                                    let _ = tx.send(PodcastAddOutcome::ConfirmReady(Box::new(chosen)));
                                    return;
                                };
                                let Some(feed_url) = chosen.feed_url.clone() else {
                                    let _ = tx.send(PodcastAddOutcome::ConfirmReady(Box::new(chosen)));
                                    return;
                                };
                                // On failure, proceed with the search result as-is (still has
                                // title/author/cover, just no description) rather than
                                // blocking the whole flow over an enrichment step that failed.
                                let enriched = match get_podcast_feed(&feed_url, &token, server_address).await {
                                    Ok(podcast) => PodcastSearchResult {
                                        title: podcast.metadata.title.filter(|s| !s.is_empty()).or(chosen.title.clone()),
                                        artist_name: podcast.metadata.author.filter(|s| !s.is_empty()).or(chosen.artist_name.clone()),
                                        description: podcast.metadata.description.filter(|s| !s.is_empty()),
                                        description_plain: podcast.metadata.description_plain.filter(|s| !s.is_empty()),
                                        cover: podcast.metadata.image.filter(|s| !s.is_empty()).or(chosen.cover.clone()),
                                        track_count: if podcast.num_episodes > 0 { Some(podcast.num_episodes) } else { chosen.track_count },
                                        genres: if podcast.metadata.categories.is_empty() { chosen.genres.clone() } else { podcast.metadata.categories },
                                        feed_url: Some(feed_url),
                                        id: chosen.id,
                                        artist_id: chosen.artist_id,
                                        release_date: chosen.release_date.clone(),
                                        page_url: chosen.page_url.clone(),
                                        explicit: chosen.explicit,
                                    },
                                    Err(_) => chosen,
                                };
                                let _ = tx.send(PodcastAddOutcome::ConfirmReady(Box::new(enriched)));
                            });
                        } else {
                            self.podcast_add_stage = Some(PodcastAddStage::Results(results));
                        }
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.podcast_add_stage = Some(PodcastAddStage::Results(results));
                        self.select_next();
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.podcast_add_stage = Some(PodcastAddStage::Results(results));
                        self.select_previous();
                    }
                    _ => {
                        self.podcast_add_stage = Some(PodcastAddStage::Results(results));
                    }
                }
                return;
            }
            PodcastAddStage::Confirm { chosen, episode_count } => {
                match key.code {
                    KeyCode::Esc => {
                        self.view_state = AppView::Library;
                    }
                    KeyCode::Char('h') => {
                        self.podcast_add_stage = Some(PodcastAddStage::Results(self.podcast_search_results_cache.clone()));
                    }
                    KeyCode::Left => {
                        let episode_count = match episode_count { 5 => 3, 10 => 5, _ => 10 };
                        self.podcast_add_stage = Some(PodcastAddStage::Confirm { chosen, episode_count });
                    }
                    KeyCode::Right => {
                        let episode_count = match episode_count { 3 => 5, 5 => 10, _ => 3 };
                        self.podcast_add_stage = Some(PodcastAddStage::Confirm { chosen, episode_count });
                    }
                    KeyCode::Enter => {
                        self.podcast_add_stage = Some(PodcastAddStage::Loading);
                        let token = self.token.clone();
                        let server_address = self.server_address.clone();
                        let library_id = self.id_selected_lib.clone();
                        let (tx, rx) = tokio::sync::oneshot::channel();
                        self.podcast_add_receiver = Some(rx);
                        tokio::spawn(async move {
                            let Some(token) = token else {
                                let _ = tx.send(PodcastAddOutcome::Failed("Not logged in".to_string()));
                                return;
                            };
                            let outcome = create_and_seed_podcast(*chosen, episode_count, library_id, token, server_address).await;
                            let _ = tx.send(outcome);
                        });
                    }
                    _ => {
                        self.podcast_add_stage = Some(PodcastAddStage::Confirm { chosen, episode_count });
                    }
                }
                return;
            }
        }
    }

    // Removing a subscribed podcast (`C` on Library, podcast mode) - a 3-way inline
    // confirm over the current Library screen, same reasoning as
    // account_removal_confirm's own doc comment (l/→, also plain navigation
    // elsewhere, must not be able to reach the actual delete a second time by
    // accident).
    if matches!(self.view_state, AppView::Library) && self.podcast_remove_confirm {
        match key.code {
            KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Char('h') | KeyCode::Char('H') => {
                let hard = matches!(key.code, KeyCode::Char('h') | KeyCode::Char('H'));
                self.podcast_remove_confirm = false;
                if let Some(item_id) = self.selected_library_book_index().and_then(|i| self.ids_library.get(i)).cloned() {
                    let token = self.token.clone();
                    let server_address = self.server_address.clone();
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    self.podcast_remove_receiver = Some(rx);
                    tokio::spawn(async move {
                        let Some(token) = token else {
                            let _ = tx.send(());
                            return;
                        };
                        if let Err(e) = delete_library_item(&item_id, hard, &token, server_address).await {
                            log::error!("[podcast_remove_confirm] {item_id}: {e}");
                        }
                        // Reload regardless of success/failure - a failed delete leaves
                        // the library unchanged, so the "reload" just confirms that.
                        let _ = tx.send(());
                    });
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.podcast_remove_confirm = false;
            }
            _ => {}
        }
        return;
    }

    match key.code {
        // PLAYER //
        // toggle playback/pause
        KeyCode::Char(' ') => {
            let _ = handle_key_player(" ", self.config.player.address.as_str(), self.config.player.port.as_str(), &mut is_playback, self.username.as_str());
        }
        // jump forward
        KeyCode::Char('p') => {
            let _ = handle_key_player("p", self.config.player.address.as_str(), self.config.player.port.as_str(), &mut is_playback, self.username.as_str());
        }

        // jump backward
        KeyCode::Char('u') => {
            let _ = handle_key_player("u", self.config.player.address.as_str(), self.config.player.port.as_str(), &mut is_playback, self.username.as_str());
        }

        // next chapter
        KeyCode::Char('P') => {
            let _  = handle_key_player("P", self.config.player.address.as_str(), self.config.player.port.as_str(), &mut is_playback, self.username.as_str());
        }

        // previous chapter
        KeyCode::Char('U') => {
            let _ = handle_key_player("U", self.config.player.address.as_str(), self.config.player.port.as_str(), &mut is_playback, self.username.as_str());
        }

        // speed rate up
        KeyCode::Char('O') => {
            let _ = handle_key_player("O", self.config.player.address.as_str(), self.config.player.port.as_str(), &mut is_playback, self.username.as_str()); 
        }

        // speed rate down
        KeyCode::Char('I') => {
            let _ = handle_key_player("I", self.config.player.address.as_str(), self.config.player.port.as_str(), &mut is_playback, self.username.as_str()); 
        }

        // volume up
        KeyCode::Char('o') => {
            let _ = handle_key_player("o", self.config.player.address.as_str(), self.config.player.port.as_str(), &mut is_playback, self.username.as_str()); 
        }

        // volume down
        KeyCode::Char('i') => {
            let _ = handle_key_player("i", self.config.player.address.as_str(), self.config.player.port.as_str(), &mut is_playback, self.username.as_str()); 
        }

        // stop playback (shuts down VLC - not the app, see Q/Esc for that)
        KeyCode::Char('X') => {
            let _ = handle_key_player("stop", self.config.player.address.as_str(), self.config.player.port.as_str(), &mut is_playback, self.username.as_str());
        }

        // show key bindings
        KeyCode::Char('B') => {
            let value = get_is_show_key_bindings(self.username.as_str());
            if value == "0" {
            let _ = update_is_show_key_bindings("1", self.username.as_str());
            } else if value == "1" {
            let _ = update_is_show_key_bindings("0", self.username.as_str());
            }
        }

        // toggle the currently-playing book's inline chapter list in Continue Listening
        KeyCode::Char('c') => {
            if matches!(self.view_state, AppView::Home) && !self.is_podcast {
                let is_now_playing_visible = get_listening_session().ok().flatten()
                    .is_some_and(|s| self._ids_cnt_list.contains(&s.id_item));
                if is_now_playing_visible {
                    // Remember which book the cursor was on (or, if it was on a chapter
                    // row, the now-playing book those chapters belong to) so it can be
                    // re-found afterward - expanding/collapsing shifts every row below
                    // the now-playing book by however many chapter rows appear/disappear.
                    let selected_row = self.list_state_cnt_list.selected()
                        .and_then(|i| self.build_home_rows().get(i).cloned());

                    self.is_chapter_list_expanded = !self.is_chapter_list_expanded;

                    let reposition_id = match selected_row {
                        Some(HomeRow::Book(i)) => self._ids_cnt_list.get(i).cloned(),
                        Some(HomeRow::Chapter { .. }) => get_listening_session().ok().flatten().map(|s| s.id_item),
                        None => None,
                    };
                    if let Some(id) = reposition_id
                        && let Some(new_pos) = self.build_home_rows().iter().position(|row| matches!(row, HomeRow::Book(i) if self._ids_cnt_list.get(*i) == Some(&id))) {
                            self.list_state_cnt_list.select(Some(new_pos));
                    }
                }
            }
        }

        // toggle speed-adjusted (real) vs raw content time for Elapsed/Left
        KeyCode::Char('T') => {
            let value = get_is_speed_adjusted_time(self.username.as_str());
            if value == "0" {
            let _ = update_is_speed_adjusted_time("1", self.username.as_str());
            } else if value == "1" {
            let _ = update_is_speed_adjusted_time("0", self.username.as_str());
            }
        }

        // toggle newest/oldest-first sort order for the podcast New & Unfinished list.
        // Re-sorts immediately using data already in memory - no need to re-fetch.
        // Gated on Home like `d`/`F` right below it - this reorders _cnt_list state that
        // only Home renders, so without the view check it silently reordered/reselected
        // that state from any screen whenever podcast mode was on (only advertised on
        // Home's footer, but was actually live everywhere).
        KeyCode::Char('D') if self.is_podcast && matches!(self.view_state, AppView::Home) => {
            self.podcast_sort_newest_first = !self.podcast_sort_newest_first;

            let selected_ep_id = self.list_state_cnt_list.selected()
                .and_then(|i| self.ids_ep_cnt_list.get(i))
                .cloned();

            let mut order: Vec<usize> = (0..self.podcast_published_at_cnt_list.len()).collect();
            if self.podcast_sort_newest_first {
                order.sort_by_key(|&i| std::cmp::Reverse(self.podcast_published_at_cnt_list[i]));
            } else {
                order.sort_by_key(|&i| self.podcast_published_at_cnt_list[i]);
            }
            self.reorder_podcast_lists(&order);

            if let Some(id) = selected_ep_id
                && let Some(new_pos) = self.ids_ep_cnt_list.iter().position(|i| *i == id) {
                    self.list_state_cnt_list.select(Some(new_pos));
            }
        }

        KeyCode::Char('S') if !self.is_podcast && matches!(self.view_state, AppView::Library) => {
            self.is_library_grouped_by_series = !self.is_library_grouped_by_series;
            // Row layout just changed shape entirely (headers spliced in/out).
            self.list_state_library.select(Some(0));
        }

        // Add a podcast subscription - opens the same kind of text-input overlay as
        // `/` search, but for a real server-side search/feed-fetch rather than a
        // client-side filter of what's already in the library (see PodcastAddStage's
        // own doc comment). Lives on Library itself, not a separate destination -
        // see known_bugs.md/the plan behind this feature for why that matters.
        KeyCode::Char('A') if self.is_podcast && matches!(self.view_state, AppView::Library) => {
            self.podcast_add_textarea = ratatui_textarea::TextArea::default();
            self.podcast_add_textarea.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Add podcast (search titles or enter RSS URL)")
                    .border_style(Style::new().fg(crate::ui::theme::ACCENT_KEY))
            );
            self.podcast_add_error = None;
            self.podcast_add_stage = Some(PodcastAddStage::Input);
        }

        // Remove a subscribed podcast - arms the inline confirm handled early in
        // this function, same reasoning as account_removal_confirm's own comment.
        KeyCode::Char('C') if self.is_podcast && matches!(self.view_state, AppView::Library) => {
            if self.selected_library_book_index().is_some() {
                self.podcast_remove_confirm = true;
            }
        }

        // Download (or remove the local copy of) the selected book, or podcast episode,
        // for offline playback - see src/utils/download_cache.rs.
        KeyCode::Char('d') if !self.is_podcast && matches!(self.view_state, AppView::Home) => {
            let selected = self.list_state_cnt_list.selected().and_then(|selected| {
                match self.build_home_rows().get(selected) {
                    Some(HomeRow::Book(i)) => Some(*i),
                    Some(HomeRow::Chapter { book_index, .. }) => Some(*book_index),
                    None => None,
                }
            });

            if let Some(i) = selected
                && let Some(id) = self._ids_cnt_list.get(i).cloned() {
                    let username = self.username.clone();
                    if is_downloaded(&username, &id) {
                        let _ = remove_download(&username, &id);
                    } else if let Some(token) = self.token.clone() {
                        let title = self._titles_cnt_list.get(i).cloned().unwrap_or_default();
                        let author = self.auth_names_cnt_list.get(i).cloned().unwrap_or_default();
                        let server_address = self.server_address.clone();
                        tokio::spawn(async move {
                            let mut stdout = stdout();
                            let _ = pop_message(&mut stdout, 3, "Downloading for offline playback...");
                            if let Err(e) = download_book(token, id.clone(), title, author, username, server_address).await {
                                error!("[handle_key][download_book] {id}: {e}");
                            }
                            let _ = clear_message(&mut stdout, 3);
                        });
                    }
            }
        }

        KeyCode::Char('d') if self.is_podcast && matches!(self.view_state, AppView::Home) => {
            if let Some(i) = self.list_state_cnt_list.selected()
                && let Some(episode_id) = self.ids_ep_cnt_list.get(i).cloned() {
                    let username = self.username.clone();
                    if is_downloaded(&username, &episode_id) {
                        let _ = remove_download(&username, &episode_id);
                    } else if let Some(token) = self.token.clone()
                        && let Some(podcast_id) = self._ids_cnt_list.get(i).cloned() {
                            let title = self._titles_cnt_list.get(i).cloned().unwrap_or_default();
                            let podcast_title = self.titles_pod_cnt_list.get(i).cloned().unwrap_or_default();
                            let server_address = self.server_address.clone();
                            tokio::spawn(async move {
                                let mut stdout = stdout();
                                let _ = pop_message(&mut stdout, 3, "Downloading for offline playback...");
                                if let Err(e) = download_episode(token, podcast_id, episode_id.clone(), title, podcast_title, username, server_address).await {
                                    error!("[handle_key][download_episode] {episode_id}: {e}");
                                }
                                let _ = clear_message(&mut stdout, 3);
                            });
                    }
            }
        }

        // Marks the selected podcast episode as finished server-side and removes it
        // from the New & Unfinished list immediately, rather than waiting on the next
        // periodic refresh to notice the server no longer considers it unfinished.
        // Reuses reorder_podcast_lists (built for resorting, not removal) by permuting
        // to every index except the removed one - same effect on all 13 parallel
        // arrays, no separate per-array removal logic needed.
        KeyCode::Char('F') if self.is_podcast && matches!(self.view_state, AppView::Home) => {
            if let Some(selected) = self.list_state_cnt_list.selected()
                && let Some(id_pod) = self._ids_cnt_list.get(selected).cloned()
                && let Some(ep_id) = self.ids_ep_cnt_list.get(selected).cloned() {
                    let duration = self.podcast_progress_cnt_list.get(selected).map(|&(_, duration, _)| duration).unwrap_or(0.0);
                    let token = self.token.clone();
                    let server_address = self.server_address.clone();

                    // If this episode is the one actively playing, the live playback
                    // task (handle_l_pod_home) owns its progress syncing - every ~10s
                    // it PATCHes progress/currentTime with no isFinished field at all,
                    // which would silently revert the isFinished=true set below the
                    // moment it next runs (the list item would vanish then reappear).
                    // Flip listening_session.is_finished instead and let that task
                    // notice it (polled every ~1s) and stop playback + mark finished
                    // itself exactly once, with nothing left to race it.
                    let currently_playing_session = if get_is_vlc_running(self.username.as_str()) == "1" {
                        match get_listening_session() {
                            Ok(Some(session)) if session.id_pod == ep_id => Some(session),
                            _ => None,
                        }
                    } else {
                        None
                    };

                    if let Some(session) = currently_playing_session {
                        let _ = update_is_finished("1", session.id_session.as_str());
                    } else {
                        // Marked at full duration (not whatever partial progress it was
                        // at) - "finished" should mean fully listened, matching what a
                        // natural playback completion would also land on.
                        tokio::spawn(async move {
                            if let Err(e) = update_media_progress2_pod(&id_pod, token.as_ref(), Some(duration as u32), &duration.to_string(), true, &ep_id, server_address).await {
                                log::warn!("[mark_finished] episode {ep_id}: {e}");
                            }
                        });
                    }

                    let order: Vec<usize> = (0..self._ids_cnt_list.len()).filter(|&i| i != selected).collect();
                    self.reorder_podcast_lists(&order);

                    let new_len = self._ids_cnt_list.len();
                    if new_len == 0 {
                        self.list_state_cnt_list.select(None);
                    } else if selected >= new_len {
                        self.list_state_cnt_list.select(Some(new_len - 1));
                    }
            }
        }



        // END PLAYER //

        KeyCode::Char('/') => {
            self.search_textarea = ratatui_textarea::TextArea::default();
            self.search_textarea.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Search")
                    // ACCENT_KEY (yellow), matching the footer/player keybind chips -
                    // not ACCENT_STRUCTURE like every other section's border, since
                    // this box is reached via one of those chips (`/`) rather than
                    // being a permanent structural section of the screen.
                    .border_style(Style::new().fg(crate::ui::theme::ACCENT_KEY))
            );
            self.is_search_active = true;
        }
        KeyCode::Char('s') => {
            self.view_state = AppView::Settings;
        }
        KeyCode::Tab => {
            if self.is_from_search_pod {
                self.is_from_search_pod = false;
            }
            self.toggle_view();
        }
        KeyCode::Char('?') => {
            self.keymap_return_view = self.view_state;
            self.scroll_offset = 0;
            self.view_state = AppView::Keymap;
        }

        KeyCode::Char('Q') | KeyCode::Esc => {

            let message_quit = "Exiting the application and syncing data, please hold on.";
            let mut stdout = stdout();
            let _ = pop_message(&mut stdout, 3, message_quit);

            // close and sync session before close the app - if a playback task is
            // still actively watching a session, quit_app defers the actual close/sync
            // to it instead of racing it (see quit_app's doc comment)
            let token = self.token.clone();
            let server_address = self.server_address.clone();
            let username = self.username.clone();
            let player_address = self.config.player.address.clone();
            let port = self.config.player.port.clone();

            tokio::spawn(async move {
                quit_app(token, server_address, username, player_address, port).await;
            });

        }

        KeyCode::Char('j') | KeyCode::Down => {
            self.select_next();
            self.scroll_offset = 0; 

        }
        // J/K/H scroll the Description panel, independent of list selection.
        KeyCode::Char('J') => self.scroll_offset += 1,
        KeyCode::Char('H') => self.scroll_offset = 0,
        KeyCode::Char('k') | KeyCode::Up => {
            self.select_previous();
            self.scroll_offset = 0;
        }

        KeyCode::Char('K') => {
            if usize::from(self.scroll_offset) > 0 {
                self.scroll_offset -= 1;
            }
        }
        KeyCode::Char('g') | KeyCode::Home => {
            self.select_first();
            self.scroll_offset = 0; 
        }        
        KeyCode::Char('G') | KeyCode::End => {
            self.select_last();
            self.scroll_offset = 0; 
        }
        KeyCode::Char('h') => {
            match self.view_state {
                AppView::SettingsAccount => {self.view_state = AppView::Settings} 
                AppView::SettingsLibrary => {self.view_state = AppView::Settings} 
                AppView::SettingsAbout => {self.view_state = AppView::Settings}
                AppView::SettingsUpdateUninstall => {self.view_state = AppView::Settings}
                AppView::SettingsAutoplay => {self.view_state = AppView::Settings}
                AppView::SettingsPerItemSpeed => {self.view_state = AppView::Settings}
                AppView::SettingsAutoDownload => {self.view_state = AppView::Settings}
                AppView::Settings => {self.view_state = AppView::Home}
                AppView::PodcastEpisode => {
                    if self.is_from_search_pod {
                        self.view_state = AppView::SearchBook;
                    } else {
                        self.view_state = AppView::Library;
                    }
                }
                AppView::Library if self.active_collection.is_some() => {
                    self.active_collection = None;
                    self.view_state = AppView::Collections;
                }
                _ => {}
            }
        }
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
            // If the chapter list is expanded under the currently-playing book, the
            // cursor may be sitting on a chapter row rather than a book row - request a
            // jump to that chapter's start instead of restarting playback entirely.
            // Routed through pending_seek rather than seeking the running VLC directly:
            // for a book split across several files, the target chapter may live in a
            // different file than whatever's currently loaded, and only the playback
            // loop that owns that VLC process (handle_l_book) knows which - see
            // update_pending_seek's doc comment.
            if matches!(self.view_state, AppView::Home) && !self.is_podcast
                && let Some(selected) = self.list_state_cnt_list.selected()
                && let Some(HomeRow::Chapter { chapter, .. }) = self.build_home_rows().get(selected)
                && let Ok(Some(session)) = get_listening_session() {
                    // Round up, not down: chapter boundaries are fractional (e.g.
                    // 9836.105873), and seeking to the truncated whole second would land
                    // just before the real boundary - long enough for one polling tick to
                    // still classify it as the previous chapter and flash the wrong marker.
                    let start = chapter.start.unwrap_or(0.0).ceil() as u32;
                    let _ = update_pending_seek(&start.to_string(), &session.id_session);
                    return;
            }

            let token = self.token.clone();
            let refresh_token = self.refresh_token.clone();
            let port = self.config.player.port.clone();
            let address_player = self.config.player.address.clone();
            let server_address = self.server_address.clone();
            let username = self.username.clone();

            let ids_cnt_list = self._ids_cnt_list.clone();
            let selected_cnt_list = if matches!(self.view_state, AppView::Home) && !self.is_podcast && self.is_chapter_list_expanded {
                // Any selection reaching here is a book row (chapter rows already handled
                // and returned above) - resolve its real index in `_ids_cnt_list`, since
                // the flat ListState index is offset by however many chapter rows are
                // spliced in above it.
                self.list_state_cnt_list.selected().and_then(|selected| {
                    match self.build_home_rows().get(selected) {
                        Some(HomeRow::Book(i)) => Some(*i),
                        _ => None,
                    }
                })
            } else {
                self.list_state_cnt_list.selected()
            };

            let ids_library = self.ids_library.clone();
            // Resolved from Library's row space (which may be collection-filtered
            // and/or series-grouped, see build_library_rows) back to a real index
            // into `ids_library` above - `None` for a series header row, same as
            // nothing being selected.
            let selected_library = self.selected_library_book_index();

            let ids_search_book = self.ids_search_book.clone();
            let selected_search_book = self.list_state_search_results.selected();

            if self.is_podcast {
                if let Some(index) = selected_library
                    && let Some(_id_pod) = ids_library.get(index) {
                        // `.get` rather than `[index]`: the background fetch this
                        // depends on (see bug_id 3f729c, poll_pod_ep_fetch) may not
                        // have delivered yet - if so, this is prep for a screen
                        // transition that itself already guards on readiness below, so
                        // there's nothing meaningful to set here yet.
                        if let Some(ids) = self.all_ids_pod_ep.get(index) {
                            self.ids_pod_ep = ids.clone();
                        }
                    }
                if let Some(index) = selected_search_book {
                    // ids_library_pod_search holds the podcast id here, not the episode id.
                    if let Some(_id_pod) = self.ids_library_pod_search.get(index) {
                        // `.get` rather than `[index]`: `ids_library_pod_search` doesn't
                        // depend on the background pod_ep fetch (bug_id 3f729c) but
                        // `all_ids_pod_ep_search` does - it's filtered from
                        // `all_ids_pod_ep`, which can legitimately be shorter (even
                        // empty) than the search results list while that fetch is still
                        // in flight. Same "nothing meaningful to set yet" reasoning as
                        // the non-search version above.
                        if let Some(ids) = self.all_ids_pod_ep_search.get(index) {
                            self.ids_pod_ep_search = ids.clone();
                        }
                    }}
            }
            let selected_account = self.list_state_settings_account.selected();
            let selected_settings_library = self.list_state_settings_library.selected();

            let start_vlc_program = self.start_vlc_program.clone();
            let is_cvlc = self.is_cvlc.clone();
            let is_cvlc_term = self.is_cvlc_term.clone();

            // Podcast Autoplay needs these to re-fetch the live "New & Unfinished"
            // queue on each transition rather than relying on a stale snapshot - see
            // handle_l_pod_home::next_autoplay_episode.
            let id_selected_lib = self.id_selected_lib.clone();
            let podcast_sort_newest_first = self.podcast_sort_newest_first;
            let initial_published_at = selected_cnt_list.and_then(|i| self.podcast_published_at_cnt_list.get(i)).copied();

            let message = "Loading the media...";

            match self.view_state {
                AppView::Home => {
                    if self.is_podcast {
                        let _selected_pod_ep = self.list_state_pod_ep.selected();
                        let ids_ep_cnt_list = self.ids_ep_cnt_list.clone();

                        tokio::spawn(async move {
                            let _ = quit_vlc(address_player.as_str(), port.as_str());

                            pkill_vlc();

                            // wait to close/sync the previous session first - see
                            // wait_prev_session_finished for what a false return means
                            if wait_prev_session_finished(username.clone()) {

                            let mut stdout = stdout();
                            let _ = pop_message(&mut stdout, 3, message);

                            handle_l_pod_home(
                                token.as_ref(),
                                refresh_token.as_ref(),
                                &ids_cnt_list,
                                selected_cnt_list,
                                port,
                                address_player,
                                ids_ep_cnt_list,
                                server_address,
                                start_vlc_program,
                                is_cvlc,
                                is_cvlc_term,
                                username,
                                id_selected_lib,
                                podcast_sort_newest_first,
                                initial_published_at,
                            ).await;
                            }
                        });
                    } else {

                        tokio::spawn(async move {

                            let _ = quit_vlc(address_player.as_str(), port.as_str());

                            pkill_vlc();

                            // wait to close/sync the previous session first - see
                            // wait_prev_session_finished for what a false return means
                            if wait_prev_session_finished(username.clone()) {

                            let mut stdout = stdout();
                            let _ = pop_message(&mut stdout, 3, message);

                            handle_l_book(
                                token.as_ref(),
                                refresh_token.as_ref(),
                                ids_cnt_list,
                                selected_cnt_list,
                                port,
                                address_player,
                                server_address,
                                start_vlc_program,
                                is_cvlc,
                                is_cvlc_term,
                                username,
                            ).await;
                            }
                        });

                    }}
                AppView::Settings => {
                    let selected = self.list_state_settings.selected();
                    // `About` has no case here deliberately - it's not something you
                    // navigate deeper into, l/→ does nothing on it (see
                    // render_info_settings/render_desc_settings, which show the
                    // changelog inline the moment it's just selected).
                    if selected == self.settings_index(SETTINGS_LIBRARY) {
                        self.view_state = AppView::SettingsLibrary;
                    } else if selected == self.settings_index(SETTINGS_PER_ITEM_SPEED) {
                        self.view_state = AppView::SettingsPerItemSpeed;
                    } else if selected == self.settings_index(SETTINGS_PODCAST_AUTOPLAY) {
                        self.view_state = AppView::SettingsAutoplay;
                    } else if selected == self.settings_index(SETTINGS_ACCOUNT) {
                        self.view_state = AppView::SettingsAccount;
                    } else if selected == self.settings_index(SETTINGS_UPDATE_UNINSTALL) {
                        self.view_state = AppView::SettingsUpdateUninstall;
                    } else if selected == self.settings_index(SETTINGS_AUTO_DOWNLOAD) {
                        self.view_state = AppView::SettingsAutoDownload;
                    }
                }
                AppView::SettingsUpdateUninstall => {
                    match self.list_state_settings_update_uninstall.selected() {
                        Some(0) => self.update_uninstall_stage = UpdateUninstallStage::Confirm(Action::Update),
                        Some(1) => self.update_uninstall_stage = UpdateUninstallStage::Confirm(Action::Uninstall),
                        _ => {}
                    }
                }
                // Arms the confirmation gate above rather than deleting directly - see
                // account_removal_confirm's doc comment.
                AppView::SettingsAccount => {
                    if selected_account.is_some() {
                        self.account_removal_confirm = true;
                    }
                }
                AppView::SettingsAutoplay => {
                    if let Some(index) = self.list_state_settings_autoplay.selected() {
                        let value = if index == 0 { "1" } else { "0" };
                        let _ = update_is_podcast_autoplay(value, &self.username);
                    }
                }
                AppView::SettingsPerItemSpeed => {
                    if let Some(index) = self.list_state_settings_per_item_speed.selected() {
                        let value = if index == 0 { "1" } else { "0" };
                        let _ = update_is_per_item_speed(value, &self.username);
                    }
                }
                AppView::SettingsAutoDownload => {
                    if let Some(index) = self.list_state_settings_auto_download.selected() {
                        let value = if index == 0 { "1" } else { "0" };
                        let _ = update_is_auto_download(value, &self.username);
                    }
                }
                AppView::SettingsLibrary => {
                    if let Some(index) = selected_settings_library {
                        let new_selected_lib = &self.libraries_ids[index];
                        let _ = update_id_selected_lib(new_selected_lib, &self.username);
                        self.library_needs_reload = true;
                    }
                }
                AppView::SettingsAbout => {
                }
                // Unreachable in practice - the Keymap guard earlier in handle_key
                // returns before this match is ever reached while Keymap is active.
                AppView::Keymap => {}
                AppView::Stats => {}
                // Unreachable in practice - PodcastAdd's own early-intercept block in
                // handle_key owns l/Right/Enter itself (Results -> Confirm, or Confirm
                // -> actually creating the subscription), same reasoning as Keymap above.
                AppView::PodcastAdd => {}
                AppView::Library => {
                    if self.is_podcast {
                        if let Some(index) = selected_library {
                            // The background fetch this depends on (bug_id 3f729c,
                            // poll_pod_ep_fetch) is usually done well before a user
                            // reaches here - Home renders in ~2s, and reaching Library
                            // and picking a podcast takes at least that long by hand -
                            // but on the rare chance it isn't yet, stay on this screen
                            // and say so rather than opening an empty episode list.
                            if index >= self.all_ids_pod_ep.len() {
                                let mut stdout = stdout();
                                let _ = pop_message(&mut stdout, 2, "Still loading episode lists, try again in a moment...");
                            } else {
                                self.titles_pod_ep = self.all_titles_pod_ep[index].clone();
                                self.subtitles_pod_ep = self.all_subtitles_pod_ep[index].clone();
                                self.seasons_pod_ep = self.all_seasons_pod_ep[index].clone();
                                self.episodes_pod_ep = self.all_episodes_pod_ep[index].clone();
                                self.authors_pod_ep = self.all_authors_pod_ep[index].clone();
                                self.descs_pod_ep = self.all_descs_pod_ep[index].clone();
                                self.titles_pod = self.all_titles_pod[index].clone();
                                self.durations_pod_ep = self.all_durations_pod_ep[index].clone();
                                self.list_state_pod_ep.select(Some(0));
                                self.view_state = AppView::PodcastEpisode;
                            }
                        }} else {

                            tokio::spawn(async move {
                                let _ = quit_vlc(address_player.as_str(), port.as_str());

                                pkill_vlc();

                                // wait to close/sync the previous session first - see
                                // wait_prev_session_finished for what a false return means
                                if wait_prev_session_finished(username.clone()) {

                                let mut stdout = stdout();
                                let _ = pop_message(&mut stdout, 3, message);

                                handle_l_book(
                                    token.as_ref(),
                                    refresh_token.as_ref(),
                                    ids_library,
                                    selected_library,
                                    port,
                                    address_player,
                                    server_address,
                                    start_vlc_program,
                                    is_cvlc,
                                    is_cvlc_term,
                                    username,
                                ).await;
                                }
                            });
                        }
                }
                AppView::Collections => {
                    if let Some(index) = self.list_state_collections.selected() {
                        self.active_collection = Some(index);
                        self.list_state_library.select(Some(0));
                        self.view_state = AppView::Library;
                    }
                }
                AppView::SearchBook => {
                    if self.is_podcast {
                        self.is_from_search_pod = true;
                        if let Some(index) = selected_search_book {
                            // `all_ids_pod_ep_search`'s length tracks the background
                            // pod_ep fetch (bug_id 3f729c), not the search-results list
                            // `index` is bounded by - see the guard note on
                            // `all_ids_pod_ep_search.get` above.
                            if index >= self.all_ids_pod_ep_search.len() {
                                let mut stdout = stdout();
                                let _ = pop_message(&mut stdout, 2, "Still loading episode lists, try again in a moment...");
                            } else {
                                self.titles_pod_ep_search = self.all_titles_pod_ep_search[index].clone();
                                self.subtitles_pod_ep_search = self.all_subtitles_pod_ep_search[index].clone();
                                self.seasons_pod_ep_search = self.all_seasons_pod_ep_search[index].clone();
                                self.episodes_pod_ep_search = self.all_episodes_pod_ep_search[index].clone();
                                self.authors_pod_ep_search = self.all_authors_pod_ep_search[index].clone();
                                self.descs_pod_ep_search = self.all_descs_pod_ep_search[index].clone();
                                self.titles_pod_search = self.all_titles_pod_search[index].clone();
                                self.durations_pod_ep_search = self.all_durations_pod_ep_search[index].clone();
                                self.list_state_pod_ep.select(Some(0));
                                self.view_state = AppView::PodcastEpisode;
                            }
                        }} else {

                            tokio::spawn(async move {
                                let _ = quit_vlc(address_player.as_str(), port.as_str());

                                pkill_vlc();

                                // wait to close/sync the previous session first - see
                                // wait_prev_session_finished for what a false return means
                                if wait_prev_session_finished(username.clone()) {

                                let mut stdout = stdout();
                                let _ = pop_message(&mut stdout, 3, message);

                                handle_l_book(
                                    token.as_ref(),
                                    refresh_token.as_ref(),
                                    ids_search_book,
                                    selected_search_book,
                                    port,
                                    address_player,
                                    server_address,
                                    start_vlc_program,
                                    is_cvlc,
                                    is_cvlc_term,
                                    username,
                                ).await;
                                }
                            });

                        }
                }
                AppView::PodcastEpisode => {
                    if self.is_from_search_pod {
                        if let Some(index) = selected_search_book
                            && let Some(id_pod) = self.ids_library_pod_search.get(index) {
                                let all_ids_pod_ep_search_clone = self.all_ids_pod_ep_search.clone();
                                let id_pod_clone = id_pod.clone();
                                let selected_pod_ep = self.list_state_pod_ep.selected();

                                tokio::spawn(async move {
                                    let _ = quit_vlc(address_player.as_str(), port.as_str());

                                    pkill_vlc();

                                    // wait to close/sync the previous session first - see
                                    // wait_prev_session_finished for what a false return means
                                    if wait_prev_session_finished(username.clone()) {

                                    let mut stdout = stdout();
                                    let _ = pop_message(&mut stdout, 3, message);

                                    handle_l_pod(
                                        token.as_ref(),
                                        refresh_token.as_ref(),
                                        &all_ids_pod_ep_search_clone[index],
                                        selected_pod_ep,
                                        port,
                                        address_player,
                                        id_pod_clone.as_str(),
                                        server_address,
                                        start_vlc_program,
                                        is_cvlc,
                                        is_cvlc_term,
                                        username,
                                    ).await;
                                    }
                                });
                            }
                    } else {
                        if let Some(index) = selected_library
                            && let Some(id_pod) = ids_library.get(index) {
                                let all_ids_pod_ep_clone = self.all_ids_pod_ep.clone();
                                self.ids_pod_ep = all_ids_pod_ep_clone[index].clone();
                                let id_pod_clone = id_pod.clone();
                                let selected_pod_ep = self.list_state_pod_ep.selected();
                                tokio::spawn(async move {
                                    let _ = quit_vlc(address_player.as_str(), port.as_str());

                                    pkill_vlc();

                                    // wait to close/sync the previous session first - see
                                    // wait_prev_session_finished for what a false return means
                                    if wait_prev_session_finished(username.clone()) {

                                    let mut stdout = stdout();
                                    let _ = pop_message(&mut stdout, 3, message);

                                    handle_l_pod(
                                        token.as_ref(),
                                        refresh_token.as_ref(),
                                        &all_ids_pod_ep_clone[index],
                                        selected_pod_ep,
                                        port,
                                        address_player,
                                        id_pod_clone.as_str(),
                                        server_address,
                                        start_vlc_program,
                                        is_cvlc,
                                        is_cvlc_term,
                                        username,
                                    ).await;
                                    }
                                });
                            }

                    }
                }
            }
        }
        _ => {}
    }
}


/// Cycles Home -> Library -> Collections -> Home. Collections is skipped
/// entirely (Home <-> Library, same 2-state toggle as before it existed) when
/// this library has none.
fn toggle_view(&mut self) {
    self.view_state = match self.view_state {
        AppView::Home => AppView::Library,
        AppView::Library => {
            // Tab means "move to the next ring stop" - any active collection filter
            // shouldn't silently persist once you've left Library. Only `h` is meant
            // to preserve the "return to Collections" path.
            self.active_collection = None;
            if self.collection_names.is_empty() { AppView::Stats } else { AppView::Collections }
        }
        AppView::Collections => AppView::Stats,
        AppView::Stats => AppView::Home,
        AppView::SearchBook => AppView::Home,
        AppView::PodcastEpisode => AppView::Home,
        AppView::Settings => AppView::Home,
        AppView::SettingsAccount => AppView::Home,
        AppView::SettingsLibrary => AppView::Home,
        AppView::SettingsAbout => AppView::Home,
        AppView::SettingsUpdateUninstall => AppView::Home,
        AppView::SettingsAutoplay => AppView::Home,
        AppView::SettingsPerItemSpeed => AppView::Home,
        AppView::SettingsAutoDownload => AppView::Home,
        // Tab deliberately falls through to here from the Keymap guard in
        // handle_key - this is what makes Tab close Keymap back to Home.
        AppView::Keymap => AppView::Home,
        // Unreachable in practice - PodcastAdd's own early-intercept block in
        // handle_key owns every key while active, Tab included, so this never runs.
        // Falls back to Library (not Home) since that's the one place this flow is
        // ever entered from and ever returns to.
        AppView::PodcastAdd => AppView::Library,

    };

    // Stats reuses `scroll_offset` for its own whole-page scroll (see render_stats),
    // the same field Description-panel scrolling (J/K/H) uses elsewhere - without this,
    // arriving here with a leftover Description scroll position renders Stats already
    // scrolled down for no visible reason. Same reset `?`/Keymap already does on entry.
    if self.view_state == AppView::Stats {
        self.scroll_offset = 0;
    }
}

/// Flattens the Continue Listening list into individual rows, splicing indented chapter
/// rows in directly beneath the currently-playing book's row when `is_chapter_list_expanded`
/// is set. Returns plain `Book` rows 1:1 with `_ids_cnt_list` for podcasts, or whenever
/// nothing is expanded - so callers never need to special-case those situations themselves.
pub fn build_home_rows(&self) -> Vec<HomeRow> {
    if self.is_podcast || !self.is_chapter_list_expanded {
        return (0..self._ids_cnt_list.len()).map(HomeRow::Book).collect();
    }

    let active_session = get_listening_session().ok().flatten();
    let chapters: Vec<Chapter> = active_session.as_ref()
        .map(|s| serde_json::from_str(&s.chapters).unwrap_or_default())
        .unwrap_or_default();

    let mut rows = Vec::new();
    for i in 0..self._ids_cnt_list.len() {
        rows.push(HomeRow::Book(i));

        let is_now_playing = active_session.as_ref()
            .is_some_and(|s| self._ids_cnt_list.get(i) == Some(&s.id_item));
        if is_now_playing {
            for chapter in &chapters {
                rows.push(HomeRow::Chapter { book_index: i, chapter: chapter.clone() });
            }
        }
    }

    rows
}

/// Flattens the Library list into rows, splicing in a `SeriesHeader` row above each
/// series' books (sorted by sequence number) when `is_library_grouped_by_series` is
/// set, and scoping to `active_collection`'s books first when one is filtering
/// Library. Returns plain `Book` rows 1:1 with the in-scope indices otherwise - so
/// callers never need to special-case those situations themselves.
pub fn build_library_rows(&self) -> Vec<LibraryRow> {
    let indices: Vec<usize> = match self.active_collection {
        Some(index) => self.collection_book_indices[index].clone(),
        None => (0..self.titles_library.len()).collect(),
    };

    if !self.is_library_grouped_by_series {
        return indices.into_iter().map(LibraryRow::Book).collect();
    }

    Self::group_library_rows(indices, &self.series_name_library, &self.series_sequence_library)
}

/// Groups `indices` into series-headed rows, sorted alphabetically by series name
/// with each group ordered by sequence number (missing/unparsable sorts last);
/// books with no series are appended last, in their existing order. Free function
/// (not a method) so it's unit-testable without constructing a full `App`.
fn group_library_rows(indices: Vec<usize>, series_names: &[Option<String>], series_sequences: &[Option<f64>]) -> Vec<LibraryRow> {
    let mut series_groups: std::collections::BTreeMap<String, Vec<usize>> = std::collections::BTreeMap::new();
    let mut standalone = Vec::new();
    for i in indices {
        match series_names.get(i).cloned().flatten() {
            Some(name) => series_groups.entry(name).or_default().push(i),
            None => standalone.push(i),
        }
    }

    let mut rows = Vec::new();
    for (name, mut members) in series_groups {
        members.sort_by(|&a, &b| {
            let seq_a = series_sequences.get(a).copied().flatten().unwrap_or(f64::MAX);
            let seq_b = series_sequences.get(b).copied().flatten().unwrap_or(f64::MAX);
            seq_a.total_cmp(&seq_b)
        });
        rows.push(LibraryRow::SeriesHeader(name));
        rows.extend(members.into_iter().map(LibraryRow::Book));
    }
    rows.extend(standalone.into_iter().map(LibraryRow::Book));
    rows
}

/// Resolves a raw `list_state_library` selection (a position in `build_library_rows`'
/// row space) back to the original book index it refers to - `None` for a header row
/// or nothing selected. Shared by rendering (Info/Description panels) and the Enter
/// dispatch, so both agree on which row a given selection actually means.
pub fn selected_library_book_index(&self) -> Option<usize> {
    Self::resolve_library_book_index(self.list_state_library.selected(), &self.build_library_rows())
}

/// Same resolution as `selected_library_book_index`, but against already-computed
/// rows - `render_library` already has to build them for display, and series
/// grouping's grouping+sort pass isn't cheap enough to redo a second time per frame
/// just to resolve the selection too.
pub fn resolve_library_book_index(selected: Option<usize>, rows: &[LibraryRow]) -> Option<usize> {
    match rows.get(selected?)? {
        LibraryRow::Book(i) => Some(*i),
        LibraryRow::SeriesHeader(_) => None,
    }
}

/// Select functions that apply to both views
/// all select functions are from `ListState` widget
pub fn select_next(&mut self) {
    match self.view_state {
        AppView::Home => { if let Some(selected) = self.list_state_cnt_list.selected() {
            if selected + 1  < self.build_home_rows().len() {
                self.list_state_cnt_list.select_next();
            } else {
                self.list_state_cnt_list.select_first();
            }}}
        AppView::Library => { if let Some(selected) = self.list_state_library.selected() {
            if selected + 1  < self.build_library_rows().len() {
                self.list_state_library.select_next();
            } else {
                self.list_state_library.select_first();
            }}}
        AppView::Collections => { if let Some(selected) = self.list_state_collections.selected() {
            if selected + 1  < self.collection_names.len() {
                self.list_state_collections.select_next();
            } else {
                self.list_state_collections.select_first();
            }}}
        AppView::SearchBook => { if let Some(selected) = self.list_state_search_results.selected() {
            if selected + 1  < self.ids_search_book.len() {
                self.list_state_search_results.select_next();
            } else {
                self.list_state_search_results.select_first();
            }}}
        AppView::PodcastEpisode => { if let Some(selected) = self.list_state_pod_ep.selected() {
            if self.is_from_search_pod {
                if selected + 1  < self.ids_pod_ep_search.len() {
                    self.list_state_pod_ep.select_next();
                } else {
                    self.list_state_pod_ep.select_first();
                }
            } else {
                if selected + 1  < self.ids_pod_ep.len() {
                    self.list_state_pod_ep.select_next();
                } else {
                    self.list_state_pod_ep.select_first();
                }}}}
        AppView::Settings => { if let Some(selected) = self.list_state_settings.selected() {
            if selected + 1  < self.settings.len() {
                self.list_state_settings.select_next();
            } else {
                self.list_state_settings.select_first();
            }}}
        AppView::SettingsAccount => { if let Some(selected) = self.list_state_settings_account.selected() {
            if selected + 1  < self.all_usernames.len() {
                self.list_state_settings_account.select_next();
            } else {
                self.list_state_settings_account.select_first();
            }}}
        AppView::SettingsLibrary => { if let Some(selected) = self.list_state_settings_library.selected() {
            if selected + 1  < self.media_types.len() {
                self.list_state_settings_library.select_next();
            } else {
                self.list_state_settings_library.select_first();
            }}}
        AppView::SettingsAbout => self.list_state_settings_about.select_next(),
        AppView::SettingsUpdateUninstall => self.list_state_settings_update_uninstall.select_next(),
        AppView::SettingsAutoplay => { if let Some(selected) = self.list_state_settings_autoplay.selected() {
            if selected + 1 < 2 {
                self.list_state_settings_autoplay.select_next();
            } else {
                self.list_state_settings_autoplay.select_first();
            }}}
        AppView::SettingsPerItemSpeed => { if let Some(selected) = self.list_state_settings_per_item_speed.selected() {
            if selected + 1 < 2 {
                self.list_state_settings_per_item_speed.select_next();
            } else {
                self.list_state_settings_per_item_speed.select_first();
            }}}
        AppView::SettingsAutoDownload => { if let Some(selected) = self.list_state_settings_auto_download.selected() {
            if selected + 1 < 2 {
                self.list_state_settings_auto_download.select_next();
            } else {
                self.list_state_settings_auto_download.select_first();
            }}}
        // Unreachable - j/Down never reach this while Keymap is active.
        AppView::Keymap => {}
        AppView::Stats => {}
        AppView::PodcastAdd => {
            if let Some(PodcastAddStage::Results(results)) = &self.podcast_add_stage
                && let Some(selected) = self.list_state_podcast_search_results.selected() {
                    if selected + 1 < results.len() {
                        self.list_state_podcast_search_results.select_next();
                    } else {
                        self.list_state_podcast_search_results.select_first();
                    }
            }
        }
    }
}

pub fn select_previous(&mut self) {
    match self.view_state {
        AppView::Home => self.list_state_cnt_list.select_previous(),
        AppView::Library => self.list_state_library.select_previous(),
        AppView::Collections => self.list_state_collections.select_previous(),
        AppView::SearchBook => self.list_state_search_results.select_previous(),
        AppView::PodcastEpisode => self.list_state_pod_ep.select_previous(),
        AppView::Settings => self.list_state_settings.select_previous(),
        AppView::SettingsAccount => self.list_state_settings_account.select_previous(),
        AppView::SettingsLibrary => self.list_state_settings_library.select_previous(),
        AppView::SettingsAbout => self.list_state_settings_about.select_previous(),
        AppView::SettingsUpdateUninstall => self.list_state_settings_update_uninstall.select_previous(),
        AppView::SettingsAutoplay => self.list_state_settings_autoplay.select_previous(),
        AppView::SettingsPerItemSpeed => self.list_state_settings_per_item_speed.select_previous(),
        AppView::SettingsAutoDownload => self.list_state_settings_auto_download.select_previous(),
        AppView::Keymap => {}
        AppView::Stats => {}
        AppView::PodcastAdd => self.list_state_podcast_search_results.select_previous(),
    }
}

pub fn select_first(&mut self) {
    match self.view_state {
        AppView::Home => self.list_state_cnt_list.select_first(),
        AppView::Library => self.list_state_library.select_first(),
        AppView::Collections => self.list_state_collections.select_first(),
        AppView::SearchBook => self.list_state_search_results.select_first(),
        AppView::PodcastEpisode => self.list_state_pod_ep.select_first(),
        AppView::Settings => self.list_state_settings.select_first(),
        AppView::SettingsAccount => self.list_state_settings_account.select_first(),
        AppView::SettingsLibrary => self.list_state_settings_library.select_first(),
        AppView::SettingsAbout => self.list_state_settings_about.select_first(),
        AppView::SettingsUpdateUninstall => self.list_state_settings_update_uninstall.select_first(),
        AppView::SettingsAutoplay => self.list_state_settings_autoplay.select_first(),
        AppView::SettingsPerItemSpeed => self.list_state_settings_per_item_speed.select_first(),
        AppView::SettingsAutoDownload => self.list_state_settings_auto_download.select_first(),
        AppView::Keymap => {}
        AppView::Stats => {}
        AppView::PodcastAdd => self.list_state_podcast_search_results.select_first(),
    }
}

pub fn select_last(&mut self) {
    match self.view_state {
        AppView::Home => {
            self.list_state_cnt_list.select(self.build_home_rows().len().checked_sub(1));
        }
        AppView::Library => {
            self.list_state_library.select(self.build_library_rows().len().checked_sub(1));
        }
        AppView::Collections => {
            self.list_state_collections.select(self.collection_names.len().checked_sub(1));
        }
        AppView::SearchBook => {
            self.list_state_search_results.select(self.ids_search_book.len().checked_sub(1));
        }
        AppView::PodcastEpisode => {
            if self.is_from_search_pod {
                self.list_state_pod_ep.select(self.ids_pod_ep_search.len().checked_sub(1));
            } else {
                self.list_state_pod_ep.select(self.ids_pod_ep.len().checked_sub(1));
            }}
        AppView::Settings => {
            self.list_state_settings.select(self.settings.len().checked_sub(1));
        }
        AppView::SettingsAccount => self.list_state_settings_account.select_last(),
        AppView::SettingsLibrary => {
            self.list_state_settings_library.select(self.media_types.len().checked_sub(1));
        }
        AppView::SettingsAbout => self.list_state_settings_about.select_last(),
        AppView::SettingsUpdateUninstall => self.list_state_settings_update_uninstall.select_last(),
        AppView::SettingsAutoplay => self.list_state_settings_autoplay.select(Some(1)),
        AppView::SettingsPerItemSpeed => self.list_state_settings_per_item_speed.select(Some(1)),
        AppView::SettingsAutoDownload => self.list_state_settings_auto_download.select(Some(1)),
        AppView::Keymap => {}
        AppView::Stats => {}
        AppView::PodcastAdd => {
            if let Some(PodcastAddStage::Results(results)) = &self.podcast_add_stage {
                self.list_state_podcast_search_results.select(results.len().checked_sub(1));
            }
        }
    }
}

// Whether some part of the app currently owns free-text keyboard input - the `/`
// search box, the podcast-add Input stage, or Update/Uninstall's password prompt.
// main.rs's own top-level `R` (refresh) handling runs completely independently of
// `handle_key` (it checks the same raw KeyEvent again afterward, regardless of what
// handle_key already did with it) - without this guard, typing a capital R into any
// of these free-text fields triggers a full app reload mid-keystroke, discarding
// whatever was being typed. Real, pre-existing gap for search/password too, not
// just the podcast-add flow that surfaced it.
pub fn is_capturing_free_text(&self) -> bool {
    self.is_search_active
        || matches!(self.podcast_add_stage, Some(PodcastAddStage::Input))
        || (matches!(self.view_state, AppView::SettingsUpdateUninstall) && matches!(self.update_uninstall_stage, UpdateUninstallStage::Password(_)))
}

// Non-blocking check for the background search_podcast/get_podcast_feed call (from
// PodcastAddStage::Input) or the create_podcast+check_new_episodes pair (from
// PodcastAddStage::Confirm) - both go through the same receiver field, one at a
// time, since the flow is strictly sequential. Called from main.rs's render loop
// every iteration, same shape as poll_pod_ep_fetch below.
pub fn poll_podcast_add_result(&mut self) {
    let Some(rx) = self.podcast_add_receiver.as_mut() else { return };
    match rx.try_recv() {
        Ok(PodcastAddOutcome::Found(results)) => {
            self.podcast_add_receiver = None;
            if results.is_empty() {
                self.podcast_add_error = Some("No podcasts found - try a different search, or paste a direct RSS feed URL".to_string());
                self.podcast_add_stage = Some(PodcastAddStage::Input);
            } else {
                self.list_state_podcast_search_results.select(Some(0));
                self.podcast_search_results_cache = results.clone();
                self.podcast_add_stage = Some(PodcastAddStage::Results(results));
            }
        }
        Ok(PodcastAddOutcome::ConfirmReady(chosen)) => {
            self.podcast_add_receiver = None;
            self.podcast_add_stage = Some(PodcastAddStage::Confirm { chosen, episode_count: 5 });
        }
        Ok(PodcastAddOutcome::Created(episode_count)) => {
            self.podcast_add_receiver = None;
            self.podcast_add_stage = None;
            log::info!("[poll_podcast_add_result] subscription created, {episode_count} episode(s) downloaded");
            self.view_state = AppView::Library;
            self.library_needs_reload = true;
        }
        Ok(PodcastAddOutcome::Failed(message)) => {
            self.podcast_add_receiver = None;
            self.podcast_add_error = Some(message);
            self.podcast_add_stage = Some(PodcastAddStage::Input);
        }
        Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
        Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
            // Sender dropped without sending - shouldn't happen (the spawned task
            // always sends before finishing) but stop polling a receiver that can
            // never resolve rather than checking it forever.
            self.podcast_add_receiver = None;
            self.podcast_add_error = Some("Something went wrong - please try again".to_string());
            self.podcast_add_stage = Some(PodcastAddStage::Input);
        }
    }
}

// Non-blocking check for the background delete_library_item call spawned from the
// podcast_remove_confirm handler - only sets library_needs_reload once the delete
// has actually finished (see podcast_remove_receiver's own doc comment for why
// that ordering matters). Called from main.rs's render loop every iteration.
pub fn poll_podcast_remove_result(&mut self) {
    let Some(rx) = self.podcast_remove_receiver.as_mut() else { return };
    match rx.try_recv() {
        Ok(()) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
            self.podcast_remove_receiver = None;
            self.library_needs_reload = true;
        }
        Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
    }
}

// Non-blocking drain of one pending update/uninstall progress event, if any - called
// from main.rs's render loop every iteration so the log panel stays live without a
// dedicated blocking sub-loop.
pub fn poll_update_uninstall_event(&mut self) -> Option<ProgressEvent> {
    self.update_uninstall_receiver.as_mut()?.try_recv().ok()
}

// Non-blocking check for the background podcast-episode-list fetch spawned in
// App::new() (see bug_id 3f729c) - called from main.rs's render loop every iteration,
// same as poll_update_uninstall_event above. Merges the batch into self the moment
// it's ready; a no-op while still in flight, and a permanent no-op once already
// consumed (pod_ep_receiver is None either way).
pub fn poll_pod_ep_fetch(&mut self) {
    let Some(rx) = self.pod_ep_receiver.as_mut() else { return };
    match rx.try_recv() {
        Ok(batch) => {
            self.all_titles_pod_ep = batch.titles;
            self.all_ids_pod_ep = batch.ids;
            self.all_subtitles_pod_ep = batch.subtitles;
            self.all_seasons_pod_ep = batch.seasons;
            self.all_episodes_pod_ep = batch.episodes;
            self.all_authors_pod_ep = batch.authors;
            self.all_descs_pod_ep = batch.descs;
            self.all_titles_pod = batch.titles_pod;
            self.all_durations_pod_ep = batch.durations;
            self.pod_ep_receiver = None;
        }
        Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
        Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
            // Sender dropped without sending - shouldn't happen (the spawned task
            // always sends before finishing) but stop polling a receiver that can
            // never resolve rather than checking it forever.
            self.pod_ep_receiver = None;
        }
    }
}

// Built lazily on ProgressEvent::NeedPassword rather than up front on confirm, since
// we don't know a password will even be asked for until sudo's own conversation (run
// over a real pty - see update_uninstall::negotiate) actually reaches that prompt,
// eg. after a fingerprint attempt falls through.
pub fn new_password_field(&self) -> TextArea<'static> {
    let mut password_field = TextArea::default();
    password_field.set_mask_char('\u{2022}');
    password_field.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title("Password")
            // ACCENT_KEY (yellow), matching the Update/Uninstall flow's Confirm/Working
            // stages and the search box - reached by taking an action, not a permanent
            // structural section of the screen.
            .border_style(Style::new().fg(crate::ui::theme::ACCENT_KEY))
    );
    password_field
}

}

#[cfg(test)]
mod library_row_tests {
    use super::*;

    #[test]
    fn groups_by_series_sorted_by_sequence_with_standalone_books_last() {
        // Indices 0-4 map to: two books in "Zeta" (out of order, one with a
        // fractional sequence), one standalone book, two in "Alpha".
        let series_names = vec![
            Some("Zeta".to_string()),
            None,
            Some("Zeta".to_string()),
            Some("Alpha".to_string()),
            Some("Zeta".to_string()),
        ];
        let series_sequences = vec![Some(2.0), None, Some(1.0), Some(1.0), Some(1.5)];

        let rows = App::group_library_rows((0..5).collect(), &series_names, &series_sequences);

        let row_kinds: Vec<String> = rows.iter().map(|r| match r {
            LibraryRow::SeriesHeader(name) => format!("H:{name}"),
            LibraryRow::Book(i) => format!("B:{i}"),
        }).collect();

        // Alphabetical series order (Alpha before Zeta), each group sequence-sorted
        // (Zeta: index 2 seq 1.0, then 4 seq 1.5, then 0 seq 2.0), standalone (index
        // 1, no series) appended last.
        assert_eq!(row_kinds, vec!["H:Alpha", "B:3", "H:Zeta", "B:2", "B:4", "B:0", "B:1"]);
    }

    #[test]
    fn missing_sequence_sorts_last_within_its_group() {
        let series_names = vec![Some("Solo".to_string()), Some("Solo".to_string())];
        let series_sequences = vec![None, Some(1.0)];

        let rows = App::group_library_rows(vec![0, 1], &series_names, &series_sequences);

        let row_kinds: Vec<String> = rows.iter().map(|r| match r {
            LibraryRow::SeriesHeader(name) => format!("H:{name}"),
            LibraryRow::Book(i) => format!("B:{i}"),
        }).collect();
        assert_eq!(row_kinds, vec!["H:Solo", "B:1", "B:0"]);
    }
}
