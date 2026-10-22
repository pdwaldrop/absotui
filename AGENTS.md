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

### 2026-10-06 — Claude Code (v0.9.5 release + README accuracy)

**What changed:** Cut v0.9.5 (patch). It contains the login hardening
(`c5d860b`: a login now fails with a clear message instead of silently saving
an empty token when `ABSOTUI_SECRET_KEY` is missing) and the new, smaller demo
GIF (`230309d`); the in-app changelog got a v0.9.5 entry and v0.9.4's was
frozen. Then corrected the README: (1) the Project status line claimed the API
is "only ever used to retrieve books/podcasts and sync playback progress",
which stopped being true in v0.9.0 - Absotui can now create podcast
subscriptions, queue episode downloads on the server, and delete library items
(a "hard" remove deletes the show's files on the server); the line now says so
and notes it needs the right Audiobookshelf permissions. (2) The AppImage
section now says VLC must be installed (the AppImage only packages the
`absotui` binary). (3) The podcast feature bullet now mentions keeping or
deleting downloaded files.

**How it was verified:** For the release: `cargo build`/`clippy`/`test` clean
(63 passed; only the 8 existing `too_many_arguments` warnings); the release
workflow and binary build succeeded; all 12 assets are present including both
AppImages and both `.zsync` files; `SHA256SUMS.txt` matches the live `stable`
files (`hello_absotui.sh`, `config.example.toml`, `absotui.desktop`,
`absotui.svg`) and the x86_64 tarball; the released binary reports 0.9.5 and
contains the login fix and the new changelog text. The v0.9.4 handoff is
closed: the x86_64 build's real glibc requirement (read from the ELF version
table, not a text search - the v0.9.4 changelog text itself contains
"GLIBC_2.39") is now 2.34, down from 2.39 on v0.9.3, and AppImageHub's PR
#6738 was merged with a passing test. README claims were checked against the
code (the list of write calls the app makes to the server) and the release
assets. Not tested: running the AppImage on a machine without FUSE.

**Open questions / handoff:** GitHub moves the `ubuntu-latest` label to
Ubuntu 26 on October 19, 2026 - the release and checksum workflows use it
(the x86_64 build is pinned to 22.04 and is unaffected), so watch the next
release after that date. The README's top "Roadmap" link may not match
GitHub's generated anchor (the heading's emoji carries an invisible
character); couldn't verify without GitHub's page. The "Recent work" line in
the Roadmap only mentions the AppImage - left as is. These docs commits are on
`main` only; `stable` (what the installer fetches) stays at the v0.9.5 bump
commit until the next release, which is fine since no installed file changed.

### 2026-10-13 — Claude Code (SSO login, issue #11)

**What changed:** Built SSO (OpenID Connect) login for issue #11 - a user whose
Audiobookshelf only allows sign-in through authentik can't use Absotui at all
today. It follows Audiobookshelf's native-app flow, worked out by reading the
server's own `Auth.js` and `OidcAuthStrategy.js` (the public docs don't
describe it): Absotui makes a PKCE request to `/auth/openid` itself (the server
keeps the PKCE state in a session cookie that the final code exchange has to
send back), opens the returned identity-provider link in the browser, catches
the redirect on `http://127.0.0.1:41729/callback`, then swaps the code for the
same tokens a password login returns. The login screen now checks the server's
public `/status` after the address is entered: SSO-only servers skip the
username/password prompts; servers offering both show "Username (leave empty
to sign in with SSO)"; anything else, or a failed check, behaves exactly as
before. If the browser can't reach this machine (SSH), a paste box accepts the
address the browser ended on. Everything after "we have tokens" was pulled out
of `auth_process` into `finish_login` so both paths share it. New optional
`[auth] sso_callback_port` in `config.toml` (missing section = default).
README documents it, including the one-time admin step: the loopback address
has to be added under Settings > Authentication > Allowed Mobile Redirect URIs
(Audiobookshelf matches it exactly, hence the fixed port).

