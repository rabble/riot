import XCTest

final class RiotTabNavigationUITests: XCTestCase {
    func testEachTabIsReachableAndCapturesAScreenshot() {
        let app = XCUIApplication()
        app.launch()

        let alert = app.alerts.firstMatch
        if alert.waitForExistence(timeout: 2) {
            alert.buttons.firstMatch.tap()
        }

        let tabs = ["Spaces", "Apps", "Board", "Compose", "Import", "Connect"]
        for tab in tabs {
            let button = app.buttons[tab]
            XCTAssertTrue(button.waitForExistence(timeout: 5), "\(tab) tab button should exist")
            button.tap()

            let screenshot = app.screenshot()
            let attachment = XCTAttachment(screenshot: screenshot)
            attachment.name = "tab-\(tab)"
            attachment.lifetime = .keepAlways
            add(attachment)
        }
    }
}
