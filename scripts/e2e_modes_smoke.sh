#!/usr/bin/env bash
set -euo pipefail
export PATH="${HOME}/.cargo/bin:${PATH}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found on PATH (tried \$HOME/.cargo/bin)" >&2
  exit 1
fi

TMP="$(mktemp -d)"
HOST_PID=""
cleanup() {
  if [[ -n "${HOST_PID}" ]]; then
    kill "$HOST_PID" 2>/dev/null || true
    wait "$HOST_PID" 2>/dev/null || true
  fi
  rm -rf "$TMP"
}
trap cleanup EXIT

PORT="$(python3 - <<'PY'
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
PY
)"
TCP_ADDR="127.0.0.1:${PORT}"

cargo build -p cookie-monster-core --bin cookie-monster-native-host --offline
BIN="$ROOT/target/debug/cookie-monster-native-host"

HOME="$TMP" COOKIE_MONSTER_TCP_ADDR="$TCP_ADDR" "$BIN" --tcp-only &
HOST_PID=$!

TOKEN_FILE=""
for _ in $(seq 1 50); do
  for candidate in \
    "$TMP/Library/Application Support/com.CookieMonster.cookie-monster/ipc.token" \
    "$TMP/Library/Application Support/cookie-monster/ipc.token" \
    "$TMP/.local/share/cookie-monster/ipc.token" \
    "$TMP/.local/share/com.CookieMonster.cookie-monster/ipc.token"
  do
    if [[ -f "$candidate" ]]; then
      TOKEN_FILE="$candidate"
      break 2
    fi
  done
  if ! kill -0 "$HOST_PID" 2>/dev/null; then
    wait "$HOST_PID" || true
    echo "native host exited early" >&2
    exit 1
  fi
  sleep 0.1
done
if [[ -z "$TOKEN_FILE" ]]; then
  echo "ipc.token not created under temp HOME=$TMP" >&2
  find "$TMP" -name 'ipc.token' 2>/dev/null || true
  exit 1
fi
TOKEN="$(tr -d '\n' < "$TOKEN_FILE")"

rpc() {
  COOKIE_MONSTER_TCP_ADDR="$TCP_ADDR" python3 - "$TCP_ADDR" "$TOKEN" "$1" <<'PY'
import json, socket, sys
addr, token, body = sys.argv[1], sys.argv[2], json.loads(sys.argv[3])
body["token"] = token
host, port = addr.split(":")
s = socket.create_connection((host, int(port)), timeout=2)
s.sendall((json.dumps(body) + "\n").encode())
buf = b""
while not buf.endswith(b"\n"):
    chunk = s.recv(4096)
    if not chunk:
        break
    buf += chunk
print(buf.decode().strip())
PY
}

rpc '{"type":"set_mode","mode":"observe"}' >/dev/null
obs="$(rpc '{"type":"cookie_event","event":{"id":"mode-evt-1","ts_ms":1,"action":"Set","party":"Third","scheme":"https","host":"ads.example","name":"_ga","value_redacted":true,"attributes":{}}}')"
echo "$obs" | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d["decision"]["kind"]=="Allow", d'

rpc '{"type":"set_mode","mode":"cookie_monster"}' >/dev/null
cm="$(rpc '{"type":"cookie_event","event":{"id":"mode-evt-2","ts_ms":2,"action":"Set","party":"Third","scheme":"https","host":"ads.example","name":"_ga","value_redacted":true,"attributes":{}}}')"
echo "$cm" | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d["decision"]["kind"]=="Deny", d'

rpc '{"type":"set_mode","mode":"moderate"}' >/dev/null
rpc '{"type":"set_allowlist","entries":[{"host":"github.com","allowed_buckets":null}]}' >/dev/null
mod="$(rpc '{"type":"cookie_event","event":{"id":"mode-evt-3","ts_ms":3,"action":"Set","party":"Third","scheme":"https","host":"api.github.com","name":"_ga","value_redacted":true,"attributes":{}}}')"
echo "$mod" | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d["decision"]["kind"]=="Allow", d'

deny="$(rpc '{"type":"cookie_event","event":{"id":"mode-evt-4","ts_ms":4,"action":"Set","party":"Third","scheme":"https","host":"ads.example","name":"_fbp","value_redacted":true,"attributes":{}}}')"
echo "$deny" | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d["decision"]["kind"]=="Deny", d'

echo "Phase-2 modes TCP smoke test passed"