**Files:** `src/api/server/oidc_login.rs` (new), `src/api/server/auth_process.rs`,
`src/logic/auth/auth_input.rs`, `src/config.rs`, `src/api/server/mod.rs`,
`Cargo.toml`/`Cargo.lock` (`sha2`, `getrandom` - both already in the tree),
`config.example.toml`, `README.md`.

**How it was verified:** `cargo build`/`clippy`/`test` clean (82 passed: 63
existing, 16 for the SSO module including the RFC 7636 PKCE test vector, 3 for
config; clippy shows only the 8 existing `too_many_arguments` warnings).
End to end against a fake Audiobookshelf + identity provider I wrote from the
server source (scratch `XDG_CONFIG_HOME`, a stub `xdg-open` so no real browser
opened; Paul's real server and config were never touched - the only thing sent
to the real server was a read-only GET of its public `/status`, to confirm its
shape). The fake checks the redirect whitelist, the session cookie, the state
lookup, and that the PKCE verifier matches the challenge. Passed: SSO-only
server (no prompts, user saved with the username from the identity provider
and both tokens encrypted); server offering both (prompt text changes,
password login still works, empty username starts SSO); redirect not
whitelisted (message names the exact address to add, nothing saved); browser
that can't reach the machine (junk paste gets a hint, the real address
finishes the login); Esc (back to the prompt, port released, retry works);
callback port already in use; server rejecting the sign-in; a forged request
with the wrong `state` and stray requests to the callback port (ignored, the
wait continues, the real sign-in still completes); password-only server
(prompts unchanged). **Not verified:** a real Audiobookshelf with a real
identity provider (this user can't reach the Docker socket, so couldn't stand
one up) - the fake is only as faithful as my reading of the server code;
macOS; a real browser; refreshing an SSO-issued token later (it should use the
existing refresh path, but nothing tested it).

**Open questions / handoff:** Committed and pushed with Paul's go-ahead the
same day, then released (see the release entry below). Issue #11 has no reply
yet - Paul approves public replies separately. Best next check is the person
who asked: they use authentik and could try the release.
Default port 41729 was picked arbitrarily; changing it later means every admin
who already allowed the old address has to add the new one. The README's
"Project status" line still says the only server changes are podcast
subscriptions - still true, SSO only creates a normal login session.

### 2026-10-13 — Claude Code (v0.10.0 release)

