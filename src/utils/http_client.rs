use reqwest::Client;
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

/// Client for ordinary API calls - JSON endpoints, cover images, ranged metadata reads.
/// Everything with a bounded, small response body.
///
/// Falls back to a plain `Client::new()` if the builder somehow fails, since losing the
/// timeout is strictly better than failing the request outright - that's exactly the
/// behavior every call site had before this existed.
pub fn api_client() -> Client {
    Client::builder()
        .timeout(API_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| Client::new())
}

/// Client for bulk downloads (offline playback copies), which legitimately run for
/// minutes on a large audiobook or a slow connection - a total-duration cap would abort
/// exactly the transfers it's meant to protect. Only the connect phase is bounded, so an
/// unreachable server still fails fast while a working-but-slow transfer runs to
/// completion. The per-read limit only trips when data stops arriving altogether, so a
/// stalled transfer fails (and resumes later) instead of hanging the download worker.
pub fn download_client() -> Client {
    Client::builder()
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
pub const MAX_CONCURRENT_REQUESTS: usize = 8;
