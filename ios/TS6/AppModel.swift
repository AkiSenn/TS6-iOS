import Foundation
import Combine
import UIKit

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

    let bridge = TsClientBridge()
    let audio = AudioBridge()

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
            self?.errorMessage = msg
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

        // Clean up the TeamSpeak connection on app termination so the server
        // doesn't keep a stale client alive after we exit.
        NotificationCenter.default.addObserver(
            forName: UIApplication.willTerminateNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.bridge.disconnect()
        }
    }

    var isConnected: Bool { state == .connected }

    func connect(address: String, port: UInt16, nickname: String,
                 serverPassword: String, channel: String, channelPassword: String) {
        errorMessage = nil
        state = .connecting
        bridge.connect(address: address, port: port, nickname: nickname,
                       serverPassword: serverPassword, channel: channel,
                       channelPassword: channelPassword)
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
