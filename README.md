# TS6 iOS — TeamSpeak 3/6 客户端（未签名，iOS 14+）

在 Windows 上开发、通过 GitHub Actions 的 macOS runner 编译的 iOS TeamSpeak
客户端。底层协议与语音完全使用开源的 Rust 库 [tslib](https://github.com/flamme-demon/tslib_multi)
（核心协议来自 [tsclientlib](https://github.com/ReSpeak/tsclientlib)），
**不需要 TeamSpeak 官方 SDK 授权**，可以连接自建 TeamSpeak 3/6 服务器。

产物是 ad-hoc 签名的 `TS6-iOS.ipa`，适合用 TrollStore（巨魔商店）直接安装；
同时保留 `TS6-iOS-app.zip`（内含 `TS6.app`），供越狱设备用 Filza/SSH 安装。

## 目录结构

```
rust/
  tslib_multi/                    # vendored tslib 源码（含我们的新 crate）
    crates/tslib-ios-ffi/         # 本项目新增的 C FFI（Swift 互操作层）
      include/tslib_ios.h         # cbindgen 生成的头文件（已提交）
ios/
  TS6.xcodeproj                   # Xcode 工程（iOS 14.0，不签名）
  TS6/                            # Swift 应用源码
.github/workflows/ios-build.yml   # macOS runner 编译工作流
```

## 工作方式

- Rust 侧：`tslib-ios-ffi` 把 tslib 封装成 C 接口。每个客户端句柄自带一个
  工作线程和 Tokio runtime，事件以 JSON 形式放入队列，Swift 轮询
  `tslib_client_poll_event()` 获取（连接/聊天/说话状态/语音帧）。
- 语音：Swift 用 AVAudioEngine 采集 48 kHz 单声道 PCM → Rust Opus 编码 →
  `tslib_client_send_audio()` 发送；收到语音帧后按用户解码、混音、播放。
- 身份：首次启动自动创建 TeamSpeak 身份并持久化到 Documents，之后使用同一
  身份连接，不会被服务器视为新用户。

## 通过 GitHub Actions 编译（推荐）

1. 把这个仓库推送到你的 GitHub（main/master 分支）。
2. 打开仓库的 **Actions** 页，运行 **Build unsigned iOS app**（push 会自动触发，
   也可以手动 `workflow_dispatch`）。
3. 构建完成后在 workflow run 的 **Artifacts** 下载 `TS6-iOS.ipa`
   （或 `TS6-iOS-app.zip`）。
4. TrollStore 用户：把 `TS6-iOS.ipa` 分享到 TrollStore 即可安装；
   越狱用户：解压 `TS6-iOS-app.zip` 得到 `TS6.app`，用 Filza/SSH 安装。

工作流内容：macOS runner 上交叉编译 `aarch64-apple-ios` 的 Rust 静态库 →
复制进 Xcode 工程 → `xcodebuild CODE_SIGNING_ALLOWED=NO` 打包 .app →
ad-hoc 签名后组装成 `Payload/TS6.app` 的 `.ipa`。
runner 无需任何代理。

## 在 Windows 本地开发/验证 Rust 层

需要 Rust（https://rustup.rs）。国内网络拉取 crates.io 和 GitHub 依赖时，
请为 cargo 配置代理（下面示例为 127.0.0.1:7890）：

```powershell
$env:HTTP_PROXY  = "http://127.0.0.1:7890"
$env:HTTPS_PROXY = "http://127.0.0.1:7890"
$env:CARGO_NET_GIT_FETCH_WITH_CLI = "true"
$env:GIT_CONFIG_COUNT = "1"
$env:GIT_CONFIG_KEY_0 = "http.proxy"
$env:GIT_CONFIG_VALUE_0 = "http://127.0.0.1:7890"

cd rust/tslib_multi
cargo check -p tslib-ios-ffi      # 快速检查
cargo build -p tslib-ios-ffi      # 本机（Windows host）完整编译
```

编译 iOS 静态库（`aarch64-apple-ios`）建议直接交给 GitHub Actions；如果本机
有 Xcode（即 macOS），也可以本地执行：

```bash
cd rust/tslib_multi
rustup target add aarch64-apple-ios
cargo build --release --target aarch64-apple-ios -p tslib-ios-ffi
cp target/aarch64-apple-ios/release/libtslib_ios_ffi.a ../../ios/RustLib/
```

然后：

```bash
cd ios
xcodebuild -project TS6.xcodeproj -target TS6 -configuration Release \
  -sdk iphoneos -derivedDataPath build CODE_SIGNING_ALLOWED=NO build
```

## 功能现状

- 连接自建 TeamSpeak 3/6 服务器（地址/端口/昵称/服务器密码/默认频道）
- 频道树 + 用户列表，点击频道切换，说话状态高亮
- Opus 语音收发（48 kHz 单声道，20 ms 帧），PTT 按住说话 + 麦克风开关
- 频道文本聊天
- 身份持久化

已知限制（后续可按需扩展）：
- 只支持 Opus 编解码（TS3 的 Speex/CELT 需要额外实现）
- 无文件管理器、私聊 UI、服务器管理命令
- 断线不会自动重连
- 播放是简单的 20 ms 帧调度，未做抖动缓冲优化

## 许可证说明

- `tslib`（含本项目的 FFI crate）：MIT OR Apache-2.0
- `tsclientlib`：MIT OR Apache-2.0
- `opus` / `audiopus`：BSD-3-Clause
- 参考仓库 `flamme-demon/TS6_Droid` 与汉化版 `YUAXI/TS6_Droid_CN`
  仅作架构参考；其中汉化版为 GPLv3，本项目未包含其代码。
