use rusqlite::{params, Connection, Result};
use crate::db::database_struct::User;
use crate::db::database_struct::ListeningSession;
use crate::db::database_struct::Others;
use crate::db::database_struct::DownloadedItem;
use crate::db::database_struct::DownloadedTrack;
use crate::utils::pop_up_message::pop_message;
use std::io::stdout;
use log::{info, error};
use std::env;
use std::path::PathBuf;

fn default_db_path() -> PathBuf {
    let config_home_path = env::var("XDG_CONFIG_HOME").map_or_else(|_| {
            let mut path = dirs::home_dir().expect("Unable to find the user's home directory");

            if cfg!(target_os = "macos") {
                path.push("Library/Preferences");
            } else {
                path.push(".config");
            }

            path
        }, PathBuf::from);

    config_home_path.join("absotui/db.sqlite3")
}

// Shared bodies for the get/update pairs below - identical apart from which `users`
// column they touch. `column` is always a hardcoded literal passed by a caller in
// this file, never external input, so interpolating it into the SQL string is safe.
fn update_user_text_column(value: &str, username: &str, column: &str, log_tag: &str) -> Result<()> {
    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {
        conn.execute(
            &format!("UPDATE users SET {column} = ?1 WHERE username = ?2"),
            params![value, username],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[{log_tag}] {err_message}");
    }

    Ok(())
}

fn get_user_text_column(username: &str, column: &str) -> String {
    let db_path = default_db_path();

    let conn = match Connection::open(db_path) {
        Ok(c) => c,
        Err(_) => return String::from("Error: unable open database"),
    };

    let mut stmt = match conn.prepare(&format!("SELECT {column} FROM users WHERE username = ?1")) {
        Ok(s) => s,
        Err(_) => return String::from("Error to prepare reqwest"),
    };

    match stmt.query_row(params![username], |row| row.get::<_, String>(0)) {
        Ok(id) => id.clone(),
        Err(_) => String::from("No db found"),
    }
}

pub fn update_is_show_key_bindings(value: &str, username: &str) -> Result<()> {
    update_user_text_column(value, username, "is_show_key_bindings", "update_is_show_key_bindings")
}

pub fn get_is_show_key_bindings(username: &str) -> String {
    get_user_text_column(username, "is_show_key_bindings")
}

pub fn update_is_speed_adjusted_time(value: &str, username: &str) -> Result<()> {
    update_user_text_column(value, username, "is_speed_adjusted_time", "update_is_speed_adjusted_time")
}

pub fn get_is_speed_adjusted_time(username: &str) -> String {
    get_user_text_column(username, "is_speed_adjusted_time")
}

pub fn update_is_podcast_autoplay(value: &str, username: &str) -> Result<()> {
    update_user_text_column(value, username, "is_podcast_autoplay", "update_is_podcast_autoplay")
}

pub fn get_is_podcast_autoplay(username: &str) -> String {
    get_user_text_column(username, "is_podcast_autoplay")
}

pub fn update_is_auto_download(value: &str, username: &str) -> Result<()> {
    update_user_text_column(value, username, "is_auto_download", "update_is_auto_download")
}

pub fn get_is_auto_download(username: &str) -> String {
    get_user_text_column(username, "is_auto_download")
}

pub fn update_is_vlc_running(value: &str, username: &str) -> Result<()> {
    update_user_text_column(value, username, "is_vlc_running", "update_is_vlc_running")
}

pub fn get_is_vlc_running(username: &str) -> String {
    get_user_text_column(username, "is_vlc_running")
}

pub fn update_speed_rate(username: &str, is_speed_rate_up: bool) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {

        // Rounded to 1 decimal on every update - repeated float addition/subtraction of
        // 0.10 drifts (1.3 becomes 1.3000001, etc.), which otherwise shows up raw in the
        // player bar and grows with every press. Rounding here every time keeps the
        // stored value clean instead of just masking the drift at display time.
        if is_speed_rate_up {
        conn.execute(
            "UPDATE users SET speed_rate = ROUND(speed_rate + 0.10, 1) WHERE username = ?1",
            params![username],
        )?;
        } else {
        conn.execute(
            "UPDATE users SET speed_rate = ROUND(speed_rate - 0.10, 1) WHERE username = ?1",
            params![username],
        )?;
        }
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[update_speed_rate] {err_message}");
    }

    Ok(())
}


