use std::env;
use std::io::Cursor;
use std::path::PathBuf;
use color_eyre::eyre::Result;
use crate::api::library_items::get_cover::get_cover;
use crate::api::library_items::get_audio_file_range::get_audio_file_prefix;
use crate::utils::http_client::api_client;

fn covers_dir() -> PathBuf {
    let config_home_path = env::var("XDG_CONFIG_HOME").map_or_else(|_| {
            let mut path = dirs::home_dir().expect("Unable to find the user's home directory");

            if cfg!(target_os = "macos") {
                path.push("Library/Preferences");
            } else {
                path.push(".config");
            }

            path
        }, PathBuf::from);

    config_home_path.join("absotui/covers")
}

/// Path a given item's cached cover would live at, regardless of whether it's been
/// fetched yet - image format is auto-detected from the file's contents at load time,
/// so no extension is needed.
///
/// `item_id` comes straight from the configured Audiobookshelf server's JSON responses,
/// unvalidated - a malicious or compromised server (or a MITM on a plain `http://`
/// connection) could hand back an id like `"../../../../home/user/.ssh/authorized_keys"`
/// and have this app write that response's bytes there via `fs::write`. Real item ids
/// are opaque UUIDs and never need anything beyond alphanumerics/hyphens/underscores, so
/// stripping everything else both closes that off (the result can never contain `/`, `\`,
/// or `.`, so it can't escape `covers_dir()` or reference `..`) and is a no-op for every
/// legitimate id.
pub fn cover_cache_path(item_id: &str) -> PathBuf {
    let safe_id: String = item_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    covers_dir().join(safe_id)
}

/// Fetches an item's cover and writes it to the local cache, if not already cached.
/// Meant to be run in a background task - rendering just polls `cover_cache_path` for
/// the file's existence rather than waiting on this directly.
pub async fn fetch_and_cache_cover(token: String, item_id: String, server_address: String) -> Result<()> {
    let path = cover_cache_path(&item_id);
    if path.exists() {
        return Ok(());
    }

    let bytes = get_cover(&token, &item_id, &server_address).await?;

    std::fs::create_dir_all(covers_dir())?;
    std::fs::write(path, bytes)?;

    Ok(())
}

/// Fetches an arbitrary public image URL (e.g. iTunes artwork for a podcast search
/// result that isn't a library item yet, so has no Audiobookshelf item id/cover
/// endpoint of its own) and writes it to the local cache under `cache_key`, if not
/// already cached. Unauthenticated - unlike `fetch_and_cache_cover`, this never sends
/// the server's own bearer token, since the URL points at a third party (iTunes), not
/// the configured Audiobookshelf server.
pub async fn fetch_and_cache_external_cover(url: String, cache_key: String) -> Result<()> {
    let path = cover_cache_path(&cache_key);
    if path.exists() {
        return Ok(());
    }

    let client = api_client();
    let response = client.get(url).send().await?;
    let bytes = response.bytes().await?;

    std::fs::create_dir_all(covers_dir())?;
    std::fs::write(path, bytes)?;

    Ok(())
}

/// Fetches a podcast episode's own embedded cover art (from its MP3 ID3 tag) and writes
/// it to the local cache, if not already cached. Leaves nothing behind - and therefore
/// falls back to the parent podcast's cover on every subsequent render - if the file
/// doesn't actually carry a picture frame within the fetched prefix; callers only invoke
/// this for episodes flagged at scan time as having one, so that should be rare.
pub async fn fetch_and_cache_episode_cover(token: String, episode_id: String, library_item_id: String, ino: String, server_address: String) -> Result<()> {
    let path = cover_cache_path(&episode_id);
    if path.exists() {
        return Ok(());
    }

    let prefix = get_audio_file_prefix(&token, &library_item_id, &ino, &server_address).await?;

    let tag = match id3::Tag::read_from2(Cursor::new(prefix)) {
        Ok(tag) => tag,
        Err(e) => {
            log::warn!("[fetch_and_cache_episode_cover] episode {episode_id}: no readable ID3 tag in fetched prefix: {e}");
            return Ok(());
        }
    };

    let Some(picture) = tag.pictures().next() else {
        log::warn!("[fetch_and_cache_episode_cover] episode {episode_id}: ID3 tag has no picture frame despite embeddedCoverArt flag");
        return Ok(());
    };

    std::fs::create_dir_all(covers_dir())?;
    std::fs::write(path, &picture.data)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legitimate_uuid_style_id_is_unchanged() {
        let id = "a1b2c3d4-e5f6-4789-a0bc-def012345678";
        assert_eq!(cover_cache_path(id).file_name().unwrap().to_str().unwrap(), id);
    }

    #[test]
    fn path_traversal_id_cannot_escape_covers_dir() {
        let malicious = "../../../../home/user/.ssh/authorized_keys";
        let path = cover_cache_path(malicious);
        assert!(path.starts_with(covers_dir()));
        assert_eq!(path.parent().unwrap(), covers_dir());
    }

    #[test]
    fn absolute_path_id_cannot_escape_covers_dir() {
        let malicious = "/home/user/.ssh/authorized_keys";
        let path = cover_cache_path(malicious);
        assert!(path.starts_with(covers_dir()));
        assert_eq!(path.parent().unwrap(), covers_dir());
    }
}
