use crate::utils::http_client::api_client;
use serde_json::Value;
use reqwest::header::AUTHORIZATION;
use color_eyre::eyre::{Result, Report};
use serde::Deserialize;
use serde::Serialize;
use log::error;


#[derive(Deserialize)]
#[serde(untagged)]
enum NumberOrText {
    Number(f64),
    Text(String),
    Missing(()),
}

// Older versions of this app (and possibly other clients) sent a position as a string, and
// the server hands back the type it was given - so a stored "623" has to read as 623.
fn lenient_f64<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    match NumberOrText::deserialize(deserializer)? {
        NumberOrText::Number(n) => Ok(n),
        NumberOrText::Text(text) => text.trim().parse().map_err(serde::de::Error::custom),
        NumberOrText::Missing(()) => Ok(0.0),
    }
}

fn lenient_opt_f64<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    match NumberOrText::deserialize(deserializer)? {
        NumberOrText::Number(n) => Ok(Some(n)),
        NumberOrText::Text(text) => Ok(text.trim().parse().ok()),
        NumberOrText::Missing(()) => Ok(None),
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub id: String,
    pub user_id: String,
    pub library_item_id: String,
    pub episode_id: Value,
    pub media_item_id: String,
    pub media_item_type: String,
    #[serde(default, deserialize_with = "lenient_opt_f64")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "lenient_f64")]
    pub progress: f64,
    #[serde(deserialize_with = "lenient_f64")]
    pub current_time: f64,
    pub is_finished: bool,
    #[serde(default)]
    pub hide_from_continue_listening: Option<bool>,
    pub ebook_location: Value,
    #[serde(default)]
    pub ebook_progress: Option<f64>,
    #[serde(default)]
    pub last_update: Option<i64>,
    #[serde(default)]
    pub started_at: Option<i64>,
    pub finished_at: Value,
}

/// This endpoint retrieves your media progress that is associated with the given library item ID or podcast episode ID.
/// <https://api.audiobookshelf.org/#get-a-media-progress>
pub async fn get_book_progress(token: &str, book_id: &String, server_address: String) -> Result<Root> {
    let client = api_client();
    let url = format!("{server_address}/api/me/progress/{book_id}");

    let response = client
        .get(url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(Report::new(std::io::Error::other(
                    "Failed to fetch data from the API",
        )));
    }

    // Fetched as text first so a parse failure can log the raw payload - reqwest's own
    // decode-error message doesn't show which field or value actually failed to parse.
    let body_text = response.text().await?;
    match serde_json::from_str::<Root>(&body_text) {
        Ok(book_progress) => Ok(book_progress),
        Err(e) => {
            error!("[get_book_progress] failed to parse response for {book_id}: {e}\nraw body: {body_text}");
            Err(Report::new(e))
        }
    }
}

/// Same endpoint family, but for a podcast episode specifically. Unlike a book (whose
/// library item ID doubles as the media's own ID), a podcast episode's progress record
/// isn't found by the bare episode ID alone - it needs the parent podcast's library item
/// ID too, the same two-ID shape `update_media_progress2_pod` already uses to write it.
pub async fn get_episode_progress(token: &str, library_item_id: &str, episode_id: &str, server_address: String) -> Result<Option<Root>> {
    let client = api_client();
    let url = format!("{server_address}/api/me/progress/{library_item_id}/{episode_id}");

    let response = client
        .get(url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await?;

    // No progress record yet (a new, never-played episode) is a 404, not a failure. The
    // body is read out either way: an unread one stops the connection from being reused.
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        let _ = response.bytes().await;
        return Ok(None);
    }
    if !response.status().is_success() {
        let _ = response.bytes().await;
        return Err(Report::new(std::io::Error::other(
                    "Failed to fetch data from the API",
        )));
    }

    let body_text = response.text().await?;
    match serde_json::from_str::<Root>(&body_text) {
        Ok(episode_progress) => Ok(Some(episode_progress)),
        Err(e) => {
            error!("[get_episode_progress] failed to parse response for {library_item_id}/{episode_id}: {e}\nraw body: {body_text}");
            Err(Report::new(e))
        }
    }
}


#[cfg(test)]
mod tests {
    use super::Root;

    // Real payload from a book that also has ebook reading progress recorded
    // alongside its audiobook progress - `ebookProgress` comes back as a fraction
    // (f64), not an integer, which previously failed to deserialize.
    #[test]
    fn parses_progress_with_ebook_progress_fraction() {
        let raw = r#"{"id":"405dd912-56ef-4169-a7fd-a143db9ec386","userId":"4d80eafb-8c7e-4a3e-867d-be97b293c1c5","libraryItemId":"ee26c2cd-2317-4ec8-b60a-d298aa1eef40","episodeId":null,"mediaItemId":"1e37a1e8-b73a-472f-98d1-b0d291626f02","mediaItemType":"book","duration":37068.113333,"progress":0.6251214146614075,"currentTime":23172,"isFinished":false,"hideFromContinueListening":false,"ebookLocation":"24","ebookProgress":0.30666666666666664,"lastUpdate":1784412231758,"startedAt":1778465557297,"finishedAt":null}"#;
        let parsed: Root = serde_json::from_str(raw).expect("should parse now that ebook_progress is f64");
        assert_eq!(parsed.current_time, 23172.0);
        assert!((parsed.progress - 0.6251214146614075).abs() < f64::EPSILON);
    }

    fn podcast_progress_with(duration: &str, progress: &str, current_time: &str) -> String {
        format!(
            r#"{{"id":"a","userId":"u","libraryItemId":"li","episodeId":"ep","mediaItemId":"mi","mediaItemType":"podcastEpisode","duration":{duration},"progress":{progress},"currentTime":{current_time},"isFinished":false,"hideFromContinueListening":false,"ebookLocation":null,"ebookProgress":null,"lastUpdate":1,"startedAt":1,"finishedAt":null}}"#
        )
    }

    // A real response from a server that had stored the position as a string (an older
    // session sync sent it that way): this used to fail with
    // `invalid type: string "623", expected f64` and the episode was treated as unfinished.
    #[test]
    fn a_position_stored_as_a_string_still_reads_as_a_number() {
        let parsed: Root = serde_json::from_str(&podcast_progress_with("1800.5", "0.34", r#""623""#)).expect("string position should parse");
        assert_eq!(parsed.current_time, 623.0);
        assert!((parsed.progress - 0.34).abs() < f64::EPSILON);
    }

    #[test]
    fn every_numeric_field_accepts_a_number_a_numeric_string_or_null() {
        let parsed: Root = serde_json::from_str(&podcast_progress_with(r#""1800.5""#, r#""0.5""#, "12")).unwrap();
        assert_eq!((parsed.duration, parsed.progress, parsed.current_time), (Some(1800.5), 0.5, 12.0));
        let parsed: Root = serde_json::from_str(&podcast_progress_with("null", "null", "null")).unwrap();
        assert_eq!((parsed.duration, parsed.progress, parsed.current_time), (None, 0.0, 0.0));
    }

    #[test]
    fn a_non_numeric_position_is_still_an_error() {
        assert!(serde_json::from_str::<Root>(&podcast_progress_with("1", "0.1", r#""soon""#)).is_err());
    }
}