pub fn get_speed_rate(username: &str) -> String {
    let db_path = default_db_path();

    let conn = match Connection::open(db_path) {
        Ok(c) => c,
        Err(_) => return String::from("Error: unable open database"),
    };

    let mut stmt = match conn.prepare("SELECT speed_rate FROM users WHERE username = ?1") {
        Ok(s) => s,
        Err(_) => return String::from("Error to prepare reqwest"),
    };

    match stmt.query_row(params![username], |row| row.get::<_, f32>(0)) {
        // Formatted to 1 decimal rather than a bare to_string() - values from before the
        // update_speed_rate rounding fix may still be sitting in the db as e.g.
        // 1.3000001, and this way they display clean immediately rather than needing
        // one more O/I press to get rounded back on write.
        Ok(id) => format!("{id:.1}"),
        Err(_) => String::from("No db found"),
    }
}

pub fn update_is_per_item_speed(value: &str, username: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {

        conn.execute(
            "UPDATE users SET is_per_item_speed = ?1 WHERE username = ?2",
            params![value, username],
        )?;

        // Turning this on resets every book/show to a clean 1.0x baseline rather than
        // inheriting whatever the shared speed happened to be - see Settings >
        // Per-Item Speed's description. Clearing existing rows (rather than just
        // changing what a brand-new item seeds from) means re-enabling after a
        // previous on/off cycle also starts fresh, not just items never touched before.
        if value == "1" {
            conn.execute(
                "DELETE FROM item_speed_rate WHERE username = ?1",
                params![username],
            )?;
        }
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[update_is_per_item_speed] {err_message}");
    }

    Ok(())
}

pub fn get_is_per_item_speed(username: &str) -> String {
    let db_path = default_db_path();

    let conn = match Connection::open(db_path) {
        Ok(c) => c,
        Err(_) => return String::from("0"),
    };

    let mut stmt = match conn.prepare("SELECT is_per_item_speed FROM users WHERE username = ?1") {
        Ok(s) => s,
        Err(_) => return String::from("0"),
    };

    stmt.query_row(params![username], |row| row.get::<_, String>(0)).unwrap_or_else(|_| "0".to_string())
}

// Per (user, item) speed rate - see item_speed_rate table, used when Settings >
// Per-Item Speed is on. `id_item` is a book's id, or a podcast show's own id (shared
// across its episodes).
pub fn get_item_speed_rate(username: &str, id_item: &str) -> Option<f32> {
    let db_path = default_db_path();

    let conn = Connection::open(db_path).ok()?;

    let mut stmt = conn.prepare("SELECT speed_rate FROM item_speed_rate WHERE username = ?1 AND id_item = ?2").ok()?;

    stmt.query_row(params![username, id_item], |row| row.get::<_, f32>(0)).ok()
}

// Seeds (or overwrites) the per-item speed rate for (username, id_item) - used the
// first time an item is played with Settings > Per-Item Speed on, starting it from
// whatever the global speed_rate currently is rather than always resetting to 1.0x.
pub fn set_item_speed_rate(username: &str, id_item: &str, value: f32) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {

        conn.execute(
            "INSERT OR REPLACE INTO item_speed_rate (username, id_item, speed_rate) VALUES (?1, ?2, ?3)",
            params![username, id_item, value],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[set_item_speed_rate] {err_message}");
    }

    Ok(())
}

// Adjusts the per-item speed rate up/down by the same step as the global speed_rate -
// see update_speed_rate. Assumes a row already exists (seeded via set_item_speed_rate
// when playback started); a no-op otherwise.
pub fn update_item_speed_rate(username: &str, id_item: &str, is_speed_rate_up: bool) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {

        // Rounded to 1 decimal on every update - see update_speed_rate's comment.
        if is_speed_rate_up {
            conn.execute(
                "UPDATE item_speed_rate SET speed_rate = ROUND(speed_rate + 0.10, 1) WHERE username = ?1 AND id_item = ?2",
                params![username, id_item],
            )?;
        } else {
            conn.execute(
                "UPDATE item_speed_rate SET speed_rate = ROUND(speed_rate - 0.10, 1) WHERE username = ?1 AND id_item = ?2",
                params![username, id_item],
            )?;
        }
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[update_item_speed_rate] {err_message}");
    }

    Ok(())
}

