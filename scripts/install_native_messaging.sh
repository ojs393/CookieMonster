#!/usr/bin/env bash
# Install Chromium/Firefox native-messaging manifests (macOS + Linux).
# Windows: write the same JSON under
#   %LOCALAPPDATA%\Google\Chrome\User Data\NativeMessagingHosts\
# (or Edge/Chromium/Firefox equivalent) and point "path" at the .exe.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOST_BIN="${COOKIE_MONSTER_NATIVE_HOST:-}"
EXT_ID="${COOKIE_MONSTER_EXTENSION_ID:-}"
BROWSER="${1:-chrome}"
OS="$(uname -s)"

if [[ -z "$HOST_BIN" ]]; then
  for candidate in \
    "$ROOT/target/release/cookie-monster-native-host" \
    "$ROOT/target/debug/cookie-monster-native-host" \
    "$ROOT/apps/desktop/src-tauri/binaries/cookie-monster-native-host" \
    "$ROOT/target/release/cookie-monster-native-host.exe" \
    "$ROOT/target/debug/cookie-monster-native-host.exe"
  do
    if [[ -x "$candidate" || -f "$candidate" ]]; then
      HOST_BIN="$(cd "$(dirname "$candidate")" && pwd)/$(basename "$candidate")"
      break
    fi
  done
fi

if [[ -z "$HOST_BIN" || ! -e "$HOST_BIN" ]]; then
  echo "Build the host first: cargo build -p cookie-monster-core --bin cookie-monster-native-host --release" >&2
  exit 1
fi

# Firefox uses a fixed addon id; Chromium needs the unpacked extension id.
if [[ "$BROWSER" != "firefox" && -z "$EXT_ID" ]]; then
  echo "Set COOKIE_MONSTER_EXTENSION_ID to the unpacked extension ID from chrome://extensions" >&2
  exit 1
fi

case "$OS" in
  Darwin)
    case "$BROWSER" in
      chrome)   DEST_DIR="${HOME}/Library/Application Support/Google/Chrome/NativeMessagingHosts" ;;
      chromium) DEST_DIR="${HOME}/Library/Application Support/Chromium/NativeMessagingHosts" ;;
      edge)     DEST_DIR="${HOME}/Library/Application Support/Microsoft Edge/NativeMessagingHosts" ;;
      firefox)  DEST_DIR="${HOME}/Library/Application Support/Mozilla/NativeMessagingHosts" ;;
      *)
        echo "Usage: $0 [chrome|chromium|edge|firefox]" >&2
        exit 1
        ;;
    esac
    ;;
  Linux)
    case "$BROWSER" in
      chrome)   DEST_DIR="${HOME}/.config/google-chrome/NativeMessagingHosts" ;;
      chromium) DEST_DIR="${HOME}/.config/chromium/NativeMessagingHosts" ;;
      edge)     DEST_DIR="${HOME}/.config/microsoft-edge/NativeMessagingHosts" ;;
      firefox)  DEST_DIR="${HOME}/.mozilla/native-messaging-hosts" ;;
      *)
        echo "Usage: $0 [chrome|chromium|edge|firefox]" >&2
        exit 1
        ;;
    esac
    ;;
  *)
    echo "Native-messaging installer supports macOS and Linux today." >&2
    echo "On Windows, place the manifest JSON under the browser NativeMessagingHosts folder." >&2
    exit 1
    ;;
esac

mkdir -p "$DEST_DIR"
TMP="$(mktemp)"
if [[ "$BROWSER" == "firefox" ]]; then
  python3 - <<PY
import json
manifest = {
  "name": "cookie-monster@cookiemonster.local",
  "description": "Cookie Monster native messaging host",
  "path": r"""$HOST_BIN""",
  "type": "stdio",
  "allowed_extensions": ["cookie-monster@cookiemonster.local"],
}
open("$TMP","w").write(json.dumps(manifest, indent=2) + "\n")
PY
  DEST="$DEST_DIR/cookie-monster@cookiemonster.local.json"
else
  python3 - <<PY
import json
manifest = {
  "name": "com.cookiemonster.host",
  "description": "CookieMonster native messaging host",
  "path": r"""$HOST_BIN""",
  "type": "stdio",
  "allowed_origins": [f"chrome-extension://$EXT_ID/"],
}
open("$TMP","w").write(json.dumps(manifest, indent=2) + "\n")
PY
  DEST="$DEST_DIR/com.cookiemonster.host.json"
fi

cp "$TMP" "$DEST"
rm -f "$TMP"
echo "Installed native messaging manifest:"
echo "  $DEST"
echo "  host=$HOST_BIN"
