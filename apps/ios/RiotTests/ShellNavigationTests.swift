import XCTest
@testable import RiotKit

final class ShellNavigationTests: XCTestCase {
    func testConferenceShellExposesOnlyTheFivePlannedSurfaces() {
        XCTAssertEqual(
            RiotDestination.phoneTabs.map(\.title),
            ["Spaces", "Incident board", "Compose & sign", "Import preview", "Connection"]
        )
        XCTAssertEqual(
            RiotDestination.phoneTabs.map(\.tabTitle),
            ["Spaces", "Board", "Compose", "Import", "Connect"]
        )
    }

    @MainActor
    func testEveryPhoneTabCanBecomeTheVisibleDestination() {
        let model = RiotAppModel()

        for destination in RiotDestination.phoneTabs {
            model.select(destination)
            XCTAssertEqual(model.destination, destination)
        }
    }

    @MainActor
    func testConnectionStartsExplicitlyOffline() {
        let model = RiotAppModel()
        XCTAssertEqual(model.connectionStatus, .offline)
        XCTAssertEqual(model.connectionDisclosure, "Offline · local device only")
    }

    @MainActor
    func testDismissingAnAlertClearsItsBackingError() {
        let model = RiotAppModel(testError: "InvalidInput")

        model.dismissError()

        XCTAssertNil(model.errorMessage)
    }
}
