# TS6 iOS

适用于 iOS 14+ 的非官方 TeamSpeak 3/6 客户端，重点兼容 TeamSpeak 3 服务器。
使用 Swift、AVAudioEngine、Rust `tslib` 和 Opus 构建，不包含 TeamSpeak 官方 SDK。

## 安装

从 [Releases](https://github.com/AkiSenn/TS6-iOS/releases/latest) 下载 `TS6-iOS.ipa`：

- TrollStore：直接导入 IPA。
- 越狱设备：使用 TrollStore、Filza 或其他签名工具安装。

应用首次启动会依次申请本地网络和麦克风权限，请全部允许。

## 兼容性

- iOS 14.0 及以上
- arm64 / arm64e 设备，包括 iPhone XR iOS 14.8
- TeamSpeak 3 Opus Voice / Opus Music

## 功能

- 服务器连接、密码及频道切换
- 频道和用户列表
- Opus 语音收发、麦克风开关和按住说话
- 频道文字聊天
- 扬声器、听筒及蓝牙语音路由
- TeamSpeak 身份持久化

暂不支持 Speex/CELT、文件管理和服务器管理。

## 编译

推送到 `main` 后，GitHub Actions 会生成未签名的 `TS6-iOS.ipa`。也可在仓库的
Actions 页面手动运行 **Build unsigned iOS app**。

本地 Rust 测试：

```powershell
cd rust/tslib_multi
cargo test -p tslib-ios-ffi
```

## 许可

`tslib`、`tsclientlib`：MIT OR Apache-2.0；Opus：BSD-3-Clause。
`YUAXI/TS6_Droid_CN` 仅作功能参考，本仓库未复制其 GPLv3 源码。
