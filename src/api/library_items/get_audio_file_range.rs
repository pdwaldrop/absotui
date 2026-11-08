use crate::utils::http_client::api_client;
use reqwest::header::{ACCEPT_ENCODING, AUTHORIZATION, RANGE};
use color_eyre::eyre::Result;

// ID3v2 tags (and any embedded picture they carry) live at the very start of an MP3
// file, so a bounded prefix is enough without pulling down the whole episode - most
// embedded podcast episode art is well under this.
const EMBEDDED_COVER_PREFIX_BYTES: u64 = 8 * 1024 * 1024;

/// Fetches the leading bytes of a podcast episode's raw audio file, identified by its
/// audio file `ino` (not the episode id) - used to read the ID3 tag for embedded cover
/// art without downloading the whole file.
/// <https://api.audiobookshelf.org/#get-a-library-item-39-s-file>
pub async fn get_audio_file_prefix(token: &str, library_item_id: &str, ino: &str, server_address: &str) -> Result<Vec<u8>> {
    let client = api_client();
    let url = format!("{server_address}/api/items/{library_item_id}/file/{ino}");

    let response = client
        .get(url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header(RANGE, format!("bytes=0-{}", EMBEDDED_COVER_PREFIX_BYTES - 1))
        .header(ACCEPT_ENCODING, "identity")
        .send()
        .await?;

    let bytes = response.bytes().await?;
    Ok(bytes.to_vec())
}
