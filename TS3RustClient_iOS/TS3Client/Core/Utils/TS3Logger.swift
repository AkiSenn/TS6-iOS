// File: TS3RustClient_iOS/TS3Client/Core/Utils/TS3Logger.swift
//
// 收集 Rust 日志与本地日志，广播给 UI（连接页日志视图）。

import Foundation

final class TS3Logger {

    static let shared = TS3Logger()
    static let didLog = Notification.Name("TS3Logger.didLog")

    private var entries: [String] = []
    private let lock = NSLock()

    func log(_ message: String) {
        let line = "[\(Self.timestamp())] \(message)"
        lock.lock()
        entries.append(line)
        if entries.count > 500 {
            entries.removeFirst(entries.count - 500)
        }
        lock.unlock()

        NSLog("TS3Rust: %@", message)
        NotificationCenter.default.post(name: Self.didLog, object: nil)
    }

    var recentEntries: [String] {
        lock.lock()
        defer { lock.unlock() }
        return entries
    }

    private static func timestamp() -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "HH:mm:ss"
        return formatter.string(from: Date())
    }
}
