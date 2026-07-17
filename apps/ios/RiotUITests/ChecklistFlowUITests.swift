import XCTest

/// Community-first shell (2A): create a community from the launch screen, then
/// approve and open the Checklist on the Tools route, add an item, and prove it
/// survives a relaunch.
final class ChecklistFlowUITests: XCTestCase {
    func testCreateCommunityApproveChecklistAddItemAndSurviveRelaunch() {
        let app = XCUIApplication()
        app.launch()
        if app.alerts.firstMatch.waitForExistence(timeout: 2) {
            app.alerts.firstMatch.buttons.firstMatch.tap()
        }

        // First-run onboarding opens on a welcome screen; advance to setup where
        // create lives. Guarded, so a leftover-community re-run skips it.
        let getStarted = app.buttons["onboarding-get-started"]
        if getStarted.waitForExistence(timeout: 3) { getStarted.tap() }

        // Create the community if this run starts fresh; a re-run against leftover
        // state already has one, in which case the Tools route is present.
        if !app.buttons["Tools"].waitForExistence(timeout: 3) {
            let name = app.textFields["community-name-field"]
            if name.waitForExistence(timeout: 5) {
                name.tap()
                name.typeText("Berlin Mutual Aid")
            }
            let create = app.buttons["create-community"]
            if create.waitForExistence(timeout: 3) { create.tap() }
        }

        app.buttons["Tools"].tap()

        // The checklist starter tool must be installed. On a fresh community it is
        // untrusted and needs the organizer's approval; if a previous run already
        // trusted it, it opens directly.
        let review = app.buttons["directory-review-Checklist"]
        let open = app.buttons["directory-open-Checklist"]
        XCTAssertTrue(
            review.waitForExistence(timeout: 5) || open.waitForExistence(timeout: 5),
            "checklist tool must be installed"
        )
        if review.exists {
            review.tap()
            let approve = app.buttons["approve-app"]
            XCTAssertTrue(approve.waitForExistence(timeout: 5))
            approve.tap()
        }

        // Open it and add an item inside the WebView.
        XCTAssertTrue(open.waitForExistence(timeout: 5))
        open.tap()
        let webView = app.webViews.firstMatch
        let field = webView.textFields["New item"]
        XCTAssertTrue(field.waitForExistence(timeout: 10), "checklist page must load")
        field.tap()
        field.typeText("Bring water")
        webView.buttons["Add"].tap()
        XCTAssertTrue(webView.staticTexts["Bring water"].waitForExistence(timeout: 10))

        let checkbox = webView.checkBoxes["Bring water"].firstMatch
        if checkbox.waitForExistence(timeout: 5) {
            checkbox.tap()
        } else {
            webView.switches["Bring water"].firstMatch.tap()
        }

        // Relaunch: the community, trust, and the item must survive.
        app.terminate()
        app.launch()
        if app.alerts.firstMatch.waitForExistence(timeout: 2) {
            app.alerts.firstMatch.buttons.firstMatch.tap()
        }
        app.buttons["Tools"].tap()
        let reopen = app.buttons["directory-open-Checklist"]
        XCTAssertTrue(reopen.waitForExistence(timeout: 10), "trust must persist across relaunch")
        reopen.tap()
        XCTAssertTrue(app.webViews.firstMatch.staticTexts["Bring water"].waitForExistence(timeout: 10),
                      "items must persist across relaunch")

        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }
}
