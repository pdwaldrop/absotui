use crate::utils::http_client::api_client;
use reqwest::header::AUTHORIZATION;
use color_eyre::eyre::{Result, Report};
use serde_json::Value;

/// Queues raw feed episodes for download with no date filter. Seeding a new podcast needs this:
/// `checknew` finds nothing, since a new podcast's last check is its creation time.
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
