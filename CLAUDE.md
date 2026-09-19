# Claude Code instructions

@AGENTS.md

## Code comments

Do not litter code with comments. Comment only when it explains something that is not immediately apparent from the code, and keep every comment short and concise.

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Absotui is a TUI (terminal UI) client for Audiobookshelf, written in Rust with `ratatui`, currently in beta, maintained solo. See `README.md` for the feature list and `known_bugs.md` for known issues.

## Commands

```bash
cargo build              # debug build
cargo run                # run against the local dev config (see "Runtime config" below)
cargo run --release      # release build/run - what real installs use
cargo test                # unit tests (a handful exist, e.g. src/utils/convert_seconds.rs, src/utils/encrypt_token.rs, src/api/me/get_media_progress.rs)
cargo test <name>         # run a single test by name
cargo clippy --all-targets
```

There is no CI test/lint workflow (`.github/workflows/` only has `release.yml`, which builds and uploads binaries on a GitHub release) - `cargo build`/`clippy`/`test` are not run automatically, so run them yourself before committing.

### Actually exercising the app

`cargo build`/`clippy`/`test` verify compilation, not behavior. This app talks to a real Audiobookshelf server and shells out to a real `vlc`/`cvlc` binary - there's no mock server or fixture data. To manually verify a change, run `cargo run` in a `tmux` session (so you can send keys and capture panes non-interactively) against a real configured server; see git log for prior sessions doing this. Requires `~/.config/absotui/config.toml` (or `~/Library/Preferences/absotui/config.toml` on macOS) to already exist with real credentials - there's no way to run the app meaningfully without one.

### Cutting a release

1. Add a new entry to `src/utils/changelog.rs` (in-app changelog, shown in Settings). The *current* entry uses `format!(... v{VERSION} ...)` with `CARGO_PKG_VERSION` baked in dynamically - freeze that one to a plain string and add a new dynamic entry for the version being released. Commit and push to `main`. This part is still manual on purpose - it's real user-facing prose, not something to generate.
2. Run the `Cut Release` workflow (`.github/workflows/cut-release.yml`, `workflow_dispatch`) with a `bump` choice (patch/minor/major) and a one-line `summary`: `gh workflow run cut-release.yml -f bump=patch -f summary="..."`, or via the Actions tab. It computes the next version from `Cargo.toml` (dropping any `-beta`-style suffix - versions are plain `X.Y.Z` now), bumps `Cargo.toml`/`Cargo.lock`, commits, tags, pushes to `main`, fast-forwards `stable` to match (`hello_absotui.sh` installs from `stable`, not `main`), and creates the GitHub release.
3. Creating the release fires the existing `release.yml`, which builds Linux (aarch64/x86_64) and macOS (universal) binaries and attaches them along with a `SHA256SUMS.txt` generated from the actual uploaded assets (so the install script never has stale hardcoded checksums). It fires twice per release (both the `created` and `published` events match its trigger) - expected, not a bug.

`hello_absotui.sh`, `config.example.toml`, and `linux/absotui.{desktop,svg}` are served live from `stable` but verified against the latest release's frozen `SHA256SUMS.txt`, so a push to `stable` that touches any of them between releases would break every install/update ("Incorrect shasum"). `.github/workflows/sync-stable-assets.yml` prevents that by re-uploading the changed file and rewriting its manifest line whenever `stable` changes one of them - don't bypass it by editing release assets by hand.
4. `gh run watch <id> --exit-status` on the resulting run(s) to confirm the binaries actually landed (`gh release view vX.Y.Z --json assets`).

## Architecture

### Entry point and main loop (`src/main.rs`)

Reads `.env` (for `ABSOTUI_SECRET_KEY`, used by `magic-crypt` to encrypt the stored auth token) from the platform config dir, opens the sqlite DB, and loops running the `AppLogin` TUI until a default user exists. Once logged in, builds the main `App` (this does the initial round of API calls) and enters a manual render loop: `terminal.draw` every iteration, poll for a keypress (200ms), dispatch to `App::handle_key`, and periodically call `refresh_podcast_home_if_stale()`. Pressing `R`, or switching libraries in Settings, fully reconstructs `App` via `App::new()` rather than mutating it in place - that's the only way most cross-cutting state gets refreshed.

