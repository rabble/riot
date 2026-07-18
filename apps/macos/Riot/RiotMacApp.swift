import RiotKit
import SwiftUI

@main
struct RiotMacApp: App {
    @StateObject private var model = RiotAppModel()

    var body: some Scene {
        WindowGroup {
            ConferenceShellView(model: model)
                .task { model.bootstrap() }
                // Riot's identity is the warm cream/newsprint zine look — a
                // light-first design. Lock the appearance so the brand stays
                // coherent instead of inverting to a muddy dark paper in the
                // system's dark mode.
                .preferredColorScheme(.light)
        }
        .defaultSize(width: 480, height: 860)
    }
}
