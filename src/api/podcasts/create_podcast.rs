use crate::utils::http_client::api_client;
use reqwest::header::AUTHORIZATION;
use color_eyre::eyre::{Result, Report};
use serde_json::json;

/// Create Podcast - subscribes to a podcast (metadata + feed URL only, no
/// episodes). Requires the logged-in user to be an admin (a non-admin gets a
/// 403). Episodes are never populated by this call - see `check_new_episodes`.
/// <https://api.audiobookshelf.org/#create-podcast>
pub struct CreatePodcastParams {
    pub library_id: String,
    pub folder_id: String,
    pub path: String,
    pub title: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub release_date: Option<String>,
    pub genres: Vec<String>,
    pub feed_url: String,
    pub image_url: Option<String>,
    pub itunes_page_url: Option<String>,
    pub itunes_id: Option<i64>,
    pub itunes_artist_id: Option<i64>,
    pub explicit: bool,
}

pub async fn create_podcast(params: CreatePodcastParams, token: &str, server_address: String) -> Result<String> {
    let client = api_client();
    let url = format!("{server_address}/api/podcasts");

    // The server's Podcast model only accepts these two as strings (`typeof
    // payload.metadata.itunesId === 'string'`) - sent as JSON numbers they'd
    // silently be dropped to null instead of erroring.
    let itunes_id = params.itunes_id.map(|id| id.to_string());
    let itunes_artist_id = params.itunes_artist_id.map(|id| id.to_string());

    let body = json!({
        "libraryId": params.library_id,
        "folderId": params.folder_id,
        "path": params.path,
        "media": {
            "metadata": {
                "title": params.title,
                "author": params.author,
                "description": params.description,
                "releaseDate": params.release_date,
                "genres": params.genres,
                "feedUrl": params.feed_url,
                "imageUrl": params.image_url,
                "itunesPageUrl": params.itunes_page_url,
                "itunesId": itunes_id,
                "itunesArtistId": itunes_artist_id,
                "explicit": params.explicit,
            },
            "autoDownloadEpisodes": false,
        },
    });

    let response = client
        .post(url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .json(&body)
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::FORBIDDEN {
        return Err(Report::new(std::io::Error::other(
                    "Adding podcasts requires admin rights on this Audiobookshelf account",
        )));
    }

    if !response.status().is_success() {
        let status = response.status();
        let response_text = response.text().await.unwrap_or_default();
        log::error!("[create_podcast] failed - status {status}, body: {response_text}");
        return Err(Report::new(std::io::Error::other(
                    format!("Server responded with HTTP {status}"),
        )));
    }

    #[derive(serde::Deserialize)]
    struct CreatedItem { id: String }
    let created: CreatedItem = response.json().await?;

    log::info!("[create_podcast] created podcast library item {}", created.id);

    Ok(created.id)
}
