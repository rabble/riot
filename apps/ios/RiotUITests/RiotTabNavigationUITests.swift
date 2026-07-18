import XCTest

/// Exercises the compact, community-first path a person should understand:
/// enter one community, identify yourself, publish twice, read the result, then
/// verify the three secondary routes remain reachable and plainly named.
@MainActor
final class RiotTabNavigationUITests: XCTestCase {
    private var app: XCUIApplication!

    func testCompactCoreFlowFromFirstRunThroughReadingAReport() {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchEnvironment["RIOT_UI_TEST_RUN_ID"] = UUID().uuidString
        app.launchEnvironment["RIOT_UI_TEST_SUPPRESS_NOTIFICATION_PERMISSION"] = "1"
        app.launchEnvironment["RIOT_UI_TEST_DISABLE_NEARBY_AUTOSTART"] = "1"

        app.launch()

        let getStarted = app.buttons["onboarding-get-started"]
        XCTAssertTrue(
            getStarted.waitForExistence(timeout: 8),
            "a unique UI-test run must always begin at first-run onboarding"
        )
        capture("01-first-run")
        getStarted.tap()

        XCTAssertFalse(
            app.buttons["find-nearby"].exists,
            "setup must not promote an ambiguous nearby exit"
        )
        XCTAssertFalse(
            app.buttons["launch-save-display-name"].exists,
            "the name is saved by a successful join/create exit, not a separate dead-end action"
        )

        let displayName = app.textFields["launch-display-name"]
        XCTAssertTrue(displayName.waitForExistence(timeout: 5))
        displayName.tap()
        displayName.typeText("Ana")

        let launchCreate = app.buttons["create-community"]
        XCTAssertTrue(launchCreate.waitForExistence(timeout: 5))
        launchCreate.tap()

        let communityName = app.textFields["create-community-name-field"]
        XCTAssertTrue(communityName.waitForExistence(timeout: 5))
        communityName.tap()
        communityName.typeText("Riverside Tenants Union")
        app.buttons["create-community-confirm"].tap()

        let home = app.buttons["Home"]
        XCTAssertTrue(home.waitForExistence(timeout: 10))
        XCTAssertTrue(app.buttons["community-name"].waitForExistence(timeout: 5))
        XCTAssertTrue(
            app.staticTexts["Riverside Tenants Union"].waitForExistence(timeout: 5),
            "Home must visibly identify the community the person just created"
        )
        capture("02-populated-home")

        app.buttons["your-profile"].tap()
        let renderedName = app.staticTexts.matching(
            NSPredicate(format: "label BEGINSWITH %@", "Ana · ")
        ).firstMatch
        XCTAssertTrue(
            renderedName.waitForExistence(timeout: 5),
            "a self-claimed name is always paired with its key-derived tag"
        )
        let profileDone = app.buttons["your-profile-done"]
        XCTAssertTrue(profileDone.waitForExistence(timeout: 5))
        profileDone.tap()
        XCTAssertTrue(home.waitForExistence(timeout: 5))

        let firstPost = app.buttons["Post the first update"].firstMatch
        XCTAssertTrue(firstPost.waitForExistence(timeout: 5))
        firstPost.tap()
        post(
            headline: "Water restored on Willow Street",
            body: "Crews reopened the line. Residents report normal pressure."
        )

        XCTAssertTrue(app.buttons["post-another"].waitForExistence(timeout: 10))
        XCTAssertFalse(
            app.keyboards.firstMatch.exists,
            "posting dismisses the keyboard so both success actions are visible"
        )
        capture("03-post-success")
        app.buttons["post-another"].tap()

        let headline = app.textFields["post-headline"]
        XCTAssertTrue(headline.waitForExistence(timeout: 5))
        XCTAssertTrue(
            app.keyboards.firstMatch.waitForExistence(timeout: 3),
            "Post another resets the draft and returns keyboard focus to Headline"
        )
        XCTAssertEqual(headline.value as? String, "Headline")
        post(
            headline: "Food pantry opens at six",
            body: "Bring a bag to the community hall. All neighbours are welcome."
        )

        let done = app.buttons["post-done"]
        XCTAssertTrue(done.waitForExistence(timeout: 10))
        done.tap()

        let readUpdate = app.buttons["Read Food pantry opens at six"]
        XCTAssertTrue(readUpdate.waitForExistence(timeout: 10))
        readUpdate.tap()
        XCTAssertTrue(app.buttons["newswire-detail-close"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["Food pantry opens at six"].exists)
        capture("04-report-detail")
        scrollToHittable(app.buttons["newswire-detail-close"])
        app.buttons["newswire-detail-close"].tap()
        XCTAssertTrue(
            readUpdate.waitForExistence(timeout: 5) && readUpdate.isHittable,
            "closing a report returns the exact named trigger to the interaction path"
        )
        // XCUITest does not expose VoiceOver's cursor. Reusing this same,
        // headline-specific element proves the restored target is actionable;
        // NewswireReportTrigger's surface+report identity is unit-tested.
        readUpdate.tap()
        XCTAssertTrue(app.buttons["newswire-detail-close"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["Food pantry opens at six"].exists)
        app.buttons["newswire-detail-close"].tap()

        visit(tab: "Tools", expectedHeading: "Tools", screenshot: "05-tools")
        visit(
            tab: "People",
            expectedHeading: "Known contributors",
            screenshot: "06-known-contributors"
        )
        XCTAssertTrue(
            app.staticTexts["Organizer"].waitForExistence(timeout: 5),
            "posting refreshes Known contributors without an app relaunch"
        )
        visit(tab: "Nearby", expectedHeading: "Nearby", screenshot: "07-nearby")
        XCTAssertTrue(
            app.buttons["nearby-find-devices"].waitForExistence(timeout: 5),
            "the deterministic UI flow must not inherit local-network peers or permission state"
        )
    }

    private func post(headline: String, body: String) {
        let headlineField = app.textFields["post-headline"]
        XCTAssertTrue(headlineField.waitForExistence(timeout: 5))
        headlineField.tap()
        headlineField.typeText(headline)

        let bodyField = app.textFields["post-body"]
        XCTAssertTrue(bodyField.waitForExistence(timeout: 5))
        bodyField.tap()
        bodyField.typeText(body)

        let submit = app.buttons["post-update"]
        scrollToHittable(submit)
        XCTAssertTrue(submit.isEnabled)
        submit.tap()
    }

    private func visit(tab: String, expectedHeading: String, screenshot: String) {
        let button = app.buttons[tab]
        XCTAssertTrue(button.waitForExistence(timeout: 5), "\(tab) route button should exist")
        button.tap()
        XCTAssertTrue(button.isSelected, "\(tab) should become the selected route")
        XCTAssertTrue(app.staticTexts[expectedHeading].waitForExistence(timeout: 5))
        capture(screenshot)
    }

    private func scrollToHittable(_ element: XCUIElement) {
        for _ in 0..<6 where !element.isHittable {
            app.swipeUp()
        }
        XCTAssertTrue(element.isHittable)
    }

    private func capture(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
