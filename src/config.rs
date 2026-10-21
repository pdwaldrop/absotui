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

/// load config from `config.toml` file
pub fn load_config() -> Result<ConfigFile> {
    let config_home_path = env::var("XDG_CONFIG_HOME").map_or_else(|_| { 
            let mut path = dirs::home_dir().expect("Unable to find the user's home directory");

            if cfg!(target_os = "macos") {
                path.push("Library/Preferences");
            } else {
                path.push(".config");
            }

            path
        }, PathBuf::from);

    let config_path = config_home_path.join("absotui/config.toml");
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
    fn a_custom_sso_port_is_honored() {
        let config = parse("[auth]\nsso_callback_port = 50000\n");
        let auth: Auth = config.get("auth").unwrap();
        assert_eq!(auth.sso_callback_port, 50000);
    }
}
