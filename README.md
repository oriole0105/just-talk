# just-talk

Cross-platform voice IME — press a hotkey, speak, get text injected into any app.

```
[Hotkey] → Record mic → Whisper (local or API) → AI refine → inject / clipboard
```

## Features

- **Global hotkey** — one key to start recording, same key to stop
- **Transcription** — local Whisper (whisper.cpp via whisper-rs) or OpenAI Whisper API
- **AI refinement** — Claude, OpenAI GPT, Ollama, or passthrough (no refinement)
- **Output** — keyboard injection (enigo) with clipboard fallback
- **Config hot-reload** — edit `config.toml` without restarting
- **Dry-run mode** — print output to stdout for scripting / testing

## Installation

### Pre-built binaries

Download the latest release for your platform from the [Releases](../../releases) page:

| Platform | File |
|----------|------|
| macOS (Apple Silicon) | `just-talk-aarch64-apple-darwin` |
| macOS (Intel) | `just-talk-x86_64-apple-darwin` |
| Linux (x86-64) | `just-talk-x86_64-unknown-linux-gnu` |
| Windows (x86-64) | `just-talk-x86_64-pc-windows-msvc.exe` |

```bash
# macOS / Linux
chmod +x just-talk-*
sudo mv just-talk-* /usr/local/bin/just-talk
```

### Build from source

**Prerequisites:**
- Rust 1.75+
- macOS/Linux: standard C toolchain
- Linux: `libasound2-dev libxtst-dev libxdo-dev libxcb1-dev libxkbcommon-dev pkg-config`
- Local Whisper only: `cmake`

```bash
git clone https://github.com/oriole0105/just-talk
cd just-talk

# Default build (remote APIs only)
cargo build --release

# With local Whisper (requires cmake)
cargo build --release --features local-whisper
```

## Setup

### 1. Download a Whisper model (local transcription only)

```bash
./scripts/download-model.sh          # downloads ggml-base.bin (~150 MB)
./scripts/download-model.sh small    # higher accuracy (~500 MB)
./scripts/download-model.sh large-v3 # best accuracy (~3 GB)
```

Models are saved to:
- macOS: `~/Library/Application Support/just-talk/models/`
- Linux: `~/.local/share/just-talk/models/`
- Windows: `%APPDATA%\just-talk\models\`

### 2. Create a config file

```bash
mkdir -p ~/.config/just-talk
just-talk --config ~/.config/just-talk/config.toml   # creates default on first run
```

Or create it manually at `~/.config/just-talk/config.toml`:

```toml
[hotkey]
key = "F4"      # Any single key: F1-F12, CapsLock, ScrollLock, etc.

[transcribe]
backend = "remote"    # "local" or "remote"
# For "local" (needs --features local-whisper):
# model_path = "/path/to/ggml-base.bin"
api_key = "sk-..."    # For "remote" (OpenAI Whisper API)
language = "auto"     # BCP-47 code or "auto"

[refine]
backend = "claude"    # "claude" | "openai" | "ollama" | "passthrough"
api_key = "sk-ant-..." # Claude API key (if backend = "claude")
# api_key = "sk-..."  # OpenAI key (if backend = "openai")
model = "claude-opus-4-7"
system_prompt = "Fix punctuation and capitalisation. Return only the corrected text."

# For Ollama:
# backend = "ollama"
# base_url = "http://localhost:11434"
# model = "gemma3:27b"

[output]
prefer_inject = true      # false = always use clipboard
clipboard_fallback = true # fall back to clipboard if inject fails
inject_delay_ms = 0       # >0 = char-by-char (useful for CJK / slow apps)
```

### 3. macOS permissions

just-talk needs two permissions granted once:

1. **Microphone** — System Settings → Privacy & Security → Microphone → enable just-talk  
2. **Accessibility** — System Settings → Privacy & Security → Accessibility → enable just-talk  
   (required for `rdev` global hotkeys and `enigo` keyboard injection)

## Usage

```bash
# Start normally
just-talk

# Custom config path
just-talk --config /path/to/config.toml

# Debug logging
just-talk --verbose

# Print output to stdout instead of injecting (for testing)
just-talk --dry-run
```

**Workflow:**
1. Press the configured hotkey → status notification "Recording…"
2. Speak
3. Press the hotkey again → "Transcribing…"
4. Wait a moment → text is injected at the cursor (or copied to clipboard)

Press **Ctrl+C** to quit.

## Supported backends

### Transcription

| Backend | Key | Notes |
|---------|-----|-------|
| `local` | — | Requires `--features local-whisper` and a model file |
| `remote` | `OPENAI_API_KEY` or `api_key` in config | OpenAI Whisper API |

### Refinement

| Backend | Key | Notes |
|---------|-----|-------|
| `claude` | `ANTHROPIC_API_KEY` or `api_key` | Anthropic Messages API |
| `openai` | `OPENAI_API_KEY` or `api_key` | OpenAI Chat Completions |
| `ollama` | — | Local Ollama server at `base_url` |
| `passthrough` | — | No refinement; raw transcript is injected |

API keys can be set in `config.toml` or as environment variables (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`).

## Development

```bash
# Run all tests (no hardware required)
cargo test

# Run a single test
cargo test full_cycle_state_sequence

# Run with verbose logging
RUST_LOG=debug cargo run -- --verbose --dry-run

# Check formatting and lints
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## License

MIT
