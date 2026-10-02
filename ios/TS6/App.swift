import SwiftUI
import AVFoundation
import Network

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
