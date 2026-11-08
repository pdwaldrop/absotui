use reqwest::Client;
use std::sync::LazyLock;
use std::time::Duration;

// reqwest's default client has *no* request timeout at all, so a server that accepts a
// connection and then never answers leaves the calling task awaiting forever. That isn't
// hypothetical here: the playback-session close/sync calls run inside the spawned
// playback task, and `wait_prev_session_finished` blocks every future play attempt until
// that task signals it finished - so one hung request could wedge playback with no way
// out but restarting the app (see known_bugs.md `bug_id: bf10cd`). Same idea as the read
// timeouts on the player control sockets.

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

/// Client for ordinary API calls - JSON endpoints, cover images, ranged metadata reads.
/// Everything with a bounded, small response body.
///
/// One shared client (a `Client` clone is a cheap handle to the same connection pool), so
/// repeated calls reuse an open connection and its TLS session instead of paying a new
/// TCP + TLS handshake - and a fresh client's setup cost - on every request.
///
/// Falls back to a plain `Client::new()` if the builder somehow fails, since losing the
/// timeout is strictly better than failing the request outright - that's exactly the
/// behavior every call site had before this existed.
pub fn api_client() -> Client {
    API_CLIENT.clone()
}

/// Client for bulk downloads (offline playback copies), which legitimately run for
/// minutes on a large audiobook or a slow connection - a total-duration cap would abort
/// exactly the transfers it's meant to protect. Only the connect phase is bounded, so an
/// unreachable server still fails fast while a working-but-slow transfer runs to
/// completion. The per-read limit only trips when data stops arriving altogether, so a
/// stalled transfer fails (and resumes later) instead of hanging the download worker.
pub fn download_client() -> Client {
    Client::builder()
        .user_agent(USER_AGENT)
        .no_gzip()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(DOWNLOAD_STALL_TIMEOUT)
        .build()
        .unwrap_or_else(|_| Client::new())
}

/// Cap on in-flight HTTP requests when a startup/refresh phase fans out one request
/// per item (per-podcast episode lists, per-item progress lookups). Bounded so a large
/// library doesn't open an unbounded number of connections at once, or look like a
/// burst to the server - while still turning `n` sequential round-trips into roughly
/// `ceil(n / this)`.
///
/// Always fan out with `futures::stream::...buffered()`, never `buffer_unordered()`:
/// callers feed results straight into index-aligned parallel arrays, which silently
/// desync if results come back in completion order instead of request order.
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
