use crate::utils::http_client::api_client;
use color_eyre::eyre::Result; 
use reqwest::header::AUTHORIZATION;
use serde_json::json;

// Numbers, not strings: the server stores whatever type it receives, and a string
// `currentTime` came back from progress lookups as a string the app then couldn't parse.
fn sync_body(current_time: Option<u32>, time_listened: u32) -> serde_json::Value {
    json!({
        "currentTime": current_time.unwrap_or(0),
        "timeListened": time_listened,
    })
}

/// This endpoint syncs the position of an open listening session from the client to the server and returns the session.
/// <https://api.audiobookshelf.org/#sync-an-open-session>
pub async fn sync_session(token: Option<&String>, session_id: &str, current_time: Option<u32>, time_listened: u32, server_address: String) -> Result<(), reqwest::Error> {
    let client = api_client();

    let params = sync_body(current_time, time_listened);

    let _response = client
        .post(format!(
                "{server_address}/api/session/{session_id}/sync"
        ))
        .header("Content-Type", "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", token.unwrap()))
        .json(&params)
        .send()
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::sync_body;

    #[test]
    fn sync_body_sends_numbers_not_strings() {
        let body = sync_body(Some(623), 10);
        assert_eq!(body["currentTime"], serde_json::json!(623));
        assert!(body["currentTime"].is_u64());
        assert!(body["timeListened"].is_u64());
    }

    #[test]
    fn sync_body_defaults_a_missing_position_to_zero() {
        assert_eq!(sync_body(None, 0)["currentTime"], serde_json::json!(0));
    }
}
