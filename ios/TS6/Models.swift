import Foundation

/// Connection state, mirrors the Rust `ConnectionState` codes.
enum ConnState: Int {
    case disconnected = 0
    case connecting = 1
    case connected = 2
    case initializing = 3
    case reconnecting = 4
}

struct ServerSnapshot: Decodable {
    let state: String
    let code: Int
    let clientId: UInt16?
    let channelId: UInt64?
    let server: ServerInfo?
    let channels: [TsChannel]
    let users: [TsUser]

    enum CodingKeys: String, CodingKey {
        case state, code, server, channels, users
        case clientId = "client_id"
        case channelId = "channel_id"
    }
}

struct ServerInfo: Decodable {
    let name: String
    let uid: String?
    let welcomeMessage: String?
    let platform: String?
    let version: String?
    let clientsOnline: UInt32?
    let maxClients: UInt32?

    enum CodingKeys: String, CodingKey {
        case name, uid, platform, version
        case welcomeMessage = "welcome_message"
        case clientsOnline = "clients_online"
        case maxClients = "max_clients"
    }
}

struct TsChannel: Decodable, Identifiable {
    let id: UInt64
    let parentId: UInt64
    let name: String
    let topic: String?
    let description: String?
    let order: Int
    let isPermanent: Bool
    let isSemiPermanent: Bool
    let isDefault: Bool
    let hasPassword: Bool
    let codec: Int
    let codecQuality: Int
    let maxClients: Int
    let maxFamilyClients: Int
    let neededTalkPower: Int

    enum CodingKeys: String, CodingKey {
        case id, name, topic, description, order, codec
        case parentId = "parent_id"
        case isPermanent = "is_permanent"
        case isSemiPermanent = "is_semi_permanent"
        case isDefault = "is_default"
        case hasPassword = "has_password"
        case codecQuality = "codec_quality"
        case maxClients = "max_clients"
        case maxFamilyClients = "max_family_clients"
        case neededTalkPower = "needed_talk_power"
    }
}

struct TsUser: Decodable, Identifiable {
    let id: UInt16
    let channelId: UInt64
    let nickname: String
    let clientType: Int
    let isTalking: Bool
    let isInputMuted: Bool
    let isOutputMuted: Bool
    let isAway: Bool
    let isTalker: Bool

    enum CodingKeys: String, CodingKey {
        case id, nickname
        case channelId = "channel_id"
        case clientType = "client_type"
        case isTalking = "is_talking"
        case isInputMuted = "is_input_muted"
        case isOutputMuted = "is_output_muted"
        case isAway = "is_away"
        case isTalker = "is_talker"
    }
}

struct TsTextMessage: Decodable {
    let senderId: UInt16
    let senderName: String
    let message: String
    let target: String

    enum CodingKeys: String, CodingKey {
        case message, target
        case senderId = "sender_id"
        case senderName = "sender_name"
    }
}

struct ChatEntry: Identifiable {
    let id = UUID()
    let sender: String
    let text: String
    let mine: Bool

    init(sender: String, text: String, mine: Bool = false) {
        self.sender = sender
        self.text = text
        self.mine = mine
    }
}
