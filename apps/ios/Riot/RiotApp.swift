import SwiftUI
import RiotKit

@main
struct RiotApp: App {
    @StateObject private var model = RiotAppModel()

    var body: some Scene {
        WindowGroup {
            ConferenceShellView(model: model)
                .task { model.bootstrap() }
        }
    }
}
