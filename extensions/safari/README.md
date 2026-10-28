# Cookie Monster — Safari

Safari ships as a **Safari Web Extension** inside a macOS wrapper app (Xcode).
The extension uses the same cookie logic as Chromium; a Swift bridge forwards
messages to `cookie-monster-native-host` over loopback IPC.

## What’s here

| Path | Role |
|------|------|
| `manifest.json` | MV3 Safari extension manifest |
| `background.js` | Built from shared Chromium TypeScript with Safari native messaging |
| `icons/` | Cookie-gear icons |
| `../../apps/safari-bridge/` | Swift handler + HostIpc TCP client |

## Build

```sh
./scripts/build_safari_extension.sh
```

Generate the Xcode wrapper and inject the bridge:

```sh
COOKIE_MONSTER_SAFARI_CONVERT=1 ./scripts/build_safari_extension.sh
open apps/safari-wrapper
```

In Xcode: select your Team, enable the Safari extension capability, Run, then
enable **Cookie Monster** under Safari → Settings → Extensions.

## Native host bridge

Safari does not use Chrome’s nativeMessaging host JSON. Instead:

1. Extension JS calls `browser.runtime.sendNativeMessage("com.cookiemonster.safari", …)`
2. `SafariWebExtensionHandler` (in the wrapper) receives the message
3. `HostIpc` forwards it to `127.0.0.1:9735` with `ipc.token`

Start Cookie Monster desktop (or `cookie-monster-native-host --tcp-only`) so the
token file exists before testing the extension.

## Permissions

cookies · native messaging · broad website access (user-granted).
All processing stays on-device.
