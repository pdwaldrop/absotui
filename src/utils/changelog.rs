const VERSION: &str = env!("CARGO_PKG_VERSION");

// The newest entry from `changelog()` (they're assembled newest-first), trimmed of its
// "Changelog Absotui vX.Y.Z (date)" header and "Enjoy!" sign-off - both are redundant
// once shown as a "what's new" preview next to the version number itself.
pub fn latest_changelog_entry() -> String {
    let full = changelog();
    let Some(entry) = full.split("####").next() else {
        return String::new();
    };
    let Some((_header, rest)) = entry.split_once('\n') else {
        return String::new();
    };
    rest.trim().trim_end_matches("Enjoy!").trim().to_string()
}

pub fn changelog() -> String {
    let mut changelog = String::new();

let changelog_01 = "Changelog Toutui v0.1.0-beta (02/21/2025) \n\
         Fixed:\n\
         \n\
         First release.
         \n\
         Changed:\n\
         \n\
         First release.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_02 = "Changelog Toutui v0.1.1-beta (02/24/2025) \n\
         Fixed:\n\
         \n\
         - App crash (out of bounds) when API send empty values.
         - Close listening session not always working (bug_id: fixed_dd9a64)
         \n\
         Changed:\n\
         \n\
         No change.
         \n\
         Enjoy and be toutui!\n
         ####\n".to_string();
let changelog_03 = "Changelog Toutui v0.1.2-beta (02/24/2025) \n\
         Fixed:\n\
         \n\
         - Partially fixed, becsause not optimal: bug_id: 9bacac Sync: If you open VLC to listen X, close VLC and quickly open VLC again to listen Y: X will still be sync — according to Y (normally, only Y has to be sync in this case).

         \n\
         Changed:\n\
         \n\
         No change.
         \n\
         Enjoy and be toutui!\n
         ####\n".to_string();
let changelog_04 = "Changelog Toutui v0.1.3-beta (02/03/2025) \n\
         Fixed:\n\
         \n\
         - Fix bug_id: 3f729c Loading time not optimized for library with a lot of items (long start loading and refresh time)
         \n\
         Changed:\n\
         \n\
         - Script `hello_toutui` to make installation easier.
         \n\
         Contributors:\n\
         \n\
         - dougy147, dhonus
         \n\
         Enjoy and be toutui!\n
         ####\n".to_string();
let changelog_05 = "Changelog Toutui v0.2.0-beta (07/03/2025) \n\
CAUTION: This version is not compatible with the previous one.  
You need to remove the database in ~/.config/toutui before proceeding. 
         Fixed:\n\
         \n\
         - From known_bugs.md, fixed:

    Find a robust solution for bug_id: 9bacac
    Fix bug_id: 86384e
    Fix bug_id: 6ac5d8
    Fix bug_id: 06e548
    Fix bug_id: e0b61c
    Fix bug_id: fc695f
    Fix bug_id: 40f48d
    Fix bug_id: bf10cd

         \n\
         Changed:\n\
         \n\
         - 
         \n\
         Contributors:\n\
         \n\
         - AlbanDAVID
         \n\
         Enjoy and be toutui!\n
         ####\n".to_string();
let changelog_06 = "Changelog Toutui v0.3.0-beta (24/03/2025) \n\
CAUTION: This version is not compatible with the previous one.  
To make it work properly, perform a fresh reinstall.
\n\
         Added:\n\
         - Integrated player. Keep calm and stay in your terminal! :)
         \n\
         Fixed:\n\
         \n\
         - Fixed: issue where pressing R twice was required to refresh the app.
         - Fixed: issue causing the cursor to disappear when the application is closed.
         - Fixed: issue if app is quitted for the first time and that listening session is empty.
         \n\
         Changed:\n\
         \n\
         - Faster loading time to play an item.
         - Improved synchronization accurary.
         - Removed warning during compilation time.
         \n\
         Contributors:\n\
         \n\
         - AlbanDAVID, dougy147
         \n\
         Enjoy and be toutui!\n
         ####\n".to_string();
let changelog_07 = "Changelog Toutui v0.3.1-beta (25/03/2025) \n\
CAUTION: This version is not compatible with v0.2.0-beta and bellow.  
To make it work properly, perform a fresh reinstall.
\n\
         Fixed:\n\
         \n\
         - Fixed: incorrect merge
         \n\
         Contributors:\n\
         \n\
         - AlbanDAVID
         \n\
         Enjoy and be toutui!\n
         ####\n".to_string();
let changelog_08 = "Changelog Toutui v0.3.2-beta (26/03/2025) \n\
         Added:\n\
         \n\
         - macOS compatibility.
         \n\
         Fixed:\n\
         \n\
         - Issue with VLC buffer (if a chapter is manually changed or during jump/backward).
         - Display issue on small monitors.
         \n\
         Changed:\n\
         \n\
         - hello_toutui script improved
         \n\
         Contributors:\n\
         \n\
         - AlbanDAVID, dougy147
         \n\
         Enjoy and be toutui!\n
         ####\n".to_string();
let changelog_09 = "Changelog Toutui v0.3.3-beta (02/04/2025) \n\
         \n\
         Changed:\n\
         \n\
         - Adding a login placeholder to specify the use of http:// or https:// for the server address.
         - Display error login message without time limit.
         \n\
         Contributors:\n\
         \n\
         - AlbanDAVID
         \n\
         Enjoy and be toutui!\n
         ####\n".to_string();
let changelog_10 = "Changelog Toutui v0.3.4-beta (23/04/2025) \n\
         \n\
         Fix:\n\
         \n\
         Handle empty podcast episode lists gracefully. Prevent panic and show 'No episodes' message. by @denispol in https://github.com/AlbanDAVID/Toutui/pull/22\n\
         Contributors:\n\
         \n\
         - AlbanDAVID, denispol
         \n\
         Enjoy and be toutui!\n
         ####\n".to_string();
