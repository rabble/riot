import XCTest

/// Community-first shell (2A): create a community from the launch screen, then
/// approve and open the Checklist on the Tools route, add an item, and prove it
/// survives a relaunch.
@MainActor
final class ChecklistFlowUITests: XCTestCase {
    func testCreateCommunityApproveChecklistAddItemAndSurviveRelaunch() throws {
        let app = XCUIApplication()
        app.launchEnvironment["RIOT_UI_TEST_RUN_ID"] = UUID().uuidString
        app.launchEnvironment["RIOT_UI_TEST_SUPPRESS_NOTIFICATION_PERMISSION"] = "1"
        app.launchEnvironment["RIOT_UI_TEST_DISABLE_NEARBY_AUTOSTART"] = "1"
        app.launch()

        let getStarted = app.buttons["onboarding-get-started"]
        XCTAssertTrue(getStarted.waitForExistence(timeout: 8))
        getStarted.tap()

        let displayName = app.textFields["launch-display-name"]
        XCTAssertTrue(displayName.waitForExistence(timeout: 5))
        displayName.tap()
        displayName.typeText("Ana")

        let create = app.buttons["create-community"]
        XCTAssertTrue(create.waitForExistence(timeout: 5))
        create.tap()

        let name = app.textFields["create-community-name-field"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        name.tap()
        name.typeText("River City Wire")
        app.buttons["create-community-confirm"].tap()

        let tools = app.buttons["Tools"]
        XCTAssertTrue(tools.waitForExistence(timeout: 10))
        tools.tap()

        let communityHeader = app.staticTexts["River City Wire"]
        let inCommunity = app.staticTexts["In River City Wire"]
        let available = app.staticTexts["Available to add"]
        XCTAssertTrue(communityHeader.waitForExistence(timeout: 10))
        XCTAssertTrue(inCommunity.waitForExistence(timeout: 5))
        XCTAssertTrue(available.waitForExistence(timeout: 5))
        XCTAssertLessThan(
            inCommunity.frame.minY,
            available.frame.minY,
            "enabled tools must be presented before tools available to add"
        )

        let addCTA = button(
            in: app,
            identifierPrefix: "directory-add-",
            label: "Add Checklist to River City Wire"
        )
        XCTAssertTrue(addCTA.waitForExistence(timeout: 10))
        let appID = String(addCTA.identifier.dropFirst("directory-add-".count))
        XCTAssertFalse(appID.isEmpty)
        addCTA.tap()

        let sheetTitle = app.staticTexts["Add Checklist to River City Wire?"]
        let permissions = app.staticTexts
            .matching(NSPredicate(format: "label ==[c] %@", "This tool can"))
            .firstMatch
        let approve = app.buttons["approve-app"]
        XCTAssertTrue(sheetTitle.waitForExistence(timeout: 5))
        XCTAssertTrue(permissions.waitForExistence(timeout: 5))
        XCTAssertTrue(approve.waitForExistence(timeout: 5))
        XCTAssertLessThan(
            permissions.frame.minY,
            approve.frame.minY,
            "permissions must be read before the organizer confirms Add"
        )
        XCTAssertEqual(approve.label, "Add to River City Wire")
        approve.tap()

        let open = app.buttons["directory-open-\(appID)"]
        XCTAssertTrue(
            open.waitForExistence(timeout: 10),
            "successful approval must replace Add with an immediate Open action"
        )
        XCTAssertEqual(open.label, "Open Checklist")
        XCTAssertTrue(open.isHittable)
        XCTAssertFalse(addCTA.exists)
        assertForbiddenDirectoryCopyIsAbsent(in: app)

        let toolsScreenshot = XCTAttachment(screenshot: app.screenshot())
        toolsScreenshot.name = "community-scoped-tools-organizer"
        toolsScreenshot.lifetime = .keepAlways
        add(toolsScreenshot)
        try auditTools(in: app, communityTitle: "River City Wire")

        // Open it and add an item inside the WebView.
        let openAfterAudit = app.buttons["directory-open-\(appID)"]
        scrollToHittable(openAfterAudit, in: app)
        openAfterAudit.tap()
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
        let reopen = app.buttons["directory-open-\(appID)"]
        XCTAssertTrue(reopen.waitForExistence(timeout: 10), "trust must persist across relaunch")
        reopen.tap()
        XCTAssertTrue(app.webViews.firstMatch.staticTexts["Bring water"].waitForExistence(timeout: 10),
                      "items must persist across relaunch")

        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }

    private func button(
        in app: XCUIApplication,
        identifierPrefix: String,
        label: String
    ) -> XCUIElement {
        app.buttons
            .matching(NSPredicate(format: "identifier BEGINSWITH %@", identifierPrefix))
            .matching(NSPredicate(format: "label == %@", label))
            .firstMatch
    }

    private func assertForbiddenDirectoryCopyIsAbsent(in app: XCUIApplication) {
        for phrase in [
            "Built in",
            "From your communities",
            "Share with this community",
            "Review Checklist",
        ] {
            let matches = app.descendants(matching: .any).matching(
                NSPredicate(format: "label CONTAINS[c] %@", phrase)
            )
            XCTAssertEqual(matches.count, 0, "Tools must not expose “\(phrase)”")
        }
    }

    private func auditTools(in app: XCUIApplication, communityTitle: String) throws {
        try app.performAccessibilityAudit(
            for: [.hitRegion, .sufficientElementDescription, .trait]
        ) { issue in
            // The shell's pre-existing compact community chooser is outside the
            // Tools surface. Keep its known hit-region debt from hiding a Tools
            // regression while every directory issue remains blocking.
            issue.element?.label != communityTitle
        }
    }

    private func scrollToHittable(_ element: XCUIElement, in app: XCUIApplication) {
        for _ in 0..<8 where !element.isHittable {
            app.swipeUp()
        }
        XCTAssertTrue(element.isHittable)
    }
}
