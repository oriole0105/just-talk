#!/usr/bin/env bash
# Download a Whisper GGML model to the platform-specific data directory.
# Usage: ./scripts/download-model.sh [MODEL]
# MODEL defaults to "base". Available: tiny, base, small, medium, large-v3

set -euo pipefail

MODEL="${1:-base}"
BASE_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main"
FILENAME="ggml-${MODEL}.bin"

# Determine platform data directory (mirrors dirs::data_local_dir in Rust)
case "$(uname -s)" in
  Darwin)
    DATA_DIR="${HOME}/Library/Application Support/just-talk/models"
    ;;
  Linux)
    DATA_DIR="${XDG_DATA_HOME:-${HOME}/.local/share}/just-talk/models"
    ;;
  CYGWIN*|MINGW*|MSYS*)
    DATA_DIR="${APPDATA}/just-talk/models"
    ;;
  *)
    echo "Unsupported OS: $(uname -s)" >&2
    exit 1
    ;;
esac

DEST="${DATA_DIR}/${FILENAME}"

if [ -f "$DEST" ]; then
  echo "Model already present: $DEST"
  exit 0
fi

echo "Downloading model: ${MODEL}"
echo "  → ${DEST}"

mkdir -p "$DATA_DIR"

URL="${BASE_URL}/${FILENAME}"

if command -v curl &>/dev/null; then
  curl -L --progress-bar -o "$DEST" "$URL"
elif command -v wget &>/dev/null; then
  wget -q --show-progress -O "$DEST" "$URL"
else
  echo "Error: neither curl nor wget found. Install one and retry." >&2
  exit 1
fi

# Verify the file is not HTML (happens when URL 404s on HuggingFace)
if file "$DEST" | grep -q "HTML"; then
  rm -f "$DEST"
  echo "Error: download returned an HTML page — model '${MODEL}' not found at ${URL}" >&2
  exit 1
fi

SIZE=$(du -sh "$DEST" | cut -f1)
echo "Done: ${DEST} (${SIZE})"
echo ""
echo "Add to your config.toml:"
echo "  [transcribe]"
echo "  backend = \"local\""
echo "  model_path = \"${DEST}\""
