import Foundation
import Combine
import UIKit

private struct ConnectionRequest {
    var address: String
    var port: UInt16
    var nickname: String
    var serverPassword: String
    var channel: String
    var channelPassword: String
}

final class AppModel: ObservableObject {
    @Published var state: ConnState = .disconnected
    @Published var serverName = ""
    @Published var channels: [TsChannel] = []
    @Published var users: [TsUser] = []
    @Published var chat: [ChatEntry] = []
    @Published var talking = Set<UInt16>()
    @Published var micEnabled = false
    @Published var errorMessage: String?
    @Published var selfId: UInt16?
    @Published var currentChannelId: UInt64?
    @Published var audioStatus = "音频未启动"
    @Published var sentAudioFrames: UInt64 = 0
    @Published var receivedAudioFrames: UInt64 = 0
    @Published var forceLoginNickname: String?

    let bridge = TsClientBridge()
    let audio = AudioBridge()
    private var lastConnection: ConnectionRequest?

    init() {
        bridge.onConnected = { [weak self] name in
            guard let self = self else { return }
            self.serverName = name
            self.state = .connected
            self.errorMessage = nil
            self.audio.start()
        }
        bridge.onDisconnected = { [weak self] _ in
            self?.teardown()
        }
        bridge.onError = { [weak self] msg in
            guard let self = self else { return }
            let failedWhileConnecting = self.state == .connecting
            self.errorMessage = msg
            if failedWhileConnecting, let request = self.lastConnection {
                self.state = .disconnected
                self.forceLoginNickname = self.nextNickname(after: request.nickname)
            }
        }
        bridge.onSnapshot = { [weak self] snap in
            guard let self = self else { return }
            self.state = ConnState(rawValue: snap.code) ?? .disconnected
            self.channels = snap.channels.sorted { $0.order < $1.order }
            self.users = snap.users
            self.selfId = snap.clientId
            self.currentChannelId = snap.channelId
        }
        bridge.onText = { [weak self] msg in
            self?.appendChat(sender: msg.senderName, text: msg.message)
        }
        bridge.onTalk = { [weak self] uid, on in
            guard let self = self else { return }
            if on { self.talking.insert(uid) } else { self.talking.remove(uid) }
        }
        bridge.onAudio = { [weak self] uid, codec, data in
            self?.audio.handleIncoming(userId: uid, codec: codec, data: data)
        }
        audio.sendFrame = { [weak self] data in
            self?.bridge.sendAudio(data) ?? false
        }
        audio.onStatus = { [weak self] message in
            self?.audioStatus = message
        }
        audio.onError = { [weak self] message in
            self?.audioStatus = message
            self?.errorMessage = message
        }
        audio.onStats = { [weak self] sent, received in
            self?.sentAudioFrames = sent
            self?.receivedAudioFrames = received
        }
        audio.onLocalTalk = { [weak self] active in
            guard let self = self, let id = self.selfId else { return }
            if active { self.talking.insert(id) } else { self.talking.remove(id) }
        }

        // Clean up the TeamSpeak connection on app termination so the server
        // doesn't keep a stale client alive after we exit.
        NotificationCenter.default.addObserver(
            forName: UIApplication.willTerminateNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.bridge.disconnect()
        }
        NotificationCenter.default.addObserver(
            forName: UIApplication.didEnterBackgroundNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            guard self?.isConnected == true else { return }
            self?.audio.prepareForBackground()
        }
        NotificationCenter.default.addObserver(
            forName: UIApplication.willEnterForegroundNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            guard self?.isConnected == true else { return }
            self?.audio.prepareForBackground()
        }
    }

    var isConnected: Bool { state == .connected }

    func connect(address: String, port: UInt16, nickname: String,
                 serverPassword: String, channel: String, channelPassword: String) {
        lastConnection = ConnectionRequest(address: address, port: port, nickname: nickname,
                                           serverPassword: serverPassword, channel: channel,
                                           channelPassword: channelPassword)
        forceLoginNickname = nil
        errorMessage = nil
        state = .connecting
        bridge.connect(address: address, port: port, nickname: nickname,
                       serverPassword: serverPassword, channel: channel,
                       channelPassword: channelPassword)
    }

    func forceLogin() {
        guard var request = lastConnection, let nickname = forceLoginNickname else { return }
        request.nickname = nickname
        UserDefaults.standard.set(nickname, forKey: "lastNickname")
        connect(address: request.address, port: request.port, nickname: nickname,
                serverPassword: request.serverPassword, channel: request.channel,
                channelPassword: request.channelPassword)
    }

    func dismissForceLogin() {
        forceLoginNickname = nil
    }

    private func nextNickname(after nickname: String) -> String {
        if let range = nickname.range(of: #"\((\d+)\)$"#, options: .regularExpression),
           let number = Int(nickname[range].dropFirst().dropLast()) {
            return String(nickname[..<range.lowerBound]) + "(\(number + 1))"
        }
        return nickname + "(1)"
    }

    func disconnect() {
        bridge.disconnect()
        teardown()
    }

    private func teardown() {
        state = .disconnected
        serverName = ""
        channels = []
        users = []
        selfId = nil
        currentChannelId = nil
        talking.removeAll()
        micEnabled = false
        audioStatus = "音频未启动"
        sentAudioFrames = 0
        receivedAudioFrames = 0
        audio.stop()
    }

    func join(channel id: UInt64) {
        bridge.move(to: id)
    }

    func sendChannelMessage(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        bridge.sendChannelMessage(trimmed)
        appendChat(sender: "我", text: trimmed, mine: true)
    }

    private func appendChat(sender: String, text: String, mine: Bool = false) {
        chat.append(ChatEntry(sender: sender, text: text, mine: mine))
        if chat.count > 300 {
            chat.removeFirst(chat.count - 300)
        }
    }

    func setMicEnabled(_ on: Bool) {
        guard micEnabled != on else { return }
        micEnabled = on
        audio.setMuted(!on)
        bridge.setInputMuted(!on)
    }
}
