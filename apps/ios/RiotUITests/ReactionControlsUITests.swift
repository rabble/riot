import XCTest

@MainActor
final class ReactionControlsUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        super.setUp()
        continueAfterFailure = false
    }

    func testLongPressNamesEveryCompactReactionWithoutChangingIt() {
        launchJoinedFixture()
        let names = [
            "support": "Support",
            "solidarity": "Solidarity",
            "important": "Important",
            "grief": "Grief",
        ]

        for (kind, name) in names {
            let control = reactionControl(kind)
            XCTAssertTrue(control.waitForExistence(timeout: 12))
            let originalValue = control.value as? String

            control.press(forDuration: 1)
            XCTAssertTrue(
                app.staticTexts[name].waitForExistence(timeout: 3),
                "long-press help must expose the full \(name) name"
            )
            app.tap()

            XCTAssertEqual(reactionControl(kind).value as? String, originalValue)
        }
    }

    func testInvalidFixtureRunIDFailsClosed() {
        app = XCUIApplication()
        app.launchEnvironment["RIOT_UI_TEST_FIXTURE"] = "reactions-joined"
        app.launchEnvironment["RIOT_UI_TEST_RUN_ID"] = "not-a-uuid"
        app.launch()

        XCTAssertTrue(app.staticTexts["ui-fixture-invalid"].waitForExistence(timeout: 8))
        XCTAssertFalse(app.staticTexts["River City Wire"].exists)
        XCTAssertFalse(app.buttons["onboarding-get-started"].exists)
    }

    private func launchJoinedFixture() {
        app = XCUIApplication()
        app.launchEnvironment["RIOT_UI_TEST_RUN_ID"] = UUID().uuidString
        app.launchEnvironment["RIOT_UI_TEST_FIXTURE"] = "reactions-joined"
        app.launchEnvironment["RIOT_UI_TEST_REACTION_DELAY_MS"] = "0"
        app.launchEnvironment["RIOT_UI_TEST_DISABLE_NEARBY_AUTOSTART"] = "1"
        app.launch()
    }

    private func reactionControl(_ kind: String) -> XCUIElement {
        app.descendants(matching: .any).matching(
            NSPredicate(format: "identifier BEGINSWITH %@", "reaction-\(kind)-")
        ).firstMatch
    }
}
