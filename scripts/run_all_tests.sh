#!/usr/bin/env bash
set -euo pipefail
export PATH="${HOME}/.cargo/bin:${PATH}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "==> cargo test --workspace"
cargo test --workspace --offline

echo "==> chromium extension tests"
npm --prefix extensions/chromium test

echo "==> fixture server unit tests"
python3 -m unittest fixtures.sites.test_server -v 2>/dev/null \
  || (cd fixtures/sites && python3 -m unittest test_server -v)

echo "==> desktop client unit tests"
CARGO_TARGET_DIR="$ROOT/target" cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --offline client::

echo "==> e2e observe"
./scripts/e2e_observe_smoke.sh

echo "==> e2e modes"
./scripts/e2e_modes_smoke.sh

echo "All Cookie Monster test suites passed"
