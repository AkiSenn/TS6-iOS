# TS3Rust Client for iOS

An open-source, unofficial TeamSpeak 3 client for iOS 14+, built on
[**ReSpeak/tsclientlib**](https://github.com/ReSpeak/tsclientlib) — a Rust
implementation of the TeamSpeak 3 protocol.

> **Disclaimer** — This project is **not affiliated with, endorsed by, or
> sponsored by TeamSpeak Systems GmbH**. It does **not** use the proprietary
> TeamSpeak SDK. The protocol layer is the open-source `tsclientlib` crate.

## Architecture

```
TS3RustClient_iOS/
├── rust_lib/                  # Rust library (C FFI around tsclientlib)
│   ├── Cargo.toml             # deps: tsclientlib, tokio, tracing
│   └── src/lib.rs             # ts3_connect / ts3_disconnect / ts3_send_message /
│                              # ts3_send_audio / ts3_set_audio_callback /
│                              # ts3_set_talk_callback / ts3_opus_* / init_logger
├── TS3Client/                 # UIKit app (iOS 14+)
│   ├── Core/Bridge/TS3RustManager.swift   # Swift <-> Rust bridge (singleton)
│   ├── Core/Bridge/AudioBridge.swift      # AVAudioEngine capture/playback + Opus
│   ├── Core/Utils/            # TS3Logger, KeychainManager, ServerConfigManager,
│   │                          # LanguageManager (in-app EN/中文 switching)
│   ├── UI/                    # MainTabBarController, ConnectViewController,
│   │                          # SettingsViewController
│   └── Resources/             # Info.plist, Localizable.strings, libs/ts3_rust.h
├── TS3Client.xcodeproj
```

The CI workflow lives at the repository root (GitHub only picks up
`.github/workflows/` from the root): `.github/workflows/ts3rust-build.yml`.
```

## Current scope

- Connect / disconnect to a self-hosted TeamSpeak 3 server
- Send channel text messages
- Voice: Opus 48 kHz mono 20 ms frames, mic toggle + hold-to-talk,
  per-user decode/mix playback, talk-status events
- Rust logs forwarded to the UI log view
- Saved server list (UserDefaults) + password via Keychain
- In-app language switching (real-time, no restart)

Voice pipeline: AVAudioEngine captures 48 kHz mono → `ts3_opus_encode` →
`ts3_send_audio` (non-blocking); incoming frames arrive via
`ts3_set_audio_callback` → per-user `ts3_opus_decode` → mix → playback.
Talk status (300 ms idle timeout) is delivered via `ts3_set_talk_callback`.

## Requirements

- Xcode 15+
- iOS 14.0+
- Rust (for local builds; CI installs it automatically)

## Build

### 1. Rust static library

```bash
cd rust_lib
rustup target add aarch64-apple-ios x86_64-apple-ios
cargo build --release --target aarch64-apple-ios
cargo build --release --target x86_64-apple-ios
```

Merge with `lipo -create ...` into `TS3Client/Resources/libs/libts3_rust.a`
(the CI workflow does this automatically).

### 2. Xcode

```bash
xcodebuild build \
  -project TS3Client.xcodeproj \
  -scheme TS3RustClient \
  -destination 'platform=iOS Simulator,name=iPhone 14,OS=16.4' \
  CODE_SIGNING_ALLOWED=NO
```

### GitHub Actions

Pushes to `main`/`develop` trigger `.github/workflows/ts3rust-build.yml`: it compiles the
Rust library for iOS, merges the universal static library, builds the unsigned
device app, ad-hoc signs it, and packages `TS3RustClient-iOS.ipa`
(`Payload/TS3RustClient.app`) — installable with TrollStore.

## License

MIT — see [LICENSE](LICENSE). The `tsclientlib` crate is MIT OR Apache-2.0.
