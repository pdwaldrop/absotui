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
    crate::utils::wake::wake_main_loop();

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
    crate::utils::wake::wake_main_loop();

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
    crate::utils::wake::wake_main_loop();

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

    #[test]
    fn covers_in_every_supported_format_decode() {
        let samples = [
        ("JPEG", "ffd8ffe000104a46494600010100000100010000ffdb004300080606070605080707070909080a0c140d0c0b0b0c1912130f141d1a1f1e1d1a1c1c20242e2720222c231c1c2837292c30313434341f27393d38323c2e333432ffdb0043010909090c0b0c180d0d1832211c213232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232ffc00011080002000303012200021101031101ffc4001f0000010501010101010100000000000000000102030405060708090a0bffc400b5100002010303020403050504040000017d01020300041105122131410613516107227114328191a1082342b1c11552d1f02433627282090a161718191a25262728292a3435363738393a434445464748494a535455565758595a636465666768696a737475767778797a838485868788898a92939495969798999aa2a3a4a5a6a7a8a9aab2b3b4b5b6b7b8b9bac2c3c4c5c6c7c8c9cad2d3d4d5d6d7d8d9dae1e2e3e4e5e6e7e8e9eaf1f2f3f4f5f6f7f8f9faffc4001f0100030101010101010101010000000000000102030405060708090a0bffc400b51100020102040403040705040400010277000102031104052131061241510761711322328108144291a1b1c109233352f0156272d10a162434e125f11718191a262728292a35363738393a434445464748494a535455565758595a636465666768696a737475767778797a82838485868788898a92939495969798999aa2a3a4a5a6a7a8a9aab2b3b4b5b6b7b8b9bac2c3c4c5c6c7c8c9cad2d3d4d5d6d7d8d9dae2e3e4e5e6e7e8e9eaf2f3f4f5f6f7f8f9faffda000c03010002110311003f00e50924e4927b7345145712d8fd168ff0e3e88fffd9"),
        ("PNG", "89504e470d0a1a0a0000000d49484452000000030000000208020000001216f14d0000001c49444154789c63382167c37542e3849c0de309391b060606060606003b56046297c2de6b0000000049454e44ae426082"),
        ("WebP lossy", "524946464600000057454250565038203a000000b001009d012a0300020001402625a80274ba0004300000fef0faa8366f1220a3e1b55656dffdb6ecdbf3fe87fdc67ffd6dd9b7e7fd0fd8400000"),
        ("WebP lossless", "5249464624000000574542505650384c170000002f024000000f702be42bf89e0265fea3052f3888e87f0800"),
        ("GIF", "474946383761030002008100000ac828c81e3c0000000000002c00000000030002000008080003000840304040003b"),
        ("BMP", "424d4e0000000000000036000000280000000300000002000000010018000000000018000000c40e0000c40e000000000000000000003c1ec83c1ec83c1ec80000003c1ec828c80a3c1ec8000000"),
        ];
        for (name, hex) in samples {
            let bytes: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            let img = image::load_from_memory(&bytes).unwrap_or_else(|e| panic!("{name} cover failed to decode: {e}"));
            assert_eq!((img.width(), img.height()), (3, 2), "{name}");
        }
    }
}
