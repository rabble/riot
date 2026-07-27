import XCTest

@MainActor
final class ReactionControlsUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUp() {
        super.setUp()
        continueAfterFailure = false
    }

    func testSupportClickReachesTheJoinedReadersCore() {
        launchJoinedFixture(width: nil, delayMilliseconds: 3_000)

        let support = reactionButton("support")
        XCTAssertTrue(
            support.waitForExistence(timeout: 12),
            "the joined reader must render the fixture post's Support control"
        )
        XCTAssertEqual(support.value as? String, "No reactions, not selected")

        support.tap()
        XCTAssertTrue(
            waitForEnabled(false, on: support, timeout: 2),
            "the deterministic fixture delay must disable the pending control"
        )
        XCTAssertTrue(
            app.staticTexts["Still saving on this device…"].waitForExistence(timeout: 3),
            "the real two-second stall timer must remain observable while pending"
        )
        XCTAssertTrue(
            waitForValue("1 reaction, selected", on: support, timeout: 8),
            "the click must commit through SwiftUI, the model, UniFFI, and the Rust core"
        )
    }

    func testAllFourReactionsAddAndRemoveAgainstTheRealCore() {
        launchJoinedFixture(width: nil)

        for kind in ["support", "solidarity", "important", "grief"] {
            let control = reactionButton(kind)
            XCTAssertTrue(control.waitForExistence(timeout: 12))
            XCTAssertEqual(control.value as? String, "No reactions, not selected")

            control.tap()
            XCTAssertTrue(waitForValue("1 reaction, selected", on: control, timeout: 8))
            control.tap()
            XCTAssertTrue(waitForValue("No reactions, not selected", on: control, timeout: 8))
        }
    }

    func testTypedPreCommitFailuresShowExactCopyAndCorrectDisableScope() {
        let cases = [
            ("retryable", "Couldn’t save your reaction. Try again."),
            ("authority", "Reactions aren’t available for this post."),
            ("capacity", "This community can’t hold another reaction right now."),
            ("clock", "Check this device’s Date & Time before reacting."),
        ]

        for (mode, message) in cases {
            launchJoinedFixture(width: nil, mode: mode)
            let support = reactionButton("support")
            let solidarity = reactionButton("solidarity")
            XCTAssertTrue(support.waitForExistence(timeout: 12))
            support.tap()
            XCTAssertTrue(app.staticTexts[message].waitForExistence(timeout: 5))
            XCTAssertEqual(support.value as? String, "No reactions, save failed")

            switch mode {
            case "retryable":
                XCTAssertTrue(support.isEnabled)
                XCTAssertTrue(solidarity.isEnabled)
            case "authority":
                XCTAssertFalse(support.isEnabled)
                XCTAssertFalse(solidarity.isEnabled)
            case "capacity", "clock":
                XCTAssertFalse(support.isEnabled)
                XCTAssertTrue(solidarity.isEnabled)
            default:
                XCTFail("closed fixture mode table")
            }
            app.terminate()
        }
    }

    func testPointerAndKeyboardActivationRetainFocus() {
        launchJoinedFixture(width: nil)
        let support = reactionButton("support")
        XCTAssertTrue(support.waitForExistence(timeout: 12))
        focusByTab(support)

        app.typeKey(" ", modifierFlags: [])
        XCTAssertTrue(waitForValue("1 reaction, selected", on: support, timeout: 8))
        XCTAssertEqual(focusedControl.identifier, support.identifier)

        app.typeKey(" ", modifierFlags: [])
        XCTAssertTrue(waitForValue("No reactions, not selected", on: support, timeout: 8))
        XCTAssertEqual(focusedControl.identifier, support.identifier)
    }

    func testHoverHelpNamesEveryReaction() {
        launchJoinedFixture(width: nil)
        for (kind, name) in [
            ("support", "Support"),
            ("solidarity", "Solidarity"),
            ("important", "Important"),
            ("grief", "Grief"),
        ] {
            let control = reactionButton(kind)
            XCTAssertTrue(control.waitForExistence(timeout: 12))
            control.hover()
            XCTAssertTrue(
                app.helpTags[name].waitForExistence(timeout: 4),
                "hover help must expose the full \(name) name"
            )
        }
    }

    func testReactionAt900() {
        assertReactionScreenshot(width: 900)
    }

    func testReactionAt1200() {
        assertReactionScreenshot(width: 1_200)
    }

    func testInvalidFixtureRunIDFailsClosed() {
        app = XCUIApplication()
        app.launchEnvironment["RIOT_UI_TEST_FIXTURE"] = "reactions-joined"
        app.launchEnvironment["RIOT_UI_TEST_RUN_ID"] = "not-a-uuid"
        app.launch()
        ensureWindow()

        XCTAssertTrue(app.staticTexts["ui-fixture-invalid"].waitForExistence(timeout: 8))
        XCTAssertFalse(app.staticTexts["River City Wire"].exists)
        XCTAssertFalse(app.buttons["onboarding-get-started"].exists)
    }

    func testMissingFixtureRunIDFailsClosed() {
        app = XCUIApplication()
        app.launchEnvironment["RIOT_UI_TEST_FIXTURE"] = "reactions-joined"
        app.launch()
        ensureWindow()

        XCTAssertTrue(app.staticTexts["ui-fixture-invalid"].waitForExistence(timeout: 8))
        XCTAssertFalse(app.staticTexts["River City Wire"].exists)
        XCTAssertFalse(app.buttons["onboarding-get-started"].exists)
    }

    private func launchJoinedFixture(
        width: Int?,
        mode: String = "success",
        delayMilliseconds: Int = 0
    ) {
        app?.terminate()
        app = XCUIApplication()
        app.launchEnvironment["RIOT_UI_TEST_RUN_ID"] = UUID().uuidString
        app.launchEnvironment["RIOT_UI_TEST_FIXTURE"] = "reactions-joined"
        app.launchEnvironment["RIOT_UI_TEST_REACTION_MODE"] = mode
        app.launchEnvironment["RIOT_UI_TEST_REACTION_DELAY_MS"] =
            String(delayMilliseconds)
        app.launchEnvironment["RIOT_UI_TEST_DISABLE_NEARBY_AUTOSTART"] = "1"
        app.launchArguments += ["-AppleKeyboardUIMode", "3"]
        if let width {
            app.launchEnvironment["RIOT_UI_TEST_WINDOW_WIDTH"] = String(width)
        }
        app.launch()
        ensureWindow()
    }

    private func ensureWindow() {
        guard !app.windows.firstMatch.waitForExistence(timeout: 1) else { return }
        app.menuBars.menuBarItems["File"].click()
        app.menuItems["New Window"].click()
        XCTAssertTrue(app.windows.firstMatch.waitForExistence(timeout: 4))
    }

    private var focusedControl: XCUIElement {
        app.descendants(matching: .any).matching(
            NSPredicate(format: "hasKeyboardFocus == true")
        ).firstMatch
    }

    private func focusByTab(_ control: XCUIElement) {
        for _ in 0..<20 {
            app.typeKey("\t", modifierFlags: [])
            if focusedControl.identifier == control.identifier {
                return
            }
        }
        XCTFail("Tab must reach \(control.identifier)")
    }

    private func assertReactionScreenshot(width: Int) {
        launchJoinedFixture(width: width)
        XCTAssertTrue(reactionButton("support").waitForExistence(timeout: 12))
        XCTAssertEqual(app.windows.firstMatch.frame.width, CGFloat(width), accuracy: 1)
        for kind in ["support", "solidarity", "important", "grief"] {
            XCTAssertTrue(reactionButton(kind).exists)
        }

        let attachment = XCTAttachment(screenshot: app.windows.firstMatch.screenshot())
        attachment.name = "reaction-macos-\(width)"
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func reactionButton(_ kind: String) -> XCUIElement {
        app.descendants(matching: .any).matching(
            NSPredicate(format: "identifier BEGINSWITH %@", "reaction-\(kind)-")
        ).firstMatch
    }

    private func waitForValue(
        _ value: String,
        on element: XCUIElement,
        timeout: TimeInterval
    ) -> Bool {
        let expectation = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "value == %@", value),
            object: element
        )
        return XCTWaiter.wait(for: [expectation], timeout: timeout) == .completed
    }

    private func waitForEnabled(
        _ enabled: Bool,
        on element: XCUIElement,
        timeout: TimeInterval
    ) -> Bool {
        let expectation = XCTNSPredicateExpectation(
            predicate: NSPredicate(format: "enabled == %@", NSNumber(value: enabled)),
            object: element
        )
        return XCTWaiter.wait(for: [expectation], timeout: timeout) == .completed
    }
}
