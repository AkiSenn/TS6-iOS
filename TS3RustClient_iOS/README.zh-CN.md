# TS3Rust Client for iOS

基于 [**ReSpeak/tsclientlib**](https://github.com/ReSpeak/tsclientlib)（Rust 实现的
TeamSpeak 3 协议库）的开源第三方 iOS 客户端，支持 iOS 14+。

> **免责声明**：本项目与 TeamSpeak Systems GmbH 无关，未经其背书或赞助。
> 不使用 TeamSpeak 官方闭源 SDK，协议层为开源的 `tsclientlib` crate。

## 架构

```
TS3RustClient_iOS/
├── rust_lib/                  # Rust 库（tsclientlib 的 C FFI 封装）
│   ├── Cargo.toml             # 依赖：tsclientlib, tokio, tracing
│   └── src/lib.rs             # ts3_connect / ts3_disconnect / ts3_send_message /
│                              # ts3_send_audio / ts3_set_audio_callback /
│                              # ts3_set_talk_callback / ts3_opus_* / init_logger
├── TS3Client/                 # UIKit 应用（iOS 14+）
│   ├── Core/Bridge/TS3RustManager.swift   # Swift <-> Rust 桥接（单例）
│   ├── Core/Bridge/AudioBridge.swift      # AVAudioEngine 采集/播放 + Opus
│   ├── Core/Utils/            # TS3Logger, KeychainManager, ServerConfigManager,
│   │                          # LanguageManager（应用内中英切换）
│   ├── UI/                    # MainTabBarController, ConnectViewController,
│   │                          # SettingsViewController
│   └── Resources/             # Info.plist, Localizable.strings, libs/ts3_rust.h
├── TS3Client.xcodeproj
└── .github/workflows/build.yml
```

## 当前功能

- 连接 / 断开自建 TeamSpeak 3 服务器
- 发送频道文本消息
- 语音：Opus 48 kHz 单声道 20 ms 帧、麦克风开关 + 按住说话、
  按用户解码混音播放、说话状态事件
- Rust 日志实时转发到界面日志视图
- 已保存服务器列表（UserDefaults）+ 密码存 Keychain
- 应用内中英文实时切换（无需重启）

语音链路：AVAudioEngine 采集 48 kHz 单声道 → `ts3_opus_encode` →
`ts3_send_audio`（非阻塞）；收包经 `ts3_set_audio_callback` 回调 →
按用户 `ts3_opus_decode` 解码混音 → 播放。说话状态（300 ms 静默判定）
由 `ts3_set_talk_callback` 通知。

## 编译要求

- Xcode 15+
- iOS 14.0+
- Rust（本地编译需要；CI 会自动安装）

## 构建

### 1. Rust 静态库

```bash
cd rust_lib
rustup target add aarch64-apple-ios x86_64-apple-ios
cargo build --release --target aarch64-apple-ios
cargo build --release --target x86_64-apple-ios
```

用 `lipo -create ...` 合并到 `TS3Client/Resources/libs/libts3_rust.a`
（CI 工作流会自动完成这一步）。

### 2. Xcode

```bash
xcodebuild build \
  -project TS3Client.xcodeproj \
  -scheme TS3RustClient \
  -destination 'platform=iOS Simulator,name=iPhone 14,OS=16.4' \
  CODE_SIGNING_ALLOWED=NO
```

### GitHub Actions

推送到 `main`/`develop` 会触发 `.github/workflows/build.yml`：先编译 iOS 版 Rust
库，合并通用静态库，构建未签名的真机应用，ad-hoc 签名后打包
`TS3RustClient-iOS.ipa`（`Payload/TS3RustClient.app`）——TrollStore 可直接安装。

## 许可证

MIT —— 见 [LICENSE](LICENSE)。`tsclientlib` crate 为 MIT OR Apache-2.0。
