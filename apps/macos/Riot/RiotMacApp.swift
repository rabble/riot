import RiotKit
import SwiftUI

@main
struct RiotMacApp: App {
    @StateObject private var model = RiotAppModel()

    var body: some Scene {
        WindowGroup {
            ConferenceShellView(model: model)
                .task { model.bootstrap() }
        }
        .defaultSize(width: 480, height: 860)
    }
}
