# AGENTS.md

Cross-tool agent instructions for this repository. Applies regardless of which
AI coding tool is being used (Claude Code, Sparky/Muse, or any future agent).

## Agent dialog protocol

This file doubles as a shared communication channel between agents working on
this repo. Use it to coordinate across sessions and tools: say what you did,
what you verified, what you decided, and what's still open — so the next agent
doesn't have to rediscover it.

1. **Append-only.** Add new entries at the end under `## Dialog`, newest last.
   Never rewrite or delete another agent's entry. Correct a mistake with a
   follow-up entry, not an edit.
2. **One entry per work session** (or per distinct task). Short and factual.
3. **Every entry has five parts:** date, agent, what changed, how it was
   verified, open questions / handoff notes.
4. **Decisions with lasting effect** go under `## Standing decisions`, dated,
   with who decided. Check that section before starting work.
5. **No secrets.** Never paste tokens, passwords, or API keys here. If a task
   involved credentials, say only that it did and where they're stored.
6. **Keep it human-readable.** Paul reads this too. Plain language, no
   internal tooling jargon.

## Standing decisions

- 2026-09-26 (Paul): No destructive actions without explicit authorization —
  never delete libraries/items, users, files, or data; never change server
  settings. Applies to every agent, every session.
- 2026-09-26 (Paul): Nothing is committed or pushed until the work has been
  shown running and Paul explicitly authorizes it.
- 2026-09-26 (Paul, clarifying the above): the no-destructive-actions rule is
  about the Audiobookshelf server and real data — it does not gate normal
  absotui repo work. Commits, pushes, and merges here are ordinary workflow,
  not destructive actions.
- 2026-09-26 (Paul, amending the "shown running" rule): a live demo is not
  required before every commit. What's actually worked, across many sessions:
  `cargo build`/`clippy`/`test` clean before any push, always; a real
  live-test (tmux against a real server, an actual build/run of anything
  shipped like a packaged binary or CI artifact, etc.) for changes with real
  behavioral risk - UI/navigation behavior, anything touching playback, new
  release/CI steps; routine changes (docs, changelog, config, simple fixes
  already covered by the test suite) just need the verification above and an
  honest description of what was and wasn't tested. Push once Paul gives an
  explicit go-ahead in chat after seeing that description - his instruction,
  given with knowledge of what was verified, is the authorization. Don't wait
  on a recorded demo or Paul personally running it himself.

## Dialog

### 2026-09-26 — Sparky (Muse)

**What changed:** Reviewed the Continue Listening and podcast Home list
pipelines and fixed a real desync bug: titles, progress, and durations were
built as separate parallel lists, so any entry missing media or metadata
shifted every list after it out of alignment (wrong progress under wrong
title). Same protection applied to podcast Home. Also added bounds protection
for podcast episode descriptions, removed per-row whole-list
formatting/cloning (each row now formats only its own values), replaced
repeated filter+contains list rebuilds with indexed gathers, and removed the
dead `format_sizes` helper (its tests now cover `format_size`).

**Files:** `src/api/utils/collect_personalized_view.rs`,
`src/api/utils/collect_personalized_view_pod.rs`, `src/app.rs`,
`src/ui/tui.rs`, `src/utils/format_size.rs`.

**How it was verified:** `cargo build` clean; `cargo test` 63 passed, 0 failed;
`cargo clippy --all-targets` shows only 8 pre-existing `too_many_arguments`
warnings in untouched code. Ran the real binary against Paul's live
Audiobookshelf server: login worked, library loaded (167 items), Continue
Listening showed 3 in-progress books with correct titles/progress/timing,
podcast Home listed 10 episodes, podcast search found and listed 62 episodes
of a show with no crashes or row misalignment. Note: the missing-metadata
edge case itself wasn't directly reproduced (no such entries existed), and
the test populated progress via API rather than in-TUI playback.

**Open questions / handoff:**
- Login only succeeds when the server address ends with `/` (without it, the
  request times out). Worth a UX fix or at least auto-appending the slash.
- Without `ABSOTUI_SECRET_KEY` set, token storage fails with a confusing
  login error. Should surface a clear message.
- `R` does a full reinit back to Library instead of preserving Home; `/`
  from the episode view jumps to global podcast search rather than searching
  episode text. Confirm whether these are intended.
- The repo is not rustfmt-formatted (`cargo fmt --check` flags nearly every
  file, including untouched ones). New code matches surrounding style instead.
  Reformatting the whole repo is a separate decision for Paul.

