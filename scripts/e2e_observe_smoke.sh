#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOST_BIN="$ROOT_DIR/target/debug/cookie-monster-native-host"
TEMP_HOME="$(mktemp -d)"
HOST_LOG="$TEMP_HOME/native-host.log"
HOST_PID=""

cleanup() {
  local status=$?
  trap - EXIT INT TERM
  if [[ -n "$HOST_PID" ]] && kill -0 "$HOST_PID" 2>/dev/null; then
    kill "$HOST_PID"
    wait "$HOST_PID" 2>/dev/null || true
  fi
  if [[ "$status" -ne 0 && -f "$HOST_LOG" ]]; then
    cat "$HOST_LOG" >&2
  fi
  rm -rf "$TEMP_HOME"
  exit "$status"
}
trap cleanup EXIT INT TERM

cd "$ROOT_DIR"
cargo build -p cookie-monster-core --bin cookie-monster-native-host

TCP_PORT="$(
  python3 - <<'PY'
import socket

with socket.socket() as sock:
    sock.bind(("127.0.0.1", 0))
    print(sock.getsockname()[1])
PY
)"
TCP_ADDR="127.0.0.1:$TCP_PORT"

HOME="$TEMP_HOME" COOKIE_MONSTER_TCP_ADDR="$TCP_ADDR" \
  "$HOST_BIN" --tcp-only >"$HOST_LOG" 2>&1 &
HOST_PID=$!

TOKEN_FILE=""
for _ in {1..100}; do
  for candidate in \
    "$TEMP_HOME/Library/Application Support/com.CookieMonster.cookie-monster/ipc.token" \
    "$TEMP_HOME/Library/Application Support/cookie-monster/ipc.token" \
    "$TEMP_HOME/.local/share/cookie-monster/ipc.token" \
    "$TEMP_HOME/.local/share/com.CookieMonster.cookie-monster/ipc.token"
  do
    if [[ -f "$candidate" ]]; then
      TOKEN_FILE="$candidate"
      break 2
    fi
  done
  if ! kill -0 "$HOST_PID" 2>/dev/null; then
    wait "$HOST_PID"
  fi
  sleep 0.05
done
[[ -n "$TOKEN_FILE" && -f "$TOKEN_FILE" ]] || {
  echo "timed out waiting for ipc.token under temp HOME=$TEMP_HOME" >&2
  exit 1
}

python3 - "$TCP_ADDR" "$TOKEN_FILE" <<'PY'
import json
import socket
import sys
import time

host, port_text = sys.argv[1].rsplit(":", 1)
token = open(sys.argv[2], encoding="utf-8").read().strip()

for attempt in range(100):
    try:
        connection = socket.create_connection((host, int(port_text)), timeout=1)
        break
    except OSError:
        if attempt == 99:
            raise
        time.sleep(0.05)

with connection:
    stream = connection.makefile("rwb")

    def request(payload):
        payload["token"] = token
        stream.write(json.dumps(payload).encode() + b"\n")
        stream.flush()
        response = stream.readline()
        if not response:
            raise AssertionError("host closed the connection without a response")
        return json.loads(response)

    event = {
        "id": "e2e-observe-smoke",
        "ts_ms": 1_700_000_000_000,
        "action": "Set",
        "party": "Third",
        "scheme": "https",
        "host": "analytics.example",
        "name": "_ga",
        "value_redacted": True,
        "attributes": {
            "secure": True,
            "http_only": False,
            "same_site": None,
            "max_age": None,
            "domain": None,
            "path": "/",
        },
    }
    decision = request({"type": "cookie_event", "event": event})
    assert decision["type"] == "decision", decision
    assert decision["decision"]["kind"] == "Allow", decision

    events = request({"type": "list_events", "limit": 10})
    assert events["type"] == "events", events
    assert len(events["items"]) == 1, events
    assert events["items"][0]["decision"]["kind"] == "Allow", events

print("Observe-mode TCP smoke test passed")
PY
