use crate::utils::http_client::api_client;
use reqwest::header::AUTHORIZATION;
use color_eyre::eyre::{Result, Report};

/// Delete Library Item - used to remove/unsubscribe a podcast. `hard = true`
/// also deletes any downloaded episode files from disk; `hard = false` just
/// removes the library entry, leaving downloaded files untouched.
/// <https://api.audiobookshelf.org/#delete-library-item>
pub async fn delete_library_item(item_id: &str, hard: bool, token: &str, server_address: String) -> Result<()> {
    let client = api_client();
    let url = format!("{server_address}/api/items/{item_id}");
    let hard_flag = if hard { "1" } else { "0" };

    let response = client
        .delete(url)
        .query(&[("hard", hard_flag)])
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::FORBIDDEN {
        return Err(Report::new(std::io::Error::other(
                    "You don't have permission to remove this on your Audiobookshelf account",
        )));
    }

    if !response.status().is_success() {
        let status = response.status();
        return Err(Report::new(std::io::Error::other(
                    format!("Server responded with HTTP {status}"),
        )));
    }

    log::info!("[delete_library_item] removed {item_id} (hard={hard})");

    Ok(())
}