let changelog_11 = "Changelog Toutui v0.3.5-beta (27/04/2025) \n\
         \n\
         Added:\n\
         - Display number of total items for continue listening, library and library settings (for books and podcasts)
         - Clap crate and a function to display the version in the CLI (e.g. `toutui --version`)
         \n\
         Fixed:\n\
         \n\
         - [macos] vlc version not displayed in listening sessions (from ABS web browser)
         - Out of bounds in Library Settings
         \n\
         Contributors:\n\
         \n\
         - AlbanDAVID
         \n\
         Enjoy and be toutui!\n
         ####\n".to_string();
let changelog_12 = "Changelog Toutui v0.4.0-beta (10/05/2025) \n\
         \n\
         Warning:\n\
         - If you're already using the app, please follow the upgrade instructions here: => 
         https://github.com/AlbanDAVID/Toutui/wiki/Major-upgrade-instruction#v--035-beta-to-v040-beta

         Added:\n\
         - Simplified installation and updates by: 
            - Downloading the binary.
            - Compiling it from source (no local clone needed).

         -  New commands available:
            - toutui --update and toutui --uninstall cmd added.

         - Notify if an update is available directly in the app.

         - [Linux only] The app can now be launched via an app launcher.
         \n\
         Contributors:\n\
         \n\
         - AlbanDAVID, dougy147
         \n\
         Enjoy and be toutui!\n
         ####\n".to_string();
let changelog_13 = "Changelog Toutui v0.4.1-beta (14/05/2025) \n\
         \n\
         Warning:\n\
         - If you're already using the app v0.3.5 or bellow, please follow the upgrade instructions here: => 
         https://github.com/AlbanDAVID/Toutui/wiki/Major-upgrade-instruction#v--035-beta-to-v040-beta

         Added:\n\
         - Archlinux users: the app is now available in the AUR (yay -S toutui)

         Changed:\n\
         - Minor changes in the installation process.

         \n\
         Contributors:\n\
         \n\
         - AlbanDAVID
         \n\
         Enjoy and be toutui!\n
         ####\n".to_string();
let changelog_14 = "Changelog Toutui v0.4.2-beta (15/05/2025) \n\
         \n\
         Warning:\n\
         - If you're already using the app v0.3.5 or bellow, please follow the upgrade instructions here: =>
         https://github.com/AlbanDAVID/Toutui/wiki/Major-upgrade-instruction#v--035-beta-to-v040-beta

         Added:\n\
         - Verifying file integrity using SHA-256 before installation via curl script

         Changed:\n\
         - Clarification of update/uninstall instructions

         \n\
         Contributors:\n\
         \n\
         - AlbanDAVID
         \n\
         Enjoy and be toutui!\n
         ####\n".to_string();
