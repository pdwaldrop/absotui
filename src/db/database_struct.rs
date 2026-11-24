use serde::{Serialize, Deserialize};
use crate::db::crud::{init_db, select_default_usr, update_login_err};
use color_eyre::Result;
use log::error;
use std::time::Duration;

pub struct Database  {
    pub users: Vec<User>,
    pub default_usr: Vec<String>,
    pub listening_session: ListeningSession,
    pub others: Others,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    pub  server_address: String,
    pub  username: String,
    pub  token: String,
    pub  is_default_usr: bool,
    pub  name_selected_lib: String,
    pub  id_selected_lib: String,
    pub  is_loop_break: String,
    pub  is_vlc_launched_first_time: String,
    pub  speed_rate: f32,
    pub  is_vlc_running: String,
    pub  is_show_key_bindings: String,
    pub  is_speed_adjusted_time: String,
    pub  is_podcast_autoplay: String,
    pub  is_per_item_speed: String,
    pub  is_auto_download: String,
    pub  refresh_token: String,
}

#[derive(Serialize, Deserialize, Debug)]
// currently use for close listening session when app is quit
// but in future could be used to sync offline items
pub struct ListeningSession {
    pub id_session: String,
    pub id_item: String,
    pub current_time: u32,
    pub duration: String,
    pub is_finished: bool,
    pub id_pod: String,
    pub elapsed_time: u32,
    pub title: String,
    pub author: String,
    pub is_playback: bool,
    pub chapter: String,
    pub chapters: String,
    pub volume: i32,
    pub pending_seek: String,
}

pub struct Others {
    pub login_err: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DownloadedItem {
    pub file_path: String,
    pub duration: String,
    pub title: String,
    pub author: String,
    pub tracks: Vec<DownloadedTrack>,
    pub chapters: String,
}

/// A download as listed on Settings > Downloads.
#[derive(Debug)]
pub struct DownloadRow {
    pub id_item: String,
    pub kind: String,
    pub is_auto: bool,
    pub item: DownloadedItem,
}

/// One locally-saved audio file backing a downloaded book - mirrors `AudioTrack`
/// (src/api/library_items/play_lib_item_or_pod.rs) but pointing at a file on disk
/// instead of a server URL. A single-file book's download is simply the 1-entry case.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DownloadedTrack {
    pub local_path: String,
    pub duration: f64,
    pub start_offset: f64,
}


impl Database {
    pub async fn new() -> Result<Self> {
        // open db and create table if there is none
        let _ = init_db();

        // init empty Vec<User> for future add of users
        let users: Vec<User> = vec![];

        // Read the default user, retrying briefly: a locked database right after a change is transient, and
        // failing silently would send a returning user to the login screen. A lasting failure is shown there.
        let mut default_usr: Vec<String> = Vec::new();
        let mut last_err = None;
        for attempt in 0..5 {
            match select_default_usr() {
                Ok(result) => {
                    default_usr = result;
                    last_err = None;
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                    if attempt < 4 {
                        tokio::time::sleep(Duration::from_millis(400)).await;
                    }
                }
            }
        }
        if let Some(e) = last_err {
            let message = format!("Couldn't read your saved login ({e}) - please log in again.");
            error!("[Database::new] {message}");
            let _ = update_login_err(&message);
        }


        // init listening_session
        let listening_session = ListeningSession {
            id_session: String::new(),
            id_item: String::new(),
            current_time: 0,
            duration: String::new(),
            is_finished: false,
            id_pod: String::new(),
            elapsed_time: 0,
            title: String::new(),
            author: String::new(),
            is_playback: false,
            chapter: String::new(),
            chapters: String::new(),
            volume: 100,
            pending_seek: String::new(),
        };

        let others = Others {
            login_err: String::new(),
        };

        Ok(Self {
            users,
            default_usr,
            listening_session,
            others
        })
    }
}