### 2026-09-26 — Sparky (Muse)

**What changed:** Fixed the `.zsync` follow-up on issue #7. The file was
already being generated by appimagetool in CI (confirmed in the v0.9.3 build
logs: "generating zsync file → Success" for both arches), but it landed in
the working directory instead of `dist/`, so the upload step (`dist/*`)
silently left it behind. `release.yml` now moves any generated
`*.AppImage.zsync` into `dist/` after the appimagetool step. Also replied on
#7 thanking shuvashish76 for submitting absotui to the AM package manager.

**How it was verified:** YAML syntax validated; the added shell is guarded
(`[ -e ]` check) so it's a safe no-op if the file isn't where expected. Full
end-to-end verification isn't possible until the next release build runs -
the `.zsync` assets should appear on the release starting with the next cut.

**Open questions / handoff:** none - watch the next release's assets for the
two `.zsync` files.

### 2026-09-29 — Claude Code

**What changed:** AppImageHub's own discovery bot (`AppImage/appimage.github.io`
PR #6738, not something we submitted) ran its compatibility test against our
v0.9.3 `absotui-x86_64.AppImage` and it failed to run in their test
environment: `GLIBC_2.38'/2.39' not found`. Root cause: the x86_64 Linux
build runs natively on `ubuntu-latest` (Ubuntu 24.04, glibc 2.39) with no
compatibility layer, so the binary inherited that runner's very recent glibc
as a hard minimum - undercutting the whole "runs on basically any Linux"
pitch for the AppImage. Pinned that matrix entry to `ubuntu-22.04` (glibc
2.35) instead. aarch64 wasn't touched - it builds via `cross`'s own Docker
image regardless of host OS, so its glibc baseline isn't tied to the runner
label; the bot's report only flagged the x86_64 asset anyway.

**How it was verified:** YAML syntax validated. `ubuntu-22.04`'s glibc
version confirmed via GitHub's own `actions/runner-images` docs. The actual
result (does the new build really need an older glibc) can't be verified
locally - only provable on the next real release build, same situation the
`.zsync` fix above is in.

**Open questions / handoff:** watch the next release for two things: the
`.zsync` files (per the entry above) and whether AppImageHub's bot (or a
manual re-test) confirms the glibc requirement actually dropped. If PR #6738
is still open then, it may be worth manually re-triggering their compat test
or commenting with the fix status.

### 2026-10-01 — Claude Code

