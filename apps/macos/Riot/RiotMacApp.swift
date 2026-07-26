import RiotKit
import SwiftUI

@main
struct RiotMacApp: App {
    @StateObject private var model = RiotAppModel()

    var body: some Scene {
        WindowGroup {
            ConferenceShellView(model: model)
                .task {
                    // Install the process-wide tracing subscriber BEFORE any
                    // sync/reply work, so the Rust spans forward to the unified
                    // log (Console.app / `log stream`). Idempotent: safe across
                    // re-entrant launches. See riot-ffi logging::init_app_logging.
                    initLogging(level: .info)
                    model.bootstrap()
                }
                // Riot's identity is the warm cream/newsprint zine look — a
                // light-first design. Lock the appearance so the brand stays
                // coherent instead of inverting to a muddy dark paper in the
                // system's dark mode.
                .preferredColorScheme(.light)
        }
        .defaultSize(width: 480, height: 860)
    }
}
