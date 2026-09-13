use crate::utils::http_client::api_client;
use reqwest::header::AUTHORIZATION;
use color_eyre::eyre::{Result, Report};
use serde_json::Value;

/// Download Episodes - takes raw feed-episode objects (as returned by
/// `get_podcast_feed`) and queues them for download directly, with no date
/// filtering at all. This is what actually seeds a freshly-created podcast's
/// first episodes - `check_new_episodes`/`checknew` can't do it: Audiobookshelf
/// stamps a new podcast's `lastEpisodeCheck` to the creation instant, so a
/// checknew call made immediately after creating one always finds zero episodes,
/// since every episode in any real feed necessarily predates "right now".
/// <https://api.audiobookshelf.org/#download-episodes>
pub async fn download_episodes(item_id: &str, episodes: &[Value], token: &str, server_address: String) -> Result<()> {
    let client = api_client();
    let url = format!("{server_address}/api/podcasts/{item_id}/download-episodes");

    let response = client
        .post(url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .json(episodes)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(Report::new(std::io::Error::other(
                    format!("Server responded with HTTP {status}"),
        )));
    }

    Ok(())
}
