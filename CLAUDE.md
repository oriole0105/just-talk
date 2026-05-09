# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`just-talk` is a cross-platform (macOS / Linux / Windows) voice input method written in Rust. The user presses a global hotkey to start recording, presses it again to stop, the audio is transcribed via Whisper (local or remote), the transcript is refined by an AI model, and the result is either injected into the focused input field or placed on the clipboard.

## Build & Run

```bash
# Development build
cargo build

# Release (required for whisper-rs FFI performance)
cargo build --release

# Run with default config
cargo run --release

# Run with custom config
cargo run --release -- --config ~/.config/just-talk/config.toml

# Run a single test
cargo test <test_name>

# Run all tests
cargo test

# Check without building
cargo check

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt
```

### Platform-specific build prerequisites

**macOS**
- Xcode Command Line Tools for whisper.cpp compilation
- No additional system deps

**Linux**
- `libasound2-dev` (ALSA, for `cpal`)
- `libdbus-1-dev` (for AT-SPI2 / `atspi`)
- `libxtst-dev` (for `enigo` X11 key injection)
- `cmake` (for whisper.cpp)

**Windows**
- MSVC toolchain (not GNU) required for whisper-rs
- `cmake` on PATH

---

## Architecture

The app is a state machine driven by a global hotkey listener. All heavy work (transcription, AI refinement) runs on a Tokio async runtime so the hotkey loop is never blocked.

```
[Global Hotkey] ──► [Audio Capture] ──► [Transcriber] ──► [Refiner] ──► [Output]
    rdev               cpal/hound         whisper-rs           API          arboard/enigo
                                        or Whisper API       or Ollama
```

### State machine (`src/app.rs`)
- `Idle` → hotkey pressed → `Recording`
- `Recording` → hotkey pressed → `Transcribing` (spawns async task)
- `Transcribing` → done → `Refining` (spawns async task)
- `Refining` → done → `Injecting` → back to `Idle`

### Module map

| Path | Responsibility |
|------|---------------|
| `src/main.rs` | CLI arg parsing (`clap`), loads config, wires up app |
| `src/config.rs` | `Config` struct, TOML deserialization, default paths |
| `src/app.rs` | State machine, orchestrates all subsystems |
| `src/hotkey.rs` | Global hotkey listener using `rdev`; sends events on a channel |
| `src/audio.rs` | Opens mic via `cpal`, accumulates f32 PCM, resamples to 16 kHz mono for Whisper |
| `src/transcribe/mod.rs` | `Transcriber` async trait |
| `src/transcribe/local.rs` | `whisper-rs` wrapper (loads ggml model from disk) |
| `src/transcribe/remote.rs` | OpenAI Whisper API (`POST /v1/audio/transcriptions`) via `reqwest` |
| `src/refine/mod.rs` | `Refiner` async trait |
| `src/refine/claude.rs` | Anthropic Messages API |
| `src/refine/openai.rs` | OpenAI Chat API |
| `src/refine/ollama.rs` | Ollama `/api/chat` endpoint |
| `src/output/mod.rs` | Decides inject-or-clipboard after checking focus |
| `src/output/focus.rs` | Platform-specific: is the focused element a text input? |
| `src/output/inject.rs` | Types text via `enigo` (simulates keyboard) |
| `src/output/clipboard.rs` | Writes to clipboard via `arboard` |

### Platform-specific focus detection (`src/output/focus.rs`)
Uses `#[cfg(target_os)]` to compile the correct backend:
- **macOS** — `accessibility` crate (wraps `AXUIElement`): checks `AXRole == AXTextField | AXTextArea | AXComboBox`
- **Windows** — `windows` crate UI Automation (`IUIAutomationElement`), checks `ControlType`
- **Linux** — `atspi` crate (AT-SPI2 D-Bus), checks `role == text`

Falls back to clipboard-only if the accessibility query fails or is unsupported.

---

## Key Dependencies

```toml
tokio          = "1"          # async runtime
rdev           = "0.5"        # global hotkey (cross-platform)
cpal           = "0.15"       # audio input (cross-platform)
hound          = "3.5"        # WAV encoding for Whisper API upload
whisper-rs     = "0.11"       # local whisper.cpp FFI
reqwest        = "0.12"       # HTTP for remote APIs
serde          = "1"          # serialization
toml           = "0.8"        # config file
clap           = "4"          # CLI
arboard        = "3"          # clipboard
enigo          = "0.2"        # keyboard injection
anyhow         = "1"          # error handling
log + env_logger = "*"        # logging

# macOS only
[target.'cfg(target_os = "macos")'.dependencies]
accessibility  = "0.1"

# Windows only
[target.'cfg(target_os = "windows")'.dependencies]
windows        = { version = "0.58", features = ["Win32_UI_Accessibility"] }

# Linux only
[target.'cfg(target_os = "linux")'.dependencies]
atspi          = "0.3"
```

---

## Config File (`~/.config/just-talk/config.toml`)

```toml
[hotkey]
key = "F4"          # or "CapsLock", etc.

[transcribe]
backend = "local"   # "local" | "openai"
model_path = "~/.local/share/just-talk/models/ggml-base.en.bin"
openai_api_key = ""

[refine]
backend = "claude"  # "claude" | "openai" | "ollama" | "none"
model = "claude-haiku-4-5-20251001"
prompt = "Fix grammar and punctuation. Output only the corrected text."
api_key = ""
ollama_url = "http://localhost:11434"

[output]
prefer_inject = true   # try to inject; fallback to clipboard
language = "zh-TW"     # hint for Whisper
```

---

## Adding a New Transcriber / Refiner Backend

1. Create `src/transcribe/<name>.rs` (or `src/refine/<name>.rs`)
2. Implement the `Transcriber` / `Refiner` async trait
3. Add the variant to the `TranscribeBackend` / `RefineBackend` enum in `config.rs`
4. Wire it in `app.rs`'s backend constructor match

---

## Notes

- `whisper-rs` compiles whisper.cpp via a build script; first build is slow (~2 min).
- `rdev` on Linux requires the process to run as the user who owns the X session; no root needed, but Wayland support is limited (use XWayland).
- Text injection via `enigo` sends individual key events; some apps intercept at WM level and may miss rapid sequences — the clipboard fallback is always safer for long texts.