**What changed:** Cut v0.10.0 (minor - SSO login is a new user-facing feature,
same call as v0.9.0's podcast management; the in-app update check and the
installer compare versions by equality only, so a two-digit minor is safe). The
SSO work from the entry above was committed and pushed with Paul's go-ahead,
followed by a v0.10.0 changelog entry (v0.9.5's was frozen first). No code
changed after the SSO commit.

**How it was verified:** Release workflow and binary build both succeeded; all
12 assets are on the release (3 tarballs, 2 AppImages, 2 `.zsync`, desktop file,
svg, `config.example.toml`, `hello_absotui.sh`, `SHA256SUMS.txt`). Downloaded the
release files and checked them against `SHA256SUMS.txt` (all match); the
released binary and the AppImage both report 0.10.0 and the binary contains the
SSO code; its highest glibc requirement is still 2.34 (read from the ELF table,
so the v0.9.4 older-systems fix is intact). `config.example.toml` changed in this
release, and the copies served live from `stable` (`config.example.toml`,
`hello_absotui.sh`, `absotui.desktop`, `absotui.svg`) are byte-identical to the
release assets, so installs and updates won't hit "Incorrect shasum".

**Open questions / handoff:** The reply on issue #11 is still unposted -
waiting on Paul's approval of the wording; it should ask whether the reporter
is their server's admin (they have to allow `http://127.0.0.1:41729/callback`)
and whether they run Absotui over SSH. The real test is the reporter trying
v0.10.0 against authentik. If that surfaces a mismatch between my fake server
and the real one, the fake in the conversation scratch space is not kept - read
`Auth.js`/`OidcAuthStrategy.js` again. Still on the list from earlier entries:
`ubuntu-latest` becomes Ubuntu 26 on 2026-10-19, so watch the first release cut
after that date.

### 2026-10-20 — Claude Code (PR #12 merged, player choice in Settings + installer)

**What changed:** Reviewed and merged PR #12 (Crashdummyy's mpv backend; thanked
them on the PR). Then, on top of it, with Paul's requirements that nobody should
have to hand-edit `config.toml` and that the installer should offer the choice:
(1) a new **Settings > Player** screen (VLC / mpv) that saves the choice as the
`backend` line under `[player]` in `config.toml` and switches the running app to
it; it refuses while something is playing and when the chosen program isn't
installed, and takes effect on the next play. The choice lives in `config.toml`
(not the sqlite settings) on purpose: it's a per-machine fact, and the installer
already keeps the user's value across updates. (2) The install script now asks
"VLC or mpv" (default VLC; skipped with `ABSOTUI_PLAYER=vlc|mpv` or when there's no
terminal), installs that package, writes `backend`, and on `--update` keeps
installing the player the existing config names instead of pulling VLC back in.
(3) The PR's `Player::quit` had widened the macOS-only `pkill VLC` to two places it
never ran before (the startup leftover-session check and the stop key); those now
use a new `quit_gracefully` without it. (4) CLAUDE.md's player section rewritten
for the `Player` enum, README and `config.example.toml` updated. (5) mpv failures
are no longer silent: an mpv control-socket path over the OS limit (108 bytes on
Linux, 104 on macOS) is refused up front with a message naming the length and the fix
(before this, mpv started anyway with no control socket and would have played on
unreachable), and any player that exits with a failure status now logs the status and
the tail of its stderr (e.g. "exited abnormally (exit status: 2): ... couldn't open
play stream" when mpv has no audio output). (6) The shared one-line status message
helper (`pop_message`, 43 call sites) now clears its row before printing: it writes
straight to the terminal, so a shorter message used to land on top of the leftover
tail of a longer one ("...change the playern try again"). Reproduced before the fix
and gone after; checked the rows it uses (3, 2 and 6 from the bottom) sit in blank
space on the Settings screens and the login screen, so clearing the row hides no UI.

**How it was verified:** `cargo build`/`clippy`/`test` clean (93 passed: 82 plus 11
new - rewriting the `backend` line, the socket-path limit, and the stderr summary; clippy shows only the existing
`too_many_arguments` warnings). Live, against a throwaway fake Audiobookshelf on
127.0.0.1 with its own config dir, VLC port and mpv socket (Paul's real config, server,
VLC port 1234 and running session were not touched - his db showed a live session, so
a copy of it was deliberately not used): the Player screen lists and saves the
choice with only that one line changed, the choice survives a restart and is
preselected, switching is refused with a clear message when the program isn't on
PATH and while the running flag is set, and a real switch mpv -> VLC without
restarting made the server see `mediaPlayer=mpv v0.41.0` and then `VLC v3.0.24` for
consecutive plays (mpv's stop key also verified). Failure logging, live: a 126-byte
socket path was refused in ~0.1 s with the clear message and no mpv left running; mpv
with no audio server logged exit status 2 plus the audio error; a normal play + stop
logged no false warning on either mpv or VLC. Installer: its new functions were
run in a harness with package installs stubbed and a scratch config dir (default,
env override, bad env value, reinstall, update-keeps-mpv, old config without the key,
and the prompt on a real tty with Enter / 1 / 2 / invalid input).

**Not verified:** a real package install through the script (apt/dnf/pacman/brew),
mpv's package name and availability on Fedora/CentOS/openSUSE/Homebrew, anything on
macOS (including the `pkill` scoping and `sed -i` via gsed), and the update path
end to end. The Settings > Player screen was not checked on a narrow terminal.

**Open questions / handoff:** Nothing here is committed yet - waiting on Paul's
go-ahead. No changelog entry was added; that belongs to the release that ships
this. `hello_absotui.sh` and `config.example.toml` changed, so the existing
stable-sync workflow will pick them up at the next release as usual. Still true for
any player: if it starts but never becomes reachable for another reason, Absotui can't
stop it (it never gets a handle on the process) - the socket-path check only closes the
one known cause. Honest framing of mpv for any
docs: it is an alternative, not a proven upgrade (measured: a cleaner control
channel and a ~0.5 s faster first position; its full dependency tree on Arch was
larger than VLC's, not smaller).

### 2026-10-20 — Claude Code (v0.11.0 release)

**What changed:** Cut v0.11.0 (minor - a new user-facing feature, same call as
v0.9.0 and v0.10.0): the player choice (VLC / mpv) in Settings > Player and in the
install script, on top of Crashdummyy's mpv backend (PR #12), plus the failure
logging, the `pkill VLC` scoping, and the status-message fix from the entry above.
The v0.10.0 changelog entry was frozen and a v0.11.0 one added (it credits
Crashdummyy) before cutting.

**How it was verified:** The Cut Release and Release workflows both succeeded and
all three binary builds plus the checksum job passed; the release has all 12 assets
(3 tarballs, 2 AppImages, 2 `.zsync`, desktop file, svg, `config.example.toml`,
`hello_absotui.sh`, `SHA256SUMS.txt`). Downloaded every file and checked it against
`SHA256SUMS.txt` (all match). The released x86_64 binary and the AppImage both report
0.11.0, and the binary contains the new code (Settings > Player text, the socket-path
check, the abnormal-exit log, the new changelog). Highest glibc requirement, read from
the ELF table, is still 2.34 on x86_64 (2.18 on aarch64). The macOS tarball is a
universal binary. The files the installer fetches live from `stable`
(`hello_absotui.sh`, `config.example.toml`, `absotui.desktop`, `absotui.svg`) are
byte-identical to the release assets, and `main`, `stable` and the tag all sit on the
bump commit. The `ubuntu-latest` jobs (cut-release and the aarch64 build) ran on
Ubuntu 24.04, so GitHub's move to 26 has not reached this repo yet and still needs
watching on a later release.

**Not verified:** running the install script for real (the new player prompt, a real
package install, and an `--update` from 0.10.0) on any machine; mpv's package name
on Fedora/CentOS/openSUSE/Homebrew; anything on macOS; the aarch64 binary was only
inspected, not run.

**Open questions / handoff:** First thing to do: run the installer on the Fedora
test box, once with VLC and once with `ABSOTUI_PLAYER=mpv`, then `--update` from an
older install. This entry is the only commit after the release bump, so `stable`
stays on the bump commit, which is fine (no installed file changed).

### 2026-10-20 — Claude Code (config merge bug in the install script's update, found after v0.11.0)

**What happened:** Right after Paul ran `absotui --update` to v0.11.0 on his main
desktop, Absotui wouldn't start: "TOML parse error at line 30 ... duplicate key" on
`[auth]` in `config.toml`. His file had the `[auth]` block twice.

**Root cause (pre-existing since v0.10.0, not caused by the player work):** the
installer merges new options into the user's `config.toml` on every update, and it
finds a section's end with the regex `^\[[^($section)]`. That's a character class,
so any header whose first letter appears in the section's name (e.g. `[auth]` after
`[player]`) doesn't end the section. The first update after v0.10.0 is fine, but it
drops the `#### SIGN-IN ####` comment line that had been acting as the boundary; the
second update then pulls `[auth]` into the `[player]` section and writes it twice.
Anyone who updated through the script twice since v0.10.0 (including 0.9.x -> 0.10.0
-> 0.11.0) gets this, and Absotui can't start. Reproduced exactly with Paul's
pre-update file, and from the shipped example config updated twice.

**What changed (uncommitted at the time of writing):** (1) `hello_absotui.sh`: a
section now ends at the next `####` line or the next `[header]` that isn't its own.
(2) A safety net: if the merged result would repeat any `[section]`, the installer
prints a warning and leaves the user's config exactly as it was (missing options fall
back to defaults, so that is always safe). (3) v0.11.1 changelog entry prepared.

**How it was verified:** In a harness that sources the script with network and
package installs stubbed, against scratch config dirs: Paul's reconstructed pre-update
file and the shipped example updated 1, 2 and 3 times (no duplicates, and update 2 ==
update 3); a copy of his currently broken file is repaired by one run; custom values
(player port/address/backend, sso port, auto-download count, colors) survive two
updates; the player-choice cases still pass; and with the old boundary re-injected
the safety net refuses to write and leaves the file byte-for-byte unchanged. Output was
parsed with a real TOML parser (`tomllib`), not just grepped. `cargo build`/`test`/
`clippy` unchanged and clean (93 passed).

**Own gap:** my v0.11.0 installer checks ran `install_config` once and only looked
at the `backend` line, so a second update (which would have shown the duplicate
header) was never exercised. The bug predates my change, but the check that would
have caught it was mine to run.

**Open questions / handoff:** The fix only reaches users via a release, because
`hello_absotui.sh` is served from `stable`. Affected users can repair themselves with
`absotui --update` once it's out (the `--update` flag is handled before the config is
read, so it works even with the broken file). Until then, deleting the second
`[auth]` block from `config.toml` fixes it by hand. Paul's own `config.toml` has not
been touched by this session.

