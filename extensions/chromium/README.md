# Cookie Monster Chromium extension

This Manifest V3 extension observes cookie activity and forwards redacted
`cookie_event` messages to the `com.cookiemonster.host` native host. It never
blocks, edits, or removes cookies.

## Build and load

```sh
npm install
npm test
npm run build
```

Open `chrome://extensions`, enable **Developer mode**, choose **Load unpacked**,
and select this `extensions/chromium` directory.

## Register the native host

Build the host, load the unpacked extension, then install the manifest (fills
path + extension ID — no manual placeholder edits):

```sh
cargo build -p cookie-monster-core --bin cookie-monster-native-host --release
COOKIE_MONSTER_EXTENSION_ID=<id-from-chrome://extensions> \
  ../../scripts/install_native_messaging.sh chrome
```

`native-host/com.cookiemonster.host.json` is a template only
(`__COOKIE_MONSTER_*__` placeholders). The installer writes the real file to:

- macOS Chrome: `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/`
- Chromium / Edge / Firefox: see `scripts/install_native_messaging.sh`
- Linux: `~/.config/google-chrome/NativeMessagingHosts/`
- Windows: register under
  `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.cookiemonster.host`

The installed filename must remain `com.cookiemonster.host.json`.

## MV3 observation limits

`chrome.cookies.onChanged` is the primary Set/removal signal. The core event
model has no Remove action, so removals are represented as `Set` storage
changes. This API does not expose the top-level site, so these events use
`party: "First"`.

Send logging is best-effort through the non-blocking
`webRequest.onBeforeSendHeaders` API. Chrome may omit request headers for
protected or extension-internal traffic. Cookie values are never forwarded;
only names and redacted metadata are sent. First/third-party classification
compares the final two hostname labels as a lightweight eTLD+1 approximation;
multi-label public suffixes such as `co.uk` can therefore be misclassified.

If the native host is unavailable, the service worker continues allowing all
browser behavior and keeps at most 100 pending events in memory. MV3 may stop
an idle worker, so that in-memory buffer is not durable.
