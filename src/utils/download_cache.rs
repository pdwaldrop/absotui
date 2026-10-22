use std::collections::{HashMap, HashSet};
use std::env;
use std::future::Future;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use color_eyre::eyre::{bail, Result};
use log::{info, error};
use reqwest::header::{AUTHORIZATION, CONTENT_RANGE, CONTENT_TYPE, RANGE};
use reqwest::{Client, Response, StatusCode};
use tokio::io::AsyncWriteExt;
use crate::api::library_items::play_lib_item_or_pod::{post_start_playback_session_book, post_start_playback_session_pod, AudioTrack};
use crate::api::sessions::close_open_session::close_session_without_send_prg_data;
use crate::db::crud::{insert_download, delete_download, get_download, list_downloaded_ids};
use crate::db::database_struct::DownloadedTrack;
use crate::utils::http_client::download_client;

fn downloads_dir() -> PathBuf {
    let config_home_path = env::var("XDG_CONFIG_HOME").map_or_else(|_| {
            let mut path = dirs::home_dir().expect("Unable to find the user's home directory");

            if cfg!(target_os = "macos") {
                path.push("Library/Preferences");
            } else {
                path.push(".config");
            }

            path
        }, PathBuf::from);

    config_home_path.join("absotui/downloads")
}

/// Same path-traversal reasoning as `cover_cache::cover_cache_path` - `item_id` comes
/// straight from the configured server, unvalidated, so both it and `ext` (also
/// server-derived, via the response's `Content-Type`) are stripped down to characters
/// that can never form `/`, `\`, or `..` before being used to build a path.
pub fn download_audio_path(item_id: &str, ext: &str) -> PathBuf {
    let safe_id: String = item_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let safe_ext: String = ext.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    downloads_dir().join(format!("{safe_id}.{safe_ext}"))
}

/// Real Audiobookshelf audio files are one of a handful of known containers - anything
/// else (or a missing/malformed header) falls back to `m4b`, the most common audiobook
/// format, rather than leaving the file extensionless (VLC's local-file demuxer
/// selection leans on the extension more than it does for network streams).
fn extension_from_content_type(content_type: Option<&str>) -> &'static str {
    match content_type.unwrap_or_default() {
        "audio/mp4" | "audio/x-m4b" | "audio/x-m4a" => "m4b",
        "audio/mpeg" => "mp3",
        "audio/ogg" => "ogg",
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/aac" => "aac",
        "audio/wav" | "audio/x-wav" => "wav",
        _ => "m4b",
    }
}

pub fn is_downloaded(username: &str, item_id: &str) -> bool {
    get_download(username, item_id)
        .is_some_and(|d| !d.tracks.is_empty() && d.tracks.iter().all(|t| std::path::Path::new(&t.local_path).exists()))
}

const DOWNLOAD_MEDIA_PLAYER: &str = "Absotui download";

// A failed auto-download isn't retried for this long, so a broken file doesn't get
// re-requested on every podcast refresh.
const RETRY_AFTER: Duration = Duration::from_secs(120);
const STALE_PARTIAL_AGE: Duration = Duration::from_secs(3 * 24 * 60 * 60);

static IN_FLIGHT: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);
static FAILED: LazyLock<Mutex<HashMap<String, Instant>>> = LazyLock::new(Default::default);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

fn item_key(username: &str, item_id: &str) -> String {
    format!("{username}\0{item_id}")
}

// Held for the duration of one item's download so two callers (auto sync and the `d`
// key, or overlapping syncs) never fetch the same item at once.
struct InFlight(String);

impl InFlight {
    fn acquire(username: &str, item_id: &str) -> Option<Self> {
        let key = item_key(username, item_id);
        let newly_added = lock(&IN_FLIGHT).insert(key.clone());
        newly_added.then(|| Self(key))
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        lock(&IN_FLIGHT).remove(&self.0);
    }
}

fn recently_failed(username: &str, item_id: &str) -> bool {
    lock(&FAILED).get(&item_key(username, item_id)).is_some_and(|at| at.elapsed() < RETRY_AFTER)
}

async fn run_auto(label: &str, username: &str, item_id: &str, download: impl Future<Output = Result<()>>) {
    let key = item_key(username, item_id);
    match download.await {
        Ok(()) => {
            lock(&FAILED).remove(&key);
        }
        Err(e) => {
            error!("[{label}] {item_id}: {e}");
            lock(&FAILED).insert(key, Instant::now());
        }
    }
}

