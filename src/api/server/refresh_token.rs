use crate::utils::http_client::api_client;
use crate::utils::encrypt_token::encrypt_token;
use crate::db::crud::update_user_tokens;
use color_eyre::eyre::{Result, Report};
use serde::Deserialize;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use log::{info, error};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct RefreshUserInfo {
    access_token: Option<String>,
    refresh_token: Option<String>,
}

#[derive(Deserialize, Debug)]
struct RefreshResponse {
    user: RefreshUserInfo,
}

// Audiobookshelf's access token defaults to a 1 hour lifetime (server-side
// `TokenManager.js`, `AccessTokenExpiry`). Refreshing this far ahead of the real
// expiry means a task that only checks once per loop tick (every ~1s during
// playback, ~200ms in the main render loop) never has a realistic chance of still
// being caught holding an expired token.
const REFRESH_MARGIN_SECS: i64 = 10 * 60;

/// Exchanges a refresh token for a new (access token, refresh token) pair via
/// Audiobookshelf's `POST /auth/refresh`. The refresh token **rotates on every use**
/// (confirmed against the server's own `TokenManager.js`/`Auth.js`) - the old value
/// only remains valid for a short grace period - so callers must persist the returned
/// refresh token, not just the access token.
/// Distinguishes "the server was reached and explicitly rejected the refresh token"
/// from any other failure (network unreachable, timeout, malformed response) - only
/// this one means the refresh token itself is actually dead. `maybe_refresh_token`
/// downcasts to this to decide between `RefreshOutcome::Failed` (delete-the-account
/// worthy) and `RefreshOutcome::TransientError` (try again next tick).
#[derive(Debug)]
struct RefreshRejected(reqwest::StatusCode);

impl std::fmt::Display for RefreshRejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Token refresh failed with status {}", self.0)
    }
}

impl std::error::Error for RefreshRejected {}

pub async fn refresh_access_token(server_address: &str, refresh_token: &str) -> Result<(String, String)> {
    let client = api_client();

    let response = client
        .post(format!("{server_address}/auth/refresh"))
        .header("x-refresh-token", refresh_token)
        .header("x-return-tokens", "true")
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(Report::new(RefreshRejected(response.status())));
    }

    let parsed: RefreshResponse = response.json().await?;
    let new_access_token = parsed.user.access_token
        .filter(|t| !t.is_empty())
        .ok_or_else(|| Report::new(std::io::Error::other("Refresh response had no accessToken")))?;
    let new_refresh_token = parsed.user.refresh_token.unwrap_or_default();

    Ok((new_access_token, new_refresh_token))
}

/// Reads the `exp` claim out of a JWT's payload segment, no signature verification -
/// this only ever inspects Absotui's own token to decide whether to proactively renew
/// it, never untrusted input. Returns `false` (don't refresh) for anything that isn't
/// a 3-part JWT, which is what makes this a no-op for legacy non-expiring tokens.
fn is_token_expiring_soon(token: &str) -> bool {
    token_exp(token).is_some_and(|exp| exp - chrono::Utc::now().timestamp() < REFRESH_MARGIN_SECS)
}

/// `Some(true)`/`Some(false)` for a JWT with an `exp` claim, `None` for anything else
/// (a legacy non-expiring token) - used only to explain a rejected login.
pub fn is_token_expired(token: &str) -> Option<bool> {
    token_exp(token).map(|exp| exp <= chrono::Utc::now().timestamp())
}

fn token_exp(token: &str) -> Option<i64> {
    let payload_b64 = token.split('.').nth(1)?;
    let payload_bytes = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    let claims = serde_json::from_slice::<serde_json::Value>(&payload_bytes).ok()?;
    claims.get("exp").and_then(serde_json::Value::as_i64)
}

#[derive(PartialEq, Eq, Debug)]
pub enum RefreshOutcome {
    /// Token wasn't close to expiring, or there's no refresh token to use.
    NotNeeded,
    /// Refreshed and persisted - `token`/`refresh_token` were updated in place.
    Refreshed,
    /// The refresh token itself is dead (expired past its ~30 day lifetime, or
    /// revoked server-side) - the caller's session cannot be kept alive.
    Failed,
    /// Couldn't complete the refresh attempt at all (network unreachable, timeout,
    /// malformed response) - the refresh token itself might still be perfectly good.
    /// Callers should just leave the existing token in place and try again next
    /// tick, not treat this the same as a confirmed-dead refresh token.
    TransientError,
}

