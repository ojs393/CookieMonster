# Cookie Monster — Firefox

Uses the same TypeScript sources as Chromium (`../chromium/src`).

## Build and load

```sh
npm install
npm run build
```

Open `about:debugging` → This Firefox → **Load Temporary Add-on** → select
`manifest.json` in this directory.

## Register the native host

Build the host, then install the Firefox native-messaging manifest (no manual
JSON edits):

```sh
cargo build -p cookie-monster-core --bin cookie-monster-native-host --release
../../scripts/install_native_messaging.sh firefox
```

That writes `cookie-monster@cookiemonster.local.json` under the Firefox
native-messaging hosts dir (macOS Application Support or Linux
`~/.mozilla/native-messaging-hosts/`) with `allowed_extensions` set to the
addon id.

Firefox uses `allowed_extensions` instead of Chrome’s `allowed_origins`.
Temporary add-ons may need reinstall after Firefox restarts — re-run the
installer if the host path changes.
