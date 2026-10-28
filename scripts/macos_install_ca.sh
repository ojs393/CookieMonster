#!/usr/bin/env bash
set -euo pipefail

CA_DIR="${COOKIE_MONSTER_DATA:-$HOME/Library/Application Support/com.CookieMonster.cookie-monster/local-ca}"
CERT="$CA_DIR/cookie-monster-ca.cer"

if [[ ! -f "$CERT" ]]; then
  echo "CA cert not found at: $CERT" >&2
  echo "Start cookie-monster-native-host once so it can create the local CA." >&2
  exit 1
fi

echo "Adding Cookie Monster Local CA to login keychain…"
security add-trusted-cert -r trustRoot -k "$HOME/Library/Keychains/login.keychain-db" "$CERT"
echo "Done. Restart browsers/apps that should trust the CA."
