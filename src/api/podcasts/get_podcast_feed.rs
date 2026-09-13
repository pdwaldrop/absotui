use crate::utils::http_client::api_client;
use reqwest::header::AUTHORIZATION;
use color_eyre::eyre::{Result, Report};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

/// Get Podcast Feed - fetches and parses a raw RSS feed URL directly, fully
/// independent of search (this is what the "add by direct URL" path uses).
/// Requires the logged-in user to be an admin (a non-admin gets a 403).
/// <https://api.audiobookshelf.org/#get-podcast-feed-from-rss-feed>

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub podcast: Podcast,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Podcast {
    pub metadata: PodcastMetadata,
    #[serde(default)]
    pub episodes: Vec<Value>,
    #[serde(default)]
    pub num_episodes: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub image: Option<String>,
    pub description: Option<String>,
    pub description_plain: Option<String>,
    pub feed_url: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    pub explicit: Option<String>,
    pub language: Option<String>,
    pub link: Option<String>,
    #[serde(rename = "type")]
    pub podcast_type: Option<String>,
}

pub async fn get_podcast_feed(rss_feed: &str, token: &str, server_address: String) -> Result<Podcast> {
    let client = api_client();
    let url = format!("{server_address}/api/podcasts/feed");

    let response = client
        .post(url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .json(&serde_json::json!({ "rssFeed": rss_feed }))
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::FORBIDDEN {
        return Err(Report::new(std::io::Error::other(
                    "Adding podcasts requires admin rights on this Audiobookshelf account",
        )));
    }

    if !response.status().is_success() {
        let status = response.status();
        return Err(Report::new(std::io::Error::other(
                    format!("Server responded with HTTP {status}"),
        )));
    }

    let root: Root = response.json().await?;

    Ok(root.podcast)
}