### 2026-10-20 — Claude Code (v0.11.1 release)

**What changed:** Cut v0.11.1 (patch) with the fix for the install script's config
merge writing `[auth]` twice on the second update (see the entry above). Fix and the
v0.11.1 changelog entry were committed and pushed with Paul's go-ahead before cutting.

**How it was verified:** Cut Release and Release workflows succeeded (all three
binary builds and the checksum job); all 12 assets are on the release and every file
matches `SHA256SUMS.txt`. The x86_64 binary and AppImage report 0.11.1, both
binaries carry the new changelog text, glibc floor unchanged (2.34 x86_64, 2.18
aarch64), macOS tarball is a universal binary. `main`, `stable` and the tag all sit on
the bump commit; the four installer-fetched files served from `stable` are
byte-identical to the release assets, and the served `hello_absotui.sh` contains the
fixed section boundary and the duplicate-section guard. End to end with the released
script: its config step, run on a copy of Paul's actual broken `config.toml` (which
failed to parse with "Cannot declare ('auth',) twice"), produced a file with one
`[auth]` that parses, and two more updates left it unchanged; a brand-new example
config updated twice also stays clean. The `ubuntu-latest` jobs again ran on Ubuntu
24.04.

**Not verified:** a full `absotui --update` run (the real binary download and package
steps) on any machine; macOS; the aarch64 binary and AppImage were not run.

