use crate::utils::http_client::api_client;
use color_eyre::eyre::Result;
use reqwest::header::AUTHORIZATION;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use serde_json::json;
use crate::api::libraries::get_library_perso_view_pod::Chapter;


const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One physical audio file backing a book. Audiobookshelf returns one of these per
/// underlying file in `audioTracks[]` - a single-file book (eg. `.m4b`) has exactly one,
/// a book uploaded as separate per-chapter files has one per file. `start_offset` is
/// cumulative across the whole book (not per-file), matching `Chapter.start`/`.end`.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTrack {
    pub content_url: String,
    pub duration: f64,
    pub start_offset: f64,
    /// This track's position within the session's own `audioTracks[]` - needed (not
    /// just `content_url`) to build the `/public/session/:id/track/:index` URL VLC
    /// actually streams from. See `post_start_playback_session_book`'s doc comment.
    /// Missing or null (older items, per Audiobookshelf's own pre-v2.21.0 caveat -
    /// see `post_start_playback_session_pod`'s identical tolerance) falls back to 0
    /// rather than failing this track's whole containing array to deserialize.
    #[serde(default, deserialize_with = "index_or_default")]
    pub index: i64,
}

fn index_or_default<'de, D>(deserializer: D) -> std::result::Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<i64>::deserialize(deserializer)?.unwrap_or(0))
}

/// Which track a book-wide `time` (seconds) falls into - the first track whose span
/// covers it, or the last track if `time` is at/past the end of the book (eg. resuming
/// right at a book's final saved position). Callers must ensure `tracks` is non-empty;
/// returns 0 otherwise since there is nothing sensible to index.
pub fn find_track_index(tracks: &[AudioTrack], time: f64) -> usize {
    tracks
        .iter()
        .position(|t| time < t.start_offset + t.duration)
        .unwrap_or_else(|| tracks.len().saturating_sub(1))
}

/// Play a Library Item or Podcast Episode
/// This endpoint starts a playback session for a library item or podcast episode.
/// <https://api.audiobookshelf.org/#play-a-library-item-or-podcast-episode>
pub async fn post_start_playback_session_book(media_player: &str, token: Option<&String>, id_library_item: &str, server_address: String) -> Result<Vec<String>, reqwest::Error> {
    let client = api_client();

    let params = json!({
        // Streams the original file (eg. .m4b) directly to the player, rather than
        // transcoding to .m3u8, so chapters/cover art stay intact and there's no added
        // transcoding latency.
        "forceDirectPlay": true,
        "mediaPlayer": media_player,
        "deviceInfo": {
            "clientName": "Absotui",
            "clientVersion": format!("v{}", VERSION),
            // Shows up as the OS in the server's user activity panel (audiobookshelf/config/users/).
            "manufacturer": std::env::consts::OS,
            "model": std::env::consts::ARCH,
        }});

    let response = client
        .post(format!(
                "{server_address}/api/items/{id_library_item}/play"
        ))
        .header("Content-Type", "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", token.unwrap()))
        .json(&params)
        .send()
        .await?;

    let v: Value = response.json().await?;

    let current_time = v["currentTime"]
        .as_f64()
        .unwrap_or(0.0);

    // A book split across multiple files (eg. one file per chapter) gets one entry per
    // file here, each carrying a book-wide cumulative `startOffset` - not just the
    // first file, confirmed via live testing against a real multi-file audiobook (see
    // find_track_index's doc comment). A single-file book is simply the 1-entry case.
    let tracks: Vec<AudioTrack> = serde_json::from_value(v["audioTracks"].clone()).unwrap_or_default();
    let duration: u32 = tracks.last()
        .map(|t| t.start_offset + t.duration)
        .unwrap_or(0.0) as u32;
    let start_track_idx = if tracks.is_empty() { 0 } else { find_track_index(&tracks, current_time) };
    let content_url = tracks.get(start_track_idx).map(|t| t.content_url.as_str()).unwrap_or("");
    let tracks_json = serde_json::to_string(&tracks).unwrap_or_default();

    let id_session = v["id"]
        .as_str()
        .unwrap_or("");
    let title = v["mediaMetadata"]["title"]
        .as_str()
        .unwrap_or("N/A");
    let subtitle = v["mediaMetadata"]["title"]
        .as_str()
        .unwrap_or("N/A");
    let author = v["displayAuthor"]
        .as_str()
        .unwrap_or("N/A");

    // book playback sessions carry full chapter title/start/end data, confirmed via
    // live testing against a real server (books were previously assumed to only
    // get a chapter count, not full metadata)
    let chapters: Vec<Chapter> = serde_json::from_value(v["chapters"].clone()).unwrap_or_default();
    let chapters_json = serde_json::to_string(&chapters).unwrap_or_default();

    let info_item = vec![
        current_time.to_string(),
        content_url.to_string(),
        duration.to_string(),
        id_session.to_string(),
        title.to_string(),
        subtitle.to_string(),
        author.to_string(),
        chapters_json,
        tracks_json,
    ];

    Ok(info_item)
}
pub async fn post_start_playback_session_pod(media_player: &str, token: Option<&String>, id_library_item: &str, pod_ep_id: &str, server_address: String) -> Result<Vec<String>, reqwest::Error> {
    let client = api_client();

    let params = json!({
        // Streams the original file directly to the player, rather than transcoding to
        // .m3u8, so chapters/cover art stay intact and there's no added transcoding
        // latency.
        "forceDirectPlay": true,
        "mediaPlayer": media_player,
        "deviceInfo": {
            "clientName": "Absotui",
            "clientVersion": format!("v{}", VERSION),
            // Shows up as the OS in the server's user activity panel (audiobookshelf/config/users/).
            "manufacturer": std::env::consts::OS,
            "model": std::env::consts::ARCH,
        }});

    let response = client
        .post(format!(
                "{server_address}/api/items/{id_library_item}/play/{pod_ep_id}",
        ))
        .header("Content-Type", "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", token.unwrap()))
        .json(&params)
        .send()
        .await?;

    let v: Value = response.json().await?;

    let current_time = v["currentTime"]
        .as_f64()
        .unwrap_or(0.0);
    let content_url = v["audioTracks"][0]["contentUrl"]
        .as_str()
        .unwrap_or("");
    // Falls back to 0 if absent - the server explicitly tolerates that for podcasts
    // (its own comment: "handles old episodes pre-v2.21.0 having null index"), so this
    // is a safe default, not a guess.
    let track_index = v["audioTracks"][0]["index"]
        .as_i64()
        .unwrap_or(0);
    let duration = v["audioTracks"][0]["duration"]
        .as_f64()
        .unwrap_or(0.0);
    let duration: u32 = duration as u32;
    let id_session = v["id"]
        .as_str()
        .unwrap_or("");
    let title = v["mediaMetadata"]["title"]
        .as_str()
        .unwrap_or("N/A");
    let subtitle = v["displayTitle"]
        .as_str()
        .unwrap_or("N/A");
    let author = v["displayAuthor"]
        .as_str()
        .unwrap_or("N/A");

    let info_item = vec![
        current_time.to_string(),
        content_url.to_string(),
        duration.to_string(),
        id_session.to_string(),
        title.to_string(),
        subtitle.to_string(),
        author.to_string(),
        track_index.to_string(),
    ];

    Ok(info_item)
}
