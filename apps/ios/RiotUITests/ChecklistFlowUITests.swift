import XCTest

final class ChecklistFlowUITests: XCTestCase {
    func testCreateSpaceApproveChecklistAddItemAndSurviveRelaunch() {
        let app = XCUIApplication()
        app.launch()
        if app.alerts.firstMatch.waitForExistence(timeout: 2) {
            app.alerts.firstMatch.buttons.firstMatch.tap()
        }

        // Create the space if this run starts fresh. The primary button style
        // uppercases its label ("CREATE PUBLIC SPACE"), so match case-insensitively.
        // Creating a space navigates to the Board tab, so return to Spaces where
        // the Tools list lives.
        let createButton = app.buttons.matching(
            NSPredicate(format: "label ==[c] %@", "Create public space")
        ).firstMatch
        if createButton.waitForExistence(timeout: 3) {
            createButton.tap()
            app.buttons["Spaces"].tap()
        }

        // The checklist starter tool must be installed. On a fresh space it is
        // untrusted and needs the organizer's approval; if a previous run on
        // this simulator already trusted it, it opens directly. Asserting on
        // either "review" or "open" keeps a clean run exercising the approval
        // path while letting the test survive a re-run against leftover state.
        let review = app.buttons["review-Checklist"]
        let open = app.buttons["open-Checklist"]
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

        // Check it off. Use firstMatch: a re-run against leftover state may have
        // more than one "Bring water" row.
        let checkbox = webView.checkBoxes["Bring water"].firstMatch
        if checkbox.waitForExistence(timeout: 5) {
            checkbox.tap()
        } else {
            webView.switches["Bring water"].firstMatch.tap() // WebKit may expose <input type=checkbox> as a switch
        }

        // Relaunch: trust and the item must survive.
        app.terminate()
        app.launch()
        if app.alerts.firstMatch.waitForExistence(timeout: 2) {
            app.alerts.firstMatch.buttons.firstMatch.tap()
        }
        let reopen = app.buttons["open-Checklist"]
        XCTAssertTrue(reopen.waitForExistence(timeout: 10), "trust must persist across relaunch")
        reopen.tap()
        XCTAssertTrue(app.webViews.firstMatch.staticTexts["Bring water"].waitForExistence(timeout: 10),
                      "items must persist across relaunch")

        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }
}