// insert (or overwrite) a downloaded book or podcast episode's local file location(s) and
// offline-playback metadata for (username, id_item). `kind` is "book" or "podcast".
// `tracks_json`/`chapters_json` are pre-serialized (see DownloadedTrack/AudioTrack) -
// empty string for either is fine (a podcast episode has no chapters; `get_download`
// falls back to synthesizing a single track from `file_path`/`duration` when
// `tracks_json` is empty, so already-downloaded single-file books need no re-download).
#[allow(clippy::too_many_arguments)]
pub fn insert_download(username: &str, id_item: &str, file_path: &str, duration: &str, title: &str, author: &str, kind: &str, tracks_json: &str, chapters_json: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {
        conn.execute(
            "INSERT OR REPLACE INTO downloads (username, id_item, file_path, duration, title, author, kind, tracks, chapters) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![username, id_item, file_path, duration, title, author, kind, tracks_json, chapters_json],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[insert_download] {err_message}");
    }

    Ok(())
}

// get a downloaded book's local file location(s) and offline-playback metadata, if it's
// been downloaded for (username, id_item)
pub fn get_download(username: &str, id_item: &str) -> Option<DownloadedItem> {

    let db_path = default_db_path();

    let conn = Connection::open(db_path).ok()?;

    let mut stmt = conn.prepare("SELECT file_path, duration, title, author, tracks, chapters FROM downloads WHERE username = ?1 AND id_item = ?2").ok()?;

    stmt.query_row(params![username, id_item], |row| {
        let file_path: String = row.get(0)?;
        let duration: String = row.get(1)?;
        let tracks_json: String = row.get(4)?;
        let chapters: String = row.get(5)?;

        // Rows written before `tracks` existed (or a podcast episode, which never
        // gets one) have it empty - synthesize the single-file case from the legacy
        // file_path/duration columns rather than requiring a re-download.
        let tracks: Vec<DownloadedTrack> = if tracks_json.is_empty() {
            vec![DownloadedTrack {
                local_path: file_path.clone(),
                duration: duration.parse::<f64>().unwrap_or(0.0),
                start_offset: 0.0,
            }]
        } else {
            serde_json::from_str(&tracks_json).unwrap_or_default()
        };

        Ok(DownloadedItem {
            file_path,
            duration,
            title: row.get(2)?,
            author: row.get(3)?,
            tracks,
            chapters,
        })
    }).ok()
}

// remove a book's download row (the local file itself is removed by the caller -
// src/utils/download_cache.rs)
pub fn delete_download(username: &str, id_item: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {
        conn.execute(
            "DELETE FROM downloads WHERE username = ?1 AND id_item = ?2",
            params![username, id_item],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[delete_download] {err_message}");
    }

    Ok(())
}

// get every id_item currently downloaded for a user of a given kind ("book" or
// "podcast") - used to prune downloads that have fallen out of Continue Listening /
// New & Unfinished when Settings > Auto Download is on. Scoped to `kind` so pruning
// one media type's stale downloads never touches the other's.
pub fn list_downloaded_ids(username: &str, kind: &str) -> Result<Vec<String>> {

    let db_path = default_db_path();

    let conn = Connection::open(db_path)?;

    let mut stmt = conn.prepare("SELECT id_item FROM downloads WHERE username = ?1 AND kind = ?2")?;
    let rows = stmt.query_map(params![username, kind], |row| row.get::<_, String>(0))?;

    let mut ids = Vec::new();
    for row in rows {
        ids.push(row?);
    }
    Ok(ids)
}

pub fn get_listening_session() -> Result<Option<ListeningSession>> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {
        // Called ~5x/sec from the render loop (via player_info -> render_player) while
        // the detached playback task writes to this same row roughly every second from
        // its own connection - without a busy_timeout, a collision surfaces as an
        // immediate SQLITE_BUSY Err instead of a short, self-resolving wait, which
        // player_tui.rs then has no valid session data to render.
        let _ = conn.busy_timeout(std::time::Duration::from_millis(500));
        let mut stmt = conn.prepare(
            "SELECT id_session, id_item, current_time_playback, duration, is_finished, id_pod, elapsed_time, title, author, is_playback, chapter, chapters, volume, pending_seek
             FROM listening_session
             LIMIT 1",
        )?;

        let mut rows = stmt.query(params![])?;

        if let Some(row) = rows.next()? {
            let session = ListeningSession {
                id_session: row.get(0)?,
                id_item: row.get(1)?,
                current_time: row.get(2)?,
                duration: row.get(3)?,
                is_finished: row.get(4)?,
                id_pod: row.get(5)?,
                elapsed_time: row.get(6)?,
                title: row.get(7)?,
                author: row.get(8)?,
                is_playback: row.get(9)?,
                chapter: row.get(10)?,
                chapters: row.get(11)?,
                volume: row.get(12)?,
                pending_seek: row.get(13)?,
            };
            return Ok(Some(session));
        }
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[get_listening_session] {err_message}");
    }

    Ok(None)
}

