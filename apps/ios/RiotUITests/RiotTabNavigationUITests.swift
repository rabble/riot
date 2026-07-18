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
            // First-run onboarding opens on a welcome screen; advance to setup.
            let getStarted = app.buttons["onboarding-get-started"]
            if getStarted.waitForExistence(timeout: 3) { getStarted.tap() }
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

    /// Opening a tool keeps the person IN the community: it pushes under the Tools
    /// tab, so the community header and the bottom tab bar stay on screen and the
    /// app host carries a "who is active" strip. Captures Home (proving the
    /// community name is not printed twice) and the opened app.
    func testOpeningAToolKeepsTheTabBarAndShowsActivity() {
        let app = XCUIApplication()
        app.launch()

        let alert = app.alerts.firstMatch
        if alert.waitForExistence(timeout: 2) { alert.buttons.firstMatch.tap() }

        // Load the seeded demo space (Riverside Tenants Union) — it ships with an
        // approved Checklist tool whose items are authored by Ana and Priya, so
        // the activity strip has real people to name.
        let home = app.buttons["Home"]
        if !home.waitForExistence(timeout: 3) {
            let getStarted = app.buttons["onboarding-get-started"]
            if getStarted.waitForExistence(timeout: 3) { getStarted.tap() }
            let demoLoad = app.buttons["demo-load"]
            XCTAssertTrue(demoLoad.waitForExistence(timeout: 5), "onboarding offers the demo space")
            demoLoad.tap()
        }
        XCTAssertTrue(home.waitForExistence(timeout: 10), "the demo space opens to its Home")

        // The community name is named ONCE, by the persistent top bar.
        XCTAssertTrue(app.buttons["community-name"].exists, "the top bar names the community")
        attach(app.screenshot(), named: "home-no-double-name")

        // Open the Checklist from its Home shortcut — the owner's scenario. It
        // must switch to Tools and mount the tool there, keeping the shell.
        let shortcut = app.buttons["home-shortcut-Checklist"]
        XCTAssertTrue(shortcut.waitForExistence(timeout: 5), "the demo surfaces a Checklist shortcut")
        shortcut.tap()

        // The tool opened UNDER Tools, framed by a "‹ Tools" back bar — proof it
        // did not cover the shell as a context-losing full-screen sheet.
        let backToTools = app.buttons["tool-back"]
        XCTAssertTrue(backToTools.waitForExistence(timeout: 5), "the tool opened under Tools with a '‹ Tools' back bar")

        // The bottom tab bar survived — the other routes are still on screen.
        XCTAssertTrue(app.buttons["People"].exists, "the tab bar stays on screen inside a tool")
        XCTAssertTrue(app.buttons["Nearby"].exists, "every route stays reachable while a tool is open")

        // The app host shows the who-is-active strip (the demo checklist is
        // authored by Ana and Priya, so it names them).
        let strip = app.descendants(matching: .any).matching(identifier: "app-activity-strip").firstMatch
        XCTAssertTrue(strip.waitForExistence(timeout: 5), "the app host shows a who-is-active strip")
        attach(app.screenshot(), named: "app-open-inside-tools")

        // Tapping back returns to the Tools list, still inside the community.
        backToTools.tap()
        XCTAssertTrue(app.buttons["directory-open-Checklist"].waitForExistence(timeout: 5), "back lands on the Tools list")
    }

    private func attach(_ screenshot: XCUIScreenshot, named name: String) {
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
