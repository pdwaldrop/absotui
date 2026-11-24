use reqwest::Client;
use std::sync::LazyLock;
use std::time::Duration;

// reqwest has no request timeout by default, so a server that never answers would hang the caller - in
// a playback task, that would block every later play.

// Generous enough not to abort a legitimately slow-but-working request (measured
// round-trips against a real server sit around 1-2s, so this is ~30x headroom, and a
// prematurely cancelled progress sync would lose real listening position) while still
// guaranteeing every call eventually returns instead of hanging forever.
const API_TIMEOUT: Duration = Duration::from_secs(60);

// Connecting is fast or not happening - an unreachable host should surface as an error
// promptly rather than sitting on the full request budget.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

const DOWNLOAD_STALL_TIMEOUT: Duration = Duration::from_secs(60);

// Idle connections are dropped well before a typical reverse proxy would close them
// itself, so a reused connection is never one the server has already hung up on.
const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Sent on every request; some proxies and firewalls refuse a client that sends none.
pub const USER_AGENT: &str = concat!("absotui/", env!("CARGO_PKG_VERSION"));

static API_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .user_agent(USER_AGENT)
        .timeout(API_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .pool_idle_timeout(POOL_IDLE_TIMEOUT)
        .build()
        .unwrap_or_else(|_| Client::new())
});

/// The shared client for API calls (small responses): reuses connections and TLS sessions across calls.
/// Falls back to `Client::new()` if building fails, losing only the timeout.
pub fn api_client() -> Client {
    API_CLIENT.clone()
}

/// Client for downloads, which can take minutes: only connecting is time-limited, plus a read limit
/// that trips only when data stops arriving, so a stalled transfer fails and resumes later.
pub fn download_client() -> Client {
    Client::builder()
        .user_agent(USER_AGENT)
        .no_gzip()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(DOWNLOAD_STALL_TIMEOUT)
        .build()
        .unwrap_or_else(|_| Client::new())
}

/// Cap on concurrent requests when fanning out one per item. Always use `buffered()`, never
/// `buffer_unordered()`: callers rely on request order for their parallel arrays.
pub const MAX_CONCURRENT_REQUESTS: usize = 12;

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn request_seen_by_server<F, Fut>(send: F) -> String
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap();
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").await.unwrap();
            String::from_utf8_lossy(&buf[..n]).to_lowercase()
        });
        send(base).await;
        server.await.unwrap()
    }

    #[tokio::test]
    async fn api_client_asks_for_gzip_and_identifies_itself() {
        let req = request_seen_by_server(|base| async move {
            let _ = api_client().get(format!("{base}/x")).send().await;
        }).await;
        assert!(req.contains("accept-encoding: gzip"), "{req}");
        assert!(req.contains(&format!("user-agent: {}", USER_AGENT.to_lowercase())), "{req}");
    }

    #[tokio::test]
    async fn download_client_never_asks_for_compression() {
        let req = request_seen_by_server(|base| async move {
            let _ = download_client().get(format!("{base}/x")).send().await;
        }).await;
        assert!(!req.contains("accept-encoding"), "{req}");
        assert!(req.contains("user-agent: absotui/"), "{req}");
    }

    #[tokio::test]
    async fn embedded_cover_read_asks_for_identity_encoding() {
        let req = request_seen_by_server(|base| async move {
            let _ = crate::api::library_items::get_audio_file_range::get_audio_file_prefix("t", "item", "1", &base).await;
        }).await;
        assert!(req.contains("range: bytes=0-"), "{req}");
        assert!(req.contains("accept-encoding: identity"), "{req}");
        assert!(!req.contains("gzip"), "{req}");
    }
}
