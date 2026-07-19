import SwiftUI
import RiotKit

@main
struct RiotApp: App {
    @StateObject private var model = RiotAppModel()

    var body: some Scene {
        WindowGroup {
            ConferenceShellView(model: model)
                .task { bootstrap() }
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
    }
}

/// An unsigned XCUITest runner cannot add Keychain items. Keep that constraint
/// inside the explicitly UUID-gated automation path: each run gets isolated
/// storage and a stable 32-byte key, while every ordinary launch still uses the
/// production Keychain store.
private struct UIAutomationWrappingKeyStore: WrappingKeyStore {
    let runID: UUID

    func loadOrCreateWrappingKey() throws -> Data {
        Data((runID.uuidString + runID.uuidString).utf8.prefix(32))
    }
}
