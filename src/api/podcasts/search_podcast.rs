use crate::utils::http_client::api_client;
use reqwest::header::AUTHORIZATION;
use color_eyre::eyre::{Result, Report};
use serde::Deserialize;
use serde::Serialize;

/// Search Podcasts (proxies Apple's iTunes podcast search - Audiobookshelf has no
/// native "browse"/chart capability of its own, only search-by-term)
/// <https://api.audiobookshelf.org/#search-podcast>

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastSearchResult {
    pub id: Option<i64>,
    pub artist_id: Option<i64>,
    pub title: Option<String>,
    pub artist_name: Option<String>,
    pub description: Option<String>,
    pub description_plain: Option<String>,
    pub release_date: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    pub cover: Option<String>,
    pub track_count: Option<i64>,
    pub feed_url: Option<String>,
    pub page_url: Option<String>,
    #[serde(default)]
    pub explicit: bool,
}

pub async fn search_podcast(term: &str, token: &str, server_address: String) -> Result<Vec<PodcastSearchResult>> {
    let client = api_client();
    let url = format!("{server_address}/api/search/podcast");

    let response = client
        .get(url)
        .query(&[("term", term)])
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(Report::new(std::io::Error::other(
                    format!("Server responded with HTTP {status}"),
        )));
    }

    let results: Vec<PodcastSearchResult> = response.json().await?;

    Ok(results)
}
