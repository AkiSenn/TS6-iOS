import SwiftUI
import UIKit

struct ConnectionView: View {
    @ObservedObject var model: AppModel
    @State private var address = ""
    @State private var port = "9987"
    @State private var nickname = ""
    @State private var serverPassword = ""
    @State private var channel = ""
    @State private var channelPassword = ""

    var body: some View {
        NavigationView {
            ZStack {
                AppBackdrop()
                ScrollView {
                    VStack(spacing: 20) {
                        VStack(spacing: 7) {
                            ZStack {
                                Circle().fill(Color.blue.opacity(0.25)).frame(width: 78, height: 78)
                                Image(systemName: "waveform.circle.fill")
                                    .font(.system(size: 58))
                                    .foregroundColor(.white)
                            }
                            Text("TS6")
                                .font(.system(size: 34, weight: .bold, design: .rounded))
                                .foregroundColor(.white)
                            Text("TeamSpeak · 清晰、轻量、自由")
                                .font(.subheadline)
                                .foregroundColor(.white.opacity(0.62))
                        }
                        .padding(.top, 34)

                        GlassPanel {
                            VStack(spacing: 2) {
                                ConnectField(icon: "network", title: "服务器地址", text: $address,
                                             keyboard: .URL)
                                fieldDivider
                                ConnectField(icon: "number", title: "端口", text: $port,
                                             keyboard: .numberPad)
                                fieldDivider
                                ConnectField(icon: "person.fill", title: "昵称", text: $nickname)
                                fieldDivider
                                ConnectField(icon: "lock.fill", title: "服务器密码（可选）",
                                             text: $serverPassword, secure: true)
                            }
                        }

                        GlassPanel {
                            VStack(spacing: 2) {
                                ConnectField(icon: "rectangle.stack.fill", title: "默认频道（可选）",
                                             text: $channel)
                                fieldDivider
                                ConnectField(icon: "key.fill", title: "频道密码（可选）",
                                             text: $channelPassword, secure: true)
                            }
                        }

                        if let error = model.errorMessage {
                            HStack(alignment: .top, spacing: 9) {
                                Image(systemName: "exclamationmark.triangle.fill")
                                Text(error).font(.footnote)
                                Spacer(minLength: 0)
                            }
                            .foregroundColor(Color(red: 1, green: 0.62, blue: 0.62))
                            .padding(.horizontal, 4)
                        }

                        Button(action: connect) {
                            HStack(spacing: 9) {
                                if model.state == .connecting {
                                    ProgressView().progressViewStyle(CircularProgressViewStyle(tint: .white))
                                    Text("连接中…")
                                } else {
                                    Image(systemName: "bolt.fill")
                                    Text("连接服务器")
                                }
                            }
                            .font(.headline)
                            .foregroundColor(.white)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 15)
                            .background(
                                LinearGradient(gradient: Gradient(colors: [.blue, .purple]),
                                               startPoint: .leading, endPoint: .trailing)
                            )
                            .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
                            .shadow(color: Color.blue.opacity(0.3), radius: 14, y: 7)
                        }
                        .disabled(model.state == .connecting || address.isEmpty || nickname.isEmpty)
                        .opacity((address.isEmpty || nickname.isEmpty) ? 0.55 : 1)

                        Text("iOS 14+ · TeamSpeak 3/6")
                            .font(.caption2)
                            .foregroundColor(.white.opacity(0.42))
                            .padding(.bottom, 24)
                    }
                    .padding(.horizontal, 20)
                }
            }
            .navigationBarHidden(true)
            .onAppear(perform: loadDefaults)
        }
        .navigationViewStyle(StackNavigationViewStyle())
        .alert(isPresented: forceLoginBinding) {
            let nickname = model.forceLoginNickname ?? "昵称(1)"
            return Alert(
                title: Text("旧连接可能仍在线"),
                message: Text("服务器可能仍保留上一次连接。是否使用“\(nickname)”强制重试？"),
                primaryButton: .default(Text("强制登录"), action: model.forceLogin),
                secondaryButton: .cancel(Text("取消"), action: model.dismissForceLogin)
            )
        }
    }

    private var fieldDivider: some View {
        Rectangle().fill(Color.white.opacity(0.09)).frame(height: 1).padding(.leading, 35)
    }

    private var forceLoginBinding: Binding<Bool> {
        Binding(
            get: { model.forceLoginNickname != nil },
            set: { if !$0 { model.dismissForceLogin() } }
        )
    }

    private func loadDefaults() {
        guard address.isEmpty else { return }
        address = UserDefaults.standard.string(forKey: "lastAddress") ?? ""
        port = UserDefaults.standard.string(forKey: "lastPort") ?? "9987"
        nickname = UserDefaults.standard.string(forKey: "lastNickname") ?? UIDevice.current.name
    }

    private func connect() {
        guard let parsedPort = UInt16(port) else { return }
        UserDefaults.standard.set(address, forKey: "lastAddress")
        UserDefaults.standard.set(port, forKey: "lastPort")
        UserDefaults.standard.set(nickname, forKey: "lastNickname")
        model.connect(address: address, port: parsedPort, nickname: nickname,
                      serverPassword: serverPassword, channel: channel,
                      channelPassword: channelPassword)
    }
}

private struct ConnectField: View {
    let icon: String
    let title: String
    @Binding var text: String
    var secure = false
    var keyboard: UIKeyboardType = .default

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: icon)
                .foregroundColor(.blue)
                .frame(width: 22)
            if secure {
                SecureField(title, text: $text)
                    .foregroundColor(.white)
            } else {
                TextField(title, text: $text)
                    .foregroundColor(.white)
                    .keyboardType(keyboard)
                    .autocapitalization(.none)
                    .disableAutocorrection(true)
            }
        }
        .padding(.vertical, 11)
    }
}
