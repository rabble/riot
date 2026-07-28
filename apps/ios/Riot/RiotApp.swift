import SwiftUI
import RiotKit

@main
struct RiotApp: App {
    @StateObject private var model = RiotAppModel()
    #if DEBUG
    private let reactionFixture = ReactionUITestEnvironment.resolve()
    #endif

    var body: some Scene {
        WindowGroup {
            Group {
                #if DEBUG
                if reactionFixture == .invalid {
                    Text("ui-fixture-invalid")
                        .accessibilityIdentifier("ui-fixture-invalid")
                } else {
                    ConferenceShellView(model: model)
                }
                #else
                ConferenceShellView(model: model)
                #endif
            }
                .task {
                    // Install the process-wide tracing subscriber BEFORE any
                    // sync/reply work, so the Rust spans forward to the unified
                    // log (Console.app / `log stream`). Idempotent: safe across
                    // re-entrant launches. See riot-ffi logging::init_app_logging.
                    initLogging(level: .info)
                    bootstrap()
                }
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

    /// UI automation must prove first-run behaviour without depending on, or
    /// deleting, a simulator's existing profile. The test runner supplies a
    /// UUID; production launches have no such environment value and continue
    /// to use the normal Application Support directory.
    private func bootstrap() {
        #if DEBUG
        switch reactionFixture {
        case let .valid(configuration):
            let directory = FileManager.default.temporaryDirectory
                .appendingPathComponent(
                    "riot-ui-\(configuration.runID.uuidString)",
                    isDirectory: true
                )
            model.bootstrapReactionUITestFixture(
                baseDirectory: directory,
                keyStore: UIAutomationWrappingKeyStore(runID: configuration.runID),
                configuration: configuration
            )
            return
        case .invalid:
            return
        case .inactive:
            break
        }

        guard
            let runID = ProcessInfo.processInfo.environment["RIOT_UI_TEST_RUN_ID"],
            let uuid = UUID(uuidString: runID)
        else {
            model.bootstrap()
            return
        }

        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("riot-ui-\(uuid.uuidString)", isDirectory: true)
        model.bootstrap(
            storageDirectory: directory,
            keyStore: UIAutomationWrappingKeyStore(runID: uuid)
        )
        #else
        model.bootstrap()
        #endif
    }
}
