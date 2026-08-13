// File: TS3RustClient_iOS/TS3Client/Core/Utils/ServerConfigManager.swift
//
// 已保存服务器列表（UserDefaults JSON 持久化）。

import Foundation

struct ServerConfig: Codable, Equatable {
    var server: String
    var port: Int
    var nickname: String
    var password: String
}

final class ServerConfigManager {

    static let shared = ServerConfigManager()

    private let storageKey = "saved_servers"

    private(set) var servers: [ServerConfig] {
        didSet { persist() }
    }

    private init() {
        if let data = UserDefaults.standard.data(forKey: storageKey),
           let decoded = try? JSONDecoder().decode([ServerConfig].self, from: data) {
            servers = decoded
        } else {
            servers = []
        }
    }

    func add(_ config: ServerConfig) {
        servers.removeAll { $0.server == config.server && $0.port == config.port }
        servers.append(config)
    }

    func remove(at index: Int) {
        guard servers.indices.contains(index) else { return }
        servers.remove(at: index)
    }

    private func persist() {
        if let data = try? JSONEncoder().encode(servers) {
            UserDefaults.standard.set(data, forKey: storageKey)
        }
    }
}
