import Foundation

/// Swift wrapper around the C FFI. Owns the identity and the client handle,
/// and polls the event queue on a background queue.
final class TsClientBridge {
    private var client: UnsafeMutablePointer<TsClient>?
    private var identity: UnsafeMutablePointer<TsLibIdentity>?
    private var identityFile: URL?

    private let pollQueue = DispatchQueue(label: "tslib.poll")
    private var timer: DispatchSourceTimer?

    var onConnected: ((String) -> Void)?
    var onDisconnected: ((String) -> Void)?
    var onError: ((String) -> Void)?
    var onText: ((TsTextMessage) -> Void)?
    var onTalk: ((UInt16, Bool) -> Void)?
    var onAudio: ((UInt16, Int, Data) -> Void)?
    var onSnapshot: ((ServerSnapshot) -> Void)?

    var isConnected: Bool { client != nil }

    // MARK: - Identity

    /// Load the persisted identity from disk, or create a new one.
    @discardableResult
    func ensureIdentity() -> Bool {
        if identity != nil { return true }

        let dir = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
        let file = dir.appendingPathComponent("ts_identity.json")
        identityFile = file

        if FileManager.default.fileExists(atPath: file.path),
           let data = try? String(contentsOf: file, encoding: .utf8) {
            identity = data.withCString { tslib_identity_import_string($0) }
        }
        if identity == nil {
            identity = tslib_identity_create()
        }
        guard identity != nil else { return false }
        saveIdentity()
        return true
    }

    private func saveIdentity() {
        guard let identity = identity, let file = identityFile else { return }
        guard let ptr = tslib_identity_export_string(identity) else { return }
        let str = String(cString: ptr)
        tslib_string_free(ptr)
        try? str.write(to: file, atomically: true, encoding: .utf8)
    }

    var uniqueId: String? {
        guard let identity = identity, let ptr = tslib_identity_unique_id(identity) else { return nil }
        let s = String(cString: ptr)
        tslib_string_free(ptr)
        return s
    }

    // MARK: - Connection

    func connect(address: String, port: UInt16, nickname: String,
                 serverPassword: String, channel: String, channelPassword: String) {
        disconnect()
        guard ensureIdentity(), let identity = identity else {
            onError?("无法创建身份")
            return
        }

        let addr = "\(address):\(port)"
        let newClient = addr.withCString { a in
            nickname.withCString { n in
                let sp = serverPassword.isEmpty ? nil : serverPassword
                let ch = channel.isEmpty ? nil : channel
                let cp = channelPassword.isEmpty ? nil : channelPassword
                return sp.withCStringOrNil { spc in
                    ch.withCStringOrNil { chc in
                        cp.withCStringOrNil { cpc in
                            tslib_client_connect(a, identity, n, spc, chc, cpc)
                        }
                    }
                }
            }
        }
        client = newClient
        guard newClient != nil else {
            onError?("连接创建失败")
            return
        }
        startPolling()
    }

    func disconnect() {
        stopPolling()
        if let client = client {
            _ = tslib_client_disconnect(client)
            tslib_client_free(client)
        }
        client = nil
    }

    // MARK: - Actions

    func sendAudio(_ data: Data) {
        guard let client = client else { return }
        data.withUnsafeBytes { raw in
            guard let base = raw.bindMemory(to: UInt8.self).baseAddress else { return }
            _ = tslib_client_send_audio_async(client, base, UInt(data.count), 4)
        }
    }

    func setInputMuted(_ muted: Bool) {
        guard let client = client else { return }
        let result = tslib_client_set_input_muted(client, muted ? 1 : 0)
        if result.rawValue != 0 {
            let action = muted ? "关闭" : "开启"
            DispatchQueue.main.async { [weak self] in
                self?.onError?("\(action)麦克风失败（错误码 \(result.rawValue)）")
            }
        }
    }

    func move(to channelId: UInt64) {
        guard let client = client else { return }
        _ = tslib_client_move_to_channel(client, channelId, nil)
    }

    func sendChannelMessage(_ text: String) {
        guard let client = client else { return }
        text.withCString { _ = tslib_client_send_channel_message(client, $0) }
    }

    // MARK: - Event polling

    private func startPolling() {
        stopPolling()
        let t = DispatchSource.makeTimerSource(queue: pollQueue)
        t.schedule(deadline: .now(), repeating: .milliseconds(20), leeway: .milliseconds(5))
        t.setEventHandler { [weak self] in self?.pollOnce() }
        t.resume()
        timer = t
    }

    private func stopPolling() {
        timer?.cancel()
        timer = nil
    }

    private func pollOnce() {
        guard let client = client else { return }
        while true {
            guard let ptr = tslib_client_poll_event(client) else { break }
            let json = String(cString: ptr)
            tslib_string_free(ptr)
            handle(eventJSON: json)
        }
    }

    private func handle(eventJSON json: String) {
        guard let data = json.data(using: .utf8),
              let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let type = obj["type"] as? String else { return }

        switch type {
        case "connected":
            let name = obj["server_name"] as? String ?? ""
            DispatchQueue.main.async { [weak self] in self?.onConnected?(name) }
            refreshSnapshot()
        case "disconnected":
            let reason = obj["reason"] as? String ?? ""
            DispatchQueue.main.async { [weak self] in self?.onDisconnected?(reason) }
        case "error", "command_error":
            let msg = obj["message"] as? String ?? ""
            DispatchQueue.main.async { [weak self] in self?.onError?(msg) }
        case "text":
            if let msg = try? JSONDecoder().decode(TsTextMessage.self, from: data) {
                DispatchQueue.main.async { [weak self] in self?.onText?(msg) }
            }
        case "talk":
            if let uid = (obj["user_id"] as? NSNumber)?.uint16Value,
               let talking = obj["talking"] as? Bool {
                DispatchQueue.main.async { [weak self] in self?.onTalk?(uid, talking) }
            }
        case "audio":
            if let uid = (obj["user_id"] as? NSNumber)?.uint16Value,
               let codec = (obj["codec"] as? NSNumber)?.intValue,
               let b64 = obj["data"] as? String,
               let audio = Data(base64Encoded: b64) {
                // Audio is already on the serial poll queue. Keep it off the
                // main thread so UI work cannot add audible latency/jitter.
                onAudio?(uid, codec, audio)
            }
        case "updated":
            refreshSnapshot()
        default:
            break
        }
    }

    private func refreshSnapshot() {
        guard let client = client else { return }
        pollQueue.async { [weak self] in
            guard let self = self, let ptr = tslib_client_snapshot(client) else { return }
            let json = String(cString: ptr)
            tslib_string_free(ptr)
            guard let data = json.data(using: .utf8),
                  let snap = try? JSONDecoder().decode(ServerSnapshot.self, from: data) else { return }
            DispatchQueue.main.async { [weak self] in self?.onSnapshot?(snap) }
        }
    }

    deinit {
        if let client = client { tslib_client_free(client) }
        if let identity = identity { tslib_identity_free(identity) }
    }
}

extension Optional where Wrapped == String {
    func withCStringOrNil<R>(_ body: (UnsafePointer<CChar>?) -> R) -> R {
        switch self {
        case .some(let s):
            return s.withCString(body)
        case .none:
            return body(nil)
        }
    }
}