**What changed:** Root-caused and fixed a login bug related to Paul's Fedora
"Couldn't reach the server... HTTP 401" report from 2026-09-19 (v0.9.1
already made that screen explain *why* a login was rejected, but didn't stop
the login from being savable broken in the first place) - and the same gap
Sparky flagged in their 2026-09-26 handoff ("Without ABSOTUI_SECRET_KEY set,
token storage fails with a confusing login error"). In `auth_process.rs`, a
failed `encrypt_token()` call (almost always a missing/unreadable
`ABSOTUI_SECRET_KEY`) was logged with `println!` and swallowed - the login
proceeded anyway and wrote a user row with an *empty* token string, so the
login screen reported success. The next launch then 401'd against that empty
token and `diagnose_rejected_login` (from v0.9.1) reported it as "revoked /
wrong server," which isn't what actually happened and doesn't point at the
fix. Now: (1) `auth_process` returns `Err` immediately on either token's
encryption failure, before anything is written to the database, so a broken
login can no longer be silently saved; (2) `diagnose_rejected_login` special-
cases an empty saved token with its own message naming `ABSOTUI_SECRET_KEY`
directly, for installs already in this state from before the fix.

**Files:** `src/api/server/auth_process.rs`, `src/logic/server_recovery.rs`.

**How it was verified:** `cargo build`/`clippy`/`test` clean (63 passed).
Reproduced the actual reported symptom end-to-end against a scratch copy of
Paul's real config (`$XDG_CONFIG_HOME` override, never the real
`~/.config/absotui`): copied `config.toml`/`.env`/`db.sqlite3`, blanked the
saved user's `token` column directly in the copy, launched the real binary
against his real server - confirmed the old message ("deleted or revoked...
or belongs to a different server") was misleading for this case, then
confirmed the new one ("No login was actually saved... check
ABSOTUI_SECRET_KEY") after the fix. Separately confirmed `encrypt_token`
itself genuinely errors with no secret key set, via a throwaway test
(written, run, then reverted - not part of the diff) proving the exact
failure this `auth_process.rs` branch now catches. Did not drive a real
login through the TUI (no test credentials for Paul's server) - the
login-time fix is verified by code-path reading (confirmed the early
`return Err` sits before the only `db_insert_usr` call) plus the
`encrypt_token`-fails repro above, not a live login attempt.

**Open questions / handoff:** Committed and pushed on 2026-10-06 with
Paul's go-ahead (see the 2026-10-06 entries below). Update: Paul's Fedora box
turned out to be on 0.9.4 and working, so the original 401 report was never
traced to this bug - treat this as hardening that closes Sparky's
2026-09-26 handoff item, not as the confirmed cause of that report.

### 2026-10-06 — Claude Code

**What changed:** No code changes - this closes the verification gap left in
the 2026-10-01 entry above (the login-time fix had only been checked by
reading the code, not by driving a real login). Synced with origin first:
nothing new upstream since v0.9.4, and the three uncommitted files from
2026-10-01 (`auth_process.rs`, `server_recovery.rs`, this file) don't overlap
anything.

**How it was verified:** Drove the real login screen end to end, twice, against
a throwaway fake Audiobookshelf server on `127.0.0.1` (a ~40-line Python
script answering only `POST /login` and `GET /api/libraries`) with made-up
test credentials, a scratch `$XDG_CONFIG_HOME` containing only `config.toml`
(no `.env`), and `ABSOTUI_SECRET_KEY` unset. Paul's real server, real
credentials, and real `~/.config/absotui` were not involved.
- *Before* (the installed v0.9.4 binary, which lacks the fix): login looked
  successful, the `users` row was saved with a **0-length token**, the next
  request went out as `Bearer ` (empty) and got a 401, and the app showed the
  misleading "deleted or revoked / different server" screen. The only trace
  of the real problem was a `println!` flashing at the top of the terminal.
- *After* (fresh build with the fix): the login screen returned to a blank
  Server address prompt showing "ERROR: Couldn't save your login: No secret
  found in .env. Do this: ...", the `users` table had **0 rows**, and the
  server saw exactly one login and one libraries call.
Fake server, tmux sessions, and scratch files were removed afterwards.

**Open questions / handoff:** Committed and pushed with Paul's go-ahead
after one more check: with `ABSOTUI_SECRET_KEY` set, a login still saves a
real encrypted token and refresh token, and every later request carries the
correctly decrypted token (tested against the same fake local server). The
original Fedora report was never traced to this bug (Paul's box is on 0.9.4
and fine). Two small leftovers, not changed: the `.env` setup instructions
inside the error message render with odd indentation, because
`encrypt_token.rs`'s message string contains the source file's own leading
whitespace; and `.env` is only read once at startup, so after adding the key
the user has to restart Absotui - the message doesn't say so.

### 2026-10-06 — Claude Code (demo GIF)

**What changed:** Replaced `assets/demo.gif` with a new one made from Paul's
screen recording of the app running against the LibriVox library (browsing,
chapters, Stats, live playback). The README already points at
`assets/demo.gif`, so nothing else changed. New file: 1078x680, 12 fps,
87 s, 4.4 MB (the old one was 1126x750, 65 s, 37 MB). Edits made to the
recording: trimmed the ~2 s blank terminal at the start, dropped two
single-frame blank flashes (a screen clear when playback starts), and
blurred the username and the server address in the header - the two small
patches only, so the icons and "Connected as" stay readable. The blur is a
heavy mosaic-then-smooth (not a light Gaussian), so the text can't be
recovered from it. Issue #8's rule (public-domain library, username/server
redacted) is still met.

**How it was verified:** Checked every one of the 1045 GIF frames
programmatically: the blurred patches have no readable edges (strongest edge
11, versus ~150 for crisp text), "Connected as" and the icons are crisp in
all frames, and there are no blank frames. Looked at a contact sheet of the
first/last and key frames (covers, Stats, chapters, playback). Scanned the
whole recording first for any other screen showing private info - none; the
header is the only place the username/server appear. Committed locally with
Paul's go-ahead; not pushed yet - waiting on his go-ahead to push.

**Open questions / handoff:** The Stats page in the recording shows Paul's
real aggregate listening totals (hours, streak, days active, book/episode
counts) because stats come from his account, not the LibriVox library - the
previous demo showed the same kind of numbers. The recording stops scrolling
before the "Most Listened / Top Authors" panels, which could list real
titles. If Paul would rather not show the Stats numbers, that segment
(about 58-71 s into the GIF) can be cut or blurred.
