#!/usr/bin/env bash
# Trust Cookie Monster Local CA on Linux (NSS user DB and/or system store).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}/cookie-monster"
# directories crate may also use com.CookieMonster.cookie-monster
for candidate in \
  "$DATA/local-ca/cookie-monster-ca.cer" \
  "$HOME/.local/share/com.CookieMonster.cookie-monster/local-ca/cookie-monster-ca.cer"
do
  if [[ -f "$candidate" ]]; then
    CERT="$candidate"
    break
  fi
done
CERT="${CERT:-}"
if [[ -z "$CERT" ]]; then
  echo "CA cert not found. Start the native host once, then re-run." >&2
  exit 1
fi

if command -v certutil >/dev/null 2>&1; then
  NSS="$HOME/.pki/nssdb"
  mkdir -p "$NSS"
  certutil -d "sql:$NSS" -A -t "C,," -n "Cookie Monster Local CA" -i "$CERT"
  echo "Trusted in $NSS"
fi

if [[ -d /usr/local/share/ca-certificates ]] || sudo mkdir -p /usr/local/share/ca-certificates; then
  sudo cp "$CERT" /usr/local/share/ca-certificates/cookie-monster-ca.crt
  sudo update-ca-certificates
  echo "Installed system CA via update-ca-certificates"
fi

echo "Done: $CERT"
