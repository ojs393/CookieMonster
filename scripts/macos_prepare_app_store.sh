#!/usr/bin/env bash
# Prepare Safari wrapper + desktop for Mac App Store / TestFlight style distribution.
# This does not upload for you — it builds artifacts and prints the Xcode / Transporter steps.
#
# Requires: Xcode, paid Apple Developer membership, App Store Connect app records.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "==> 1. Safari Web Extension + Xcode wrapper"
COOKIE_MONSTER_SAFARI_CONVERT=1 "$ROOT/scripts/build_safari_extension.sh"

WRAPPER="$ROOT/apps/safari-wrapper"
if [[ ! -d "$WRAPPER" ]]; then
  echo "Safari wrapper missing at $WRAPPER (converter may have failed)." >&2
  exit 1
fi

echo "==> 2. Desktop release app (for companion / system shield)"
"$ROOT/scripts/macos_install_app.sh"

cat <<'EOF'

==> Next steps in Xcode / App Store Connect

Safari extension app
  1. open apps/safari-wrapper/*.xcodeproj
  2. Signing & Capabilities: your Team, unique bundle ids
     - App: com.cookiemonster.safari
     - Extension: com.cookiemonster.safari.extension (typical converter layout)
  3. Add App Groups if the converter template requires shared container
  4. Product → Archive → Distribute App → App Store Connect
  5. In App Store Connect: create the macOS app, fill privacy (local-only; no tracking)
  6. Submit for review with notes that the extension talks to a local helper on loopback

Desktop Cookie Monster.app (Developer ID / optional MAS)
  - Outside MAS: ./scripts/macos_notarize_app.sh
  - Mac App Store desktop: enable App Sandbox carefully — the system shield MITM
    proxy + native host may not fit strict sandbox; prefer Developer ID notarized
    builds for the full product, and MAS for the Safari extension host if needed.

Credentials never go in git. Use env vars from macos_notarize_app.sh / Xcode keychain.
EOF
