# just-talk — 產品規格 & 技術架構文件

版本：v0.1  
日期：2026-05-09  
狀態：草稿

---

## 目錄

1. [產品規格 (Product Spec)](#1-產品規格)
2. [技術架構 (Architecture)](#2-技術架構)
3. [模組設計 (Module Design)](#3-模組設計)
4. [資料流 (Data Flow)](#4-資料流)
5. [設定檔規格 (Config Schema)](#5-設定檔規格)
6. [錯誤處理策略](#6-錯誤處理策略)
7. [相依套件選型](#7-相依套件選型)
8. [實作 Todo List](#8-實作-todo-list)

---

## 1. 產品規格

### 1.1 概述

`just-talk` 是一個跨平台語音輸入法工具，讓使用者透過按下全域快捷鍵啟動錄音，再按一次結束，錄音內容自動轉為文字並經 AI 修飾後，注入當前聚焦的輸入框或複製到剪貼板。

### 1.2 目標平台

| 平台 | 最低版本 | 備註 |
|------|---------|------|
| macOS | 12 (Monterey) | Apple Silicon & Intel |
| Linux | Ubuntu 22.04+ | X11（Wayland 需 XWayland） |
| Windows | 10 (1903+) | x86_64 only |

### 1.3 使用者旅程 (User Journey)

```
1. 使用者在任意應用程式中（瀏覽器、編輯器、聊天軟體…）
2. 按下全域快捷鍵（預設 F4）→ 顯示錄音指示（系統通知 / tray icon 變色）
3. 說話
4. 再次按下快捷鍵 → 錄音結束，顯示「轉錄中…」
5. Whisper 轉錄完成 → 顯示「AI 修飾中…」
6. AI 修飾完成 → 
   (a) 若游標在輸入框：自動貼上
   (b) 否則：存入剪貼板，顯示通知「已複製到剪貼板」
```

### 1.4 功能需求 (Functional Requirements)

| ID | 需求 |
|----|------|
| FR-01 | 全域快捷鍵，可在任何應用程式前景時觸發 |
| FR-02 | 單鍵 toggle：第一次按下開始錄音，第二次結束 |
| FR-03 | 錄音時有視覺/音效回饋（系統通知或 tray icon） |
| FR-04 | 支援本地 Whisper 模型（ggml 格式，whisper.cpp） |
| FR-05 | 支援遠端 OpenAI Whisper API |
| FR-06 | 轉錄後文字送 AI model 做語法/標點修飾 |
| FR-07 | 支援 Claude API、OpenAI API、Ollama（本地）作為 AI backend |
| FR-08 | AI 修飾可設為「不修飾」（pass-through） |
| FR-09 | 若聚焦元素為輸入框，自動注入文字 |
| FR-10 | 若無輸入框聚焦，將文字寫入剪貼板 |
| FR-11 | 所有 API Key、設定可透過 TOML 設定檔配置 |
| FR-12 | 支援多語言轉錄（透過 Whisper language 參數） |
| FR-13 | 提供 CLI 旗標覆寫設定（config path、backend 選擇…） |
| FR-14 | 應用程式以 system tray icon 常駐 |

### 1.5 非功能需求 (Non-Functional Requirements)

| ID | 需求 |
|----|------|
| NFR-01 | 熱鍵響應延遲 < 50ms |
| NFR-02 | 本地 Whisper 轉錄延遲 < 3s（base 模型，10s 語音） |
| NFR-03 | 記憶體佔用 idle 時 < 80MB |
| NFR-04 | 不需要 root / Administrator 權限 |
| NFR-05 | 設定檔變更後可熱重載（不需重啟） |
| NFR-06 | 所有錯誤以人類可讀訊息呈現，不 panic |

### 1.6 Out of Scope（本版本不做）

- GUI 設定介面（只有 CLI + TOML）
- 即時串流轉錄（STT streaming）
- 自訂喚醒詞（wake word）
- 語者辨識（speaker diarization）
- Wayland 原生支援（僅 XWayland fallback）

---

## 2. 技術架構

### 2.1 高階架構圖

```
┌─────────────────────────────────────────────────────────┐
│                    just-talk process                  │
│                                                         │
│  ┌──────────┐   event   ┌─────────────────────────────┐ │
│  │ HotkeyMgr│──────────►│       App (State Machine)   │ │
│  │ (rdev)   │           │                             │ │
│  └──────────┘           │  Idle ──► Recording         │ │
│                         │       ──► Transcribing       │ │
│  ┌──────────┐  pcm buf  │       ──► Refining           │ │
│  │AudioCapt │◄──────────│       ──► Injecting          │ │
│  │ (cpal)   │           │                             │ │
│  └──────────┘           └──────┬──────────────────────┘ │
│                                │                         │
│         ┌──────────────────────┼──────────────────────┐  │
│         ▼                      ▼                      ▼  │
│  ┌─────────────┐  ┌─────────────────┐  ┌───────────────┐│
│  │ Transcriber │  │    Refiner      │  │ OutputManager ││
│  │  ┌────────┐ │  │  ┌──────────┐  │  │  ┌─────────┐  ││
│  │  │ Local  │ │  │  │  Claude  │  │  │  │FocusDet.│  ││
│  │  │whisper │ │  │  │  OpenAI  │  │  │  │ Inject  │  ││
│  │  │ -rs    │ │  │  │  Ollama  │  │  │  │Clipboard│  ││
│  │  ├────────┤ │  │  │  None    │  │  │  └─────────┘  ││
│  │  │Remote  │ │  │  └──────────┘  │  └───────────────┘│
│  │  │OpenAI  │ │  └─────────────────┘                  │
│  │  └────────┘ │                                        │
│  └─────────────┘                                        │
│                                                         │
│  ┌──────────────────────────────────────────────────┐   │
│  │  Config (TOML)    Notification    TrayIcon        │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### 2.2 執行緒模型

```
Main thread
 ├── rdev::listen()          ← blocking, global hotkey events
 └── Tokio runtime (multi-thread)
      ├── audio_capture task  ← cpal callback → ring buffer
      ├── transcribe task     ← triggered on hotkey-off
      ├── refine task         ← triggered after transcribe
      └── output task         ← triggered after refine
```

`rdev::listen` 必須在 main thread 執行（macOS/Linux 要求）。其他所有非同步任務跑在 Tokio runtime。兩者透過 `tokio::sync::mpsc` channel 溝通。

### 2.3 狀態機定義

```
狀態              事件                  下一狀態       副作用
─────────────────────────────────────────────────────────────
Idle              HotkeyPressed         Recording      start_audio_capture()
                                                       show_notification("Recording…")

Recording         HotkeyPressed         Transcribing   stop_audio_capture()
                                                       spawn(transcribe_task)
                                                       show_notification("Transcribing…")

Recording         Error(e)              Idle           show_error(e)

Transcribing      TranscribeDone(text)  Refining       spawn(refine_task)
                                                       show_notification("Refining…")

Transcribing      Error(e)              Idle           show_error(e)

Refining          RefineDone(text)      Injecting      spawn(output_task)

Refining          Error(e)              Idle           show_error(e)

Injecting         OutputDone            Idle           show_notification("Done ✓")

Injecting         Error(e)              Idle           show_error(e)
```

---

## 3. 模組設計

### 3.1 目錄結構

```
just-talk/
├── Cargo.toml
├── Cargo.lock
├── CLAUDE.md
├── SPEC.md
├── build.rs                    # whisper-rs / platform feature flags
├── src/
│   ├── main.rs                 # CLI parsing, runtime init, app entry
│   ├── app.rs                  # State machine, event loop
│   ├── config.rs               # Config struct, TOML load/save, hot-reload
│   ├── error.rs                # JustTalkError enum (anyhow wrapper)
│   ├── notification.rs         # Cross-platform OS notification
│   ├── tray.rs                 # System tray icon (tray-icon crate)
│   ├── hotkey.rs               # Global hotkey listener (rdev)
│   ├── audio.rs                # Microphone capture, PCM ring buffer, resampler
│   ├── transcribe/
│   │   ├── mod.rs              # Transcriber trait, factory fn
│   │   ├── local.rs            # whisper-rs backend
│   │   └── remote.rs           # OpenAI Whisper API backend
│   ├── refine/
│   │   ├── mod.rs              # Refiner trait, factory fn
│   │   ├── claude.rs           # Anthropic Claude API
│   │   ├── openai.rs           # OpenAI Chat API
│   │   ├── ollama.rs           # Ollama local API
│   │   └── passthrough.rs      # No-op refiner
│   └── output/
│       ├── mod.rs              # OutputManager: orchestrate focus→inject/clipboard
│       ├── focus.rs            # Focused element type detection (platform-specific)
│       ├── inject.rs           # Keyboard injection via enigo
│       └── clipboard.rs        # Clipboard write via arboard
└── tests/
    ├── integration_transcribe.rs
    ├── integration_refine.rs
    └── integration_output.rs
```

### 3.2 核心 Trait 定義

```rust
// transcribe/mod.rs
#[async_trait]
pub trait Transcriber: Send + Sync {
    async fn transcribe(&self, pcm: &[f32], sample_rate: u32) -> Result<String>;
}

// refine/mod.rs
#[async_trait]
pub trait Refiner: Send + Sync {
    async fn refine(&self, raw_text: &str) -> Result<String>;
}

// output/focus.rs
pub enum FocusedElement {
    TextInput,   // cursor is in an editable text field
    Other,       // focused element is not a text input
    Unknown,     // accessibility query failed / not supported
}

pub fn get_focused_element_type() -> FocusedElement { ... }
```

### 3.3 App Event 型別

```rust
pub enum AppEvent {
    HotkeyPressed,
    AudioChunk(Vec<f32>),
    TranscribeDone(String),
    RefineDone(String),
    OutputDone,
    Error(JustTalkError),
    Quit,
    ReloadConfig,
}
```

### 3.4 Config 結構

```rust
#[derive(Deserialize, Clone)]
pub struct Config {
    pub hotkey: HotkeyConfig,
    pub transcribe: TranscribeConfig,
    pub refine: RefineConfig,
    pub output: OutputConfig,
}

#[derive(Deserialize, Clone)]
pub struct HotkeyConfig {
    pub key: String,              // e.g. "F4", "CapsLock"
    pub modifiers: Vec<String>,   // e.g. ["Ctrl", "Shift"]
}

#[derive(Deserialize, Clone)]
pub struct TranscribeConfig {
    pub backend: TranscribeBackend,   // Local | OpenAI
    pub model_path: Option<PathBuf>,  // for Local
    pub language: Option<String>,     // e.g. "zh", "en", "auto"
    pub openai_api_key: Option<String>,
    pub openai_model: String,         // default: "whisper-1"
}

#[derive(Deserialize, Clone)]
pub struct RefineConfig {
    pub backend: RefineBackend,   // Claude | OpenAI | Ollama | None
    pub model: String,
    pub system_prompt: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>, // for Ollama or custom endpoints
}

#[derive(Deserialize, Clone)]
pub struct OutputConfig {
    pub prefer_inject: bool,       // default: true
    pub inject_delay_ms: u64,      // default: 50ms between keystrokes
    pub clipboard_fallback: bool,  // default: true
}
```

---

## 4. 資料流

### 4.1 完整資料流時序

```
Hotkey Thread          App State Machine        Audio Task
     │                       │                      │
     │── HotkeyPressed ──────►│                      │
     │                       │── start_capture() ───►│
     │                       │   state = Recording   │
     │                       │                  [收集 f32 PCM]
     │── HotkeyPressed ──────►│                      │
     │                       │── stop() ────────────►│ returns Vec<f32>
     │                       │   state = Transcribing│
     │                       │                      │
     │              Transcribe Task                  │
     │                       │── transcribe(pcm) ───►│
     │                       │                    [whisper / API]
     │                       │◄── TranscribeDone(text)
     │                       │   state = Refining    │
     │                       │                      │
     │              Refine Task                      │
     │                       │── refine(text) ──────►│
     │                       │                  [LLM API]
     │                       │◄── RefineDone(text)   │
     │                       │   state = Injecting   │
     │                       │                      │
     │              Output Task                      │
     │                       │── get_focused_elem()  │
     │                       │  ┌── TextInput ──► enigo.type_text()
     │                       │  └── Other/Unknown ─► arboard clipboard
     │                       │◄── OutputDone         │
     │                       │   state = Idle        │
```

### 4.2 音訊處理流程

```
cpal input callback
  → f32 samples (device sample rate, e.g. 44100 Hz, stereo)
  → downmix to mono
  → resample to 16000 Hz (rubato resampler)
  → push to Vec<f32> buffer

On stop:
  → buffer 整塊送給 Transcriber
  → 如果是遠端 API：先 encode 為 WAV bytes（hound）再 multipart upload
  → 如果是本地：直接傳 &[f32] 給 whisper-rs
```

---

## 5. 設定檔規格

位置（依平台優先順序）：
1. CLI `--config <path>` 參數
2. `$JUST_TALK_CONFIG` 環境變數
3. `~/.config/just-talk/config.toml`（XDG）
4. 同執行檔目錄的 `config.toml`

### 完整預設 config.toml

```toml
[hotkey]
key = "F4"
modifiers = []          # 空 = 單鍵；可填 ["Ctrl", "Alt"] 等

[transcribe]
backend = "local"       # "local" | "openai"
model_path = "~/.local/share/just-talk/models/ggml-base.bin"
language = "auto"       # "auto" | "zh" | "en" | "ja" ...
openai_api_key = ""
openai_model = "whisper-1"

[refine]
backend = "claude"      # "claude" | "openai" | "ollama" | "none"
model = "claude-haiku-4-5-20251001"
system_prompt = """
你是一個語音輸入修飾助手。請將以下語音辨識結果修正錯別字、標點，使其通順自然。
只輸出修正後的文字，不要加任何解釋。
"""
api_key = ""
base_url = ""           # 留空使用預設；Ollama 填 http://localhost:11434

[output]
prefer_inject = true
inject_delay_ms = 20
clipboard_fallback = true
```

---

## 6. 錯誤處理策略

### 6.1 錯誤分類

```rust
#[derive(thiserror::Error, Debug)]
pub enum JustTalkError {
    #[error("Audio capture failed: {0}")]
    AudioCapture(String),

    #[error("Transcription failed: {0}")]
    Transcription(String),

    #[error("AI refinement failed: {0}")]
    Refinement(String),

    #[error("Output failed: {0}")]
    Output(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Hotkey registration failed: {0}")]
    Hotkey(String),
}
```

### 6.2 錯誤恢復策略

| 錯誤場景 | 策略 |
|---------|------|
| 音訊設備無法開啟 | 顯示通知 + 回 Idle，不崩潰 |
| 本地 model 不存在 | 啟動時檢查，提示下載指令 |
| 網路 API 超時 / 4xx | 顯示錯誤通知 + 回 Idle，支援重試（最多 2 次） |
| AI 修飾失敗 | fallback：直接輸出原始 Whisper 文字 |
| 注入失敗（無障礙權限） | fallback：寫入剪貼板 |
| 快捷鍵衝突 | 啟動時顯示警告，繼續執行 |

---

## 7. 相依套件選型

### 7.1 主要相依

| Crate | 版本 | 用途 | 選型理由 |
|-------|------|------|---------|
| `tokio` | 1.x | async runtime | 業界標準，完整生態 |
| `rdev` | 0.5 | 全域快捷鍵 | 跨平台，純 Rust API，不需 root |
| `cpal` | 0.15 | 音訊輸入 | 跨平台，支援 ASIO/WASAPI/CoreAudio/ALSA |
| `rubato` | 0.14 | PCM resampler | 高品質，無 FFI |
| `hound` | 3.5 | WAV 編解碼 | 輕量，純 Rust |
| `whisper-rs` | 0.11 | 本地 Whisper | 封裝 whisper.cpp，GPU 可選 |
| `reqwest` | 0.12 | HTTP client | tokio 整合，multipart support |
| `arboard` | 3.x | 剪貼板 | 跨平台，支援文字與圖片 |
| `enigo` | 0.2 | 鍵盤注入 | 跨平台，純 Rust API |
| `serde` + `toml` | latest | 設定檔 | 標準組合 |
| `clap` | 4.x | CLI 解析 | derive macro，功能完整 |
| `thiserror` | latest | 錯誤定義 | 減少樣板 |
| `anyhow` | 1.x | 錯誤傳播 | 應用層使用 |
| `tracing` | latest | 結構化日誌 | 優於 log crate |
| `tray-icon` | 0.14 | System tray | 跨平台 tray，與 winit 整合 |
| `notify-rust` | 4.x | OS 通知 | 跨平台系統通知 |
| `dirs` | 5.x | XDG 路徑 | config 路徑解析 |
| `notify` | 6.x | 檔案監控 | config 熱重載 |

### 7.2 平台專用相依

| 平台 | Crate | 用途 |
|------|-------|------|
| macOS | `accessibility` 0.1 | AXUIElement 聚焦偵測 |
| Windows | `windows` 0.58 | UI Automation 聚焦偵測 |
| Linux | `atspi` 0.3 | AT-SPI2 聚焦偵測 |

### 7.3 Feature Flags

```toml
[features]
default = ["local-whisper", "tray"]
local-whisper = ["whisper-rs"]   # 關閉可縮小 binary，只用 remote
tray = ["tray-icon"]             # 關閉則 headless 模式（daemon）
gpu = ["whisper-rs/cuda"]        # CUDA 加速（Linux/Windows）
```

---

## 8. 實作 Todo List

### Phase 0：專案初始化

- [x] P0-01 `cargo new just-talk --bin` 初始化專案
- [x] P0-02 撰寫完整 `Cargo.toml`（所有 dependency + features + target-specific）
- [x] P0-03 建立 `build.rs`（platform detection, whisper-rs cfg）
- [x] P0-04 建立完整目錄結構（所有 mod 的空檔案 + `mod.rs`）
- [x] P0-05 `cargo check` 確認專案結構無語法錯誤

### Phase 1：基礎設施

- [x] P1-01 `src/error.rs`：定義 `JustTalkError` 與 `thiserror`
- [x] P1-02 `src/config.rs`：Config struct + TOML 反序列化 + 預設值
- [x] P1-03 `src/config.rs`：config 路徑搜尋邏輯（CLI > env > XDG > 同目錄）
- [x] P1-04 `src/config.rs`：`notify` crate 實作 config 熱重載（發送 `ReloadConfig` event）
- [x] P1-05 `src/main.rs`：`clap` CLI 定義（`--config`, `--verbose`, `--dry-run`）
- [x] P1-06 `src/main.rs`：`tracing_subscriber` 初始化（verbose flag 控制 level）
- [x] P1-07 撰寫 config 載入的 unit test（合法 TOML、欄位缺失 fallback 預設值）

### Phase 2：通知 & Tray

- [ ] P2-01 `src/notification.rs`：封裝 `notify-rust`，`show(title, body)` 跨平台
- [ ] P2-02 `src/tray.rs`：建立 tray icon，支援 Idle / Recording 兩種圖示狀態
- [ ] P2-03 `src/tray.rs`：tray 右鍵選單（Quit、Reload Config）
- [ ] P2-04 確認 macOS / Linux / Windows 通知各自正常顯示

### Phase 3：全域快捷鍵

- [x] P3-01 `src/hotkey.rs`：用 `rdev::listen` 監聽鍵盤事件
- [x] P3-02 `src/hotkey.rs`：從 `HotkeyConfig` 解析目標按鍵與 modifier 組合
- [x] P3-03 `src/hotkey.rs`：偵測到目標快捷鍵時，透過 `mpsc::Sender` 送出 `AppEvent::HotkeyPressed`
- [x] P3-04 `src/hotkey.rs`：在獨立 thread 執行（`std::thread::spawn`），不阻塞 Tokio runtime
- [x] P3-05 測試快捷鍵 toggle 事件是否正確送出（mock sender）

### Phase 4：音訊擷取

- [ ] P4-01 `src/audio.rs`：用 `cpal` 列舉並選擇預設輸入設備
- [ ] P4-02 `src/audio.rs`：建立 `AudioCapture` struct，`start()` 開始收集 f32 PCM
- [ ] P4-03 `src/audio.rs`：`stop()` 返回完整 `Vec<f32>` buffer
- [ ] P4-04 `src/audio.rs`：stereo → mono downmix
- [ ] P4-05 `src/audio.rs`：用 `rubato` 將任意 sample rate resample 到 16000 Hz
- [ ] P4-06 `src/audio.rs`：最大錄音時長保護（預設 120s，超過自動停止）
- [ ] P4-07 撰寫 resampler unit test（輸入 44100→16000，驗證 sample count 比例）

### Phase 5：語音轉錄

- [ ] P5-01 `src/transcribe/mod.rs`：定義 `Transcriber` trait（`async fn transcribe`）
- [ ] P5-02 `src/transcribe/mod.rs`：工廠函式 `create_transcriber(config)` → `Box<dyn Transcriber>`
- [ ] P5-03 `src/transcribe/local.rs`：用 `whisper-rs` 載入 ggml 模型（lazy_static 或 OnceCell）
- [ ] P5-04 `src/transcribe/local.rs`：實作 `transcribe()`，傳入 16kHz f32 PCM
- [ ] P5-05 `src/transcribe/local.rs`：支援 `language` 參數（auto / 指定語言）
- [ ] P5-06 `src/transcribe/remote.rs`：用 `hound` 將 f32 PCM 編碼為 WAV bytes（in-memory）
- [ ] P5-07 `src/transcribe/remote.rs`：`reqwest` multipart POST 到 OpenAI `/v1/audio/transcriptions`
- [ ] P5-08 `src/transcribe/remote.rs`：解析回應 JSON，提取 `text` 欄位
- [ ] P5-09 `src/transcribe/remote.rs`：實作 retry（最多 2 次，exponential backoff）
- [ ] P5-10 integration test：用本地 model 轉錄測試 WAV 檔，驗證輸出非空

### Phase 6：AI 文字修飾

- [ ] P6-01 `src/refine/mod.rs`：定義 `Refiner` trait（`async fn refine`）
- [ ] P6-02 `src/refine/mod.rs`：工廠函式 `create_refiner(config)` → `Box<dyn Refiner>`
- [ ] P6-03 `src/refine/passthrough.rs`：no-op 實作（backend = "none"）
- [ ] P6-04 `src/refine/claude.rs`：呼叫 Anthropic Messages API（`POST /v1/messages`）
- [ ] P6-05 `src/refine/claude.rs`：使用 `system_prompt` + user message 組合 payload
- [ ] P6-06 `src/refine/claude.rs`：解析回應，提取 `content[0].text`
- [ ] P6-07 `src/refine/openai.rs`：呼叫 OpenAI Chat Completions API
- [ ] P6-08 `src/refine/openai.rs`：解析回應 `choices[0].message.content`
- [ ] P6-09 `src/refine/ollama.rs`：呼叫 Ollama `/api/chat`（non-streaming）
- [ ] P6-10 `src/refine/ollama.rs`：解析 NDJSON 回應
- [ ] P6-11 所有 refiner：API 錯誤時 fallback 原始文字（不丟棄使用者輸入）
- [ ] P6-12 integration test：mock HTTP server 驗證 Claude refiner 請求格式正確

### Phase 7：輸出模組

- [ ] P7-01 `src/output/clipboard.rs`：用 `arboard` 寫入文字到剪貼板
- [ ] P7-02 `src/output/inject.rs`：用 `enigo` 模擬鍵盤輸入文字
- [ ] P7-03 `src/output/inject.rs`：中文字元支援（`enigo::Key::Unicode`）
- [ ] P7-04 `src/output/inject.rs`：輸入間隔控制（`inject_delay_ms` config）
- [ ] P7-05 `src/output/focus.rs`：定義 `FocusedElement` enum
- [ ] P7-06 `src/output/focus.rs`：macOS 實作（`accessibility` crate，AXRole 判斷）
- [ ] P7-07 `src/output/focus.rs`：Windows 實作（UI Automation，ControlType 判斷）
- [ ] P7-08 `src/output/focus.rs`：Linux 實作（`atspi`，role == "text" 判斷）
- [ ] P7-09 `src/output/mod.rs`：`OutputManager::send(text)` 流程（focus check → inject or clipboard）
- [ ] P7-10 `src/output/mod.rs`：注入失敗時 fallback 到剪貼板
- [ ] P7-11 測試剪貼板寫入後能正確讀回

### Phase 8：App 狀態機整合

- [ ] P8-01 `src/app.rs`：定義 `AppState` enum 與 `AppEvent` enum
- [ ] P8-02 `src/app.rs`：建立 `App::new(config, ...)` — 初始化所有子系統
- [ ] P8-03 `src/app.rs`：`App::run()` — 啟動 rdev hotkey thread、Tokio runtime、event loop
- [ ] P8-04 `src/app.rs`：實作完整狀態轉移（參考 §2.3 狀態機表）
- [ ] P8-05 `src/app.rs`：`ReloadConfig` event → 重新建構 transcriber / refiner
- [ ] P8-06 `src/app.rs`：`Quit` event → 正常 shutdown（join threads, flush logs）
- [ ] P8-07 `src/main.rs`：`--dry-run` 模式：跑完整流程但 output 只印到 stdout
- [ ] P8-08 整合測試：mock hotkey events → 驗證狀態轉移序列正確

### Phase 9：打包 & 發佈

- [ ] P9-01 撰寫 `.github/workflows/ci.yml`（build + test on ubuntu / macos / windows）
- [ ] P9-02 撰寫 `.github/workflows/release.yml`（tag push → cross-compile + GitHub Release）
- [ ] P9-03 macOS：建立 `.app` bundle（`cargo-bundle` 或手動 Info.plist）
- [ ] P9-04 macOS：`entitlements.plist`（麥克風 + 無障礙 API 權限）
- [ ] P9-05 Windows：建立 NSIS 或 WiX installer
- [ ] P9-06 Linux：建立 `.deb` / `.rpm` / AppImage
- [ ] P9-07 撰寫 `README.md`（安裝、設定、模型下載說明）
- [ ] P9-08 撰寫模型下載 helper script（`scripts/download-model.sh`）

---

## 優先實作順序建議

```
Phase 0 → Phase 1 → Phase 3 → Phase 4 → Phase 5(local) 
  → Phase 6(none/passthrough) → Phase 7(clipboard) → Phase 8
  → Phase 5(remote) → Phase 6(claude/openai/ollama)
  → Phase 7(inject+focus) → Phase 2(tray) → Phase 9
```

核心功能最小可用版本（MVP）只需：P0 + P1 + P3 + P4 + P5-local + P6-passthrough + P7-clipboard + P8。

---

*文件結束*
