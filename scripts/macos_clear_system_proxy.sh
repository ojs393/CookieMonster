#!/usr/bin/env bash
set -euo pipefail

SERVICE="${COOKIE_MONSTER_NETWORK_SERVICE:-Wi-Fi}"

echo "Disabling HTTP/HTTPS proxy on \"$SERVICE\""
networksetup -setwebproxystate "$SERVICE" off
networksetup -setsecurewebproxystate "$SERVICE" off
echo "Done."
