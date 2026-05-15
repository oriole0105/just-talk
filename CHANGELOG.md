# Changelog

All notable changes to just-talk will be documented in this file.

## [Unreleased]

### Added
- Menu bar app conversion (macOS): just-talk now lives in the system menu bar
  instead of the Dock, with no terminal window required
  - Menu bar icon reflects current state (Idle / Recording / Processing)
  - Menu items: Open Config, Quit
  - `LSUIElement = true` hides the app from the Dock and App Switcher
- File logging via `tracing-appender`: logs are written to
  `~/Library/Logs/just-talk/just-talk.log` with daily rotation, enabling
  debugging without a terminal window open
- `.app` bundle packaging script for macOS distribution

## [0.2.0] - 2026-05-14

### Added
- Floating overlay window (eframe/egui): waveform visualiser during recording,
  status text during transcription/refinement, fade-out animation on completion
- Double-tap hotkey trigger mode (`trigger = "double_tap"`, configurable
  `double_tap_ms`) to avoid accidental activation
- `RightCmd` / `LeftCmd` as valid standalone hotkey keys

### Fixed
- **macOS 26.4 SIGTRAP crash (hotkey)**: replaced rdev with
  `CGEventSourceKeyState` polling on macOS — rdev's CGEventTap callback called
  `TSMGetInputSourceProperty` from a background thread, which macOS 26.4's new
  `dispatch_assert_queue` enforcement turned into a fatal trap
- **macOS 26.4 SIGTRAP crash (text output)**: replaced `enigo::text()` with
  clipboard write + `CGEventPost` Cmd+V simulation — enigo's Unicode→keycode
  lookup also called TSM from a background thread
- Overlay window no longer steals keyboard focus (`with_active(false)`)

### Changed
- macOS text output now uses clipboard + Cmd+V instead of character-by-character
  injection, improving reliability with Chinese IME input methods

## [0.1.0] - 2026-05-13

### Added
- Initial release
- Global hotkey listener (rdev on Linux/Windows, CGEventSourceKeyState on macOS)
- Audio capture via cpal with rubato resampling to 16 kHz mono
- Transcription backends: local whisper-rs and remote OpenAI-compatible API
  (tested with Groq `whisper-large-v3-turbo`)
- Refinement backends: Claude, OpenAI, Ollama, none
- Output: focus-aware text injection (enigo) with clipboard fallback (arboard)
- TOML config file with hot-reload via file watcher
- `--dry-run` CLI flag for testing without text injection
