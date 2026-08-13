import SwiftUI

@main
struct TS6App: App {
    @StateObject private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            ContentView(model: model)
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
