// File: TS3RustClient_iOS/TS3Client/UI/Connect/ConnectViewController.swift

import UIKit

final class ConnectViewController: UIViewController {

    private let serverField = UITextField()
    private let portField = UITextField()
    private let nicknameField = UITextField()
    private let passwordField = UITextField()
    private let connectButton = UIButton(type: .system)
    private let statusLabel = UILabel()
    private let messageField = UITextField()
    private let sendButton = UIButton(type: .system)
    private let micButton = UIButton(type: .system)
    private let pttButton = UIButton(type: .system)
    private let logView = UITextView()

    private var connectionId: UInt64?
    private var micEnabled = false
    private var holdingPTT = false

    override func viewDidLoad() {
        super.viewDidLoad()
        title = LanguageManager.shared.localizedString("app_title")
        view.backgroundColor = .systemBackground
        buildUI()

        NotificationCenter.default.addObserver(
            self,
            selector: #selector(logsChanged(_:)),
            name: TS3Logger.didLog,
            object: nil
        )
    }

    deinit {
        NotificationCenter.default.removeObserver(self)
    }

    // MARK: - Actions

    @objc private func connectTapped() {
        view.endEditing(true)

        if let connectionId = connectionId {
            TS3RustManager.shared.disconnect(connectionId: connectionId)
            stopAudio()
            self.connectionId = nil
            updateConnectUI(connected: false)
            return
        }

        guard let server = serverField.text, !server.isEmpty,
              let portText = portField.text, let port = Int(portText) else {
            statusLabel.text = "请输入服务器地址和端口"
            return
        }
        let nickname = nicknameField.text?.isEmpty == false ? nicknameField.text! : "TS3Rust-iOS"
        let password = passwordField.text ?? ""

        statusLabel.text = LanguageManager.shared.localizedString("status_connecting")
        connectButton.isEnabled = false

        TS3RustManager.shared.connect(
            server: server,
            port: UInt16(port),
            nickname: nickname,
            password: password
        ) { [weak self] id, error in
            guard let self = self else { return }
            self.connectButton.isEnabled = true
            if let id = id {
                self.connectionId = id
                self.statusLabel.text = LanguageManager.shared.localizedString("status_connected")
                self.updateConnectUI(connected: true)
                self.startAudio(connectionId: id)
                ServerConfigManager.shared.add(
                    ServerConfig(server: server, port: port, nickname: nickname, password: password)
                )
                TS3Logger.shared.log("已连接，connection id = \(id)")
            } else {
                self.statusLabel.text = error ?? "连接失败"
                TS3Logger.shared.log("连接失败: \(error ?? "未知错误")")
            }
        }
    }

    @objc private func sendTapped() {
        guard let connectionId = connectionId,
              let text = messageField.text, !text.isEmpty else { return }
        TS3RustManager.shared.sendMessage(connectionId: connectionId, message: text)
        messageField.text = ""
    }

    @objc private func logsChanged(_ notification: Notification) {
        logView.text = TS3Logger.shared.recentEntries.joined(separator: "\n")
        let bottom = NSRange(location: max(0, logView.text.count - 1), length: 1)
        logView.scrollRangeToVisible(bottom)
    }

    // MARK: - Voice

    private func startAudio(connectionId: UInt64) {
        let audio = AudioBridge()
        TS3RustManager.shared.audioHandler = audio
        TS3RustManager.shared.onTalk = { [weak self] userId, talking in
            TS3Logger.shared.log(talking ? "用户 \(userId) 开始说话" : "用户 \(userId) 停止说话")
            _ = self
        }
        audio.sendFrame = { data in
            TS3RustManager.shared.sendAudio(connectionId: connectionId, data: data)
        }
        audio.start()
    }

    private func stopAudio() {
        TS3RustManager.shared.audioHandler?.stop()
        TS3RustManager.shared.audioHandler = nil
        TS3RustManager.shared.onTalk = nil
        micEnabled = false
        holdingPTT = false
        micButton.setTitle(LanguageManager.shared.localizedString("microphone_off"), for: .normal)
        pttButton.isEnabled = false
    }

    @objc private func toggleMic() {
        micEnabled.toggle()
        setMic(micEnabled)
    }

    @objc private func startPTT() {
        guard !holdingPTT else { return }
        holdingPTT = true
        setMic(true)
    }

    @objc private func endPTT() {
        guard holdingPTT else { return }
        holdingPTT = false
        setMic(false)
    }

