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
            Form {
                Section(header: Text("服务器")) {
                    TextField("地址 (IP 或域名)", text: $address)
                        .autocapitalization(.none)
                        .disableAutocorrection(true)
                        .keyboardType(.URL)
                    TextField("端口", text: $port)
                        .keyboardType(.numberPad)
                    TextField("昵称", text: $nickname)
                        .autocapitalization(.none)
                    SecureField("服务器密码（可选）", text: $serverPassword)
                }
                Section(header: Text("默认频道（可选）")) {
                    TextField("频道名", text: $channel)
                    SecureField("频道密码", text: $channelPassword)
                }
                if let errorMessage = model.errorMessage {
                    Section {
                        Text(errorMessage)
                            .font(.footnote)
                            .foregroundColor(.red)
                    }
                }
                Section {
                    Button(action: connect) {
                        if model.state == .connecting {
                            Text("连接中…")
                        } else {
                            Text("连接")
                        }
                    }
                    .disabled(model.state == .connecting)
                }
            }
            .navigationBarTitle("TS6", displayMode: .inline)
            .onAppear {
                if address.isEmpty {
                    address = UserDefaults.standard.string(forKey: "lastAddress") ?? ""
                    port = UserDefaults.standard.string(forKey: "lastPort") ?? "9987"
                    nickname = UserDefaults.standard.string(forKey: "lastNickname")
                        ?? UIDevice.current.name
                }
            }
        }
        .navigationViewStyle(StackNavigationViewStyle())
    }

    private func connect() {
        guard let p = UInt16(port) else { return }
        UserDefaults.standard.set(address, forKey: "lastAddress")
        UserDefaults.standard.set(port, forKey: "lastPort")
        UserDefaults.standard.set(nickname, forKey: "lastNickname")
        model.connect(address: address, port: p, nickname: nickname,
                      serverPassword: serverPassword, channel: channel,
                      channelPassword: channelPassword)
    }
}
