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
        ZStack {
            AppBackdrop()
            VStack(spacing: 0) {
                header
                ScrollView {
                    LazyVStack(spacing: 11) {
                        ForEach(channelRows()) { channelCard($0) }
                        if !model.chat.isEmpty { chatCard }
                    }
                    .padding(.horizontal, 14)
                    .padding(.vertical, 12)
                }
                bottomPanel
            }
        }
    }

    private var header: some View {
        HStack(spacing: 12) {
            ZStack {
                Circle().fill(Color.green.opacity(0.17)).frame(width: 42, height: 42)
                Image(systemName: "antenna.radiowaves.left.and.right")
                    .foregroundColor(.green)
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(model.serverName.isEmpty ? "TeamSpeak" : model.serverName)
                    .font(.headline)
                    .foregroundColor(.white)
                    .lineLimit(1)
                Text("已连接 · \(model.users.count) 位用户")
                    .font(.caption)
                    .foregroundColor(.white.opacity(0.58))
            }
            Spacer()
            Button(action: model.disconnect) {
                Image(systemName: "rectangle.portrait.and.arrow.right")
                    .font(.system(size: 17, weight: .semibold))
                    .foregroundColor(Color(red: 1, green: 0.55, blue: 0.55))
                    .frame(width: 38, height: 38)
                    .background(Color.white.opacity(0.08))
                    .clipShape(Circle())
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 10)
        .padding(.bottom, 9)
        .background(BlurView(style: .systemThinMaterialDark))
    }

    private func channelCard(_ row: ChannelRow) -> some View {
        let channel = row.channel
        let members = model.users.filter { $0.channelId == channel.id }
        return GlassPanel {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 9) {
                    Image(systemName: channel.hasPassword ? "lock.fill" : "wave.3.right.circle.fill")
                        .foregroundColor(channel.id == model.currentChannelId ? .green : .blue)
                    Text(channel.name)
                        .font(.system(.subheadline, design: .rounded).weight(.semibold))
                        .foregroundColor(.white)
                    Spacer()
                    if channel.id == model.currentChannelId {
                        Text("当前")
                            .font(.caption2.weight(.bold))
                            .foregroundColor(.green)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(Color.green.opacity(0.13))
                            .clipShape(Capsule())
                    } else {
                        Text("\(members.count)")
                            .font(.caption)
                            .foregroundColor(.white.opacity(0.45))
                    }
                }
                .contentShape(Rectangle())
                .onTapGesture { model.join(channel: channel.id) }

                if !members.isEmpty {
                    Rectangle().fill(Color.white.opacity(0.08)).frame(height: 1)
                    ForEach(members) { userRow($0) }
                }
            }
        }
        .padding(.leading, CGFloat(row.depth) * 11)
    }

    private func userRow(_ user: TsUser) -> some View {
        let active = model.talking.contains(user.id) || user.isTalking
        return HStack(spacing: 10) {
            ZStack {
                Circle()
                    .fill(active ? Color.green.opacity(0.2) : Color.white.opacity(0.07))
                    .frame(width: 31, height: 31)
                Image(systemName: active ? "waveform" : "person.fill")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundColor(active ? .green : .white.opacity(0.52))
            }
            Text(user.nickname)
                .font(.subheadline)
                .foregroundColor(.white.opacity(0.9))
            if user.id == model.selfId {
                Text("我")
                    .font(.caption2.weight(.bold))
                    .foregroundColor(.blue)
                    .padding(.horizontal, 7)
                    .padding(.vertical, 3)
                    .background(Color.blue.opacity(0.14))
                    .clipShape(Capsule())
            }
            Spacer()
            if user.isInputMuted {
                Image(systemName: "mic.slash.fill").font(.caption).foregroundColor(.white.opacity(0.35))
            }
        }
    }

    private var chatCard: some View {
        GlassPanel {
            VStack(alignment: .leading, spacing: 10) {
                Label("频道消息", systemImage: "bubble.left.and.bubble.right.fill")
                    .font(.subheadline.weight(.semibold))
                    .foregroundColor(.white)
                ForEach(Array(model.chat.suffix(6))) { entry in
                    HStack(alignment: .top, spacing: 7) {
                        Text(entry.sender)
                            .font(.caption.weight(.semibold))
                            .foregroundColor(entry.mine ? .blue : .green)
                        Text(entry.text)
                            .font(.caption)
                            .foregroundColor(.white.opacity(0.76))
                        Spacer(minLength: 0)
                    }
                }
            }
        }
    }

    private var bottomPanel: some View {
        VStack(spacing: 10) {
            HStack(spacing: 9) {
                TextField("发送频道消息…", text: $draft, onCommit: send)
                    .foregroundColor(.white)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 9)
                    .background(Color.white.opacity(0.08))
                    .clipShape(RoundedRectangle(cornerRadius: 11, style: .continuous))
                Button(action: send) {
                    Image(systemName: "paperplane.fill")
                        .foregroundColor(.white)
                        .frame(width: 38, height: 38)
                        .background(Color.blue)
                        .clipShape(Circle())
                }
            }

            HStack(spacing: 10) {
                Button(action: { model.setMicEnabled(!model.micEnabled) }) {
                    Label(model.micEnabled ? "麦克风开" : "麦克风关",
                          systemImage: model.micEnabled ? "mic.fill" : "mic.slash.fill")
                        .controlPill(active: model.micEnabled, color: .blue)
                }
                Button(action: {}) {
                    Label("按住说话", systemImage: holdingPTT ? "waveform" : "hand.tap.fill")
                        .controlPill(active: holdingPTT, color: .green)
                }
                .simultaneousGesture(
                    DragGesture(minimumDistance: 0)
                        .onChanged { _ in startPTT() }
                        .onEnded { _ in endPTT() }
                )
                Spacer(minLength: 0)
            }

            HStack(spacing: 5) {
                Circle().fill(model.micEnabled ? Color.green : Color.white.opacity(0.35))
                    .frame(width: 6, height: 6)
                Text("\(model.audioStatus) · 发 \(model.sentAudioFrames) / 收 \(model.receivedAudioFrames)")
                    .font(.caption2)
                    .foregroundColor(.white.opacity(0.48))
                    .lineLimit(1)
                Spacer()
            }
        }
        .padding(.horizontal, 13)
        .padding(.top, 10)
        .padding(.bottom, 8)
        .background(BlurView(style: .systemThinMaterialDark))
    }

    private func channelRows() -> [ChannelRow] {
        var rows: [ChannelRow] = []
        let byParent = Dictionary(grouping: model.channels) { $0.parentId }
        func walk(_ parent: UInt64, _ depth: Int) {
            for channel in byParent[parent] ?? [] {
                rows.append(ChannelRow(id: channel.id, channel: channel, depth: depth))
                walk(channel.id, depth + 1)
            }
        }
        walk(0, 0)
        return rows
    }

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

private extension View {
    func controlPill(active: Bool, color: Color) -> some View {
        self
            .font(.caption.weight(.semibold))
            .foregroundColor(active ? .white : .white.opacity(0.65))
            .padding(.horizontal, 12)
            .padding(.vertical, 9)
            .background(active ? color : Color.white.opacity(0.08))
            .clipShape(Capsule())
    }
}
