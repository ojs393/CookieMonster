#!/usr/bin/env bash
# Build Safari Web Extension resources and optionally generate + patch the Xcode wrapper.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SAFARI="$ROOT/extensions/safari"
CHROME="$ROOT/extensions/chromium"
BRIDGE="$ROOT/apps/safari-bridge"

echo "Building Safari background (shared TS + Safari native bridge)…"
npm --prefix "$CHROME" install
npm --prefix "$CHROME" run build:safari

mkdir -p "$SAFARI/icons"
cp "$CHROME/icons/"icon-*.png "$SAFARI/icons/"

echo "Safari Web Extension resources ready at $SAFARI"
echo "  manifest.json + background.js + icons"
echo "  bridge sources at $BRIDGE"

inject_bridge() {
  local out="$1"
  [[ -d "$out" ]] || return 0
  echo "Injecting Safari TCP bridge into $out"
  local handler
  while IFS= read -r handler; do
    [[ -n "$handler" ]] || continue
    cp "$BRIDGE/SafariWebExtensionHandler.swift" "$handler"
    cp "$BRIDGE/HostIpc.swift" "$(dirname "$handler")/HostIpc.swift"
    echo "  patched $handler (+ HostIpc.swift)"
  done < <(find "$out" -name 'SafariWebExtensionHandler.swift' 2>/dev/null || true)
}

if command -v xcrun >/dev/null 2>&1; then
  OUT="$ROOT/apps/safari-wrapper"
  if [[ "${COOKIE_MONSTER_SAFARI_CONVERT:-}" == "1" ]]; then
    rm -rf "$OUT"
    echo "Running safari-web-extension-converter → $OUT"
    if xcrun safari-web-extension-converter "$SAFARI" \
      --project-location "$OUT" \
      --app-name "Cookie Monster Safari" \
      --bundle-identifier "com.cookiemonster.safari" \
      --swift \
      --force \
      --no-open; then
      inject_bridge "$OUT"
      echo "Open $OUT in Xcode, select your Team, enable the Safari extension, then Run."
      echo "Start Cookie Monster desktop (or native host) so ipc.token exists."
    else
      echo "Converter failed or unavailable; keep using $SAFARI as the extension bundle source." >&2
      echo "Swift bridge sources remain at $BRIDGE for manual Xcode wiring." >&2
      exit 0
    fi
  else
    echo "Skip Xcode convert (set COOKIE_MONSTER_SAFARI_CONVERT=1 to generate wrapper + inject bridge)."
    echo "Bridge sources: $BRIDGE"
  fi
else
  echo "xcrun not found — Web Extension files only (no Xcode wrapper)."
  echo "Bridge sources: $BRIDGE"
fi
