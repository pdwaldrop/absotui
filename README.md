[![GitHub release](https://img.shields.io/github/v/release/pdwaldrop/absotui?label=Latest%20Release&color=green&cacheSeconds=3600)](https://github.com/pdwaldrop/absotui/releases/latest)
[![Release](https://github.com/pdwaldrop/absotui/actions/workflows/release.yml/badge.svg)](https://github.com/pdwaldrop/absotui/actions/workflows/release.yml)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-CE422B?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS-informational)

<h1 align="center">
  <img src="linux/absotui.svg" width="75" valign="middle" alt="Absotui icon"> Absotui
</h1>
<p align="center">
    <strong>"ABS" (Audiobookshelf) + "TUI" (terminal user interface)</strong><em> — read it like "absolutely."</em>
</p>

<p align="center">
    <img src="assets/screenshot.png" alt="📖 Screenshot">
</p>

<p align="center">
    A fast, gorgeous, keyboard-driven terminal client for your self-hosted <a href="https://www.audiobookshelf.org/">Audiobookshelf</a> server.
    Browse by library, series, or collection, blow through chapters, and never lose your place — no mouse, no browser tab, no leaving the terminal.
</p>

<p align="center">
  <a href="#-features">Features</a> •
  <a href="#-installation">Installation</a> •
  <a href="#-roadmap">Roadmap</a> •
  <a href="#-notes">More info</a>
</p>

---

## ✨ Features

**Never touch a mouse.** Every screen, every action — browsing, playing, searching, downloading, sorting your whole library — is one keypress away, with a complete, always-accurate reference a single <kbd>?</kbd> away too.

**Genuinely fast.** A minimalist Rust 🦀 TUI that opens instantly and stays that way, even against a huge library.

- **Books & podcasts, both first-class:** a unified "New & Unfinished" podcast home, instant mark-as-finished (<kbd>F</kbd>), and autoplay straight into the next unfinished episode
- **Browse it your way:** flip through your full Library, or jump to Collections and series — grouped, sequence-ordered, one <kbd>Tab</kbd>/<kbd>S</kbd> away
- **See your listening habits:** a full Stats page — streaks, a day-of-week chart, a year-long activity heatmap, recent sessions, and top rankings for what and who you listen to most
- **Stream or take it offline:** download (<kbd>d</kbd>) any book or episode for offline listening, with a downloaded copy preferred automatically the moment you have one — Auto Download can keep your active listens ready without you thinking about it
- **Real cover art, right in your terminal:** book and episode artwork rendered inline (Kitty/Sixel/iTerm2 terminals), not just text
- **Chapter-level navigation:** browse a book's full chapter list inline in Continue Listening, with live per-chapter progress
- **Desktop integration:** a custom app icon, a taskbar/dock icon on supported terminals, and a window title showing what's playing
- **Matches your terminal, automatically:** no theme file to maintain — the whole UI just uses whatever color scheme your terminal is already set to
- **In-app updates:** Settings > Update / Uninstall runs the whole install flow — live progress, fingerprint-or-password auth, and a relaunch on success
- **Per-item playback speed:** optionally (Settings > Per-Item Speed) save a separate speed for each book or podcast instead of sharing one across everything
- **Rock-solid sync:** accurate progress bars even at non-1x speed, a live now-playing marker, and a graceful retry screen instead of the app just closing if your server's briefly unreachable

---

## 📌 Project status
Actively developed, single-maintainer project. Low-risk by design: the Audiobookshelf API is only ever used to retrieve books/podcasts and sync playback progress, so there's no destructive operation to worry about — at worst you'd see a sync hiccup, never data loss. Check [known bugs](known_bugs.md) or [open an issue](https://github.com/pdwaldrop/absotui/issues) if something looks off.

---

## 🚀 Installation

### ⚡ Easy installation (install script)

**Run the following in your terminal, then follow the on-screen instructions:**

```bash
bash -c 'tmpfile=$(mktemp) && curl -LsSf https://github.com/pdwaldrop/absotui/raw/stable/hello_absotui.sh -o "$tmpfile" && bash "$tmpfile" install && rm -f "$tmpfile"'
```

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

In `~/.config/absotui` (Linux) or `~/Library/Preferences` (macOS) — the default path when `XDG_CONFIG_HOME` is empty:
- `.env` — contains the secret key
- `config.toml` — configuration file
- `absotui.log` — log file
- `db.sqlite3` — SQLite database file
- `covers/` — on-disk cache of cover art (populated automatically as covers load)
- `downloads/` — books and podcast episodes downloaded for offline playback (only appears once you download something)

In `~/.local/share/applications` (Linux):
- `absotui.desktop` — lets you launch Absotui from a launcher app

<details>
<summary><h3>🔧 Install from source</h3></summary>

**Requirements:** `Rust`, `VLC`

<sub>Optional: the `cvlc_term` config setting (off by default) opens a terminal to control `cvlc` directly, which additionally needs the `kitty` terminal installed.</sub>

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

---

## 🛠️ Roadmap
Recent work: a refreshed app icon; and a broad bug/optimization pass across the Stats and Collections/Series screens — fixed a book-playback position bug, a misleading "Incorrect password" error during Update, a Library series-grouping display bug, plus several efficiency and code-quality cleanups.

**Under consideration:**
- Managing podcast subscriptions from the app (add/remove)

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
Absotui renders using your terminal's own color theme automatically, with a small set of accent colors pulled from it for structure (borders, active items, keybind hints) — there's no separate config to keep in sync with your terminal. The font and emoji you see may vary depending on your terminal.

### 🖥️ Terminal compatibility
Absotui works in any modern terminal, but two features scale with what your terminal supports:

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

**Works best overall:** **Kitty**, **Ghostty**, or **WezTerm** — full-fidelity cover art and desktop integration together.

### 🙏 Credits
Absotui began as a fork of [Toutui](https://github.com/AlbanDAVID/Toutui) by [AlbanDAVID](https://github.com/AlbanDAVID), archived in December 2025 ("I'm not able to properly maintain this project anymore... please don't wait for any new releases and issue fixing."). Thanks to the original author for the foundation this project builds on. Toutui itself took its name from the French phrase "tout ouïe" ("all ears").

### 📄 License
[GPL-3.0](LICENSE)