fn parse_content_range_start(value: &str) -> Option<u64> {
    value.strip_prefix("bytes ")?.split('-').next()?.trim().parse().ok()
}

async fn send_ranged(client: &Client, url: &str, token: &str, offset: u64) -> Result<Response> {
    let mut request = client.get(url).header(AUTHORIZATION, format!("Bearer {token}"));
    if offset > 0 {
        request = request.header(RANGE, format!("bytes={offset}-"));
    }
    Ok(request.send().await?)
}

/// Streams `url` into `{stem}.part` and renames it into place once complete, so memory
/// stays flat and a half-written file never looks downloaded. A `.part` left by an
/// earlier failed attempt is resumed with a Range request when the server honors it.
async fn fetch_audio(client: &Client, url: &str, token: &str, stem: &str) -> Result<(PathBuf, u64)> {
    std::fs::create_dir_all(downloads_dir())?;
    let part = download_audio_path(stem, "part");
    let mut offset = std::fs::metadata(&part).map_or(0, |m| m.len());
    let mut response = send_ranged(client, url, token, offset).await?;

    if offset > 0 {
        let usable = match response.status() {
            StatusCode::OK => {
                offset = 0;
                true
            }
            StatusCode::PARTIAL_CONTENT => response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .and_then(parse_content_range_start)
                == Some(offset),
            StatusCode::RANGE_NOT_SATISFIABLE => false,
            _ => true,
        };
        if !usable {
            let _ = std::fs::remove_file(&part);
            offset = 0;
            response = send_ranged(client, url, token, 0).await?;
        }
    }
    let mut response = response.error_for_status()?;

    let ext = extension_from_content_type(response.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()));
    let expected = response.content_length().map(|remaining| remaining + offset);
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(offset > 0)
        .truncate(offset == 0)
        .open(&part)
        .await?;

    let mut written = offset;
    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk).await?;
        written += chunk.len() as u64;
    }
    file.flush().await?;
    drop(file);

    if let Some(expected) = expected
        && expected != written {
        bail!("incomplete download: got {written} of {expected} bytes");
    }

    let path = download_audio_path(stem, ext);
    std::fs::rename(&part, &path)?;
    Ok((path, written))
}

