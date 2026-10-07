use crate::utils::http_client::api_client;
use serde::{Deserialize, Serialize};
use color_eyre::eyre::{Result, Report};
use crate::db::crud::db_insert_usr;
use crate::db::database_struct::User;
use crate::api::libraries::get_all_libraries::get_all_libraries;
use crate::api::utils::collect_get_all_libraries::{collect_library_names, collect_media_types, collect_library_ids};
use crate::utils::encrypt_token::encrypt_token;
use log::info;


#[derive(Serialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Deserialize, Debug)]
struct LoginResponse {
    user: UserInfo,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct UserInfo {
    token: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
}

impl UserInfo {
    fn effective_token(&self) -> &str {
        if let Some(ref access_token) = self.access_token
            && !access_token.is_empty() {
                return access_token;
        }
        self.token.as_deref().unwrap_or("")
    }
}

/// Login
/// <https://api.audiobookshelf.org/#server>
/// The login function takes a username, password, url ans  makes a POST request and returns a token.
/// After, some data are fetched with this token and written in database
pub async fn auth_process(username: &str, password: &str, server_address: &str) -> Result<()> {
    let login_url = format!("{server_address}/login");
    let client = api_client();

    // Struct for data request
    let login_data = LoginRequest {
        username: username.to_string(),
        password: password.to_string(),
    };

    // Send POST request. `x-return-tokens: true` is required for Audiobookshelf to
    // return `refreshToken` in the response body at all - without it, a server on
    // v2.26+ (when this JWT flow was introduced) only sets it as an httpOnly cookie,
    // which is useless to a non-browser client. Confirmed against the server's own
    // `Auth.js`/`TokenManager.js`, since the public API docs are stale.
    let response = client
        .post(login_url)
        .header("Content-Type", "application/json")
        .header("x-return-tokens", "true")
        .json(&login_data)
        .send()
        .await?;

    // Checking the status of the response and fetch data
    if response.status().is_success() {
        let login_response: LoginResponse = response.json().await?;
        let effective_token = login_response.user.effective_token();
        if effective_token.is_empty() {
            return Err(Report::new(std::io::Error::other("No authentication token received from server")));
        }

        let all_libraries = get_all_libraries(effective_token, server_address.to_string()).await?;
        let library_names = collect_library_names(&all_libraries).await;
        let _media_types = collect_media_types(&all_libraries).await;
        let library_ids = collect_library_ids(&all_libraries).await;

        // A fresh server, or an account restricted from every library, legitimately has
        // zero accessible libraries - indexing [0] below would panic inside this spawned
        // task before update_auth_in_progress("0") ever runs (see auth_input.rs), leaving
        // the login screen stuck on "authenticating" forever with no visible error.
        if library_names.is_empty() || library_ids.is_empty() {
            return Err(Report::new(std::io::Error::other(
                "This account has no accessible libraries - grant it at least one library on the server and try again.",
            )));
        }

        // Token encryption before insert it in the database. A failure here (almost
        // always a missing/unreadable ABSOTUI_SECRET_KEY) used to be logged and
        // swallowed, leaving `token_encrypted` as an empty string that still got
        // written to the users table - login looked successful, but the very next
        // launch would 401 against that empty token with no indication why. Fail the
        // login itself instead, so nothing unusable ever reaches the database.
        let token_encrypted = match encrypt_token(effective_token) {
            Ok(encrypted_token) => {
                info!("Token successfully encrypted");
                encrypted_token
            }
            Err(e) => {
                return Err(Report::new(std::io::Error::other(format!(
                    "Couldn't save your login: {e}"
                ))));
            }
        };

        // Empty when the server didn't return one (legacy `token`-only auth, or a
        // server too old for the JWT flow) - the refresh mechanism (see
        // src/api/server/refresh_token.rs) treats an empty refresh token as "nothing
        // to refresh with" and simply never fires, which is the correct behavior for
        // a token that isn't a short-lived JWT to begin with.
        let mut refresh_token_encrypted = String::new();
        if let Some(refresh_token) = login_response.user.refresh_token.filter(|t| !t.is_empty()) {
            match encrypt_token(&refresh_token) {
                Ok(encrypted) => refresh_token_encrypted = encrypted,
                Err(e) => {
                    return Err(Report::new(std::io::Error::other(format!(
                        "Couldn't save your login: {e}"
                    ))));
                }
            }
        }

        // Init for handle_l
        let is_loop_break = "0".to_string();
        let is_vlc_running = "0".to_string();
        let is_vlc_launched_first_time = "1".to_string();


        // Writting in database : 

        // init a new user
        let users = vec![
            User {
                server_address: server_address.to_string(),
                username: username.to_string(),
                token: token_encrypted,
                is_default_usr: true,
                name_selected_lib: library_names[0].clone(), // by default we take the first library
                id_selected_lib: library_ids[0].clone(),
                is_loop_break,
                is_vlc_launched_first_time,
                speed_rate: 1.0,
                is_vlc_running,
                is_show_key_bindings: "1".to_string(),
                is_speed_adjusted_time: "1".to_string(),
                is_podcast_autoplay: "0".to_string(),
                is_per_item_speed: "0".to_string(),
                is_auto_download: "0".to_string(),
                refresh_token: refresh_token_encrypted,
            }
        ];

        // insert the new user in database
        let _ = db_insert_usr(&users);

        Ok(()) 
    } else {
        Err(Report::new(std::io::Error::other("Login failed"))) 
    }
}
