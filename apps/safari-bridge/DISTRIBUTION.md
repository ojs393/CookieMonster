# Safari / macOS distribution

## Local / TestFlight-style

```sh
COOKIE_MONSTER_SAFARI_CONVERT=1 ./scripts/build_safari_extension.sh
./scripts/macos_prepare_app_store.sh
```

Then Archive in Xcode and upload via Organizer or Transporter.

## Developer ID (direct download, notarized)

```sh
./scripts/macos_install_app.sh
export APPLE_TEAM_ID=…
export APPLE_SIGNING_IDENTITY="Developer ID Application: …"
# APPLE_ID + APPLE_APP_SPECIFIC_PASSWORD  OR  API key trio
./scripts/macos_notarize_app.sh
```

## App Store notes

- Safari Web Extension ships inside a macOS container app (`com.cookiemonster.safari`).
- The extension bridge needs the Cookie Monster native host running (desktop or `--tcp-only`).
- Full system-shield MITM often conflicts with App Sandbox — ship the **Safari host** on MAS if you want store distribution, and the **full desktop** as a notarized Developer ID build.
- Privacy nutrition labels: data not collected; processing on-device.

Never commit Apple API keys, `.p8` files, or app-specific passwords.