pub fn insert_listening_session(
    id_session: String,
    id_item: String,
    current_time: u32,
    duration: String,
    id_pod: String,
    elapsed_time: u32,
    title: String,
    author: String,
    is_playback: bool,
    chapter: String,
    chapters: String,

) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {
        conn.execute("DELETE FROM listening_session", params![])?;
        conn.execute(
            "INSERT INTO listening_session (id_session, id_item, current_time_playback, duration, is_finished, id_pod, elapsed_time, title, author, is_playback, chapter, chapters)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![id_session, id_item, current_time, duration, id_pod, elapsed_time, title, author, is_playback, chapter, chapters],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[insert_listening_session] {err_message}");
    }

    Ok(())
}

// Update pending_seek (for `listening_session` table) - a book-wide seconds value
// requested by the UI (chapter list jump, P/U next/prev chapter), consumed and cleared
// by the playback loop that owns the running VLC process (see handle_l_book.rs), since
// only it knows whether fulfilling it means seeking the current file or swapping to a
// different one. Empty string means no pending request.
pub fn update_pending_seek(value: &str, id_session: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {

        conn.execute(
            "UPDATE listening_session SET pending_seek = ?1 WHERE id_session = ?2",
            params![value, id_session],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[update_pending_seek] {err_message}");
    }

    Ok(())
}

// VLC's own volume is only ever adjusted relatively (volup/voldown - see
// handle_key_player.rs), never queried, so this is absotui's own tracked value for the
// volume indicator - clamped to 0-200 (VLC's typical amplification ceiling, with 100
// being VLC's own unamplified normal level) rather than VLC's real internal number.
pub fn update_volume_up(id_session: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {

        conn.execute(
            "UPDATE listening_session SET volume = MIN(200, volume + 5) WHERE id_session = ?1",
            params![id_session],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[update_volume_up] {err_message}");
    }

    Ok(())
}

pub fn update_volume_down(id_session: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {

        conn.execute(
            "UPDATE listening_session SET volume = MAX(0, volume - 5) WHERE id_session = ?1",
            params![id_session],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[update_volume_down] {err_message}");
    }

    Ok(())
}
pub fn update_is_playback(value: &str, id_session: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {

        conn.execute(
            "UPDATE listening_session SET is_playback = ?1 WHERE id_session = ?2",
            params![value, id_session],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[update_is_playback] {err_message}");
    }

    Ok(())
}
pub fn update_current_time(value: u32, id_session: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {

        conn.execute(
            "UPDATE listening_session SET current_time_playback = ?1 WHERE id_session = ?2",
            params![value, id_session],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[update_current_time] {err_message}");
    }

    Ok(())
}

pub fn update_elapsed_time(value: u32, id_session: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {

        conn.execute(
            "UPDATE listening_session SET elapsed_time = elapsed_time + ?1 WHERE id_session = ?2",
            params![value, id_session],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[update_elapsed_time] {err_message}");
    }

    Ok(())
}

pub fn update_is_finished(value: &str, id_session: &str) -> Result<()> {
    
    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {

        conn.execute(
            "UPDATE listening_session SET is_finished = ?1 WHERE id_session = ?2",
            params![value, id_session],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[update_is_finished] {err_message}");
    }

    Ok(())
}

pub fn delete_user(username: &str) -> Result<()> {
    
    let db_path = default_db_path();

    let message = format!("User '{username}' deleted. Please restart the app to apply the changes.");
    let err_message = "Error connecting to the database.";
    if let Ok(conn) = Connection::open(db_path) {

        let rows_deleted = conn.execute(
            "DELETE FROM users WHERE username = ?1",
            params![username],
        )?;

        if rows_deleted > 0 {
            let mut stdout = stdout();
            let _ = pop_message(&mut stdout, 3, message.as_str());
            info!("[delete_user] User deleted.");
        }
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[delete user] {err_message}");
    }

    Ok(())
}

pub fn update_is_loop_break(value: &str, username: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {

        conn.execute(
            "UPDATE users SET is_loop_break = ?1 WHERE username = ?2",
            params![value, username],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[update_is_loop_break] {err_message}");
    }

    Ok(())
}

// Atomically claims the "no other session is starting/running" slot for `username` by
// flipping is_loop_break from "1" to "0" in a single UPDATE...WHERE, returning whether
// this call was the one that flipped it. Unlike a separate get_is_loop_break() read
// followed by a later update_is_loop_break("0", ...) write (what wait_prev_session_finished
// used to do), this can't race: two concurrent callers issuing the same UPDATE can't both
// get rows_affected()==1, since sqlite serializes writes to the same row - closes the
// window that let two "l" presses close enough together both pass the old check-then-set
// and start two VLC sessions at once (the second's insert_listening_session write would
// then clobber the first's singleton listening_session row).
pub fn try_claim_playback_slot(username: &str) -> bool {
    let db_path = default_db_path();

    let Ok(conn) = Connection::open(db_path) else {
        return false;
    };
    let _ = conn.busy_timeout(std::time::Duration::from_millis(500));

    conn.execute(
        "UPDATE users SET is_loop_break = '0' WHERE username = ?1 AND is_loop_break = '1'",
        params![username],
    ).map(|rows_affected| rows_affected == 1).unwrap_or(false)
}


pub fn get_is_loop_break(username: &str) -> String {
    let db_path = default_db_path();

    let conn = match Connection::open(db_path) {
        Ok(c) => c,
        Err(_) => return String::from("Error: unable open database"),
    };

    let mut stmt = match conn.prepare("SELECT is_loop_break FROM users WHERE username = ?1") {
        Ok(s) => s,
        Err(_) => return String::from("Error to prepare reqwest"),
    };

    match stmt.query_row(params![username], |row| row.get::<_, String>(0)) {
        Ok(id) => id,
        Err(_) => String::from("No db found"),
    }
}

pub fn update_is_vlc_launched_first_time(value: &str, username: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {

        conn.execute(
            "UPDATE users SET is_vlc_launched_first_time = ?1 WHERE username = ?2",
            params![value, username],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[is_vlc_launched_first_time] {err_message}");
    }

    Ok(())
}
pub fn get_is_vlc_launched_first_time(username: &str) -> String {
    let db_path = default_db_path();

    let conn = match Connection::open(db_path) {
        Ok(c) => c,
        Err(_) => return String::from("Error: unable open database"),
    };

    let mut stmt = match conn.prepare("SELECT is_vlc_launched_first_time FROM users WHERE username = ?1") {
        Ok(s) => s,
        Err(_) => return String::from("Error to prepare reqwest"),
    };

    match stmt.query_row(params![username], |row| row.get::<_, String>(0)) {
        Ok(id) => id,
        Err(_) => String::from("No db found"),
    }
}
pub fn update_id_selected_lib(id_selected_lib: &str, username: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";
    if let Ok(conn) = Connection::open(db_path) {

        conn.execute(
            "UPDATE users SET id_selected_lib = ?1 WHERE username = ?2",
            params![id_selected_lib, username],
        )?;
        info!("[update_id_selected_lib] The library has been updated");

    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[update_id_selected_lib] {err_message}");
    }

    Ok(())
}

pub fn update_server_address(server_address: &str, username: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";
    if let Ok(conn) = Connection::open(db_path) {

        conn.execute(
            "UPDATE users SET server_address = ?1 WHERE username = ?2",
            params![server_address, username],
        )?;
        info!("[update_server_address] The server address has been updated");

    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[update_server_address] {err_message}");
    }

    Ok(())
}

// Persists a freshly-refreshed (access token, refresh token) pair - see
// src/api/server/refresh_token.rs. Both are already encrypted by the caller, same as
// the pair `auth_process` stores at login.
pub fn update_user_tokens(username: &str, encrypted_access_token: &str, encrypted_refresh_token: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";
    if let Ok(conn) = Connection::open(db_path) {

        conn.execute(
            "UPDATE users SET token = ?1, refresh_token = ?2 WHERE username = ?3",
            params![encrypted_access_token, encrypted_refresh_token, username],
        )?;
        info!("[update_user_tokens] Refreshed token stored for {username}");

    } else {
        error!("[update_user_tokens] {err_message}");
    }

    Ok(())
}

pub fn db_insert_usr(users : &Vec<User>)  -> Result<()> {   
    let db_path = default_db_path();

    let conn = Connection::open(db_path)?;
    for user in users {
        conn.execute(
            "INSERT OR REPLACE INTO users (username, server_address, token, is_default_usr, name_selected_lib, id_selected_lib, is_loop_break, is_vlc_launched_first_time, speed_rate, is_vlc_running, is_show_key_bindings, is_speed_adjusted_time, is_podcast_autoplay, is_per_item_speed, is_auto_download, refresh_token)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
            user.username,
            user.server_address,
            user.token,
            i32::from(user.is_default_usr),
            user.name_selected_lib,
            user.id_selected_lib,
            user.is_loop_break,
            user.is_vlc_launched_first_time,
            user.speed_rate,
            user.is_vlc_running,
            user.is_show_key_bindings,
            user.is_speed_adjusted_time,
            user.is_podcast_autoplay,
            user.is_per_item_speed,
            user.is_auto_download,
            user.refresh_token,
            ],
        )?;
    }
    Ok(())
}

pub fn get_others() -> Result<Option<Others>> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {
        let mut stmt = conn.prepare(
            "SELECT login_err
             FROM others
             LIMIT 1",
        )?;

        let mut rows = stmt.query(params![])?;

        if let Some(row) = rows.next()? {
            let others = Others {
                login_err: row.get(0)?,
            };
            return Ok(Some(others));
        }
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[get_others] {err_message}");
    }

    Ok(None)
}
pub fn update_login_err(value: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {
        conn.execute(
            "INSERT INTO others (login_err) SELECT '' WHERE NOT EXISTS (SELECT 1 FROM others LIMIT 1)",
            [],
        )?;
        conn.execute(
            "UPDATE others SET login_err = ?1 WHERE rowid = 1",
            params![value],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[update_login_err] {err_message}");
    }

    Ok(())
}

// Update auth_in_progress (for `others` table) - set to "1" right before spawning
// the async auth_process call in auth_input.rs, and back to "0" once that spawned
// task actually completes (success or failure). Lets main.rs's login loop wait for
// the real result instead of guessing a fixed delay before re-checking the database -
// see wait_for_auth_to_finish in main.rs.
pub fn update_auth_in_progress(value: &str) -> Result<()> {

    let db_path = default_db_path();

    let err_message = "Error connecting to the database.";

    if let Ok(conn) = Connection::open(db_path) {
        conn.execute(
            "INSERT INTO others (login_err) SELECT '' WHERE NOT EXISTS (SELECT 1 FROM others LIMIT 1)",
            [],
        )?;
        conn.execute(
            "UPDATE others SET auth_in_progress = ?1 WHERE rowid = 1",
            params![value],
        )?;
    } else {
        let mut stdout = stdout();
        let _ = pop_message(&mut stdout, 3, err_message);
        error!("[update_auth_in_progress] {err_message}");
    }

    Ok(())
}

// get auth_in_progress (for `others` table) - defaults to "0" (not in progress) if
// the table/row doesn't exist yet, same as a fresh `others` row's own column default.
pub fn get_auth_in_progress() -> String {

    let db_path = default_db_path();

    let conn = match Connection::open(db_path) {
        Ok(c) => c,
        Err(_) => return String::from("0"),
    };

    let mut stmt = match conn.prepare("SELECT auth_in_progress FROM others LIMIT 1") {
        Ok(s) => s,
        Err(_) => return String::from("0"),
    };

    match stmt.query_row(params![], |row| row.get::<_, String>(0)) {
        Ok(value) => value,
        Err(_) => String::from("0"),
    }
}

pub fn select_default_usr() -> Result<Vec<String>> {
    let db_path = default_db_path();

    let conn = Connection::open(db_path)?;

    let mut stmt = conn.prepare(
        "SELECT username, server_address, token, is_default_usr, name_selected_lib, id_selected_lib, is_loop_break, is_vlc_launched_first_time, speed_rate, is_vlc_running, is_show_key_bindings, is_speed_adjusted_time, is_podcast_autoplay, is_per_item_speed, is_auto_download, refresh_token
         FROM users WHERE is_default_usr = 1 LIMIT 1"
    )?;


    let user_iter = stmt.query_map([], |row| {
        Ok(User {
            username: row.get(0)?,
            server_address: row.get(1)?,
            token: row.get(2)?,
            is_default_usr: row.get::<_, i32>(3)? != 0,
            name_selected_lib: row.get(4)?,
            id_selected_lib: row.get(5)?,
            is_loop_break: row.get(6)?,
            is_vlc_launched_first_time: row.get(7)?,
            speed_rate: row.get(8)?,
            is_vlc_running: row.get(9)?,
            is_show_key_bindings: row.get(10)?,
            is_speed_adjusted_time: row.get(11)?,
            is_podcast_autoplay: row.get(12)?,
            is_per_item_speed: row.get(13)?,
            is_auto_download: row.get(14)?,
            refresh_token: row.get(15)?,
        })
    })?;

    let mut result = Vec::new();

    for user in user_iter {
        match user {
            Ok(user) => {
                result.push(user.username);
                result.push(user.server_address);
                result.push(user.token);
                result.push(user.is_default_usr.to_string());
                result.push(user.name_selected_lib);
                result.push(user.id_selected_lib);
                result.push(user.is_loop_break);
                result.push(user.is_vlc_launched_first_time);
                result.push(user.speed_rate.to_string());
                result.push(user.is_vlc_running);
                result.push(user.is_show_key_bindings);
                result.push(user.is_speed_adjusted_time);
                result.push(user.is_podcast_autoplay);
                result.push(user.is_per_item_speed);
                result.push(user.is_auto_download);
                result.push(user.refresh_token);
            }
            Err(e) => {
                println!("Error occurred: {e}");
            }
        }
    }

    Ok(result)  
}

pub fn init_db() -> Result<()> {
    let db_path = default_db_path();

    let conn = Connection::open(db_path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
                username TEXT PRIMARY KEY,
                server_address TEXT NOT NULL,
                token TEXT NOT NULL,
                is_default_usr INTEGER NOT NULL DEFAULT 0,
                name_selected_lib TEXT NOT NULL,
                id_selected_lib TEXT NOT NULL,
                is_loop_break TEXT NOT NULL,
                is_vlc_launched_first_time TEXT NOT NULL,
                speed_rate FLOAT NOT NULL,
                is_vlc_running TEXT NOT NULL,
                is_show_key_bindings TEXT NOT NULL,
                is_speed_adjusted_time TEXT NOT NULL DEFAULT '1',
                is_podcast_autoplay TEXT NOT NULL DEFAULT '0',
                is_per_item_speed TEXT NOT NULL DEFAULT '0',
                is_auto_download TEXT NOT NULL DEFAULT '0',
                refresh_token TEXT NOT NULL DEFAULT ''
            )",
        [],
    )?;

    // Migration for databases created before `is_speed_adjusted_time` existed.
    // SQLite has no "ADD COLUMN IF NOT EXISTS", so we just ignore the error
    // when the column is already there.
    let _ = conn.execute(
        "ALTER TABLE users ADD COLUMN is_speed_adjusted_time TEXT NOT NULL DEFAULT '1'",
        [],
    );

    // Migration for databases created before `is_podcast_autoplay` existed. Defaults
    // to off, since existing users didn't opt into podcasts auto-advancing.
    let _ = conn.execute(
        "ALTER TABLE users ADD COLUMN is_podcast_autoplay TEXT NOT NULL DEFAULT '0'",
        [],
    );

    // Migration for databases created before `is_per_item_speed` existed. Defaults to
    // off - existing users keep the single global speed_rate behavior unless they
    // opt into per book/podcast speeds via Settings.
    let _ = conn.execute(
        "ALTER TABLE users ADD COLUMN is_per_item_speed TEXT NOT NULL DEFAULT '0'",
        [],
    );

    // Migration for databases created before `is_auto_download` existed. Defaults to
    // off - existing users keep the current manual-download-only behavior unless they
    // opt into automatically downloading Continue Listening books via Settings.
    let _ = conn.execute(
        "ALTER TABLE users ADD COLUMN is_auto_download TEXT NOT NULL DEFAULT '0'",
        [],
    );

    // Migration for databases created before `refresh_token` existed. Defaults to
    // empty - an existing logged-in user has no refresh token until they log in again,
    // and the refresh flow (see src/api/server/refresh_token.rs) treats an empty value
    // as "nothing to refresh with," so these users just keep working exactly as before
    // until their current access token naturally expires and they re-authenticate.
    let _ = conn.execute(
        "ALTER TABLE users ADD COLUMN refresh_token TEXT NOT NULL DEFAULT ''",
        [],
    );

    // Create table `item_speed_rate` if there is none - one row per (user, book or
    // podcast show) when Settings > Per-Item Speed is on. Keyed by id_item, which for
    // podcasts is the show's own id (shared across all its episodes, see
    // insert_listening_session's callers) - so this is "per book" for books and "per
    // show" for podcasts, not per individual episode.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS item_speed_rate (
            username TEXT NOT NULL,
            id_item TEXT NOT NULL,
            speed_rate REAL NOT NULL,
            PRIMARY KEY (username, id_item)
            )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS listening_session (
            id_session TEXT PRIMARY KEY,
            id_item TEXT NOT NULL,
            current_time_playback INTEGER NOT NULL,
            duration TEXT NOT NULL,
            is_finished INTEGER NOT NULL DEFAULT 0,
            id_pod TEXT NOT NULL,
            elapsed_time INTEGER NOT NULL,
            title TEXT NOT NULL,
            author TEXT NOT NULL,
            is_playback INTEGER NOT NULL DEFAULT 1,
            chapter TEXT NOT NULL,
            chapters TEXT NOT NULL DEFAULT '',
            volume INTEGER NOT NULL DEFAULT 100,
            pending_seek TEXT NOT NULL DEFAULT ''
            )",
        [],
    )?;

    // Migration for databases created before `chapters` existed.
    // SQLite has no "ADD COLUMN IF NOT EXISTS", so we just ignore the error
    // when the column is already there.
    let _ = conn.execute(
        "ALTER TABLE listening_session ADD COLUMN chapters TEXT NOT NULL DEFAULT ''",
        [],
    );

    // Migration for databases created before `volume` existed. VLC's own volume is only
    // ever adjusted relatively (volup/voldown - see handle_key_player.rs), so absotui
    // tracks this itself for the volume indicator rather than querying VLC, resetting to
    // the default (100%, i.e. VLC's own unamplified normal level) each time a new
    // playback session starts.
    let _ = conn.execute(
        "ALTER TABLE listening_session ADD COLUMN volume INTEGER NOT NULL DEFAULT 100",
        [],
    );

    // Migration for databases created before `pending_seek` existed - see
    // update_pending_seek's doc comment for what it's for.
    let _ = conn.execute(
        "ALTER TABLE listening_session ADD COLUMN pending_seek TEXT NOT NULL DEFAULT ''",
        [],
    );

    // Create table `downloads` if there is none - one row per (user, book or podcast
    // episode) that's been downloaded for offline playback. `file_path` is the local
    // audio file on disk; `duration`/`title`/`author` are snapshotted at download time
    // so offline playback (server unreachable) doesn't need a network call just to
    // render the player/sync progress locally. `kind` ("book" or "podcast") lets each
    // media type's Auto Download sync prune only its own downloads - without it, an
    // Auto Download sync for one media type would delete the other's downloads the
    // moment its own current-list check doesn't recognize their ids.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS downloads (
            username TEXT NOT NULL,
            id_item TEXT NOT NULL,
            file_path TEXT NOT NULL,
            duration TEXT NOT NULL,
            title TEXT NOT NULL,
            author TEXT NOT NULL,
            kind TEXT NOT NULL DEFAULT 'book',
            tracks TEXT NOT NULL DEFAULT '',
            chapters TEXT NOT NULL DEFAULT '',
            PRIMARY KEY (username, id_item)
            )",
        [],
    )?;

    // Migration for databases created before podcast Auto Download (and `kind`)
    // existed. Every download made before this existed was a book.
    let _ = conn.execute(
        "ALTER TABLE downloads ADD COLUMN kind TEXT NOT NULL DEFAULT 'book'",
        [],
    );

    // Migration for databases created before multi-file book downloads existed - see
    // DownloadedTrack. Existing rows keep working via get_download's fallback that
    // synthesizes a single track from the legacy file_path/duration columns.
    let _ = conn.execute(
        "ALTER TABLE downloads ADD COLUMN tracks TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE downloads ADD COLUMN chapters TEXT NOT NULL DEFAULT ''",
        [],
    );

    conn.execute(
        "CREATE TABLE IF NOT EXISTS others (
            login_err TEXT NOT NULL DEFAULT '',
            auth_in_progress TEXT NOT NULL DEFAULT '0'
        )",
        [],
    )?;

    // Migration for databases created before `auth_in_progress` existed.
    // SQLite has no "ADD COLUMN IF NOT EXISTS", so we just ignore the error
    // when the column is already there.
    let _ = conn.execute(
        "ALTER TABLE others ADD COLUMN auth_in_progress TEXT NOT NULL DEFAULT '0'",
        [],
    );

    Ok(())
}


