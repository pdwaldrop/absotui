use config::{Config as ConfigLib, File};
use serde::Deserialize;
use color_eyre::eyre::{Result, Report};
use std::env;
use std::path::PathBuf;
use crate::player::backend::Backend;

#[derive(Debug, Deserialize)]
pub struct ConfigFile {
    pub player: Player,
    pub downloads: Downloads,
    pub auth: Auth,
}

#[derive(Debug, Deserialize)]
pub struct Auth {
    #[serde(default = "default_sso_callback_port")]
    pub sso_callback_port: u16,
}

impl Default for Auth {
    fn default() -> Self {
        Auth { sso_callback_port: default_sso_callback_port() }
    }
}

// Existing config.toml files predate SSO login, so `[auth]` is typically missing
// entirely - `load_config` falls back to this same default when the section is absent.
// The matching `http://127.0.0.1:<port>/callback` address is what a server admin has to
// whitelist in Audiobookshelf, so changing this means changing that too.
fn default_sso_callback_port() -> u16 {
    crate::api::server::oidc_login::DEFAULT_SSO_PORT
}

#[derive(Debug, Deserialize)]
pub struct Player {
    #[serde(default)]
    pub backend: Backend,
    pub cvlc: String,
    pub cvlc_term: String,
    pub address: String,
    pub port: String,
}

#[derive(Debug, Deserialize)]
pub struct Downloads {
    #[serde(default = "default_auto_download_count")]
    pub auto_download_count: usize,
}

impl Default for Downloads {
    fn default() -> Self {
        Downloads { auto_download_count: default_auto_download_count() }
    }
}

// Existing config.toml files predate Auto Download entirely, so the whole
// `[downloads]` section is typically missing rather than just this field -
// `load_config` falls back to this same default when the section itself is absent.
fn default_auto_download_count() -> usize {
    5
}

fn config_path() -> PathBuf {
    let config_home_path = env::var("XDG_CONFIG_HOME").map_or_else(|_| {
            let mut path = dirs::home_dir().expect("Unable to find the user's home directory");

            if cfg!(target_os = "macos") {
                path.push("Library/Preferences");
            } else {
                path.push(".config");
            }

            path
        }, PathBuf::from);

    config_home_path.join("absotui/config.toml")
}

/// Rewrites the `backend` line under `[player]` (adding it if the file predates it),
/// leaving every other line and comment untouched.
fn with_backend(toml: &str, backend: Backend) -> String {
    let new_line = format!("backend = \"{}\"", backend.as_str());
    let mut lines: Vec<String> = toml.lines().map(str::to_string).collect();
    let mut in_player = false;
    let mut player_header = None;
    let mut backend_line = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_player = trimmed.starts_with("[player]");
            if in_player {
                player_header = Some(i);
            }
        } else if in_player
            && trimmed.strip_prefix("backend").is_some_and(|rest| rest.trim_start().starts_with('='))
        {
            backend_line = Some(i);
        }
    }

    match (backend_line, player_header) {
        (Some(i), _) => lines[i] = new_line,
        (None, Some(i)) => lines.insert(i + 1, new_line),
        (None, None) => {
            lines.push("[player]".to_string());
            lines.push(new_line);
        }
    }

    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Saves the player choice into `config.toml`. Written to a temp file and renamed,
/// so a crash can't leave a half-written config behind.
pub fn set_backend(backend: Backend) -> Result<()> {
    // Resolved first so a symlinked config (dotfile managers) keeps its link.
    let path = std::fs::canonicalize(config_path())?;
    let current = std::fs::read_to_string(&path)?;
    let tmp_path = path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, with_backend(&current, backend))?;
    std::fs::rename(&tmp_path, &path)?;
    Ok(())
}

/// load config from `config.toml` file
pub fn load_config() -> Result<ConfigFile> {
    let config_path = config_path();
    let config_path_str = config_path.to_str().unwrap().to_string();

    let config = ConfigLib::builder()
        .add_source(File::with_name(&config_path_str))
        .build()
        .map_err(|e| Report::new(e))?;

    let player: Player = config.get("player")
        .map_err(|e| Report::new(e))?;
    let downloads: Downloads = config.get("downloads").unwrap_or_default();
    let auth: Auth = config.get("auth").unwrap_or_default();

    Ok(ConfigFile { player, downloads, auth })
}


#[cfg(test)]
mod tests {
    use super::*;
    use config::FileFormat;

    fn parse(toml: &str) -> ConfigLib {
        ConfigLib::builder()
            .add_source(File::from_str(toml, FileFormat::Toml))
            .build()
            .unwrap()
    }

    #[test]
    fn example_config_parses_and_matches_the_built_in_sso_port() {
        let config = parse(include_str!("../config.example.toml"));
        let auth: Auth = config.get("auth").unwrap();
        assert_eq!(auth.sso_callback_port, default_sso_callback_port());
        config.get::<Player>("player").unwrap();
    }

    #[test]
    fn config_without_an_auth_section_falls_back_to_the_default_port() {
        let config = parse("[player]\ncvlc = \"1\"\ncvlc_term = \"0\"\naddress = \"localhost\"\nport = \"1234\"\n");
        let auth: Auth = config.get("auth").unwrap_or_default();
        assert_eq!(auth.sso_callback_port, default_sso_callback_port());
    }

    #[test]
    fn example_config_defaults_to_vlc() {
        let config = parse(include_str!("../config.example.toml"));
        let player: Player = config.get("player").unwrap();
        assert_eq!(player.backend, Backend::Vlc);
    }

    #[test]
    fn with_backend_replaces_the_existing_line_and_keeps_everything_else() {
        let before = "#### PLAYER OPTIONS ####\n[player]\n# Which player\nbackend = \"vlc\"\ncvlc = \"1\"\n\n[auth]\nsso_callback_port = 41729\n";
        let after = with_backend(before, Backend::Mpv);
        assert_eq!(after, before.replace("backend = \"vlc\"", "backend = \"mpv\""));
        assert_eq!(with_backend(&after, Backend::Vlc), before);
    }

    #[test]
    fn with_backend_adds_the_line_when_the_config_predates_it() {
        let before = "[player]\ncvlc = \"1\"\n\n[downloads]\nbackend = \"not ours\"\n";
        let after = with_backend(before, Backend::Mpv);
        assert_eq!(after, "[player]\nbackend = \"mpv\"\ncvlc = \"1\"\n\n[downloads]\nbackend = \"not ours\"\n");
        assert_eq!(parse(&after).get::<String>("player.backend").unwrap(), "mpv");
    }

    #[test]
    fn with_backend_ignores_a_commented_out_line() {
        let after = with_backend("[player]\n# backend = \"vlc\"\ncvlc = \"1\"\n", Backend::Mpv);
        assert_eq!(after, "[player]\nbackend = \"mpv\"\n# backend = \"vlc\"\ncvlc = \"1\"\n");
    }

    #[test]
    fn a_custom_sso_port_is_honored() {
        let config = parse("[auth]\nsso_callback_port = 50000\n");
        let auth: Auth = config.get("auth").unwrap();
        assert_eq!(auth.sso_callback_port, 50000);
    }
}