let changelog_15 = "Changelog Absotui v0.5.0-beta (18/07/2026) \n\
         \n\
         This is a fork of Toutui, renamed Absotui, continuing development independently.
         \n\
         Added:\n\
         - Progress bars and time/duration display for both books and podcast episodes
         - Podcast Home list reworked into a \"New & Unfinished\" view (merging Continue
           Listening and Newest Episodes, actively filtered by real finished status)
         - Now-playing marker, age labels (\"1Day\", \"2Weeks\"...), and a sort-by-age
           toggle (D) for the podcast list
         - Podcast Autoplay setting: automatically start the next episode when one finishes
         - Speed-adjusted vs raw content time toggle (T) for Elapsed/Left display
         - Switching libraries in Settings now applies immediately, no manual refresh needed

         Fixed:\n\
         - Podcast episodes were never actually detected as finished (progress lookup used
           the wrong API shape), so finished episodes never left the list
         - Crash (integer underflow) when jumping backward during podcast playback
         - Podcast player/list title inconsistency (now always \"Episode Title | Podcast Title\")
         - Progress showing over 100% at non-1x playback speed
         - ebookProgress deserialization failure for items with mixed audio/ebook progress
         \n\
         Changed:\n\
         - Modernized dependencies and Rust edition (2021 to 2024)
         - Renamed project from Toutui to Absotui throughout
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_16 = "Changelog Absotui v0.5.1-beta (19/07/2026) \n\
         \n\
         Fixed:\n\
         - Install script's OS/distro detection only matched a handful of hardcoded
           distro names, so derivatives like CachyOS, Manjaro, Pop!_OS, and Linux Mint
           fell through to \"unknown\" and aborted the install. Now reads the
           standardized ID_LIKE field from /etc/os-release instead.
         - The checksums for config.example.toml, absotui.desktop, and the release
           binaries were still the original Toutui project's values, which would have
           failed verification for every install regardless of distro.
         - The hello_absotui.sh checksum baked into `absotui --update`/`--uninstall`
           was stale after the above script fixes, breaking update/uninstall for
           anyone who had already installed the binary.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_17 = "Changelog Absotui v0.5.2-beta (19/07/2026) \n\
         \n\
         Fixed:\n\
         - The install script's release checksums (for config.example.toml,
           absotui.desktop, and the binaries) went stale again immediately after the
           previous release, since CI only builds the real files after a release is
           cut. Checksums are no longer hardcoded in the script at all - it now
           fetches a SHA256SUMS.txt manifest that CI generates from the release's
           actual uploaded assets, so this class of bug can't recur.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_18 = "Changelog Absotui v0.5.3-beta (19/07/2026) \n\
         \n\
         Added:\n\
         - Podcast episodes now show cover art next to their description in Continue
           Listening, matching audiobooks - preferring the episode's own embedded
           artwork when its audio file has one, falling back to the podcast's cover
           otherwise.
         \n\
         Fixed:\n\
         - The podcast Home list's selection cursor could appear to drift to a
           different episode on its own every few seconds - the periodic background
           refresh (which keeps finished episodes from lingering in the list) reordered
           the list without preserving which episode was selected.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_19 = "Changelog Absotui v0.5.4-beta (19/07/2026) \n\
         \n\
         Added:\n\
         - A volume indicator in the player bar (VLC's own volume is only ever
           adjusted relatively, so absotui now tracks and displays it).
         - Settings > Per-Item Speed: books and podcast shows can each remember their
           own playback speed instead of sharing a single global speed.
         - A custom app icon, replacing the generic system one in your application
           launcher.
         \n\
         Fixed:\n\
         - Descriptions with HTML markup could lose text after a stray \"<\", double-decode
           entities, or fail to recognize self-closing <br/> tags.
         - Mouse wheel/trackpad scroll no longer hijacks the list selection.
         - Podcast Autoplay: the previous episode's VLC process wasn't being closed
           before starting the next one, breaking pause and progress sync for it; a
           race could start two playback sessions at once after a manual replay; the
           next episode is now picked from the live list instead of a stale snapshot;
           a blocking network call could freeze the whole UI right after a transition;
           and a finished session could keep getting shown as \"now playing\".
         - Playback speed no longer displays as an ugly float like 1.3000001 after
           repeated adjustments.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_20 = "Changelog Absotui v0.5.5-beta (19/07/2026) \n\
         \n\
         Fixed:\n\
         - The install/update/uninstall script's self-integrity check referenced
           variables an outer wrapper never actually set, which crashed the installer
           immediately for virtually everyone before any real install/update/uninstall
           logic could run - and, in the narrower case of running the script from
           inside a checked-out clone, could delete the script's own source file
           instead of just failing safely. The check now verifies the running script
           against the latest release's real published checksum, the same mechanism
           already used for every other downloaded file, and no longer deletes
           anything on a mismatch.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_21 = "Changelog Absotui v0.5.6-beta (19/07/2026) \n\
         \n\
         Fixed:\n\
         - `absotui --update`/`--uninstall` embedded a checksum for hello_absotui.sh
           directly in the compiled binary, which went stale the moment the script
           changed again (as it just did) and silently broke update/uninstall for
           anyone on an older binary - the exact class of bug this checksum handling
           was already supposed to have eliminated everywhere else. The binary no
           longer hardcodes anything here; hello_absotui.sh already verifies itself
           against the latest release's real checksum at run time, so there's nothing
           left for it to keep in sync. `--update`/`--uninstall` also now correctly
           report failure instead of always exiting 0 regardless of what happened.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_22 = "Changelog Absotui v0.5.7-beta (19/07/2026) \n\
         \n\
         Added:\n\
         - `F` in the podcast Home list marks the selected episode finished and
           removes it from New & Unfinished immediately, without waiting on it to
           actually be played through.
         \n\
         Fixed:\n\
         - Cleaned up inconsistent footer key-hint text across screens: \"Settings\"
           was capitalized while other screen names (like \"library\"/\"home\") weren't,
           \"top/bot\" vs \"top/bottom\" varied by screen for the same binding, some
           footers spelled out \"J(down) K(up) H(top)\" while others used arrows, and
           one Settings screen's footer had a leftover typo (\"Scroll :\" missing a
           word). All footers now share the same wording, built from one place so
           this can't drift between screens again as new ones get added.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_23 = "Changelog Absotui v0.5.8-beta (19/07/2026) \n\
         \n\
         Fixed:\n\
         - `absotui --update` never refreshed absotui.desktop or the app icon, only a
           fresh install did - so the custom icon added in v0.5.4-beta went unnoticed
           by anyone who updated instead of reinstalling. Updating now refreshes both,
           same as installing does.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_24 = "Changelog Absotui v0.5.9-beta (19/07/2026) \n\
         \n\
         Added:\n\
         - The installer now detects if you're running a terminal that supports a
           custom window class (Ghostty, Kitty, Alacritty, Foot, WezTerm) and, if so,
           gives the launcher entry its own window class instead of the terminal's
           default one. Previously the app icon only showed up in the launcher/pinned
           icon - the actual running window still showed as a generic terminal window
           in the taskbar/dock, since that's controlled by the window's class, not the
           .desktop file's Icon= line. Terminals not on that list still work exactly as
           before (the desktop environment picks the terminal, no custom icon on the
           live window).
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_25 = "Changelog Absotui v0.5.10-beta (19/07/2026) \n\
         \n\
         Added:\n\
         - The terminal window title now shows \"Absotui - <book/podcast name>\" while
           something's playing, and just \"Absotui\" otherwise, instead of whatever the
           terminal defaults to (usually just \"absotui\", the binary name). Makes the
           window identifiable from a taskbar/dock/window-switcher without opening it.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_26 = "Changelog Absotui v0.5.11-beta (19/07/2026) \n\
         \n\
         Fixed:\n\
         - The window title from v0.5.10-beta prefixed itself with \"Absotui - \", which
           duplicated the app name on taskbars/docks that already show it separately
           (from the .desktop file's Name=) next to the window title - showing up as
           \"Absotui • Absotui - <name>\". The title is now just the book/podcast
           name while playing, and blank (not \"Absotui\") otherwise, which most docks
           fall back to showing as just the app name with nothing to duplicate.
         - Fixed a cosmetic typo in the installer's terminal-detection log line during
           `--update`/`--install` that ran $TERM_PROGRAM and $TERM together with no
           separator (e.g. \"ghosttyxterm-ghostty\").
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_27 = "Changelog Absotui v0.5.12-beta (20/07/2026) \n\
         \n\
         Fixed:\n\
         - The app used to just close, with no explanation, if it couldn't reach your
           Audiobookshelf server (off your home network, server down, etc.) - whether
           at startup or from a mid-session refresh/library switch. It now shows a
           recovery screen instead, with the server address, the error, and the
           option to retry, enter a different server address, or quit; the two
           mid-session cases can also cancel back to what was already loaded instead
           of forcing a fix-or-quit loop.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_28 = "Changelog Absotui v0.5.13-beta (20/07/2026) \n\
         \n\
         Fixed:\n\
         - Marking the currently-playing podcast episode as finished (F) didn't
           actually work - it looked like it did for a moment, then the episode
           reappeared in New & Unfinished a few seconds later and kept playing. It
           now stops playback immediately and stays marked finished for good.
         - Logging in could sometimes require two attempts even with correct
           credentials, on a slower connection or server response.
         - Updating could silently drop any config.toml setting not present in the
           current config.example.toml (custom or just old), instead of preserving
           it like a normal config value.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_29 = "Changelog Absotui v0.5.14-beta (20/07/2026) \n\
         \n\
         Fixed:\n\
         - Sometimes at launch the app would just drop you back to the login screen
           with no explanation, even though you had a valid saved session -
           especially noticeable right after deleting/changing a saved account and
           quickly restarting. The app now retries briefly instead of giving up on
           the first hiccup, and shows an actual error if it's still failing.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_30 = "Changelog Absotui v0.5.15-beta (20/07/2026) \n\
         \n\
         Added:\n\
         - Settings > Update and uninstall can now update or uninstall right from
           inside the app - confirm, enter your password if needed, and watch it run
           in a live log panel instead of leaving to a terminal. A successful update
           reloads the app into the new version automatically, no manual restart.
           The absotui --update / --uninstall terminal flags still work exactly as
           before, unchanged.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_31 = "Changelog Absotui v0.5.16-beta (21/07/2026) \n\
         \n\
         Added:\n\
         - Settings > Update / Uninstall (renamed from \"Update and uninstall\") now
           authenticates the same way a real terminal would: it tries your
           fingerprint reader first if your system has one configured for sudo, and
           falls back to a password prompt automatically if that doesn't work out -
           instead of always asking for a typed password up front.
         \n\
         Fixed:\n\
         - A full code review turned up and fixed a long list of crash/hang bugs:
           several screens could crash on certain inputs (empty search results,
           deleting your only account, a podcast with missing metadata), a stalled
           VLC connection could freeze that session's syncing forever, a failed
           playback start could permanently block all future play attempts until
           quitting the app, and two rapid play presses close together could start
           two playback sessions at once.
         - The install/update script no longer leaves prompts hanging (Arch and
           macOS specifically) that the in-app updater's password-prompt detection
           could otherwise misread.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_32 = "Changelog Absotui v0.5.17-beta (21/07/2026) \n\
         \n\
         Fixed:\n\
         - Settings > Update / Uninstall's fingerprint authentication could time out
           on a \"cold\" fingerprint reader (idle for a few minutes since your last
           sudo use) - measured up to ~65s just for the reader itself to give up and
           fall back, which didn't leave enough margin. Timeout bumped well above
           that, and the screen now mentions fingerprint auth can take a bit so a
           long wait doesn't look like it's stuck.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_33 = "Changelog Absotui v0.5.18-beta (21/07/2026) \n\
         \n\
         Fixed:\n\
         - Settings > Update / Uninstall could hang indefinitely during sudo
           authentication (fingerprint especially) even after you'd successfully
           authenticated - it only detected that sudo was done by watching for the
           pty to close, which isn't reliable (sudo's own privilege-separated
           process can hold that open after sudo itself has already exited). Now
           detects the moment sudo actually exits instead.
         - Security: cover art caching could be tricked by a malicious or
           compromised Audiobookshelf server (or a network man-in-the-middle on a
           plain http:// connection) into writing files outside the app's cover
           cache directory, since item IDs from the server were used directly as
           filenames with no validation. IDs are now sanitized before use.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_34 = "Changelog Absotui v0.5.19-beta (22/07/2026) \n\
         \n\
         Fixed:\n\
         - Settings > Update / Uninstall could report a false \"Incorrect shasum\"
           error that looked like a corrupted/tampered download, when it actually
           meant the installer couldn't reach GitHub's API to find the latest
           version (e.g. rate limiting) - it now reports that distinctly instead
           of comparing against a blank expected value.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_35 = "Changelog Absotui v0.5.20-beta (22/07/2026) \n\
         \n\
         Fixed:\n\
         - Settings > Update / Uninstall could time out and fail partway through a
           normal binary download on a slower connection, reporting it the same as
           a genuine hang. The download/install phase's time limit is now much more
           generous.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_36 = "Changelog Absotui v0.5.21-beta (25/07/2026) \n\
         \n\
         Added:\n\
         - Offline mode now covers podcasts, matching books: press `d` on a podcast
           episode to download or remove it for local playback, and a downloaded
           episode is preferred over streaming automatically.
         - Settings > Auto Download now keeps podcasts' New & Unfinished episodes
           downloaded too, alongside books.
         - The number of books Auto Download keeps downloaded is now configurable
           via `auto_download_count` under `[downloads]` in config.toml, instead of
           always downloading everything in Continue Listening.
         - The Home info panel now shows each item's file size, after Duration.
         \n\
         Fixed:\n\
         - The `d` download/remove keybinding worked on Home but was never
           advertised in the footer.
         - Downloaded books and podcast episodes are now marked with a small ⬇
           prefix on the title instead of a trailing \"[offline]\" suffix - a suffix
           could get cut off entirely by title truncation on long rows. Podcast
           episodes previously showed no downloaded indicator at all regardless of
           this truncation issue; they now do.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_37 = "Changelog Absotui v0.5.22-beta (25/07/2026) \n\
         \n\
         Fixed:\n\
         - The install/update script (hello_absotui.sh) failed on distros that
           don't ship `shasum` (e.g. Fedora's minimal/KDE spins) with \"shasum:
           command not found\", even though the download itself was fine. It now
           tries `sha256sum` (coreutils) first, falls back to `shasum`, and then
           `openssl` as a last resort.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_38 = "Changelog Absotui v0.5.23-beta (26/07/2026) \n\
         \n\
         Changed:\n\
         - Refreshed the app icon with a cleaner vector rework of the
           headphones-parrot-and-terminal design.
         \n\
         Fixed:\n\
         - The install script no longer blocks on a \"provide a secret key\"
           prompt during install/update - that key only encrypts your
           Audiobookshelf login at rest locally and is never something you need
           to type yourself, so it's now generated automatically.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_39 = "Changelog Absotui v0.5.24-beta (26/07/2026) \n\
         \n\
         Fixed:\n\
         - The install script now fails immediately with a clear message (e.g.
           \"check df -h /tmp for free disk space\") instead of a confusing wall
           of unrelated errors if it can't create a temp file/directory.
         - It also no longer claims dependencies (e.g. VLC) installed
           successfully when the package manager actually failed.
         - Settings > Update could pop the password prompt multiple times
           during a single update even though sudo only needed to authenticate
           once - a package manager's own progress output was sometimes
           mistaken for a password prompt. It's now recognized correctly.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_40 = "Changelog Absotui v0.5.25-beta (28/07/2026) \n\
         \n\
         Fixed:\n\
         - Audiobooks uploaded as separate files per chapter (rather than one
           .m4b) only ever played the first file - chapters past the first
           couldn't be reached, progress showed against just that one file's
           duration instead of the whole book, and downloading for offline use
           silently saved only the first file while marking the book fully
           downloaded. Playback now sequences through every file automatically,
           chapter jumps (and P/U) work across file boundaries, resuming picks
           the correct file and offset, and downloads fetch every file.
         - Book/podcast descriptions written as plain text (no HTML) still
           relied on real newlines to separate a numbered list of
           chapters/episodes - these rendered as one run-on block instead of
           one item per line.
         - Shrinking the terminal enough to wrap the footer's key-hint text
           could make an entire line of hints disappear instead of wrapping -
           the footer now sizes itself to however many rows its (wrapped) text
           actually needs.
         \n\
         Changed:\n\
         - Reworded a couple of footer key hints for clarity (\"B: play keys\"
           instead of \"B: toggle player ctrl\") and fixed one that mislabeled a
           list-jump shortcut as a description-scroll shortcut.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_41 = "Changelog Absotui v0.5.26-beta (29/07/2026) \n\
         \n\
         Fixed:\n\
         - Quitting with Q while something was playing could leave the
           listening session still open on the Audiobookshelf server, and
           could leave VLC running in the background after the app had
           exited. Quitting now shuts VLC down reliably and waits for the
           playback task's own cleanup to finish before exiting.
         - In some cases Q could fail to quit at all, leaving the app hanging
           with the terminal stuck in a broken state and the cursor hidden.
         - Starting or quitting a track very quickly - before playback had
           actually reported a position - could sync that item's progress as
           0 seconds, appearing to wipe your place in it.
         - Switching from one title to another closed the outgoing item's
           listening session twice on the server instead of once.
         - If the app crashed or was killed, the session it left open stayed
           open and unsynced until you next played something. It's now closed
           and synced automatically at startup instead.
         - After a crash, the app could start up showing a frozen \"now
           playing\" player for a session that was no longer running.
         - Network requests had no timeout, so a server that accepted a
           connection but never answered could block playback indefinitely
           with no way out but restarting the app. All requests are now
           bounded (large downloads still run as long as they need to).
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_42 = "Changelog Absotui v0.5.27-beta (30/07/2026) \n\
         \n\
         Changed:\n\
         - App startup is significantly faster, especially with a large
           podcast library - measured about 17.5s down to roughly 7s on a
           22-podcast library. Several requests that were previously made one
           at a time (episode lists, per-item progress lookups, and three
           independent startup fetches) now run concurrently instead.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_43 = "Changelog Absotui v0.5.28-beta (30/07/2026) \n\
         \n\
         Changed:\n\
         - App startup with a large podcast library is faster still - a
           22-podcast library now measures about 4-5 seconds, down from
           roughly 7 seconds in the previous release (and about 17.5
           seconds two releases ago). Podcast episode lists now load in
           the background after the app opens instead of blocking
           startup - if you open a podcast's episode list or search for
           an episode before that finishes (rarely more than a second
           or two after the app is usable), you'll briefly see a
           \"still loading\" message instead.
         \n\
         Fixed:\n\
         - On macOS, the `cvlc` setting in config.toml had no effect -
           the app always launched VLC the same way regardless of how
           it was set. Setting `cvlc = \"0\"` now shows VLC's normal
           window, matching Linux behavior.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_44 = "Changelog Absotui v0.5.29-beta (25/08/2026) \n\
         \n\
         Fixed:\n\
         - Login failed against Audiobookshelf v2.36+, which no longer
           returns the older plain token Absotui relied on - fixed by
           using the newer accessToken instead.
         - That newer accessToken only lasts about an hour by default,
           so without a way to renew it, the fix above would have meant
           logging back in roughly every hour. Absotui now renews it
           automatically in the background before it expires, and keeps
           renewing indefinitely as long as you open the app at least
           once a month - no more periodic forced re-logins.
         \n\
         Contributors:\n\
         \n\
         - sarielhp
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_45 = "Changelog Absotui v0.5.30-beta (26/08/2026) \n\
         \n\
         Fixed:\n\
         - The install/update script could report \"Could not determine
           the expected shasum for hello_absotui.sh from GitHub\" with
           no way to tell whether that meant a GitHub API/rate-limit
           issue or a local temp-file problem - the two are now
           reported distinctly.
         - The same script could occasionally misread GitHub's release
           API response and compare against a completely wrong value
           (a chunk of the release notes instead of the version number)
           if GitHub happened to return that response as compact JSON
           instead of pretty-printed - now parsed in a way that doesn't
           depend on that formatting.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_46 = "Changelog Absotui v0.5.31-beta (27/08/2026) \n\
         \n\
         Fixed:\n\
         - Reopening Absotui after it had been closed for a while (roughly
           an hour or more) could fail immediately with \"401
           Unauthorized\", even though the automatic token renewal added
           in v0.5.29-beta was meant to prevent exactly this - it only
           ran once the app was already up and running, never before that
           first startup check, so retrying couldn't recover either. The
           app now renews an expiring token before that first check too.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_47 = "Changelog Absotui v0.5.32-beta (27/08/2026) \n\
         \n\
         Fixed:\n\
         - A momentary network hiccup while renewing your access token in
           the background (a dropped connection, a brief timeout) could be
           mistaken for the server actually rejecting your login, silently
           deleting your whole saved account and forcing a full re-login -
           even though the connection would often recover moments later.
           Only an explicit rejection from the server now triggers that;
           a plain connectivity blip is retried instead.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_48 = "Changelog Absotui v0.5.33-beta (29/08/2026) \n\
         \n\
         Changed:\n\
         - The app now always renders using your terminal's own color
           theme (background, text, accents) instead of a fixed dark
           palette - no more custom [colors] section in config.toml.
         \n\
         Fixed:\n\
         - If your saved session's refresh token was actually dead (30+
           days idle, or revoked server-side), startup got stuck showing
           a raw \"401 Unauthorized\" error that Retry/Change-address
           couldn't fix. It now clears the stale account and tells you to
           restart and log in again, matching how this was already
           handled mid-session.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_49 = "Changelog Absotui v0.5.34-beta (29/08/2026) \n\
         \n\
         Fixed:\n\
         - The install/update script could print the entire raw GitHub API
           response instead of just the latest version number (e.g.
           \"[INFO] GitHub: 0.5.33-beta \\\"draft\\\": false, ...\") - GitHub
           doesn't guarantee pretty-printed JSON, and two of the three
           places parsing it still assumed one-field-per-line, unlike a
           similar fix already applied elsewhere. All three now parse the
           same, formatting-independent way.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_50 = "Changelog Absotui v0.5.35-beta (29/08/2026) \n\
         \n\
         Fixed:\n\
         - The previous release fixed the install script printing raw JSON
           for the version check, but missed an identical problem in the
           changelog preview shown during an update - on a compact GitHub
           API response it printed the entire raw document instead of the
           release notes. Fixed the same way as the version check.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_51 = "Changelog Absotui v0.5.36-beta (29/08/2026) \n\
         \n\
         Added:\n\
         - Every screen now shows outlined, labeled sections (list,
           info, description, header) instead of floating unstyled text,
           with a small set of accent colors pulled from your terminal's
           own theme (not fixed colors) - matching how apps like
           superfile and CLIAMP look in any color scheme.
         - The footer/keybind hints changed from \"key: description\" to
           \"[key] description\" highlighted chips, and the now-playing
           marker and player progress bar are colored instead of plain
           reverse video.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_52 = "Changelog Absotui v0.5.38-beta (30/08/2026) \n\
         \n\
         Added:\n\
         - Press `?` on any screen for a full Keymap screen (matching
           superfile/CLIAMP's own keymap screens), showing every key that
           actually works there. Footers now show a shorter, curated set
           of hints instead of cramming everything in (dropping the
           purely-navigational ones like move/top-bottom, and the player
           key-bindings legend toggle, all of which are still in Keymap).
         - The player bar now gets its own accent-colored box (matching
           the play icon), the same full width as every other panel.
         - Home's cover art now sits inside the Description box instead
           of floating unboxed beside it.
         \n\
         Changed:\n\
         - Settings is now lowercase `s` (was `S`), and stopping the
           currently-playing item is now `S` (was `Y`) - `Y`'s \"quit\"
           label was easy to confuse with `Q`/Esc's actual app-quit right
           next to it, even though it only stops that one item.
         - Settings no longer advertises `R: refresh` in its footer - it
           still works, but its only visible effect there is landing back
           on Home, which `Tab` already does.
         - Settings > Library's footer said \"change library\", Keymap said
           \"switch library\" - picked one wording for both.
         - Footer/Keymap hint descriptions are now Sentence case (\"Play\",
           \"Back to Settings\") instead of all-lowercase, matching CLIAMP's
           own convention.
         \n\
         Fixed:\n\
         - Whichever keybind chip happened to be first in a footer (e.g.
           `l/→` or `h`) rendered with no left padding, jamming the key
           against the highlight's edge unlike every other chip.
         - The top-level Settings screen's footer was missing `h: back`,
           even though the key already worked there.
         - `D` (podcast sort) only checked podcast mode, not which screen
           you were on - it silently reordered Home's list from anywhere
           when podcast mode was active, even though it's only shown on
           Home's footer.
         - `h: back` appeared after the primary action on the Podcast
           Episode footer, opposite of every other footer; every Settings
           screen had the reverse mismatch between its footer and its
           Keymap entry. Both now consistently show back first.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_53 = "Changelog Absotui v0.5.39-beta (30/08/2026) \n\
         \n\
         Fixed:\n\
         - Book progress percentages in list rows (e.g. \"(59%)\") could get cut off
           on the right - the row-width calculation only accounted for the list's
           highlight-symbol column, not the bordered box's own left/right border
           columns added when list borders were introduced.
         - The search box (`/`) drew through its own separate terminal session
           instead of the app's normal render pass, which could land it on top of
           the Now Playing player bar, leave stale text or a corrupted cover art
           image visible behind it, or corrupt the whole screen depending on
           terminal and window size. Rebuilt as a normal overlay on the same
           render pass as everything else.
         - Pressing `R` (refresh) or switching libraries could silently erase the
           player bar's bottom border until something else forced a full repaint
           (e.g. resizing the window) - the \"Refreshing...\"/\"Switching library...\"
           messages write straight to the terminal, bypassing the same diff cache
           the search box already needed a similar fix for.
         \n\
         Changed:\n\
         - The search box now opens over the Info panel (author/year/duration -
           a quick glance away regardless) instead of the Description panel, so
           the actual synopsis text and cover art stay visible while typing.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_54 = "Changelog Absotui v0.5.40-beta (30/08/2026) \n\
         \n\
         Changed:\n\
         - Search (`/`) now matches on author too, not just title - e.g. searching
           an author's name finds every one of their books/podcasts even if none
           of the titles themselves contain it.
         - The search box's label and border are now the same yellow as the
           footer/player keybind chips, since that's how you get there in the
           first place, instead of the blue every permanent section uses.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_55 = "Changelog Absotui v0.5.41-beta (30/08/2026) \n\
         \n\
         Changed:\n\
         - A footer that wraps onto 2+ lines now gets a little breathing room -
           one blank row after the first line - instead of reading as a single
           crowded block.
         \n\
         Fixed:\n\
         - Starting playback could occasionally leave a \"Syncing your last
           listening session...\" message stuck on screen indefinitely,
           overlapping the footer/Now Playing area - it's written straight to
           the terminal and could desync from the next redraw's own cache.
         - Pressing the play key twice in quick succession on the same item
           could freeze that second attempt forever, waiting on a playback
           slot that had already been claimed and wouldn't be released again
           for a long time - it now gives up cleanly after about 20 seconds
           instead.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_56 = "Changelog Absotui v0.5.42-beta (30/08/2026) \n\
         \n\
         Fixed:\n\
         - The Now Playing box could lose its top border and title (on a wide
           window with a single-line footer) or overlap the footer (on a
           narrower one) - it now positions itself against the footer's real
           height every frame instead of guessing.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_57 = "Changelog Absotui v0.5.43-beta (31/08/2026) \n\
         \n\
         Changed:\n\
         - Refreshing (`R`) and switching libraries are noticeably faster - two
           checks that used to re-run every single time (asking the terminal
           what image protocol it supports, and checking GitHub for the latest
           release) now only happen once per session instead.
         \n\
         Fixed:\n\
         - Removing a downloaded multi-file audiobook (one uploaded as several
           files, e.g. per-chapter) didn't actually free the disk space it
           used - only one of the book's files ever got deleted, leaving the
           rest behind with no way to clean them up from the app.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_58 = "Changelog Absotui v0.5.44-beta (31/08/2026) \n\
         \n\
         Changed:\n\
         - Settings menu reordered: Library, Per-Item Speed, Podcast Autoplay,
           Auto Download, Update/Uninstall, About, Account - routine settings
           first, Account (which can delete your saved login) last.
         - Screens reached by taking an action (the search box, Update/Uninstall's
           Confirm/Working/Password stages, and the Library/Per-Item
           Speed/Podcast Autoplay/Auto Download/Account screens in Settings) are
           now visually distinct from ordinary browsing screens via their border
           color.
         \n\
         Fixed:\n\
         - Settings > Account's \"Remove saved user\" deleted the saved account
           (server address, token, every per-user setting) on a single
           keypress, with no warning and no confirmation - a stray keypress
           while browsing Settings could silently wipe a saved login with
           nothing to undo it. The screen now always explains what removal
           does, and l/→ asks for confirmation first.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_59 = "Changelog Absotui v0.5.45-beta (31/08/2026) \n\
         \n\
         Fixed:\n\
         - The whole screen would briefly flash (blank, then repaint) every
           time playback started - a leftover status message being cleared up
           could force a full-screen repaint that was landing right before a
           short pause in the render loop, instead of right before the next
           actual repaint.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_60 = "Changelog Absotui v0.5.46-beta (01/09/2026) \n\
         \n\
         Changed:\n\
         - Internal code cleanup only - no user-visible changes. Trimmed
           low-value comments codebase-wide and removed a large amount of
           duplicated code in the database layer.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_61 = "Changelog Absotui v0.5.46-beta (02/09/2026) \n\
         \n\
         Fixed:\n\
         - Dropped a redundant server call during playback: every 10s progress
           sync called both /sync and /progress for the same update, which
           Audiobookshelf itself flags as a race-condition risk (items stuck
           in Continue Listening). Now uses /sync only, the correct one for
           periodic updates.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_62 = "Changelog Absotui v0.5.47-beta (02/09/2026) \n\
         \n\
         New:\n\
         - Browse by Collections - Tab now cycles Home -> Library ->
           Collections -> Home whenever the current library actually has any
           Audiobookshelf collections. Selecting one filters the Library list
           down to just its books.
         - Group Library by series - press S on the Library screen (book
           libraries only) to group the list by series, sorted by sequence
           number.
         \n\
         Changed:\n\
         - \"Stop playback\" moved from S to X, freeing up S for the new
           series-grouping toggle above.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_63 = "Changelog Absotui v0.6.0 (03/09/2026) \n\
         \n\
         Changed:\n\
         - Update/Uninstall now shows what's actually in a pending update
           (New/Changed/Fixed) before you commit to it, instead of just a
           blank confirmation.
         - The update-available notice is shorter and easier to scan.
         \n\
         Fixed:\n\
         - Updating or uninstalling could ask for your password twice - once
           up front, once partway through - if the install took long enough
           for the first authorization to expire. Now asks once.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_64 = "Changelog Absotui v0.7.0 (03/09/2026) \n\
         \n\
         New:\n\
         - A Stats page - press Tab to cycle to it alongside Home/Library.
           Total/today/week/month listening time, current and best streaks,
           a 7-day chart, and top-5 rankings for most-listened items,
           authors, genres, and narrators.
         \n\
         Changed:\n\
         - Podcast and audiobook libraries now show different icons in the
           header (a microphone vs. a stack of books) instead of sharing one.
         - The Library screen's Description panel now shows cover art, same
           as Home already did.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_65 = "Changelog Absotui v0.7.1 (03/09/2026) \n\
         \n\
         Fixed:\n\
         - The \"ask for your password/fingerprint only once\" fix from v0.6.0
           didn't actually work - a race in how it kept your authorization
           alive meant it silently never ran, so updating or uninstalling
           could still prompt twice. Rebuilt so there's nothing left to race:
           only one prompt is ever shown now, right when it's actually needed.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_66 = "Changelog Absotui v0.8.0 (03/09/2026) \n\
         \n\
         New:\n\
         - The Stats page now shows an average-by-day-of-week chart, a
           year-long activity heatmap (with month labels), a Recent
           Sessions list, and book/episode counts - alongside what v0.7.0
           already had. It's grown enough to need scrolling (J/K/H), same
           as long descriptions elsewhere.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_67 = "Changelog Absotui v0.8.1 (04/09/2026) \n\
         \n\
         New:\n\
         - Refreshed the app icon with a new vector rework.
         \n\
         Fixed:\n\
         - A multi-file audiobook could resume playback at the wrong
           position/track if the server ever left out a file's index.
         - Settings > Update could show \"Incorrect password\" for a
           completely unrelated failure (network drop, disk full) after
           you'd already authenticated correctly.
         - Library could attribute the wrong series/sequence to a book
           once \"group by series\" was on, if any item in your library
           had missing metadata.
         - The new Stats page could open already scrolled down if you'd
           just scrolled a description elsewhere.
         \n\
         Changed:\n\
         - Several internal performance and code-quality improvements
           from a full review pass - Library and the Stats heatmap both
           do noticeably less repeated work now.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_68 = "Changelog Absotui v0.8.2 (04/09/2026) \n\
         \n\
         Fixed:\n\
         - The install/update script didn't recognize Solus at all, and
           on any other unlisted Linux distro it aborted the whole
           install instead of just skipping what it couldn't
           auto-install - both now work: Solus gets real support, and
           an unlisted distro degrades to \"install this yourself\"
           instead of refusing to install Absotui at all.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_69 = "Changelog Absotui v0.8.3 (04/09/2026) \n\
         \n\
         Changed:\n\
         - Another app icon refresh - a new mascot rework, sized to fill
           its full 512x512 canvas so it stays legible even at small
           sizes (taskbar, favicon).
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_70 = "Changelog Absotui v0.8.4 (05/09/2026) \n\
         \n\
         Fixed:\n\
         - The install script could wrongly claim Absotui was \"already
           installed\" (blocking a clean install behind an extra
           confirmation) whenever a leftover config directory existed
           with no binary actually in place - most likely to happen
           after an earlier failed/partial install. It now checks for
           the actual binary only.
         - Installing the binary failed outright on a system without a
           pre-existing /usr/local/bin (confirmed live on a fresh Solus
           VM) - the install script now creates it if missing instead
           of assuming it's already there.
         - The install script asked for your sudo password several
           times during one install - most of those uses were on files
           entirely inside your own home directory, which never needed
           root in the first place. Down to one prompt, for the one
           step that actually needs it.
         \n\
         Enjoy!\n
         ####\n".to_string();
