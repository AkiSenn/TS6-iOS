import SwiftUI
import AVFoundation
import Network
import UIKit

/// iOS 14 has no public "request local-network permission" API. Starting a
/// Bonjour browser is Apple's supported way to make the system evaluate the
/// permission. Once that evaluation has completed, ask for microphone access
/// so the two system sheets are not presented at the same time.
final class LaunchPermissionCoordinator {
    static let shared = LaunchPermissionCoordinator()

    private let queue = DispatchQueue(label: "tslib.permissions")
    private var browser: NWBrowser?
    private var started = false
    private var finishedNetworkRequest = false

    private init() {}

    func requestPermissions() {
        queue.async { [weak self] in
            guard let self = self, !self.started else { return }
            self.started = true

            let parameters = NWParameters.udp
            parameters.includePeerToPeer = true
            let browser = NWBrowser(
                for: .bonjour(type: "_ts3._udp", domain: "local."),
                using: parameters
            )
            self.browser = browser
            browser.stateUpdateHandler = { [weak self] state in
                switch state {
                case .ready, .failed, .waiting:
                    self?.finishNetworkRequest()
                case .setup, .cancelled:
                    break
                @unknown default:
                    self?.finishNetworkRequest()
                }
            }
            browser.start(queue: self.queue)

            // Avoid withholding the microphone prompt forever if mDNS never
            // supplies a terminal browser state on a particular iOS build.
            self.queue.asyncAfter(deadline: .now() + 15) { [weak self] in
                self?.finishNetworkRequest()
            }
        }
    }

    private func finishNetworkRequest() {
        guard !finishedNetworkRequest else { return }
        finishedNetworkRequest = true
        browser?.cancel()
        browser = nil

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) {
            AVAudioSession.sharedInstance().requestRecordPermission { _ in }
        }
    }
}

@main
struct TS6App: App {
    @StateObject private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            ContentView(model: model)
                .onAppear {
                    LaunchPermissionCoordinator.shared.requestPermissions()
                }
        }
    }
}

struct ContentView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        if model.isConnected {
            ServerView(model: model)
        } else {
            ConnectionView(model: model)
        }
    }
}

struct AppBackdrop: View {
    var body: some View {
        ZStack {
            LinearGradient(
                gradient: Gradient(colors: [
                    Color(red: 0.035, green: 0.055, blue: 0.13),
                    Color(red: 0.12, green: 0.08, blue: 0.25),
                    Color(red: 0.03, green: 0.16, blue: 0.22)
                ]),
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            Circle()
                .fill(Color.blue.opacity(0.22))
                .frame(width: 300, height: 300)
                .offset(x: 150, y: -260)
                .blur(radius: 28)
            Circle()
                .fill(Color.purple.opacity(0.18))
                .frame(width: 260, height: 260)
                .offset(x: -170, y: 310)
                .blur(radius: 32)
        }
        .edgesIgnoringSafeArea(.all)
    }
}

struct BlurView: UIViewRepresentable {
    let style: UIBlurEffect.Style

    func makeUIView(context: Context) -> UIVisualEffectView {
        UIVisualEffectView(effect: UIBlurEffect(style: style))
    }

    func updateUIView(_ uiView: UIVisualEffectView, context: Context) {}
}

struct GlassPanel<Content: View>: View {
    let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        content
            .padding(14)
            .background(
                ZStack {
                    BlurView(style: .systemUltraThinMaterialDark)
                    Color.white.opacity(0.055)
                }
            )
            .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .stroke(Color.white.opacity(0.14), lineWidth: 0.7)
            )
            .shadow(color: Color.black.opacity(0.2), radius: 16, y: 8)
    }
}
