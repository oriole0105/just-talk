#!/usr/bin/env bash
# Build just-talk.app for macOS.
#
# Usage:
#   ./scripts/package-macos.sh           # native arch (arm64 or x86_64)
#   ./scripts/package-macos.sh universal  # fat binary (arm64 + x86_64)
#
# Output: dist/just-talk.app  +  dist/just-talk-vX.Y.Z-macos[-universal].zip
#
# First-run checklist after installing the .app:
#   1. Right-click → Open  (bypasses Gatekeeper for unsigned builds)
#   2. Allow Microphone access when prompted
#   3. System Settings → Privacy & Security → Accessibility → add just-talk.app
#   4. (Optional) System Settings → General → Login Items → add just-talk.app

set -euo pipefail

# Ensure cargo is on PATH (handles both login shells and non-login invocations)
export PATH="$HOME/.cargo/bin:$PATH"

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_MACOS="$REPO_ROOT/build/macos"
DIST="$REPO_ROOT/dist"
APP_NAME="just-talk"
APP_BUNDLE="$DIST/${APP_NAME}.app"

# ---------------------------------------------------------------------------
# Version from Cargo.toml
# ---------------------------------------------------------------------------
VERSION=$(grep '^version' "$REPO_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo "==> just-talk v${VERSION}"

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
cd "$REPO_ROOT"

MODE="${1:-native}"

if [[ "$MODE" == "universal" ]]; then
    echo "==> Building universal binary (arm64 + x86_64)"
    rustup target add aarch64-apple-darwin x86_64-apple-darwin 2>/dev/null || true
    cargo build --release --target aarch64-apple-darwin
    cargo build --release --target x86_64-apple-darwin
    BINARY="$DIST/${APP_NAME}-universal-tmp"
    lipo -create \
        "target/aarch64-apple-darwin/release/${APP_NAME}" \
        "target/x86_64-apple-darwin/release/${APP_NAME}" \
        -output "$BINARY"
    ARCH_LABEL="universal"
else
    echo "==> Building native binary"
    cargo build --release
    BINARY="target/release/${APP_NAME}"
    ARCH_LABEL=$(uname -m)   # arm64 or x86_64
fi

# ---------------------------------------------------------------------------
# Assemble .app bundle
# ---------------------------------------------------------------------------
echo "==> Assembling ${APP_NAME}.app"
rm -rf "$APP_BUNDLE"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
mkdir -p "$APP_BUNDLE/Contents/Resources"

# Binary
cp "$BINARY" "$APP_BUNDLE/Contents/MacOS/${APP_NAME}"
chmod +x "$APP_BUNDLE/Contents/MacOS/${APP_NAME}"

# Info.plist — substitute __VERSION__ placeholder
sed "s/__VERSION__/${VERSION}/g" \
    "$BUILD_MACOS/Info.plist" \
    > "$APP_BUNDLE/Contents/Info.plist"

# PkgInfo (required by some macOS tools)
echo -n "APPL????" > "$APP_BUNDLE/Contents/PkgInfo"

# ---------------------------------------------------------------------------
# Icon (generate with Python + iconutil)
# ---------------------------------------------------------------------------
if command -v python3 &>/dev/null && command -v iconutil &>/dev/null; then
    echo "==> Generating app icon"
    ICONSET_DIR="$DIST/${APP_NAME}.iconset"
    mkdir -p "$ICONSET_DIR"

    # Draw a grey mic-like circle PNG at each required size.
    python3 - "$ICONSET_DIR" << 'PYEOF'
import sys, struct, zlib, os, math

def make_circle_png(path, size):
    """Write a filled circle PNG (RGBA) at the given pixel size."""
    cx = size / 2.0
    r  = size * 0.42          # circle radius
    ri = size * 0.22          # inner cutout radius (gives mic shape)

    raw = bytearray()
    for y in range(size):
        raw += b'\x00'        # filter: None
        for x in range(size):
            dx = x + 0.5 - cx
            dy = y + 0.5 - cx
            dist = math.sqrt(dx*dx + dy*dy)
            if dist <= r and dist >= ri:
                raw += bytes([80, 80, 80, 255])   # dark grey, opaque
            else:
                raw += bytes([0, 0, 0, 0])         # transparent

    def chunk(tag, data):
        c = zlib.crc32(tag + data) & 0xFFFFFFFF
        return struct.pack('>I', len(data)) + tag + data + struct.pack('>I', c)

    ihdr = struct.pack('>IIBBBBB', size, size, 8, 6, 0, 0, 0)
    out  = (b'\x89PNG\r\n\x1a\n'
            + chunk(b'IHDR', ihdr)
            + chunk(b'IDAT', zlib.compress(bytes(raw)))
            + chunk(b'IEND', b''))
    with open(path, 'wb') as f:
        f.write(out)

iconset = sys.argv[1]
sizes = [16, 32, 64, 128, 256, 512]
for s in sizes:
    make_circle_png(os.path.join(iconset, f'icon_{s}x{s}.png'), s)
    make_circle_png(os.path.join(iconset, f'icon_{s}x{s}@2x.png'), s * 2)
print(f"Icons written to {iconset}")
PYEOF

    iconutil --convert icns \
        --output "$APP_BUNDLE/Contents/Resources/${APP_NAME}.icns" \
        "$ICONSET_DIR"
    rm -rf "$ICONSET_DIR"

    # Tell Info.plist about the icon
    /usr/libexec/PlistBuddy \
        -c "Add :CFBundleIconFile string ${APP_NAME}" \
        "$APP_BUNDLE/Contents/Info.plist" 2>/dev/null || \
    /usr/libexec/PlistBuddy \
        -c "Set :CFBundleIconFile ${APP_NAME}" \
        "$APP_BUNDLE/Contents/Info.plist"
else
    echo "==> Skipping icon (python3 or iconutil not found)"
fi

# ---------------------------------------------------------------------------
# Ad-hoc code sign (no Apple Developer account required)
# ---------------------------------------------------------------------------
echo "==> Code signing (ad-hoc)"
codesign \
    --deep \
    --force \
    --sign - \
    --entitlements "$BUILD_MACOS/entitlements.plist" \
    --options runtime \
    "$APP_BUNDLE"

# Remove macOS quarantine flag so Gatekeeper doesn't nag on first launch.
xattr -dr com.apple.quarantine "$APP_BUNDLE" 2>/dev/null || true

# ---------------------------------------------------------------------------
# Create distributable zip
# ---------------------------------------------------------------------------
ZIP_NAME="${APP_NAME}-v${VERSION}-macos-${ARCH_LABEL}.zip"
echo "==> Creating $ZIP_NAME"
cd "$DIST"
zip -r --quiet "$ZIP_NAME" "${APP_NAME}.app"
cd "$REPO_ROOT"

# Clean up universal tmp binary if created
[[ "$MODE" == "universal" ]] && rm -f "$DIST/${APP_NAME}-universal-tmp"

# ---------------------------------------------------------------------------
# Install to /Applications
# ---------------------------------------------------------------------------
INSTALL_PATH="/Applications/${APP_NAME}.app"
echo "==> Installing to $INSTALL_PATH"
rm -rf "$INSTALL_PATH"
cp -r "$APP_BUNDLE" "$INSTALL_PATH"

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------
echo ""
echo "Built and installed successfully:"
echo "  App:     $INSTALL_PATH"
echo "  Archive: $DIST/$ZIP_NAME"
echo ""
echo "First-run setup (one-time):"
echo "  1. Double-click /Applications/just-talk.app to launch"
echo "     (or: open /Applications/just-talk.app)"
echo "  2. Allow Microphone access when prompted"
echo "  3. System Settings → Privacy & Security → Accessibility → add just-talk.app"
echo ""
echo "Auto-start on login:"
echo "  System Settings → General → Login Items → add just-talk.app"
