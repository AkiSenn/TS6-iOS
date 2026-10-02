import SwiftUI

struct ChannelRow: Identifiable {
    let id: UInt64
    let channel: TsChannel
    let depth: Int
}

struct ServerView: View {
    @ObservedObject var model: AppModel
    @State private var draft = ""
    @State private var holdingPTT = false

    var body: some View {
        VStack(spacing: 0) {
            List {
                Section(header: Text(model.serverName.isEmpty ? "频道" : model.serverName)) {
                    ForEach(channelRows()) { row in
                        channelRow(row)
                    }
                }
                if !model.chat.isEmpty {
                    Section(header: Text("聊天")) {
                        ForEach(model.chat) { entry in
                            VStack(alignment: .leading, spacing: 2) {
                                Text(entry.sender)
                                    .font(.caption)
                                    .foregroundColor(entry.mine ? .blue : .secondary)
                                Text(entry.text)
                            }
                            .padding(.vertical, 2)
                        }
                    }
                }
            }
            .listStyle(InsetGroupedListStyle())

            Divider()

            HStack(spacing: 8) {
                TextField("发送到当前频道…", text: $draft, onCommit: send)
                    .textFieldStyle(RoundedBorderTextFieldStyle())
                Button("发送", action: send)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)

            HStack(spacing: 12) {
                Button(action: { model.setMicEnabled(!model.micEnabled) }) {
                    Label(model.micEnabled ? "麦克风开" : "麦克风关",
                          systemImage: model.micEnabled ? "mic.fill" : "mic.slash.fill")
                }
                .foregroundColor(model.micEnabled ? .blue : .secondary)

                Button(action: {}) {
                    Text("按住说话")
                }
                .buttonStyle(PTTButtonStyle(isActive: holdingPTT))
                .simultaneousGesture(
                    DragGesture(minimumDistance: 0)
                        .onChanged { _ in startPTT() }
                        .onEnded { _ in endPTT() }
                )

                Spacer()

                Button("断开", action: { model.disconnect() })
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)

            Text("\(model.audioStatus) · 发 \(model.sentAudioFrames) / 收 \(model.receivedAudioFrames)")
                .font(.caption2)
                .foregroundColor(.secondary)
                .lineLimit(2)
                .padding(.horizontal, 12)
                .padding(.bottom, 6)
        }
        .navigationBarTitle("TS6", displayMode: .inline)
    }

    // MARK: - Channel tree

    private func channelRows() -> [ChannelRow] {
        var rows: [ChannelRow] = []
        let byParent = Dictionary(grouping: model.channels) { $0.parentId }

        func walk(_ parent: UInt64, _ depth: Int) {
            for ch in byParent[parent] ?? [] {
                rows.append(ChannelRow(id: ch.id, channel: ch, depth: depth))
                walk(ch.id, depth + 1)
            }
        }
        walk(0, 0)
        return rows
    }

    private func channelRow(_ row: ChannelRow) -> some View {
        let ch = row.channel
        let usersInChannel = model.users.filter { $0.channelId == ch.id }
        return VStack(alignment: .leading, spacing: 2) {
            HStack {
                Image(systemName: "number")
                    .foregroundColor(.secondary)
                Text(ch.name)
                Spacer()
                if ch.id == model.currentChannelId {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundColor(.green)
                }
            }
            .contentShape(Rectangle())
            .onTapGesture { model.join(channel: ch.id) }

            ForEach(usersInChannel) { user in
                HStack(spacing: 6) {
                    Image(systemName: "person.fill")
                        .foregroundColor(talkingColor(user))
                    Text(user.nickname)
                        .font(.caption)
                    if user.id == model.selfId {
                        Text("(我)")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                    }
                }
                .padding(.leading, 16)
            }
        }
        .padding(.leading, CGFloat(row.depth) * 16)
    }

    private func talkingColor(_ user: TsUser) -> Color {
        if model.talking.contains(user.id) || user.isTalking {
            return .green
        }
        return .secondary
    }

    // MARK: - Actions

    private func send() {
        let text = draft
        draft = ""
        model.sendChannelMessage(text)
    }

    private func startPTT() {
        guard !holdingPTT else { return }
        holdingPTT = true
        model.setMicEnabled(true)
    }

    private func endPTT() {
        guard holdingPTT else { return }
        holdingPTT = false
        model.setMicEnabled(false)
    }
}

struct PTTButtonStyle: ButtonStyle {
    var isActive: Bool

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(isActive ? Color.green : Color(.systemGray5))
            .foregroundColor(isActive ? .white : .primary)
            .cornerRadius(8)
    }
}
