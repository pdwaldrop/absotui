use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use color_eyre::eyre::{Report, Result};
use reqwest::{Client, StatusCode, Url, header, redirect};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::UnboundedReceiver;

// Signing in through an identity provider (authentik, Keycloak, ...) uses Audiobookshelf's
// native-app OpenID flow: PKCE, with the browser redirected back to a loopback address.
// Confirmed against the server's own `Auth.js` and `OidcAuthStrategy.js`, since the public
// docs don't describe it. The server only accepts redirect addresses its admin has listed
// under "Allowed Mobile Redirect URIs", so the port is fixed rather than random.

pub const DEFAULT_SSO_PORT: u16 = 41729;
const CALLBACK_PATH: &str = "/callback";
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

fn err(msg: impl Into<String>) -> Report {
    Report::new(std::io::Error::other(msg.into()))
}

pub fn redirect_uri(port: u16) -> String {
    format!("http://127.0.0.1:{port}{CALLBACK_PATH}")
}

/// The address without surrounding whitespace or trailing slashes, so `{base}/login`
/// never becomes `//login`.
pub fn base_url(server_address: &str) -> &str {
    server_address.trim().trim_end_matches('/')
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthMethods {
    pub local: bool,
    pub openid: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusResponse {
    #[serde(default)]
    auth_methods: Vec<String>,
}

pub fn parse_auth_methods(body: &str) -> Option<AuthMethods> {
    let status: StatusResponse = serde_json::from_str(body).ok()?;
    let has = |method: &str| status.auth_methods.iter().any(|m| m == method);
    Some(AuthMethods {
        // Servers too old to report `authMethods` only ever had password login.
        local: status.auth_methods.is_empty() || has("local"),
        openid: has("openid"),
    })
}

/// Which login methods the server offers, from its public `/status`. `None` on any
/// failure, so callers fall back to the ordinary password login.
pub async fn fetch_auth_methods(server_address: &str) -> Option<AuthMethods> {
    let client = Client::builder().timeout(PROBE_TIMEOUT).build().ok()?;
    let body = client
        .get(format!("{}/status", base_url(server_address)))
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    parse_auth_methods(&body)
}

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

pub fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn random_token(bytes: usize) -> Result<String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf)
        .map_err(|e| err(format!("Couldn't generate a secure random value: {e}")))?;
    Ok(URL_SAFE_NO_PAD.encode(buf))
}

pub fn generate_pkce() -> Result<Pkce> {
    let verifier = random_token(32)?;
    let challenge = challenge_for(&verifier);
    Ok(Pkce { verifier, challenge })
}

fn authorize_url(server_address: &str, redirect_uri: &str, challenge: &str, state: &str) -> Result<Url> {
    let mut url = Url::parse(&format!("{}/auth/openid", base_url(server_address)))
        .map_err(|e| err(format!("That doesn't look like a valid server address: {e}")))?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state);
    Ok(url)
}

/// What `begin_authorization` hands back: where to send the browser, and the server-side
/// session the later code exchange has to present again.
pub struct AuthSession {
    pub idp_url: String,
    cookies: String,
}