fn remove_stale_partials() {
    let Ok(entries) = std::fs::read_dir(downloads_dir()) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_stale = path.extension().is_some_and(|e| e == "part")
            && entry.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > STALE_PARTIAL_AGE);
        if is_stale {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Downloads every audio file backing a book for offline playback, if not already
/// downloaded - a book uploaded as separate per-chapter files gets one local file per
/// server-side track, not just the first. Meant to be run in a background task, same
/// as `cover_cache::fetch_and_cache_cover`.
///
/// There's no lighter-weight endpoint to resolve a book's direct-play URL(s) than
/// actually starting a playback session (that's how `handle_l_book` itself gets them),
/// so this opens one purely to read `audioTracks`/`duration`/`chapters` off the
/// response and immediately closes it again - the server never sees a real listen out
/// of this.
pub async fn download_book(token: String, item_id: String, title: String, author: String, username: String, server_address: String) -> Result<()> {
    if is_downloaded(&username, &item_id) {
        return Ok(());
    }
    let Some(_in_flight) = InFlight::acquire(&username, &item_id) else {
        return Ok(());
    };

    let info_item = post_start_playback_session_book(DOWNLOAD_MEDIA_PLAYER, Some(&token), &item_id, server_address.clone()).await?;
    // info_item: [current_time, content_url, duration, id_session, title, subtitle, author, chapters_json, tracks_json]
    let duration = &info_item[2];
    let id_session = &info_item[3];
    let chapters_json = &info_item[7];
    let tracks: Vec<AudioTrack> = serde_json::from_str(&info_item[8]).unwrap_or_default();

    if let Err(e) = close_session_without_send_prg_data(Some(&token), id_session, server_address.clone()).await {
        error!("[download_book] failed to close transient session for {item_id}: {e}");
    }

    let client = download_client();

    // One at a time, same as every other download in this file - a multi-hundred-MB
    // audiobook already saturates the connection without also parallelizing per-file.
    let mut downloaded_tracks = Vec::with_capacity(tracks.len());
    let mut total_bytes: u64 = 0;
    for (idx, track) in tracks.iter().enumerate() {
        // Per-track filename (item_id_index) - a single-file book still gets exactly
        // one file, named "{item_id}_0".
        let (path, bytes) = fetch_audio(&client, &format!("{server_address}{}", track.content_url), &token, &format!("{item_id}_{idx}")).await?;
        total_bytes += bytes;

        downloaded_tracks.push(DownloadedTrack {
            local_path: path.to_string_lossy().into_owned(),
            duration: track.duration,
            start_offset: track.start_offset,
        });
    }

    // Legacy `file_path` column - kept pointing at track 0's local file so any other
    // code still reading it directly degrades gracefully; `tracks` is authoritative.
    let file_path = downloaded_tracks.first().map(|t| t.local_path.clone()).unwrap_or_default();
    let tracks_json = serde_json::to_string(&downloaded_tracks).unwrap_or_default();

    insert_download(&username, &item_id, &file_path, duration, &title, &author, "book", &tracks_json, chapters_json)?;
    info!("[download_book] downloaded {item_id} ({} file(s), {total_bytes} bytes)", downloaded_tracks.len());

    Ok(())
}

/// Downloads a podcast episode's audio file for offline playback, if not already
/// downloaded - same approach as `download_book`, but a podcast episode's play-session
/// endpoint needs both the parent podcast's id and the episode's own id (see
/// `post_start_playback_session_pod`), and it's the episode id that's used as the
/// dedupe/storage key (`item_id` below) - it's the one that's actually unique per row.
pub async fn download_episode(token: String, podcast_id: String, episode_id: String, title: String, podcast_title: String, username: String, server_address: String) -> Result<()> {
    if is_downloaded(&username, &episode_id) {
        return Ok(());
    }
    let Some(_in_flight) = InFlight::acquire(&username, &episode_id) else {
        return Ok(());
    };

    let info_item = post_start_playback_session_pod(DOWNLOAD_MEDIA_PLAYER, Some(&token), &podcast_id, &episode_id, server_address.clone()).await?;
    // info_item: [current_time, content_url, duration, id_session, podcast_title, episode_title, author]
    let content_url = &info_item[1];
    let duration = &info_item[2];
    let id_session = &info_item[3];

    if let Err(e) = close_session_without_send_prg_data(Some(&token), id_session, server_address.clone()).await {
        error!("[download_episode] failed to close transient session for {episode_id}: {e}");
    }

    let client = download_client();
    let (path, bytes) = fetch_audio(&client, &format!("{server_address}{content_url}"), &token, &episode_id).await?;

    insert_download(&username, &episode_id, &path.to_string_lossy(), duration, &title, &podcast_title, "podcast", "", "")?;
    info!("[download_episode] downloaded {episode_id} ({bytes} bytes) to {path:?}");

    Ok(())
}

/// Removes a book's or episode's local download and its db row, if any.
pub fn remove_download(username: &str, item_id: &str) -> Result<()> {
    if let Some(downloaded) = get_download(username, item_id) {
        // `file_path` is a legacy pointer at track 0's file only (see `download_book`'s
        // comment) - removing just that one left every other track of a multi-file
        // book (audiobooks split across several files, per-chapter or otherwise)
        // orphaned on disk with no db row left to ever clean them up (confirmed live:
        // a 9-track/318MB book's download row was deleted and the UI's "downloaded"
        // marker cleared, but only `file_path` - never any of the other 8 files -
        // was ever a remove_file target, so all 318MB silently stayed on disk).
        // `tracks` is empty for a podcast episode (single file, `file_path` alone is
        // authoritative there), so removing `file_path` too keeps that case working -
        // redundant with `tracks[0].local_path` for a book, which just no-ops the
        // second time via the same `let _ =` used everywhere else in this file.
        let _ = std::fs::remove_file(&downloaded.file_path);
        for track in &downloaded.tracks {
            let _ = std::fs::remove_file(&track.local_path);
        }
    }
    delete_download(username, item_id)?;
    Ok(())
}

/// Settings > Auto Download: keeps the local download set mirroring the `count` most
/// recently played books in Continue Listening (`ids` is already server-ordered
/// most-recent-first) - downloads any of them not already downloaded, and removes any
/// existing download whose id has fallen out of that top-`count` window (finished,
/// pushed out by newer activity, or the window shrank), so disk usage stays bounded
/// rather than growing forever. Runs in the background and is meant to be called once
/// per Continue Listening refresh (see `App::new()`) - downloads happen one at a time
/// rather than in parallel, so this doesn't try to saturate the connection with
/// several hundred-MB-plus files at once.
pub fn sync_auto_downloads(username: String, token: String, server_address: String, ids: Vec<String>, titles: Vec<String>, authors: Vec<String>, count: usize) {
    submit(&BOOK_SYNC, BookSync { username, token, server_address, ids, titles, authors, count }, sync_books);
}

async fn sync_books(request: BookSync) {
    let BookSync { username, token, server_address, ids, titles, authors, count } = request;
    remove_stale_partials();
    let ids: Vec<String> = ids.into_iter().take(count).collect();

    if let Ok(existing) = list_downloaded_ids(&username, "book") {
        for stale_id in existing.into_iter().filter(|id| !ids.contains(id)) {
            if let Err(e) = remove_download(&username, &stale_id) {
                error!("[sync_auto_downloads] failed to prune {stale_id}: {e}");
            }
        }
    }

    for (i, id) in ids.iter().enumerate() {
        if is_downloaded(&username, id) || recently_failed(&username, id) {
            continue;
        }
        let title = titles.get(i).cloned().unwrap_or_default();
        let author = authors.get(i).cloned().unwrap_or_default();
        run_auto("sync_auto_downloads", &username, id, download_book(token.clone(), id.clone(), title, author, username.clone(), server_address.clone())).await;
    }
}

/// Settings > Auto Download for podcasts: keeps the local download set mirroring
/// *every* episode currently in "New & Unfinished" - unlike books, there's no top-N
/// cap, since the New & Unfinished list is already naturally bounded (episodes drop
/// out once finished or no longer new). `episode_ids`/`podcast_ids`/`titles`/
/// `podcast_titles` are parallel arrays (same row = same episode, matching
/// `App`'s `ids_ep_cnt_list`/`_ids_cnt_list`/`_titles_cnt_list`/`titles_pod_cnt_list`).
/// Pruning and downloading both key off episode id, same as `sync_auto_downloads` keys
/// off book id - scoped to `kind = "podcast"` so this can never prune a downloaded book.
pub fn sync_auto_downloads_podcasts(username: String, token: String, server_address: String, podcast_ids: Vec<String>, episode_ids: Vec<String>, titles: Vec<String>, podcast_titles: Vec<String>) {
    submit(&PODCAST_SYNC, PodcastSync { username, token, server_address, podcast_ids, episode_ids, titles, podcast_titles }, sync_podcasts);
}

async fn sync_podcasts(request: PodcastSync) {
    let PodcastSync { username, token, server_address, podcast_ids, episode_ids, titles, podcast_titles } = request;
    remove_stale_partials();

    if let Ok(existing) = list_downloaded_ids(&username, "podcast") {
        for stale_id in existing.into_iter().filter(|id| !episode_ids.contains(id)) {
            if let Err(e) = remove_download(&username, &stale_id) {
                error!("[sync_auto_downloads_podcasts] failed to prune {stale_id}: {e}");
            }
        }
    }

    for (i, episode_id) in episode_ids.iter().enumerate() {
        if is_downloaded(&username, episode_id) || recently_failed(&username, episode_id) {
            continue;
        }
        let Some(podcast_id) = podcast_ids.get(i).cloned() else { continue };
        let title = titles.get(i).cloned().unwrap_or_default();
        let podcast_title = podcast_titles.get(i).cloned().unwrap_or_default();
        run_auto("sync_auto_downloads_podcasts", &username, episode_id, download_episode(token.clone(), podcast_id, episode_id.clone(), title, podcast_title, username.clone(), server_address.clone())).await;
    }
}

struct BookSync {
    username: String,
    token: String,
    server_address: String,
    ids: Vec<String>,
    titles: Vec<String>,
    authors: Vec<String>,
    count: usize,
}

struct PodcastSync {
    username: String,
    token: String,
    server_address: String,
    podcast_ids: Vec<String>,
    episode_ids: Vec<String>,
    titles: Vec<String>,
    podcast_titles: Vec<String>,
}

struct SyncSlot<T> {
    pending: Option<T>,
    running: bool,
}

static BOOK_SYNC: Mutex<SyncSlot<BookSync>> = Mutex::new(SyncSlot { pending: None, running: false });
static PODCAST_SYNC: Mutex<SyncSlot<PodcastSync>> = Mutex::new(SyncSlot { pending: None, running: false });

// Frees the slot if the worker dies mid-pass, so a panic can't stop auto-download for good.
struct Worker<T: 'static> {
    slot: &'static Mutex<SyncSlot<T>>,
    finished: bool,
}