/// Checked once per loop tick by both the main render loop (`App::refresh_token_if_needed`)
/// and every long-running playback task (`handle_l_book`/`handle_l_pod`/`handle_l_pod_home`) -
/// each holds its own copy of the token (see CLAUDE.md's "one owner" note on why the
/// playback task never reaches into the live `App`), so each independently keeps its own
/// copy fresh rather than relying on the other to have refreshed it.
///
/// Placed at the top of a loop body, before that iteration's own API calls, this also
/// doubles as the reactive backstop for a token that went stale while nothing was
/// ticking (eg. the OS suspended mid-playback) - the very next tick catches it before
/// any call that iteration would otherwise fail with a 401.
pub async fn maybe_refresh_token(
    token: &mut String,
    refresh_token: &mut String,
    username: &str,
    server_address: &str,
) -> RefreshOutcome {
    if refresh_token.is_empty() || !is_token_expiring_soon(token) {
        return RefreshOutcome::NotNeeded;
    }

    match refresh_access_token(server_address, refresh_token).await {
        Ok((new_access_token, new_refresh_token)) => {
            match (encrypt_token(&new_access_token), encrypt_token(&new_refresh_token)) {
                (Ok(enc_access), Ok(enc_refresh)) => {
                    let _ = update_user_tokens(username, &enc_access, &enc_refresh);
                }
                _ => error!("[maybe_refresh_token] Refreshed the token but couldn't encrypt it for storage - the DB will keep the old one until next login"),
            }
            *token = new_access_token;
            *refresh_token = new_refresh_token;
            info!("[maybe_refresh_token] Access token refreshed for {username}");
            RefreshOutcome::Refreshed
        }
        Err(e) => {
            error!("[maybe_refresh_token] Refresh failed for {username}: {e}");
            if e.downcast_ref::<RefreshRejected>().is_some() {
                RefreshOutcome::Failed
            } else {
                RefreshOutcome::TransientError
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_jwt_with_exp(exp: i64) -> String {
        let payload = serde_json::json!({ "exp": exp, "userId": "abc", "type": "access" });
        let payload_b64 = URL_SAFE_NO_PAD.encode(payload.to_string());
        format!("header.{payload_b64}.signature")
    }

    #[test]
    fn token_expiring_within_the_margin_is_flagged() {
        let exp = chrono::Utc::now().timestamp() + 60; // 1 minute left
        assert!(is_token_expiring_soon(&fake_jwt_with_exp(exp)));
    }

    #[test]
    fn token_expiring_well_outside_the_margin_is_not_flagged() {
        let exp = chrono::Utc::now().timestamp() + 3600; // 1 hour left
        assert!(!is_token_expiring_soon(&fake_jwt_with_exp(exp)));
    }

    #[test]
    fn already_expired_token_is_flagged() {
        let exp = chrono::Utc::now().timestamp() - 60; // expired 1 minute ago
        assert!(is_token_expiring_soon(&fake_jwt_with_exp(exp)));
    }

    #[test]
    fn is_token_expired_distinguishes_expired_valid_and_non_jwt() {
        let now = chrono::Utc::now().timestamp();
        assert_eq!(is_token_expired(&fake_jwt_with_exp(now - 60)), Some(true));
        assert_eq!(is_token_expired(&fake_jwt_with_exp(now + 3600)), Some(false));
        assert_eq!(is_token_expired("opaque-legacy-token"), None);
    }

    #[test]
    fn legacy_non_jwt_token_is_never_flagged() {
        // No dots at all - a legacy Audiobookshelf `token` (pre-JWT auth) or any other
        // opaque string. This is what makes maybe_refresh_token a no-op for it.
        assert!(!is_token_expiring_soon("not-a-jwt-at-all"));
    }

    #[tokio::test]
    async fn maybe_refresh_token_is_a_no_op_without_a_refresh_token() {
        let mut token = fake_jwt_with_exp(chrono::Utc::now().timestamp() - 60);
        let mut refresh_token = String::new();
        let outcome = maybe_refresh_token(&mut token, &mut refresh_token, "someone", "http://unreachable.invalid").await;
        assert_eq!(outcome, RefreshOutcome::NotNeeded);
    }

    // Regression test for a real incident: a momentary network blip while refreshing
    // was indistinguishable from the server actually rejecting the refresh token, so
    // every caller (see RefreshOutcome::Failed's callers) deleted the user's whole
    // local account over a connection hiccup that resolved itself moments later.
    #[tokio::test]
    async fn maybe_refresh_token_reports_transient_error_when_the_server_is_unreachable() {
        let mut token = fake_jwt_with_exp(chrono::Utc::now().timestamp() - 60);
        let mut refresh_token = "some-refresh-token".to_string();
        // Port 1 is privileged/unbound - connection refused immediately, no DNS
        // dependency, deterministic and fast unlike a real timeout.
        let outcome = maybe_refresh_token(&mut token, &mut refresh_token, "someone", "http://127.0.0.1:1").await;
        assert_eq!(outcome, RefreshOutcome::TransientError);
    }

    #[tokio::test]
    async fn maybe_refresh_token_reports_failed_when_the_server_actually_rejects_it() {
        use tokio::io::AsyncWriteExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let _ = socket
                    .write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await;
            }
        });

        let mut token = fake_jwt_with_exp(chrono::Utc::now().timestamp() - 60);
        let mut refresh_token = "dead-refresh-token".to_string();
        let outcome = maybe_refresh_token(&mut token, &mut refresh_token, "someone", &format!("http://{addr}")).await;
        assert_eq!(outcome, RefreshOutcome::Failed);
    }
}
