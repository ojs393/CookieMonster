#!/usr/bin/env bash
# Sign (Developer ID) + notarize a Cookie Monster .app for distribution outside the App Store.
#
# Required env:
#   APPLE_TEAM_ID          e.g. ABCD123456
#   APPLE_SIGNING_IDENTITY e.g. "Developer ID Application: Oliver Smith (ABCD123456)"
# Auth (pick one):
#   APPLE_ID + APPLE_APP_SPECIFIC_PASSWORD
#   or APPLE_API_KEY_PATH + APPLE_API_KEY_ID + APPLE_API_ISSUER
#
# Usage:
#   ./scripts/macos_install_app.sh          # builds + installs locally
#   ./scripts/macos_notarize_app.sh [/path/to/Cookie Monster.app]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="${1:-}"
if [[ -z "$APP" ]]; then
  for candidate in \
    "/Applications/Cookie Monster.app" \
    "$ROOT/apps/desktop/src-tauri/target/release/bundle/macos/Cookie Monster.app"
  do
    if [[ -d "$candidate" ]]; then
      APP="$candidate"
      break
    fi
  done
fi
[[ -d "${APP:-}" ]] || {
  echo "App not found. Pass path or run macos_install_app.sh first." >&2
  exit 1
}

: "${APPLE_TEAM_ID:?set APPLE_TEAM_ID}"
: "${APPLE_SIGNING_IDENTITY:?set APPLE_SIGNING_IDENTITY}"

ENTITLEMENTS="$ROOT/scripts/macos/CookieMonster.entitlements"
if [[ ! -f "$ENTITLEMENTS" ]]; then
  echo "Missing $ENTITLEMENTS" >&2
  exit 1
fi

echo "Signing $APP"
codesign --force --deep --options runtime \
  --entitlements "$ENTITLEMENTS" \
  --sign "$APPLE_SIGNING_IDENTITY" \
  "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

ZIP="$(mktemp -t cookie-monster-notarize).zip"
cleanup() { rm -f "$ZIP"; }
trap cleanup EXIT

echo "Zipping for notarytool…"
ditto -c -k --keepParent "$APP" "$ZIP"

echo "Submitting to Apple notary service…"
if [[ -n "${APPLE_API_KEY_PATH:-}" ]]; then
  : "${APPLE_API_KEY_ID:?}"
  : "${APPLE_API_ISSUER:?}"
  xcrun notarytool submit "$ZIP" \
    --key "$APPLE_API_KEY_PATH" \
    --key-id "$APPLE_API_KEY_ID" \
    --issuer "$APPLE_API_ISSUER" \
    --wait
else
  : "${APPLE_ID:?set APPLE_ID or API key env}"
  : "${APPLE_APP_SPECIFIC_PASSWORD:?}"
  xcrun notarytool submit "$ZIP" \
    --apple-id "$APPLE_ID" \
    --team-id "$APPLE_TEAM_ID" \
    --password "$APPLE_APP_SPECIFIC_PASSWORD" \
    --wait
fi

echo "Stapling ticket…"
xcrun stapler staple "$APP"
xcrun stapler validate "$APP"
spctl --assess --type execute -vv "$APP" || true

echo "Notarized: $APP"
echo "Distribute this .app (or wrap in a .dmg). For Mac App Store, use scripts/macos_prepare_app_store.sh."