If `is_vlc_running` (an sqlite flag, not part of `App`) is set, the loop renders `render_player` (`src/ui/player_tui.rs`) instead of the normal `App` widget - the player overlay and the main app are two independent render paths gated on that flag, not a variant of the same view.

### `App` (`src/app.rs`) - one large struct of parallel arrays

`App` is a single flat struct holding essentially all UI state - Continue Listening, Library, podcast episode list, search, settings, etc. Books and podcasts each get their own **parallel arrays** rather than a `Vec<Book>`/`Vec<Episode>`: e.g. for the podcast Home ("New & Unfinished") list, `_ids_cnt_list[i]`, `ids_ep_cnt_list[i]`, `subtitles_pod_cnt_list[i]`, `podcast_published_at_cnt_list[i]`, etc. all describe the same row `i`.

This is the most important thing to know before touching Home/Library list code: whenever the list is fetched, sorted, or reordered (see `reorder_podcast_lists`, `fetch_podcast_home_data`, `refresh_podcast_home_if_stale`), *every one* of these arrays must be permuted/populated together or they silently desync - symptoms range from wrong titles next to wrong progress bars to the selection cursor appearing to jump to a different row on its own (this happened for real - periodic refresh replaced the arrays without re-finding the selected row's new index). If you add a new per-row field, thread it through: the struct field, `PodcastHomeData`, `fetch_podcast_home_data`'s collection + sort, the initial-load site in `App::new`, `refresh_podcast_home_if_stale`, and `reorder_podcast_lists`.

Rendering is `impl Widget for &mut App` in `src/ui/tui.rs`, dispatching on the `AppView` enum to one `render_*` method per screen.

### `src/api/` - typed REST client

Thin `reqwest` + `serde` wrappers around the Audiobookshelf API, one file per endpoint, grouped by resource (`libraries/`, `library_items/`, `me/`, `sessions/`, `server/`). Each endpoint defines its **own** response structs (`Root`, `Episode`, `AudioFile`, etc.) matching that specific endpoint's JSON shape, even when another endpoint returns conceptually the same entity - e.g. `get_pod_ep.rs` and `get_library_perso_view_pod.rs` each have a separate `Episode`/`AudioFile` definition, because the two endpoints' JSON shapes differ. Don't assume structs are shared across endpoint files.

`src/api/utils/collect_*.rs` are the adapters between raw API responses (`Vec<Root>`) and the parallel-array shape `App` wants - most new per-row data starts as a new `collect_*` function here.

### `src/db/` - sqlite as settings/state store, not a relational model

`rusqlite` (bundled), used for durable per-user settings (one row per user, columns like `is_podcast_autoplay`, `speed_rate`) and the single current `listening_session` row - not a general data model. Every function in `crud.rs` opens its own connection and runs synchronously (blocking) even though it's called from async code; there's no `spawn_blocking` wrapping it.

### `src/player/` - VLC via its RC protocol, not a library binding

`player/vlc/` shells out to launch `cvlc`/`vlc` with `--extraintf rc --rc-host <addr>:<port>`, then talks to it over that raw line-based TCP "remote control" protocol (`utils/vlc_tcp_stream.rs`) - not JSON, not the `vlc-rc` crate's own transport. `player/integrated/` builds higher-level playback state (position, current chapter, speed) on top of that.

Playback itself is **not** driven by the main render loop: `src/logic/handle_input/handle_l_*.rs` (one per context - book, podcast episode, podcast home) is what actually starts it, spawning a detached `tokio::spawn` task that owns the whole session (launch VLC, poll it, sync progress to the Audiobookshelf server and to the sqlite `listening_session` row, handle podcast autoplay-to-next-episode, clean up on quit). That task never touches the live `App` struct directly - it communicates purely through sqlite and the log file, which is why a full `App::new()` reinit (via `R` or the periodic podcast refresh) is how the UI picks up what the playback task has been doing.

### Runtime config and data

Platform config dir: `~/.config/absotui/` (Linux) or `~/Library/Preferences/absotui/` (macOS), overridable via `XDG_CONFIG_HOME`. Contains `config.toml` (copied from `config.example.toml` at install time - player/VLC connection and download settings; the UI always renders using the terminal's own color theme rather than any configurable palette), `.env` (`ABSOTUI_SECRET_KEY`), `db.sqlite3`, `absotui.log`, and `covers/` (on-disk cover art cache, background-fetched and picked up by polling for the cached file's existence rather than a channel).
