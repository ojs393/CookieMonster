#!/usr/bin/env bash
set -euo pipefail

PROXY_HOST="${COOKIE_MONSTER_PROXY_HOST:-127.0.0.1}"
PROXY_PORT="${COOKIE_MONSTER_PROXY_PORT:-8877}"
SERVICE="${COOKIE_MONSTER_NETWORK_SERVICE:-Wi-Fi}"

echo "Configuring HTTP/HTTPS proxy on \"$SERVICE\" → ${PROXY_HOST}:${PROXY_PORT}"
networksetup -setwebproxy "$SERVICE" "$PROXY_HOST" "$PROXY_PORT"
networksetup -setsecurewebproxy "$SERVICE" "$PROXY_HOST" "$PROXY_PORT"
networksetup -setwebproxystate "$SERVICE" on
networksetup -setsecurewebproxystate "$SERVICE" on
echo "Done. Revert with: scripts/macos_clear_system_proxy.sh"
echo "Override service with COOKIE_MONSTER_NETWORK_SERVICE=\"Ethernet\" if needed."
