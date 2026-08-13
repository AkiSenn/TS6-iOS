// File: TS3RustClient_iOS/TS3Client/Core/Bridge/TS3RustManager.swift
//
// Swift <-> Rust 桥接单例。加载的是 libts3_rust.a
// （Rust 编译产物，链接 ReSpeak/tsclientlib），不是官方 libts3client。

import Foundation

final class TS3RustManager {

    static let shared = TS3RustManager()

    /// 所有 FFI 调用放到后台队列，避免阻塞主线程（ts3_connect 最长阻塞约 20 秒）。
    private let bridgeQueue = DispatchQueue(label: "ts3rust.bridge")

    /// 音频桥（由连接页在连接成功后注入）。
    var audioHandler: AudioBridge?
    /// 说话状态回调（主线程）。
    var onTalk: ((UInt16, Bool) -> Void)?

    private init() {
        init_logger()
        ts3_set_log_callback(Self.rustLogCallback)
        ts3_set_audio_callback(Self.audioCallback)
        ts3_set_talk_callback(Self.talkCallback)
    }

    /// 静态 C 回调：接收 Rust 日志，转发给 TS3Logger。
    /// 注意：可能从 Rust 工作线程回调，这里只做线程安全入队，不碰 UI。
    private static let rustLogCallback: @convention(c) (UnsafePointer<CChar>?) -> Void = { message in
        guard let message = message else { return }
        TS3Logger.shared.log(String(cString: message))
    }

    /// Rust 语音回调（worker 线程）：拷贝数据后转到主线程交给 AudioBridge。
    private static let audioCallback: @convention(c) (UInt64, UInt16, UInt8, UnsafePointer<UInt8>?, UInt) -> Void = { _, userId, codec, data, len in
        guard let data = data else { return }
        let frame = Data(bytes: data, count: Int(len))
        DispatchQueue.main.async {
            TS3RustManager.shared.audioHandler?
                .handleIncoming(userId: userId, codec: Int(codec), data: frame)
        }
    }

    /// Rust 说话状态回调（worker 线程）。
    private static let talkCallback: @convention(c) (UInt64, UInt16, Int32) -> Void = { _, userId, talking in
        DispatchQueue.main.async {
            TS3RustManager.shared.onTalk?(userId, talking != 0)
        }
    }

    // MARK: - Public API

    func connect(server: String, port: UInt16, nickname: String,
                 password: String, completion: @escaping (UInt64?, String?) -> Void) {
        bridgeQueue.async {
            let id = server.withCString { serverPtr in
                nickname.withCString { nickPtr in
                    password.withCString { pwdPtr in
                        ts3_connect(serverPtr, port, nickPtr, pwdPtr)
                    }
                }
            }
            DispatchQueue.main.async {
                if id == 0 {
                    completion(nil, "连接失败（超时或被服务器拒绝）")
                } else {
                    completion(id, nil)
                }
            }
        }
    }

    func disconnect(connectionId: UInt64) {
        bridgeQueue.async {
            ts3_disconnect(connectionId)
        }
    }

    func sendMessage(connectionId: UInt64, message: String) {
        bridgeQueue.async {
            _ = message.withCString { messagePtr in
                ts3_send_message(connectionId, messagePtr)
            }
        }
    }

    /// 发送一帧 Opus 语音（非阻塞，可从实时音频线程调用）。
    func sendAudio(connectionId: UInt64, data: Data) {
        data.withUnsafeBytes { raw in
            guard let base = raw.bindMemory(to: UInt8.self).baseAddress else { return }
            _ = ts3_send_audio(connectionId, base, UInt(data.count), 4)
        }
    }
}

extension Optional where Wrapped == String {
    func withCStringOrNil<R>(_ body: (UnsafePointer<CChar>?) -> R) -> R {
        switch self {
        case .some(let string):
            return string.withCString(body)
        case .none:
            return body(nil)
        }
    }
}