let changelog_71 = "Changelog Absotui v0.8.5 (05/09/2026) \n\
         \n\
         Fixed:\n\
         - The app menu launcher could fail with \"Requested executable
           not found\" even right after a successful install (confirmed
           live on Solus) - it relied on absotui being on PATH, but a
           desktop menu's PATH doesn't necessarily match a terminal's.
           The launcher now points straight at the installed binary.
         \n\
         Enjoy!\n
         ####\n".to_string();


    changelog.push_str(&changelog_71);
    changelog.push_str(&changelog_70);
    changelog.push_str(&changelog_69);
    changelog.push_str(&changelog_68);
    changelog.push_str(&changelog_67);
    changelog.push_str(&changelog_66);
    changelog.push_str(&changelog_65);
    changelog.push_str(&changelog_64);
    changelog.push_str(&changelog_63);
    changelog.push_str(&changelog_62);
    changelog.push_str(&changelog_61);
    changelog.push_str(&changelog_60);
    changelog.push_str(&changelog_59);
    changelog.push_str(&changelog_58);
    changelog.push_str(&changelog_57);
    changelog.push_str(&changelog_56);
    changelog.push_str(&changelog_55);
    changelog.push_str(&changelog_54);
    changelog.push_str(&changelog_53);
    changelog.push_str(&changelog_52);
    changelog.push_str(&changelog_51);
    changelog.push_str(&changelog_50);
    changelog.push_str(&changelog_49);
    changelog.push_str(&changelog_48);
    changelog.push_str(&changelog_47);
    changelog.push_str(&changelog_46);
    changelog.push_str(&changelog_45);
    changelog.push_str(&changelog_44);
    changelog.push_str(&changelog_43);
    changelog.push_str(&changelog_42);
    changelog.push_str(&changelog_41);
    changelog.push_str(&changelog_40);
    changelog.push_str(&changelog_39);
    changelog.push_str(&changelog_38);
    changelog.push_str(&changelog_37);
    changelog.push_str(&changelog_36);
    changelog.push_str(&changelog_35);
    changelog.push_str(&changelog_34);
    changelog.push_str(&changelog_33);
    changelog.push_str(&changelog_32);
    changelog.push_str(&changelog_31);
    changelog.push_str(&changelog_30);
    changelog.push_str(&changelog_29);
    changelog.push_str(&changelog_28);
    changelog.push_str(&changelog_27);
    changelog.push_str(&changelog_26);
    changelog.push_str(&changelog_25);
    changelog.push_str(&changelog_24);
    changelog.push_str(&changelog_23);
    changelog.push_str(&changelog_22);
    changelog.push_str(&changelog_21);
    changelog.push_str(&changelog_20);
    changelog.push_str(&changelog_19);
    changelog.push_str(&changelog_18);
    changelog.push_str(&changelog_17);
    changelog.push_str(&changelog_16);
    changelog.push_str(&changelog_15);
    changelog.push_str(&changelog_14);
    changelog.push_str(&changelog_13); 
    changelog.push_str(&changelog_12); 
    changelog.push_str(&changelog_11); 
    changelog.push_str(&changelog_10); 
    changelog.push_str(&changelog_09); 
    changelog.push_str(&changelog_08); 
    changelog.push_str(&changelog_07); 
    changelog.push_str(&changelog_06); 
    changelog.push_str(&changelog_05); 
    changelog.push_str(&changelog_04); 
    changelog.push_str(&changelog_03); 
    changelog.push_str(&changelog_02); 
    changelog.push_str(&changelog_01); 


changelog
}
