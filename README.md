# Small Window Translator (`swtrans`)

Select text in any app, press a hotkey, get a small popup with translations.

**swtrans** is a Rust desktop translator for **macOS, Windows, and Linux**. It is written from scratch (GPUI, not Electron), and licensed MIT.

<p align="center">
  <img src="resources/icons/128x128.png" width="64" height="64" alt="Small Window Translator icon">
</p>

## Features

- Global hotkey (default **Alt+D**) — capture the current selection and translate
- Floating query window: source text, language bar, stacked provider cards
- Providers (run in parallel): **DeepL**, **OpenAI-compatible**, **LibreTranslate**, unofficial **Google** (off by default)
- Speech-to-text (mic → Whisper `/v1/audio/transcriptions`)
- Text-to-speech (system voice: `say` / SAPI / espeak)
- Settings window: hotkey, start at login, languages, keys, IPC port
- Tray menu: Translate selection, Settings, Quit
- Local trigger for Wayland and scripts (`swtrans translate-selection`)

## Install

### From a release

GitHub Actions packages installers when you push a `v*.*.*` tag, or from **Actions → Package**:

| Platform                    | Artifact              |
| --------------------------- | --------------------- |
| macOS Apple Silicon / Intel | `.dmg`                |
| Windows x64 / ARM64         | NSIS `.exe`           |
| Linux x64 / ARM64           | `.AppImage` or `.deb` |

macOS builds are **ad-hoc signed**, not notarized. Apple Silicon otherwise reports the app as **damaged**. After you copy it to `/Applications`:

```bash
xattr -cr "/Applications/Small Window Translator.app"
```

Then open it. Right-click → Open also works once the bundle is ad-hoc signed. Notarization needs an Apple Developer ID.

### From source

Needs a recent stable Rust (edition 2024).

```bash
git clone https://github.com/<you>/small-window-translator.git
cd small-window-translator
cargo run --release
```

The binary is `swtrans`.

On Linux, install build deps first:

```bash
sudo apt-get install -y build-essential pkg-config clang \
  libx11-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libxdo-dev \
  libasound2-dev libayatana-appindicator3-dev \
  libwayland-dev libfontconfig1-dev
```

On macOS, `gpui` is built with the `macos-blade` backend so you do not need a full Xcode Metal toolchain.

## Usage

1. Start `swtrans`. Settings opens on first launch; the app stays in the tray. Enable **Start at login** in General if you want it after reboot.
2. Add at least one provider key (or enable unofficial Google).
3. Select text in another app and press **Alt+D** (or the hotkey you recorded).
4. In the popup: edit the query, pick languages, copy, speak, or dictate.

| Control | Action                                                                 |
| ------- | ---------------------------------------------------------------------- |
| **🔊**  | Speak source or a translation                                          |
| **🎤**  | Speech-to-text (needs an OpenAI-compatible key or a local Whisper URL) |
| **☰**  | Settings (same window)                                                 |
| **📌**  | Pin — keep the popup open                                              |
| **Esc** | Close (or go back from Settings)                                       |

### Permissions

- **macOS:** System Settings → Privacy & Security → **Accessibility** (read selection) and **Microphone** (dictation). Enable **swtrans**.
- **Windows:** UI Automation is used when the focused control exposes a text pattern.
- **Linux:** AT-SPI when available. On **Wayland**, in-app global hotkeys do not work — bind a compositor shortcut to `swtrans translate-selection`.

If Accessibility is missing, swtrans falls back to a clipboard snapshot (copy / restore).

## Providers

Enable any combination. Empty keys are skipped.

| Provider            | Notes                                                                                                                 |
| ------------------- | --------------------------------------------------------------------------------------------------------------------- |
| DeepL               | API key. Optional Pro endpoint.                                                                                       |
| OpenAI-compatible   | `/v1/chat/completions` for translate; `/v1/audio/transcriptions` for STT. Works with OpenAI, OpenRouter, Ollama, etc. |
| LibreTranslate      | Self-hosted or public instance URL.                                                                                   |
| Google (unofficial) | No key. **Off by default** — can break and may violate Google ToS.                                                    |

## Command line

```text
swtrans                      Start the app
swtrans settings             Open Settings on a running instance
swtrans translate-selection  Trigger select-translate
swtrans --help
```

The running app listens on `127.0.0.1:18765` (configurable):

```bash
curl http://127.0.0.1:18765/selection_translate
curl http://127.0.0.1:18765/settings
```

## Config

Saved as TOML via the `directories` crate (`swtrans/config.toml`):

- macOS: `~/Library/Application Support/dev.swtrans.swtrans/config.toml`
- Linux: `~/.config/swtrans/config.toml`
- Windows: `%APPDATA%\swtrans\swtrans\config.toml`

If you already used the old `sw-dict` name, the previous config file is still read until you Save (then it writes the new path).

Defaults: hotkey `Alt+D`, source `auto`, target `zh`, IPC port `18765`.

## Package locally

```bash
cargo install cargo-packager --locked
cargo packager --release --formats app,dmg    # macOS
cargo packager --release --formats nsis       # Windows
cargo packager --release --formats appimage,deb
```

Output: `target/packager/`. Icons: `python3 scripts/gen-icon.py`.

## License

MIT
