#!/usr/bin/env bash
# Toggle GNOME system HTTP(S) proxy to Cookie Monster (127.0.0.1:8877).
set -euo pipefail
ACTION="${1:-on}"
if ! command -v gsettings >/dev/null 2>&1; then
  echo "gsettings not found. Set your desktop/browser proxy to 127.0.0.1:8877 manually." >&2
  exit 1
fi
case "$ACTION" in
  on|enable|1)
    gsettings set org.gnome.system.proxy mode manual
    gsettings set org.gnome.system.proxy.http host 127.0.0.1
    gsettings set org.gnome.system.proxy.http port 8877
    gsettings set org.gnome.system.proxy.https host 127.0.0.1
    gsettings set org.gnome.system.proxy.https port 8877
    echo "GNOME proxy → 127.0.0.1:8877"
    ;;
  off|disable|0)
    gsettings set org.gnome.system.proxy mode none
    echo "GNOME proxy disabled"
    ;;
  *)
    echo "Usage: $0 [on|off]" >&2
    exit 1
    ;;
esac