**Open questions / handoff:** Paul's own `~/.config/absotui/config.toml` still has the
duplicate `[auth]` block as of this check (this session was blocked from editing it).
Running `absotui --update` (handled before the config is read) should now repair it, or
delete the second `[auth]` block by hand. Anyone else affected can do the same. Consider
telling users who updated via the script twice since v0.10.0.

### 2026-10-21 — Claude Code (clock-skew handling for saved logins, v0.11.2)

**What happened:** After Paul updated his Fedora test box, Absotui showed "Your saved
login isn't working ... even though it hasn't expired". Diagnosed from his output: that
box's system clock was about an hour slow (`date -u` 03:38 vs real ~04:4x; `System clock
synchronized: no`). Absotui decides whether to refresh a token by comparing its `exp`
with the LOCAL clock (Audiobookshelf access tokens last 1 h), so with a clock an hour slow
a token the server had already expired still looked valid for ~55 more minutes: no
refresh, the server answered 401, and the screen blamed the login. It clears by itself,
which is why the same box's Sept 19 401 vanished. The startup token code was identical to
v0.9.4, so the 0.11.x update did not cause it.

**What changed:** (1) `sync_clock_with_server` (refresh_token.rs) reads the `Date` header
of the server's public `/ping` at startup and stores the offset; every "expired / about
to expire" check (startup, the render loop's refresh, the playback tasks') now uses the
server's time. (2) At startup, a 401 on the first API call now triggers one forced
refresh (`refresh_now`) and one retry before any error is shown; any other refresh
outcome falls through to the old diagnosis (no account deletion). (3) When the offset is
>= 2 minutes the error screen leads with "This computer's clock is about N behind/ahead
of the server's ... Fix the clock ... then press Retry".

**How it was verified:** 96 unit tests (new: expiry judged on the server's clock, offset
from a Date header, the skew wording) plus clippy and build clean. Live against a fake
Audiobookshelf whose clock runs 65 min ahead and whose login tokens last 30 s (a faithful
copy of the Fedora situation; Paul's real server and config untouched - its `/ping` was
only checked for a `Date` header): the old v0.11.1 binary reproduced the exact Fedora
screen with a 401; the new binary on the same saved state measured +3900 s, refreshed
once and started with zero 401s; with no clock problem but a server that rejects an
unexpired token, the retry recovered (1 401, 1 refresh); with the clock off and the login
genuinely dead, the screen led with the clock note and stayed bounded (2 refreshes, 2
401s, no loop); a normal run did no refresh and logged no clock warning.

**Not verified:** a real Audiobookshelf with a skewed client clock (only the fake); the
mid-session path (the offset is a process-wide value measured at startup and on `R`, so
the render loop and playback tasks use it, but I did not run a long playback session with
a skewed clock); a server whose `/ping` is missing or blocked (then the offset stays 0 and
behaviour is exactly as before); Fedora/macOS.

**Open questions / handoff:** The Fedora box still has a slow clock until Paul fixes NTP
(`sudo chronyc makestep`, then check `timedatectl`/`chronyc tracking`); the app now copes,
but other things on that machine will keep drifting. The existing `RefreshOutcome::Failed`
path still deletes the local account on ANY non-success refresh response (including a 5xx
from a proxy) - separate pre-existing risk, not changed here.

### 2026-10-21 — Claude Code (performance check of v0.11.2, no code changes)

**What changed:** Nothing in the repo - Paul asked whether recent updates had hurt
speed, so this was measurement only. Answer: no. Startup, memory (~21 MB), idle CPU
and key response are the same on v0.9.5, v0.10.0 and v0.11.2. What it did find are
long-standing costs, ranked by value:
1. *Auto Download repeats itself and buffers whole files in RAM.* The podcast version
   is re-started on every ~8 s home refresh with no "already running" check, so the same
   episodes download again and again (Paul's real log: 896 finished downloads of only 132
   different episodes, about 64 GB moved for 7.6 GB of actual need; roughly a third of
   attempts fail and restart from zero). Each download is held fully in memory first
   (a 60 MB file peaks near 400 MB of RAM). Same behavior on v0.9.5 and v0.11.2; the code
   dates from late July.
2. *The main loop sleeps a fixed 50 ms between handling a key and drawing it.* Removing
   it (prototype): key to screen 58 -> 13 ms, and 30 queued keys go from 8-10 to ~35 per
   second.
3. *Playback start looks slow because the UI checks the player once a second while it
   loads.* VLC starts fetching audio ~0.2 s after the key, but the player bar only shows
   a moving position at ~2.5 s. A 150 ms check while loading (one line in
   `src/player/session.rs`, prototype) brings that to ~1.45 s.
4. The podcast home refresh runs inside the draw loop (~0.6 s input freeze every ~8.7 s on
   a 110 ms server) and logs one INFO line per episode each time (the 50 MB log is 88% that).
5. A new `reqwest` client is built for every request (33 places); sharing one would save
   a connection handshake each time.
6. Smaller: new covers appear ~375 ms after selecting (the loop only notices a finished
   fetch on its next 200 ms tick); very large cover images (>1400 px) stall the UI while
   decoding (Paul's own covers are all under 42 KB, so not an issue for him).

**How it was verified:** Against a fake Audiobookshelf on 127.0.0.1 with a scratch config
(Paul's real server, config and listening session were not touched; the only real-data read
was a listing of his cover cache sizes and his existing log). Release binaries of the three
versions were checksum-verified and compared in alternating runs. Timing used a bare
pseudo-terminal that answers the terminal-capability query the way Ghostty does. The fixed
50 ms sleep and the 1 s poll were prototyped in a throwaway worktree (since deleted) and
re-measured; nothing was committed. The machine is busy and on a power-saving CPU setting,
so absolute milliseconds are inflated - only same-moment comparisons are trusted.

**Not verified:** Behavior against Paul's real server and real terminal; macOS; mpv (VLC
only); the Auto Download fix itself (only the problem was measured). An early reading in
this session (startup ~2.4 s, first keypress ignored) was a test-setup artifact: a terminal
that never answers the image query makes `Picker::from_query_stdio` wait its full 2 s and
leaves a reader thread that swallows the next keypress. On a normal terminal the first
screen takes ~0.4 s. That same thing can happen to real users in terminals that don't answer
(e.g. tmux without passthrough) - a shorter query timeout would help them.

**Open questions / handoff:** Waiting on Paul to choose what to fix. Suggested order:
Auto Download (1), then the loop sleep (2) and loading poll (3), which are one-line changes.
Any fix needs the usual live test before push; re-measure with a pty harness, not a
detached tmux session.

### 2026-10-22 — Claude Code (fixes for the 2026-10-21 performance findings 1-3)

**What changed:** Paul asked for items 1, 2 and 3 from the entry above. All
uncommitted, waiting on his go-ahead.
1. *Auto Download* (`src/utils/download_cache.rs`, `src/utils/http_client.rs`):
   each item can only download once at a time (so the `d` key and auto sync can't
   collide); auto sync is now one worker per kind (books, podcasts) that always runs
   the newest list instead of stacking overlapping passes; an auto-download that fails
   is not retried for 2 minutes; files stream to `<id>.part` on disk and are renamed
   into place only when complete (memory stays flat), and a leftover `.part` is resumed
   with a Range request (a server that ignores Range just restarts cleanly); a 60 s
   "no data arriving" limit on download connections so a stuck transfer errors out
   instead of hanging the worker; `.part` files untouched for 3 days are deleted. Side
   effect: a non-success HTTP status (e.g. 404) is now a failed download - before, the
   error page could be saved as the audio file.
2. *Main loop* (`src/main.rs`): removed the fixed 50 ms sleep, and the idle wait went from
   200 to 250 ms so an idle app redraws exactly as often as before.
3. *Player loading* (`src/player/session.rs`): while a track is still opening, the player
   is checked every 150 ms instead of every second.

**How it was verified:** `cargo build`, `cargo test` (100 passed: 96 plus 4 new for Range
parsing, the in-flight guard, the retry cooldown and the one-worker behavior) and
`cargo clippy --all-targets` (only the 5 existing `too_many_arguments` warnings, none in
touched files). A unit test caught a real deadlock in my first version of the in-flight
guard (`Option::then_some` built the guard eagerly and its `Drop` re-locked the same
mutex) - fixed before anything else ran. Live, against a fake Audiobookshelf with a scratch
config (Paul's server/config/session untouched), old v0.11.2 vs new build, every downloaded
file checked byte for byte:
- 75 s podcast Auto Download: old 18 downloads / 1,080 MB sent for 9 files, memory peak
  325 MB; new 9 downloads / 540 MB, memory peak 32 MB.
- Books, two 150 MB files: memory peak 202 MB old vs 26 MB new.
- Dropped connection: retry after the cooldown sent `Range: bytes=20971520-` and got the
  remaining 40 MB (206); server that ignores Range: clean full restart; stalled transfer:
  errored out after about a minute and then resumed from the stall point.
- Key to screen: 65 -> 13 ms median. 30 queued keys: old 3.5-6.8 keys/s, new about 10.7
  (same-moment alternating runs; the laptop was on battery at 800 MHz, so absolute numbers
  are low, an earlier run at 1.6 GHz showed about 35 keys/s for the same change).
- Play start (key to a moving position on screen): 2.55 s -> 1.78 s on this machine.
- Idle CPU and memory: unchanged (4.9-5.1% vs 4.9%, 22 MB), after the 250 ms wait fix (a
  200 ms wait had made idle CPU about 25% higher).

**Not verified:** Paul's real server and terminal; macOS; mpv (VLC only); a real server
dropping connections mid-download (only the fake); the stall limit with a very slow but
steady link (60 s without any data is the cutoff, which should not trip a slow transfer).
Multi-track books still re-download tracks that already finished if a later track fails
(unchanged, noted only). No changelog entry yet - that belongs to the release.

**Open questions / handoff:** Needs Paul's go-ahead to commit and push; suggest a patch
release after the usual live check on his own setup. Remaining ideas from the earlier entry
(inline podcast refresh, shared HTTP client, event-driven wakeups) are untouched.

### 2026-10-22 — Claude Code (screen damage after the `d` key, uncommitted)

**What changed:** `clear_message` (`src/utils/pop_up_message.rs`) now asks for a full
repaint itself by setting `NEEDS_TERMINAL_CLEAR`, with one new unit test. Cause: Paul saw stray
letters from the Library screen on Continue Listening (cells the app never repainted). I could
not reproduce those exact letters (tmux, old and new build, three scenarios) and I can't say
what triggered them - that tab ran in the desktop app's terminal panel, which was hidden and
then resized after launch. While looking I found and reproduced a real, older bug of the same
family: the status-line helpers write straight to the terminal, bypassing the drawing code's
picture of the screen, and `clear_message` blanks the whole row 3 from the bottom - on Home with
a player running that row is the Now Playing box's bottom border. The `d` (download) key
popped "Downloading..." there and cleared it afterwards without asking for a repaint, so the
border stayed blank until something else forced one. Playback start, `R` and a few others
already set the flag; `d` (books and podcasts) did not. Fixing it in `clear_message` covers every
caller, including any added later.

**How it was verified:** `cargo build`, `cargo test` (101 passed) and `cargo clippy
--all-targets` (only the 5 existing `too_many_arguments` warnings). Live against a fake server
with a fresh scratch config per run, old v0.11.2 vs the fixed build, comparing the screen with a
forced full repaint: pressing `d` while a book plays started a real download in 5/5 runs on both
builds; the old build left one full border row blank in 5/5, the fixed build in 0/5. `R` still
leaves a clean screen with no stuck "Refreshing..." message on both. The damage detector itself
was checked by injecting two stray letters (it found both).

**Not verified:** the stray `L`/`J` themselves (not reproduced); the desktop app's terminal panel
(different emulator, not available to test); Paul's real terminal. A related risk is untouched:
the background threads that print those messages can still interleave with a redraw in progress
(never seen in testing). The ambiguous-width `⬇` download marker (plain U+2B07, no emoji
selector) is a possible contributor in some terminals - also untested.

**Open questions / handoff:** Needs Paul's go-ahead to commit and push (pairs naturally with the
earlier commit as part of a patch release). If the stray letters come back in a normal terminal,
get the exact keys pressed beforehand and re-test with the detector in the scratchpad approach
(see memory: dev-environment-gotchas).