    private func setMic(_ on: Bool) {
        micEnabled = on
        TS3RustManager.shared.audioHandler?.setMuted(!on)
        micButton.setTitle(
            LanguageManager.shared.localizedString(on ? "microphone_on" : "microphone_off"),
            for: .normal
        )
    }

    private func updateConnectUI(connected: Bool) {
        connectButton.setTitle(
            LanguageManager.shared.localizedString(connected ? "disconnect" : "connect"),
            for: .normal
        )
        [serverField, portField, nicknameField, passwordField].forEach {
            $0.isEnabled = !connected
        }
        micButton.isEnabled = connected
        pttButton.isEnabled = connected
    }

    // MARK: - UI

    private func buildUI() {
        let formStack = UIStackView(arrangedSubviews: [
            makeField(serverField, placeholder: LanguageManager.shared.localizedString("server_address"),
                      keyboardType: .URL),
            makeField(portField, placeholder: LanguageManager.shared.localizedString("port"),
                      keyboardType: .numberPad, defaultText: "9987"),
            makeField(nicknameField, placeholder: LanguageManager.shared.localizedString("nickname")),
            makeField(passwordField, placeholder: LanguageManager.shared.localizedString("password"),
                      secure: true)
        ])
        formStack.axis = .vertical
        formStack.spacing = 8

        connectButton.setTitle(LanguageManager.shared.localizedString("connect"), for: .normal)
        connectButton.addTarget(self, action: #selector(connectTapped), for: .touchUpInside)

        statusLabel.text = LanguageManager.shared.localizedString("status_disconnected")
        statusLabel.textColor = .secondaryLabel
        statusLabel.font = .preferredFont(forTextStyle: .footnote)

        sendButton.setTitle(LanguageManager.shared.localizedString("send"), for: .normal)
        sendButton.addTarget(self, action: #selector(sendTapped), for: .touchUpInside)
        messageField.placeholder = LanguageManager.shared.localizedString("chat")
        messageField.borderStyle = .roundedRect
        messageField.returnKeyType = .send

        let inputRow = UIStackView(arrangedSubviews: [messageField, sendButton])
        inputRow.axis = .horizontal
        inputRow.spacing = 8

        micButton.setTitle(LanguageManager.shared.localizedString("microphone_off"), for: .normal)
        micButton.addTarget(self, action: #selector(toggleMic), for: .touchUpInside)
        micButton.isEnabled = false

        pttButton.setTitle(LanguageManager.shared.localizedString("ptt"), for: .normal)
        pttButton.isEnabled = false
        pttButton.addGestureRecognizer(
            UILongPressGestureRecognizer(target: self,
                                         action: #selector(pttPressed(_:)))
        )

        let audioRow = UIStackView(arrangedSubviews: [micButton, pttButton])
        audioRow.axis = .horizontal
        audioRow.spacing = 16

        logView.isEditable = false
        logView.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
        logView.backgroundColor = .secondarySystemBackground
        logView.layer.cornerRadius = 8

        let topStack = UIStackView(arrangedSubviews: [formStack, connectButton, statusLabel, audioRow, inputRow, logView])
        topStack.axis = .vertical
        topStack.spacing = 12
        topStack.isLayoutMarginsRelativeArrangement = true
        topStack.layoutMargins = UIEdgeInsets(top: 16, left: 16, bottom: 16, right: 16)
        topStack.translatesAutoresizingMaskIntoConstraints = false

        view.addSubview(topStack)
        NSLayoutConstraint.activate([
            topStack.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            topStack.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            topStack.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            topStack.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor),
            logView.heightAnchor.constraint(greaterThanOrEqualToConstant: 180)
        ])
    }

    private func makeField(_ field: UITextField, placeholder: String,
                           keyboardType: UIKeyboardType = .default,
                           defaultText: String? = nil, secure: Bool = false) -> UITextField {
        field.placeholder = placeholder
        field.borderStyle = .roundedRect
        field.keyboardType = keyboardType
        field.autocapitalizationType = .none
        field.autocorrectionType = .no
        field.isSecureTextEntry = secure
        if let defaultText = defaultText {
            field.text = defaultText
        }
        return field
    }

    @objc private func pttPressed(_ gesture: UILongPressGestureRecognizer) {
        switch gesture.state {
        case .began:
            startPTT()
        case .ended, .cancelled, .failed:
            endPTT()
        default:
            break
        }
    }
}