fn cookie_header(headers: &header::HeaderMap) -> String {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|cookie| cookie.split(';').next())
        .map(str::trim)
        .filter(|cookie| !cookie.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

fn flow_client() -> Result<Client> {
    Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .redirect(redirect::Policy::none())
        .build()
        .map_err(|e| err(format!("Couldn't start the sign-in request: {e}")))
}

fn short(body: &str) -> String {
    body.trim().chars().take(200).collect()
}

fn authorization_error(status: StatusCode, body: &str, redirect_uri: &str) -> Report {
    if status == StatusCode::BAD_REQUEST && body.contains("Invalid redirect_uri") {
        return err(format!(
            "The server doesn't allow this sign-in address yet. Ask the server admin to add {redirect_uri} under Settings > Authentication > Allowed Mobile Redirect URIs in Audiobookshelf, then try again."
        ));
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        return err("The server is rate-limiting sign-in attempts - wait a few minutes and try again.");
    }
    if status == StatusCode::NOT_FOUND {
        return err("This server doesn't offer SSO sign-in (OpenID Connect isn't enabled, or the server is too old).");
    }
    err(format!("The server refused to start SSO sign-in (HTTP {status}): {}", short(body)))
}

/// Asks the server to start the flow. Done by Absotui itself rather than the browser
/// because the server keeps the PKCE state in a session cookie that the final code
/// exchange needs to send back.
pub async fn begin_authorization(server_address: &str, redirect_uri: &str, pkce: &Pkce, state: &str) -> Result<AuthSession> {
    let url = authorize_url(server_address, redirect_uri, &pkce.challenge, state)?;
    let response = flow_client()?
        .get(url.clone())
        .send()
        .await
        .map_err(|e| err(format!("Couldn't reach the server to start SSO sign-in: {e}")))?;
    let status = response.status();

    if status.is_redirection() {
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| err("The server started SSO sign-in but didn't say where to send the browser."))?;
        let idp_url = url
            .join(location)
            .map_err(|e| err(format!("The server gave an unusable sign-in address: {e}")))?
            .to_string();
        return Ok(AuthSession { idp_url, cookies: cookie_header(response.headers()) });
    }

    let body = response.text().await.unwrap_or_default();
    Err(authorization_error(status, &body, redirect_uri))
}

#[derive(Debug, Default, PartialEq)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

fn params_from_url(url: &Url) -> CallbackParams {
    let mut params = CallbackParams::default();
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "code" => params.code = Some(value.into_owned()),
            "state" => params.state = Some(value.into_owned()),
            "error_description" => params.error = Some(value.into_owned()),
            "error" if params.error.is_none() => params.error = Some(value.into_owned()),
            _ => {}
        }
    }
    params
}

/// The request target from a browser's `GET <target> HTTP/1.1` line, if it's the
/// callback address.
pub fn parse_callback_target(target: &str) -> Option<CallbackParams> {
    let url = Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
    (url.path() == CALLBACK_PATH).then(|| params_from_url(&url))
}

/// A pasted callback address, or just its query string - for when the browser can't
/// reach this machine's loopback address (Absotui running over SSH, say).
pub fn parse_pasted_callback(input: &str) -> Option<CallbackParams> {
    let input = input.trim();
    let url = if input.contains("://") {
        Url::parse(input).ok()?
    } else {
        Url::parse(&format!("http://127.0.0.1{CALLBACK_PATH}?{}", input.trim_start_matches('?'))).ok()?
    };
    let params = params_from_url(&url);
    (params.code.is_some() || params.error.is_some()).then_some(params)
}

pub async fn bind_listener(port: u16) -> Result<TcpListener> {
    TcpListener::bind(("127.0.0.1", port)).await.map_err(|e| {
        err(format!(
            "Couldn't listen on 127.0.0.1:{port} for the sign-in redirect ({e}). Is something else using that port? You can change `sso_callback_port` under [auth] in config.toml - the server admin then has to allow the matching address."
        ))
    })
}

async fn read_request_target(stream: &mut TcpStream) -> Option<String> {
    let mut buf = vec![0u8; 8192];
    let read = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buf)).await.ok()?.ok()?;
    let head = String::from_utf8_lossy(&buf[..read]);
    let mut parts = head.lines().next()?.split_whitespace();
    if parts.next()? != "GET" {
        return None;
    }
    parts.next().map(str::to_string)
}

