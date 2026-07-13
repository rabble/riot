import XCTest

final class RiotTabNavigationUITests: XCTestCase {
    func testEachTabIsReachableAndCapturesAScreenshot() {
        let app = XCUIApplication()
        app.launch()

        let alert = app.alerts.firstMatch
        if alert.waitForExistence(timeout: 2) {
            alert.buttons.firstMatch.tap()
        }

        // Must match `RiotDestination.tabTitle` for every case in `phoneTabs`.
        // A stale list here is why this test was red: a committed change removed
        // the Import tab and this list kept asserting it.
        let tabs = ["Spaces", "Apps", "Board", "Post", "Connect"]
        for tab in tabs {
            let button = app.buttons[tab]
            XCTAssertTrue(button.waitForExistence(timeout: 5), "\(tab) tab button should exist")
            button.tap()
            XCTAssertTrue(button.isSelected, "\(tab) should become the selected tab")

            if tab == "Post" {
                XCTAssertTrue(app.staticTexts["post-review"].waitForExistence(timeout: 2))
                XCTAssertTrue(app.buttons["post-update"].exists)
                XCTAssertEqual(app.switches["Started with model assistance"].value as? String, "0")
            }

            let screenshot = app.screenshot()
            let attachment = XCTAttachment(screenshot: screenshot)
            attachment.name = "tab-\(tab)"
            attachment.lifetime = .keepAlways
            add(attachment)
        }
    }
}
