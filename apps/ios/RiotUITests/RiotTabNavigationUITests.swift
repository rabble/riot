import XCTest

/// The community-first shell: a launch screen with no community, then the four
/// routes — Home, Tools, People, Nearby — reachable as tabs once a community
/// exists. Post an update is a primary Home action, not a fifth tab.
final class RiotTabNavigationUITests: XCTestCase {
    func testEachRouteIsReachableAndCapturesAScreenshot() {
        let app = XCUIApplication()
        app.launch()

        let alert = app.alerts.firstMatch
        if alert.waitForExistence(timeout: 2) {
            alert.buttons.firstMatch.tap()
        }

        // On a clean launch there is no community, so the four route tabs are not
        // on screen yet. Create one from the launch screen; a re-run against
        // leftover state may already have a community, in which case Home is
        // present and we skip creation.
        let home = app.buttons["Home"]
        if !home.waitForExistence(timeout: 3) {
            let nameField = app.textFields["community-name-field"]
            if nameField.waitForExistence(timeout: 5) {
                nameField.tap()
                nameField.typeText("Riverside Tenants Union")
            }
            let create = app.buttons["create-community"]
            XCTAssertTrue(create.waitForExistence(timeout: 5), "the launch screen offers Create a community")
            create.tap()
        }

        // The four routes, each selectable, in canonical order.
        let tabs = ["Home", "Tools", "People", "Nearby"]
        for tab in tabs {
            let button = app.buttons[tab]
            XCTAssertTrue(button.waitForExistence(timeout: 5), "\(tab) route button should exist")
            button.tap()
            XCTAssertTrue(button.isSelected, "\(tab) should become the selected route")

            // Post an update is a primary Home action, off by default for model
            // assistance — never a fifth destination.
            if tab == "Home" {
                XCTAssertTrue(app.staticTexts["post-review"].waitForExistence(timeout: 2))
                XCTAssertTrue(app.buttons["post-update"].exists)
                XCTAssertEqual(app.switches["Started with model assistance"].value as? String, "0")
                // The two relocated identity paths are both present and distinct.
                XCTAssertTrue(app.buttons["your-profile"].exists, "the avatar opens Your profile")
                XCTAssertTrue(app.buttons["community-settings"].exists, "a gear opens Community settings")
            }

            let screenshot = app.screenshot()
            let attachment = XCTAttachment(screenshot: screenshot)
            attachment.name = "route-\(tab)"
            attachment.lifetime = .keepAlways
            add(attachment)
        }
    }
}
