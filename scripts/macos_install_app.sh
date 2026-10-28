#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export PATH="${HOME}/.cargo/bin:${PATH}"

echo "Building native host…"
cargo build -p cookie-monster-core --bin cookie-monster-native-host --release

BIN_DIR="$ROOT/apps/desktop/src-tauri/binaries"
mkdir -p "$BIN_DIR"
cp -f "$ROOT/target/release/cookie-monster-native-host" "$BIN_DIR/cookie-monster-native-host"
chmod +x "$BIN_DIR/cookie-monster-native-host"

echo "Building Cookie Monster.app…"
npm --prefix "$ROOT/apps/desktop" install
npm --prefix "$ROOT/apps/desktop" run tauri -- build --bundles app

APP_SRC="$ROOT/apps/desktop/src-tauri/target/release/bundle/macos/Cookie Monster.app"
if [[ ! -d "$APP_SRC" ]]; then
  APP_SRC="$(find "$ROOT/apps/desktop" -path '*/bundle/macos/Cookie Monster.app' -type d 2>/dev/null | head -1 || true)"
fi
if [[ -z "${APP_SRC:-}" || ! -d "$APP_SRC" ]]; then
  echo "Built .app not found under bundle/macos" >&2
  exit 1
fi

HOST_DEST="$APP_SRC/Contents/MacOS/cookie-monster-native-host"
cp -f "$ROOT/target/release/cookie-monster-native-host" "$HOST_DEST"
chmod +x "$HOST_DEST"
mkdir -p "$APP_SRC/Contents/Resources/binaries"
cp -f "$ROOT/target/release/cookie-monster-native-host" \
  "$APP_SRC/Contents/Resources/binaries/cookie-monster-native-host"
chmod +x "$APP_SRC/Contents/Resources/binaries/cookie-monster-native-host"

DEST="/Applications/Cookie Monster.app"
DESKTOP="$HOME/Desktop/Cookie Monster.app"
echo "Installing to $DEST and $DESKTOP"
rm -rf "$DEST" "$DESKTOP"
cp -R "$APP_SRC" "$DEST"
cp -R "$APP_SRC" "$DESKTOP"
touch "$DEST" "$DESKTOP"
echo "Installed. Open with: open \"$DEST\""
