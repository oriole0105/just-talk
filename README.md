# just-talk

Cross-platform voice IME — press a hotkey, speak, get text injected into any app.

```
[Hotkey] → Record mic → Whisper (local or API) → AI refine → inject / clipboard
```

## Features

- **Global hotkey** — double-tap Right Command (macOS) or configurable key to start/stop recording
- **Menu bar icon** — runs headless in the background; no open terminal required (macOS / Windows)
- **Transcription** — local Whisper (whisper.cpp) or any OpenAI-compatible API (Groq, OpenAI, etc.)
- **AI refinement** — Claude, OpenAI GPT, Ollama, or passthrough (no refinement)
- **Output** — keyboard injection with clipboard fallback
- **Config hot-reload** — edit `config.toml` without restarting
- **File logging** — logs written to `~/Library/Logs/just-talk/` (macOS) for debugging without a terminal

## Installation — macOS

### Option A: .app bundle (recommended)

The `.app` bundle integrates properly with macOS: Login Items, Accessibility permissions, and the menu bar icon all work reliably.

**Step 1 — Build the app**

```bash
git clone https://github.com/oriole0105/just-talk
cd just-talk
./scripts/package-macos.sh          # native arch (arm64 or x86_64)
# ./scripts/package-macos.sh universal   # fat binary for both arches
```

This builds the binary, assembles `just-talk.app`, and installs it to `/Applications/just-talk.app`.

**Prerequisites:** Xcode Command Line Tools (`xcode-select --install`), Rust (`rustup`).

**Step 2 — First launch**

Double-click `/Applications/just-talk.app` (or `open /Applications/just-talk.app`).  
macOS will prompt for Microphone access — allow it.

**Step 3 — Accessibility permission (required once)**

```
System Settings → Privacy & Security → Accessibility → add just-talk
```

**Step 4 — Auto-start on login (optional)**

```
System Settings → General → Login Items → add just-talk
```

After this, just-talk starts automatically on login and runs silently in the menu bar.

---

### Option B: raw binary (simpler, no menu bar auto-start)

Use this if you just want to run just-talk from a terminal or script.

```bash
# Apple Silicon
curl -L https://github.com/oriole0105/just-talk/releases/latest/download/just-talk-aarch64-apple-darwin -o just-talk

# Intel Mac
curl -L https://github.com/oriole0105/just-talk/releases/latest/download/just-talk-x86_64-apple-darwin -o just-talk

chmod +x just-talk
sudo mv just-talk /usr/local/bin/just-talk
```

> **Note:** With the raw binary, Login Items won't work (macOS only accepts `.app` bundles there).
> Accessibility permissions may also reset after system updates. Use Option A for a permanent install.

---

### Transferring to another Mac

The quickest way to move just-talk (including your API keys) to a new Mac:

```bash
# On the old Mac — AirDrop or scp the app + config
airdrop /Applications/just-talk.app          # drag to AirDrop in Finder
scp ~/.config/just-talk/config.toml newmac:~/config_just_talk.toml
```

On the new Mac:

```bash
# Accept the AirDrop .app → move to /Applications
mkdir -p ~/.config/just-talk
mv ~/config_just_talk.toml ~/.config/just-talk/config.toml
```

Then repeat the Accessibility + Microphone permission steps (Step 2–3 above) — permissions are machine-specific.

---

## Installation — Linux

```bash
curl -L https://github.com/oriole0105/just-talk/releases/latest/download/just-talk-x86_64-unknown-linux-gnu -o just-talk
chmod +x just-talk
sudo mv just-talk /usr/local/bin/just-talk
```

**System dependencies:** `libasound2 libxtst6 libxdo3`

## Installation — Windows

Download `just-talk-x86_64-pc-windows-msvc.exe` from the [Releases](../../releases) page.  
The binary is statically linked — no Visual C++ Redistributables required.

