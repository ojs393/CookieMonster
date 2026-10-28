# Safari bridge

Swift sources that finish Safari support: the Web Extension talks to this
handler; the handler talks to `cookie-monster-native-host` on loopback IPC
(`127.0.0.1:9735` + `ipc.token`), same as the desktop app.

## Files

| File | Role |
|------|------|
| `SafariWebExtensionHandler.swift` | `NSExtensionRequestHandling` entry |
| `HostIpc.swift` | JSON-line TCP client + token read |

## Wire into Xcode

```sh
COOKIE_MONSTER_SAFARI_CONVERT=1 ./scripts/build_safari_extension.sh
```

The script:

1. Builds Safari `background.js` (Safari native messaging path)
2. Runs `safari-web-extension-converter` into `apps/safari-wrapper/` (gitignored)
3. Copies these Swift files over the generated handler stubs

Then in Xcode: select your Team, enable the Safari extension capability, Run,
and turn on **Cookie Monster** under Safari → Settings → Extensions.

Desktop or `cookie-monster-native-host --tcp-only` must be running so
`ipc.token` exists.
