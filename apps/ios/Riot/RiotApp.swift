import SwiftUI
import RiotKit

@main
struct RiotApp: App {
    @StateObject private var model = RiotAppModel()

    var body: some Scene {
        WindowGroup {
            ConferenceShellView(model: model)
                .task { model.bootstrap() }
                // "Open in Riot" from the public web newswire: verify links
                // (riot://open?namespace=&entry=) and the existing join reference
                // (riot://newswire/join/v1/...) both route through the model.
                .onOpenURL { model.handleDeepLink($0) }
                // Riot's identity is the warm cream/newsprint zine look — keep it
                // coherent instead of inverting to dark paper in the system's dark
                // mode.
                .preferredColorScheme(.light)
        }
    }
}