To run at startup: add the `.exe` to **Task Scheduler** or copy to your Startup folder (`shell:startup`).

---

## Build from source

```bash
git clone https://github.com/oriole0105/just-talk
cd just-talk

# Default (remote APIs — Groq, OpenAI, etc.)
cargo build --release

# With local Whisper (requires cmake + LLVM)
cargo build --release --features local-whisper
```

**Linux build deps:** `libasound2-dev libxtst-dev libxdo-dev libxcb1-dev libxkbcommon-dev pkg-config`  
**Windows local-whisper deps:** `cmake`, `llvm` (via `choco install cmake llvm`)

---

## Configuration

Config file location: `~/.config/just-talk/config.toml`

```bash
mkdir -p ~/.config/just-talk
# just-talk creates a default config on first run, or create it manually:
```

### Minimal config — Groq API (recommended)

Groq provides a free Whisper API endpoint that is OpenAI-compatible and very fast:

```toml
[hotkey]
key = "RightMeta"   # double-tap to start/stop recording

[transcribe]
backend = "openai"
base_url = "https://api.groq.com/openai/v1"
api_key = "gsk_..."     # your Groq API key
model = "whisper-large-v3-turbo"
language = "auto"

[refine]
backend = "passthrough"   # no AI refinement; raw transcript is injected

[output]
prefer_inject = true
clipboard_fallback = true
```

### Config with AI refinement

```toml
[refine]
backend = "claude"
api_key = "sk-ant-..."
model = "claude-haiku-4-5-20251001"
system_prompt = "Fix punctuation and capitalisation. Return only the corrected text."
```

### Local Whisper (offline, no API key needed)

Requires a model file and `--features local-whisper` build:

```bash
# Download a model
mkdir -p ~/Library/Application\ Support/just-talk/models
curl -L https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin \
  -o ~/Library/Application\ Support/just-talk/models/ggml-base.bin
```

```toml
[transcribe]
backend = "local"
model_path = "/Users/<you>/Library/Application Support/just-talk/models/ggml-base.bin"
language = "auto"
```

---

## Usage

```
just-talk [OPTIONS]

Options:
  --config <PATH>   Path to config file (default: ~/.config/just-talk/config.toml)
  --verbose         Enable debug logging
  --dry-run         Print output to stdout instead of injecting (for testing)
```

**Workflow:**
1. Double-tap Right Command (or configured hotkey) → overlay shows "Recording…"
2. Speak
3. Double-tap again → "Transcribing…" → "Refining…" → text injected at cursor

The menu bar icon reflects the current state:
- **Grey circle** — idle
- **Red circle** — recording
- **Yellow circle** — transcribing / refining

Right-click the menu bar icon to open the config file or quit.

---

## Supported backends

### Transcription

| Backend | Config `backend` | Notes |
|---------|-----------------|-------|
| Groq Whisper API | `"openai"` + `base_url = "https://api.groq.com/openai/v1"` | Fast, free tier available |
| OpenAI Whisper API | `"openai"` | Requires OpenAI API key |
| Local whisper.cpp | `"local"` | Requires `--features local-whisper` + model file |

### Refinement

| Backend | Config `backend` | Notes |
|---------|-----------------|-------|
| None | `"passthrough"` | Raw transcript injected as-is |
| Claude | `"claude"` | Anthropic Messages API |
| OpenAI | `"openai"` | OpenAI Chat Completions |
| Ollama | `"ollama"` | Local Ollama server |

---

## Debugging

Logs are written to:
- **macOS:** `~/Library/Logs/just-talk/just-talk.log.YYYY-MM-DD`
- **Linux/Windows:** `~/.local/share/just-talk/logs/`

```bash
# Follow live logs
tail -f ~/Library/Logs/just-talk/just-talk.log.*

# Or run with terminal output
just-talk --verbose
```

---

## Development

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
RUST_LOG=debug cargo run -- --verbose --dry-run
```

---

## License

MIT
