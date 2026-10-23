[![GitHub release](https://img.shields.io/github/v/release/pdwaldrop/absotui?label=Latest%20Release&color=green&cacheSeconds=3600)](https://github.com/pdwaldrop/absotui/releases/latest)
[![Release](https://github.com/pdwaldrop/absotui/actions/workflows/release.yml/badge.svg)](https://github.com/pdwaldrop/absotui/actions/workflows/release.yml)
[![CI](https://github.com/pdwaldrop/absotui/actions/workflows/ci.yml/badge.svg)](https://github.com/pdwaldrop/absotui/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-CE422B?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS-informational)
[![Awesome Ratatui](https://img.shields.io/badge/Awesome-Ratatui-e43716?style=flat&logo=rust&logoColor=white)](https://github.com/ratatui/awesome-ratatui)

<h1 align="center">
  <img src="linux/absotui.svg" width="75" valign="middle" alt="Absotui icon"> Absotui
</h1>
<p align="center">
    <strong>"ABS" (Audiobookshelf) + "TUI" (terminal user interface)</strong><em> — read it like "absolutely."</em>
</p>

<p align="center">
    <img src="assets/demo.gif" alt="📖 Demo">
</p>

<p align="center">
    A fast, keyboard-driven terminal client for your self-hosted <a href="https://www.audiobookshelf.org/">Audiobookshelf</a> server.
    Browse by library, series, or collection, and navigate chapters.
</p>

<p align="center">
  <a href="#-features">Features</a> •
  <a href="#-installation">Installation</a> •
  <a href="#%EF%B8%8F-roadmap">Roadmap</a> •
  <a href="#-notes">More info</a>
</p>

---

## ✨ Features

Absotui is a minimalist TUI built in Rust, for both audiobooks and podcasts. It opens fast, barely touches your CPU while it's just sitting there, and stays quick even with a huge library.

- **Podcast home:** one "New & Unfinished" list across all your shows, instant mark-as-finished (<kbd>F</kbd>), and auto-play straight into the next unfinished episode
- **Manage podcast subscriptions:** add a show by name or a direct RSS URL (<kbd>A</kbd>), with cover art and a description shown as you browse the results, or remove one (<kbd>C</kbd>) and choose whether to keep or delete its downloaded files. It's all done from Library, with no separate screen
- **Browse libraries:** flip through your Library, or jump to Collections and series (grouped, in sequence order) with <kbd>Tab</kbd>/<kbd>S</kbd>
- **Listening stats:** a Stats page with streaks, a day-of-week chart, a year-long activity heat-map, recent sessions, and top lists for what and who you listen to most
- **Stream or listen offline:** download (<kbd>d</kbd>) any book or episode for offline listening. A downloaded copy is used automatically when you have one, and Auto Download can keep your current listens downloaded for you
- **Cover art in your terminal:** book and episode artwork shown inline (Kitty/Ghostty/Sixel/iTerm2 terminals)
- **Chapter navigation:** browse a book's chapter list inline in Continue Listening, with progress for each chapter
- **Desktop integration:** a custom app icon, a taskbar/dock icon on supported terminals, and a window title showing what's playing
- **Uses your terminal's colors:** follows your terminal color scheme/theme
- **In-app updates:** Settings > Update / Uninstall runs the install flow without leaving the app
- **Pick your player:** VLC or mpv, whichever you like better. The installer asks, and you can switch any time in Settings > Player
- **Per-item playback speed:** optionally (Settings > Per-Item Speed) save a separate speed for each book or podcast instead of sharing one across everything
- **Progress sync:** keeps audiobook and podcast episode progress in sync with your server
- **SSO sign-in:** log in with a username and password, or through your server's identity provider (OpenID Connect). See [Signing in with SSO](#-signing-in-with-sso)

---

## 📌 Project status
Actively developed, single-maintainer project. Absotui reads your library and syncs playback progress, and the only other thing it changes on your server is podcast subscriptions: <kbd>A</kbd> adds a show (and downloads the episodes you pick) and <kbd>C</kbd> removes one. A "hard" remove also deletes that show's downloaded files, and it always asks first. Both need an Audiobookshelf account with permission to do that. Check [known bugs](known_bugs.md) or [open an issue](https://github.com/pdwaldrop/absotui/issues) if something looks off.

---

## 🚀 Installation

### ⚡ Easy installation (install script)

**Run the following in your terminal, then follow the on-screen instructions:**

```bash
bash -c 'tmpfile=$(mktemp) && curl -LsSf https://github.com/pdwaldrop/absotui/raw/stable/hello_absotui.sh -o "$tmpfile" && bash "$tmpfile" install && rm -f "$tmpfile"'
```

The installer asks which player to use, VLC (the default) or mpv, and installs that one. You can switch later in **Settings > Player** (the other player has to be installed). To skip the question, set `ABSOTUI_PLAYER=vlc` or `ABSOTUI_PLAYER=mpv` before running it.

#### Update
Run `absotui --update`, or quit the app and run the following in your terminal:

```bash
bash -c 'tmpfile=$(mktemp) && curl -LsSf https://github.com/pdwaldrop/absotui/raw/stable/hello_absotui.sh -o "$tmpfile" && bash "$tmpfile" update && rm -f "$tmpfile"'
```

#### Uninstall
Run `absotui --uninstall`, or quit the app and run the following in your terminal:

```bash
bash -c 'tmpfile=$(mktemp) && curl -LsSf https://github.com/pdwaldrop/absotui/raw/stable/hello_absotui.sh -o "$tmpfile" && bash "$tmpfile" uninstall && rm -f "$tmpfile"'
```

#### Files installed
In `/usr/local/bin` (option 1, from install script) or `~/.cargo/bin` (option 2, from install script):
- `absotui` — the binary file

In `~/.config/absotui` (Linux) or `~/Library/Preferences/absotui` (macOS) — the default path when `XDG_CONFIG_HOME` is empty:
- `.env` — contains the secret key
- `config.toml` — configuration file
- `absotui.log` — log file (it starts over at 5 MB, and the last one is kept as `absotui.log.old`)
- `db.sqlite3` — SQLite database file
- `covers/` — on-disk cache of cover art (populated automatically as covers load)
- `downloads/` — books and podcast episodes downloaded for offline playback (only appears once you download something)

In `~/.local/share/applications` (Linux):
- `absotui.desktop` — lets you launch Absotui from a launcher app

### 📦 AppImage (Linux)

Download it, make it executable, and run it:

```bash
curl -LO https://github.com/pdwaldrop/absotui/releases/latest/download/absotui-x86_64.AppImage
chmod +x absotui-x86_64.AppImage
./absotui-x86_64.AppImage
```

Playback needs VLC (`cvlc`) or mpv installed on your system. The AppImage doesn't bundle either (the install script above installs the one you pick).  
VLC is used by default. To use `mpv` instead, switch it in **Settings > Player** (or set `backend = "mpv"` under `[player]` in `config.toml`).

On aarch64, use `absotui-aarch64.AppImage` instead. The AppImage includes update information, so AppImageUpdate and AppImageLauncher can find new releases on their own. It's separate from the install script above, so use whichever you prefer.

<details>
<summary><h3>🔧 Install from source</h3></summary>

**Requirements:** `Rust`, `VLC` (default) or `mpv` (switch in **Settings > Player**, or set `backend = "mpv"` under `[player]` in `config.toml`)

<sub>Optional (VLC only): the `cvlc_term` config setting (off by default) opens a terminal to control `cvlc` directly, which additionally needs the `kitty` terminal installed.</sub>

Note: `main` might be unstable — prefer `git clone --branch stable --single-branch https://github.com/pdwaldrop/absotui` if you want the last stable release.

```bash
git clone https://github.com/pdwaldrop/absotui
cd absotui/
mkdir -p ~/.config/absotui
cp config.example.toml ~/.config/absotui/config.toml
```

Token encryption in the database (<u>**NOTE**</u>: replace `secret`):
```bash
echo ABSOTUI_SECRET_KEY=secret >> ~/.config/absotui/.env
```

```bash
cargo run --release
```

#### Update
When a new release is available:
```bash
git pull https://github.com/pdwaldrop/absotui
cargo run --release
```

#### Notes
Run the binary directly:
```bash
cd target/release
./absotui
```

Files installed — same as above (`.env`, `config.toml`, `absotui.log`, `db.sqlite3`, `covers/`, `downloads/`), all under `~/.config/absotui`.

</details>

### 🔑 Signing in with SSO

If your Audiobookshelf server signs people in through an identity provider (authentik, Keycloak, ...), Absotui can too. Enter your server address as usual. If the server offers SSO next to passwords, leave the username empty; if it only offers SSO, Absotui goes straight to it. Your browser opens to your identity provider and you're signed in when you come back.

**One-time setup for the server admin:** Absotui receives the sign-in on your own machine, at `http://127.0.0.1:41729/callback`. Audiobookshelf only accepts addresses it's been told about, so add that one under **Settings > Authentication > Allowed Mobile Redirect URIs**. If the Audiobookshelf mobile app already signs in with SSO on your server, nothing needs to change on the identity provider's side. Absotui tells you the exact address if the server turns it down.

Something else already using port 41729? Set `sso_callback_port` under `[auth]` in `config.toml`, and allow the matching address instead.

Running over SSH, or on a machine without a browser? Absotui shows the sign-in link. Open it in any browser, and when that browser ends up on a page that won't load, copy the address from its address bar and paste it into the box on the Absotui screen.

This uses the same OpenID support as the Audiobookshelf mobile app, so it needs a reasonably recent server.

---

## 🛠️ Roadmap
Recent work: you can play through mpv as well as VLC, sign in through your server's SSO (OpenID Connect), and grab the AppImage on Linux. I also spent a while making it lighter, so it uses a lot less CPU while it's open and no longer waits on GitHub to start.

Planned: an AUR package (`absotui-bin`), on hold until AUR registrations reopen.

See [known bugs](known_bugs.md) for what's still outstanding.

---

## 📝 Notes

### 🐛 Issues
Check the [issues](https://github.com/pdwaldrop/absotui/issues) list first, then open a new one if yours isn't there.

### 🤝 Contributing
Contributions of code, ideas, or feedback are welcome — see the [contributing guidelines](CONTRIBUTING.md) first.

### 🔁 Branching workflow
This project follows [this branching workflow](https://gist.github.com/digitaljhelms/4287848).

### 🎨 UI
Absotui uses your terminal's own color theme, with a few accent colors taken from it for borders, active items, and keybind hints. There's no separate theme config to keep in sync. The font and emoji you see depend on your terminal.

### 🖥️ Terminal compatibility
Absotui works in any modern terminal, but cover art and the taskbar/dock icon depend on what your terminal supports:

| Terminal | Cover art | Taskbar/dock icon & title |
|---|:---:|:---:|
| **Kitty** | ✅ Full | ✅ |
| **Ghostty** | ✅ Full | ✅ |
| **WezTerm** | ✅ Full | ✅ |
| iTerm2 (macOS) | ✅ Full | — |
| Alacritty | Fallback¹ | ✅ |
| Foot | Fallback¹ | ✅ |
| Other | Fallback¹ | Terminal default |

<sub>¹ Cover art still shows via a Unicode-block fallback instead of a real image — the terminal just doesn't speak the Kitty graphics protocol, the iTerm2 image protocol, or Sixel, which are auto-detected at startup.</sub>

**Works best in:** **Kitty**, **Ghostty**, or **WezTerm**, which support both cover art and the taskbar/dock icon.

### 🙏 Credits
Absotui began as a fork of [Toutui](https://github.com/AlbanDAVID/Toutui) by [AlbanDAVID](https://github.com/AlbanDAVID), archived in December 2025 ("I'm not able to properly maintain this project anymore... please don't wait for any new releases and issue fixing."). Thanks to the original author for getting it started. Toutui itself took its name from the French phrase "tout ouïe" ("all ears").

### 📄 License
[GPL-3.0](LICENSE)
