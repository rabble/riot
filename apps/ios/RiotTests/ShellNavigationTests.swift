import XCTest
@testable import RiotKit

final class ShellNavigationTests: XCTestCase {
    func testConferenceShellExposesOnlyWorkingSurfaces() {
        XCTAssertEqual(
            RiotDestination.phoneTabs.map(\.title),
            [
                "Spaces",
                "App directory",
                "Incident board",
                "Compose & sign",
                "Connection",
            ]
        )
        XCTAssertEqual(
            RiotDestination.phoneTabs.map(\.tabTitle),
            ["Spaces", "Apps", "Board", "Compose", "Connect"]
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