async fn respond(stream: &mut TcpStream, status: &str, message: &str) {
    let html = format!(
        "<!doctype html><meta charset=utf-8><title>Absotui</title><body style=\"font-family:sans-serif;margin:3em\">{message}</body>"
    );
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// Waits for the browser to land on the callback address. Requests that aren't the
/// callback, or carry the wrong `state`, are answered and ignored rather than ending the
/// wait - so a stray request from another program on this machine can't abort a sign-in.
pub async fn wait_for_callback(listener: &TcpListener, expected_state: &str) -> Result<String> {
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|e| err(format!("The sign-in listener stopped: {e}")))?;
        let Some(target) = read_request_target(&mut stream).await else {
            continue;
        };
        let Some(params) = parse_callback_target(&target) else {
            respond(&mut stream, "404 Not Found", "Not found.").await;
            continue;
        };
        if params.state.as_deref() != Some(expected_state) {
            respond(&mut stream, "400 Bad Request", "This sign-in doesn't match the one Absotui started. Go back to Absotui and try again.").await;
            continue;
        }
        if let Some(error) = params.error {
            respond(&mut stream, "400 Bad Request", "Sign-in failed. Go back to Absotui for details.").await;
            return Err(err(format!("Sign-in wasn't completed: {error}")));
        }
        match params.code {
            Some(code) => {
                respond(&mut stream, "200 OK", "Signed in. You can close this tab and go back to Absotui.").await;
                return Ok(code);
            }
            None => respond(&mut stream, "400 Bad Request", "No sign-in code was received.").await,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct SsoTokens {
    pub username: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct ExchangeResponse {
    user: ExchangeUser,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExchangeUser {
    username: String,
    access_token: Option<String>,
    token: Option<String>,
    refresh_token: Option<String>,
}

fn exchange_error(status: StatusCode, body: &str) -> Report {
    if status == StatusCode::BAD_REQUEST && body.contains("No session") {
        return err("The server lost track of this sign-in (a proxy in front of it may be dropping cookies). Try again.");
    }
    if status == StatusCode::UNAUTHORIZED {
        return err("The server turned the sign-in down. This usually means no Audiobookshelf user matches that account - ask the server admin whether SSO matching or auto-registration is set up.");
    }
    err(format!("The server couldn't finish the sign-in (HTTP {status}): {}", short(body)))
}

pub async fn exchange_code(server_address: &str, session: &AuthSession, code: &str, state: &str, verifier: &str) -> Result<SsoTokens> {
    let mut url = Url::parse(&format!("{}/auth/openid/callback", base_url(server_address)))
        .map_err(|e| err(format!("That doesn't look like a valid server address: {e}")))?;
    url.query_pairs_mut()
        .append_pair("code", code)
        .append_pair("state", state)
        .append_pair("code_verifier", verifier);

    let response = flow_client()?
        .get(url)
        .header(header::COOKIE, &session.cookies)
        .send()
        .await
        .map_err(|e| err(format!("Couldn't reach the server to finish SSO sign-in: {e}")))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(exchange_error(status, &body));
    }

    let parsed: ExchangeResponse = response
        .json()
        .await
        .map_err(|e| err(format!("The server's sign-in reply wasn't what Absotui expected: {e}")))?;
    let user = parsed.user;
    let access_token = user
        .access_token
        .filter(|t| !t.is_empty())
        .or(user.token.filter(|t| !t.is_empty()))
        .ok_or_else(|| err("No authentication token received from server"))?;
    Ok(SsoTokens {
        username: user.username,
        access_token,
        refresh_token: user.refresh_token.filter(|t| !t.is_empty()),
    })
}

pub fn open_browser(url: &str) -> bool {
    let program = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    match Command::new(program)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(mut child) => {
            // Reaped in the background so the opener doesn't linger as a zombie.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            true
        }
        Err(_) => false,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SsoUpdate {
    Url(String),
    Status(String),
}

async fn wait_for_paste(
    pasted: &mut UnboundedReceiver<String>,
    expected_state: &str,
    on_update: &mut impl FnMut(SsoUpdate),
) -> Result<String> {
    while let Some(text) = pasted.recv().await {
        if let Some(params) = parse_pasted_callback(&text)
            && params.state.as_deref().is_none_or(|state| state == expected_state)
        {
            if let Some(error) = params.error {
                return Err(err(format!("Sign-in wasn't completed: {error}")));
            }
            if let Some(code) = params.code {
                return Ok(code);
            }
        }
        on_update(SsoUpdate::Status(
            "That doesn't look like the address the browser ended up on - paste the whole address from its address bar.".to_string(),
        ));
    }
    // The paste box went away; keep waiting on the listener alone.
    std::future::pending().await
}

/// The whole browser sign-in: start it with the server, send the browser to the identity
/// provider, wait for the redirect (or a pasted address), and swap the code for tokens.
pub async fn sso_login(
    server_address: &str,
    port: u16,
    mut pasted: UnboundedReceiver<String>,
    mut on_update: impl FnMut(SsoUpdate),
) -> Result<SsoTokens> {
    let state = random_token(24)?;
    let pkce = generate_pkce()?;
    let redirect = redirect_uri(port);

    let listener = bind_listener(port).await?;
    let session = begin_authorization(server_address, &redirect, &pkce, &state).await?;

    on_update(SsoUpdate::Url(session.idp_url.clone()));
    on_update(SsoUpdate::Status(
        if open_browser(&session.idp_url) {
            "Opening your browser - finish signing in there..."
        } else {
            "Couldn't open a browser from here - open the link above in any browser."
        }
        .to_string(),
    ));

    let code = tokio::time::timeout(SIGN_IN_TIMEOUT, async {
        tokio::select! {
            result = wait_for_callback(&listener, &state) => result,
            result = wait_for_paste(&mut pasted, &state, &mut on_update) => result,
        }
    })
    .await
    .map_err(|_| err("Timed out waiting for the sign-in to finish (5 minutes). Try again."))??;

    on_update(SsoUpdate::Status("Signed in - finishing up...".to_string()));
    exchange_code(server_address, &session, &code, &state, &pkce.verifier).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_drops_whitespace_and_trailing_slashes_only() {
        assert_eq!(base_url("  https://abs.example.com/ "), "https://abs.example.com");
        assert_eq!(base_url("http://192.168.1.5:13378///"), "http://192.168.1.5:13378");
        assert_eq!(base_url("https://example.com/audiobookshelf"), "https://example.com/audiobookshelf");
    }

    #[test]
    fn pkce_challenge_matches_rfc7636_vector() {
        assert_eq!(
            challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn generated_pkce_is_valid_and_unique() {
        let a = generate_pkce().unwrap();
        let b = generate_pkce().unwrap();
        assert_eq!(a.verifier.len(), 43);
        assert!(a.verifier.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_eq!(a.challenge, challenge_for(&a.verifier));
        assert_ne!(a.verifier, b.verifier);
    }

    #[test]
    fn redirect_uri_uses_loopback_ip_and_port() {
        assert_eq!(redirect_uri(41729), "http://127.0.0.1:41729/callback");
    }

    #[test]
    fn auth_methods_cover_local_only_sso_only_and_both() {
        let local = parse_auth_methods(r#"{"authMethods":["local"]}"#).unwrap();
        assert_eq!(local, AuthMethods { local: true, openid: false });
        let sso_only = parse_auth_methods(r#"{"authMethods":["openid"]}"#).unwrap();
        assert_eq!(sso_only, AuthMethods { local: false, openid: true });
        let both = parse_auth_methods(r#"{"authMethods":["local","openid"],"authFormData":{}}"#).unwrap();
        assert_eq!(both, AuthMethods { local: true, openid: true });
    }

    #[test]
    fn old_servers_without_auth_methods_count_as_password_only() {
        let old = parse_auth_methods(r#"{"app":"audiobookshelf","isInit":true}"#).unwrap();
        assert_eq!(old, AuthMethods { local: true, openid: false });
    }

    #[test]
    fn non_json_status_is_not_a_server_answer() {
        assert_eq!(parse_auth_methods("<html>nope</html>"), None);
    }

    #[test]
    fn authorize_url_carries_pkce_and_encodes_the_redirect() {
        let url = authorize_url("https://abs.example.com/", "http://127.0.0.1:41729/callback", "CHAL", "STATE").unwrap();
        assert_eq!(url.path(), "/auth/openid");
        let query: Vec<(String, String)> = url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
        assert!(query.contains(&("response_type".into(), "code".into())));
        assert!(query.contains(&("redirect_uri".into(), "http://127.0.0.1:41729/callback".into())));
        assert!(query.contains(&("code_challenge".into(), "CHAL".into())));
        assert!(query.contains(&("code_challenge_method".into(), "S256".into())));
        assert!(query.contains(&("state".into(), "STATE".into())));
        assert!(url.as_str().contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A41729%2Fcallback"));
    }

    #[test]
    fn authorize_url_rejects_a_server_address_that_is_not_a_url() {
        assert!(authorize_url("not a url", "http://127.0.0.1:1/callback", "c", "s").is_err());
    }

    #[test]
    fn cookie_header_replays_every_cookie_without_attributes() {
        let mut headers = header::HeaderMap::new();
        headers.append(header::SET_COOKIE, "connect.sid=abc123; Path=/; HttpOnly".parse().unwrap());
        headers.append(header::SET_COOKIE, "auth_method=openid-mobile; Max-Age=999; HttpOnly".parse().unwrap());
        assert_eq!(cookie_header(&headers), "connect.sid=abc123; auth_method=openid-mobile");
    }

    #[test]
    fn callback_target_yields_code_and_state() {
        let params = parse_callback_target("/callback?code=abc%2F123&state=xyz").unwrap();
        assert_eq!(params.code.as_deref(), Some("abc/123"));
        assert_eq!(params.state.as_deref(), Some("xyz"));
        assert_eq!(params.error, None);
    }

    #[test]
    fn callback_target_ignores_other_paths() {
        assert_eq!(parse_callback_target("/favicon.ico"), None);
        assert_eq!(parse_callback_target("/"), None);
    }

    #[test]
    fn callback_target_prefers_the_error_description() {
        let params = parse_callback_target("/callback?error=access_denied&error_description=User%20said%20no&state=s").unwrap();
        assert_eq!(params.error.as_deref(), Some("User said no"));
        let reversed = parse_callback_target("/callback?error_description=User%20said%20no&error=access_denied&state=s").unwrap();
        assert_eq!(reversed.error.as_deref(), Some("User said no"));
    }

    #[test]
    fn pasted_input_accepts_a_full_address_or_just_the_query() {
        let full = parse_pasted_callback("  http://127.0.0.1:41729/callback?code=C&state=S \n").unwrap();
        assert_eq!((full.code.as_deref(), full.state.as_deref()), (Some("C"), Some("S")));
        let other_host = parse_pasted_callback("http://localhost:9999/whatever?code=C&state=S").unwrap();
        assert_eq!(other_host.code.as_deref(), Some("C"));
        let query = parse_pasted_callback("code=C&state=S").unwrap();
        assert_eq!(query.code.as_deref(), Some("C"));
        let with_mark = parse_pasted_callback("?code=C&state=S").unwrap();
        assert_eq!(with_mark.code.as_deref(), Some("C"));
    }

    #[test]
    fn pasted_input_without_a_code_or_error_is_rejected() {
        assert_eq!(parse_pasted_callback("hello there"), None);
        assert_eq!(parse_pasted_callback("http://127.0.0.1:41729/callback"), None);
        assert_eq!(parse_pasted_callback(""), None);
    }

    #[test]
    fn rejected_redirect_names_the_exact_address_to_allow() {
        let error = authorization_error(StatusCode::BAD_REQUEST, "Invalid redirect_uri", "http://127.0.0.1:41729/callback");
        let message = error.to_string();
        assert!(message.contains("http://127.0.0.1:41729/callback"));
        assert!(message.contains("Allowed Mobile Redirect URIs"));
    }

    #[test]
    fn other_authorization_failures_get_their_own_messages() {
        assert!(authorization_error(StatusCode::NOT_FOUND, "", "u").to_string().contains("doesn't offer SSO"));
        assert!(authorization_error(StatusCode::TOO_MANY_REQUESTS, "", "u").to_string().contains("rate-limiting"));
        assert!(authorization_error(StatusCode::INTERNAL_SERVER_ERROR, "boom", "u").to_string().contains("boom"));
    }
}