impl<T> Drop for Worker<T> {
    fn drop(&mut self) {
        if !self.finished {
            lock(self.slot).running = false;
        }
    }
}

// One worker per slot, always working off the newest request: a request that arrives
// mid-pass replaces any older waiting one and runs once the pass ends, instead of
// starting a second overlapping pass that downloads the same files again.
fn submit<T, F, Fut>(slot: &'static Mutex<SyncSlot<T>>, request: T, pass: F)
where
    T: Send + 'static,
    F: Fn(T) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send,
{
    {
        let mut state = lock(slot);
        state.pending = Some(request);
        if state.running {
            return;
        }
        state.running = true;
    }
    tokio::spawn(async move {
        let mut worker = Worker { slot, finished: false };
        loop {
            let next = {
                let mut state = lock(slot);
                let next = state.pending.take();
                if next.is_none() {
                    state.running = false;
                    worker.finished = true;
                }
                next
            };
            let Some(request) = next else { return };
            pass(request).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legitimate_uuid_style_id_is_unchanged() {
        let id = "a1b2c3d4-e5f6-4789-a0bc-def012345678";
        assert_eq!(download_audio_path(id, "m4b").file_name().unwrap().to_str().unwrap(), format!("{id}.m4b"));
    }

    #[test]
    fn path_traversal_id_cannot_escape_downloads_dir() {
        let malicious = "../../../../home/user/.ssh/authorized_keys";
        let path = download_audio_path(malicious, "m4b");
        assert!(path.starts_with(downloads_dir()));
        assert_eq!(path.parent().unwrap(), downloads_dir());
    }

    #[test]
    fn absolute_path_id_cannot_escape_downloads_dir() {
        let malicious = "/home/user/.ssh/authorized_keys";
        let path = download_audio_path(malicious, "m4b");
        assert!(path.starts_with(downloads_dir()));
        assert_eq!(path.parent().unwrap(), downloads_dir());
    }

    #[test]
    fn malicious_ext_cannot_introduce_a_path_separator() {
        let path = download_audio_path("real-id", "../../../etc/passwd");
        assert!(path.starts_with(downloads_dir()));
        assert_eq!(path.parent().unwrap(), downloads_dir());
    }

    #[test]
    fn content_range_start_reads_the_first_byte_offset() {
        assert_eq!(parse_content_range_start("bytes 100-199/1000"), Some(100));
        assert_eq!(parse_content_range_start("bytes */1000"), None);
        assert_eq!(parse_content_range_start("items 1-2/3"), None);
    }

    #[test]
    fn same_item_cannot_be_downloaded_twice_at_once() {
        let first = InFlight::acquire("test-inflight-user", "item-a").expect("first acquire");
        assert!(InFlight::acquire("test-inflight-user", "item-a").is_none());
        assert!(InFlight::acquire("test-inflight-user", "item-b").is_some());
        drop(first);
        assert!(InFlight::acquire("test-inflight-user", "item-a").is_some());
    }

    #[test]
    fn failed_item_is_skipped_until_the_cooldown_passes() {
        lock(&FAILED).insert(item_key("test-failed-user", "fresh"), Instant::now());
        lock(&FAILED).insert(item_key("test-failed-user", "old"), Instant::now() - RETRY_AFTER - Duration::from_secs(1));
        assert!(recently_failed("test-failed-user", "fresh"));
        assert!(!recently_failed("test-failed-user", "old"));
        assert!(!recently_failed("test-failed-user", "never-tried"));
    }

    static TEST_SYNC: Mutex<SyncSlot<u32>> = Mutex::new(SyncSlot { pending: None, running: false });

    #[tokio::test]
    async fn requests_during_a_pass_collapse_into_one_follow_up() {
        let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let pass = move |n: u32| {
            let log = log.clone();
            async move {
                tokio::time::sleep(Duration::from_millis(80)).await;
                lock(&log).push(n);
            }
        };

        submit(&TEST_SYNC, 1, pass.clone());
        tokio::time::sleep(Duration::from_millis(20)).await;
        submit(&TEST_SYNC, 2, pass.clone());
        submit(&TEST_SYNC, 3, pass);
        tokio::time::sleep(Duration::from_millis(400)).await;

        assert_eq!(*lock(&seen), vec![1, 3]);
        assert!(!lock(&TEST_SYNC).running);
    }
}
