import XCTest

/// The Riverside demo is loaded by an ordinary MEMBER — a fresh subspace in the
/// space's namespace, never the organizer coordinate. Before Unit 0B the demo
/// carried no organizer, so its tools were untrusted: a member saw only a
/// "Review" gate they could never pass (approval is the organizer's call), and
/// the demo could not demo. Now the founding collective's signed Trust markers
/// travel in the bundle, so a member evaluates every tool as organizer-trusted
/// and opens it directly — no Review, no dead end.
///
/// Community-first shell (2A): the demo is loaded from the no-community launch
/// screen; its tools live on the Tools route.
@MainActor
final class RiversideMemberToolUITests: XCTestCase {
    func testDemoMemberOpensAnOrganizerTrustedToolWithoutAReviewGate() throws {
        let app = XCUIApplication()
        app.launchEnvironment["RIOT_UI_TEST_RUN_ID"] = UUID().uuidString
        app.launchEnvironment["RIOT_UI_TEST_SUPPRESS_NOTIFICATION_PERMISSION"] = "1"
        app.launchEnvironment["RIOT_UI_TEST_DISABLE_NEARBY_AUTOSTART"] = "1"
        app.launch()

        // First-run onboarding opens on a welcome screen; advance to the setup
        // screen where create / join / demo live. Guarded, so a re-run that
        // already has a community (shell present, no welcome) simply skips it.
        let getStarted = app.buttons["onboarding-get-started"]
        if getStarted.waitForExistence(timeout: 3) { getStarted.tap() }

        // Load the seeded Riverside space from the launch screen. Offered only
        // when the profile has no community of its own; on a clean launch that is
        // the case.
        let demoLoad = app.buttons["demo-load"]
        if demoLoad.waitForExistence(timeout: 5) {
            demoLoad.tap()
        }

        // Loading a community opens its Home; the tools live on the Tools route.
        let tools = app.buttons["Tools"]
        XCTAssertTrue(tools.waitForExistence(timeout: 10), "a loaded community shows the four routes")
        tools.tap()

        XCTAssertTrue(
            app.staticTexts["Riverside Tenants Union"].waitForExistence(timeout: 10),
            "Tools must visibly name the selected community"
        )
        XCTAssertTrue(app.staticTexts["In Riverside Tenants Union"].waitForExistence(timeout: 5))

        // As a member of an organizer-shaped space, the Checklist must be OPENABLE
        // straight away…
        let open = app.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier BEGINSWITH %@", "directory-open-"))
            .matching(NSPredicate(format: "label == %@", "Open Checklist"))
            .firstMatch
        XCTAssertTrue(
            open.waitForExistence(timeout: 10),
            "an organizer-trusted tool must be openable by a demo member"
        )

        // …and there must be NO Review gate. A member cannot approve, so a
        // Review affordance here would be the exact dead end 0B removes.
        XCTAssertFalse(
            app.descendants(matching: .any)
                .matching(NSPredicate(format: "label CONTAINS[c] %@", "Review Checklist"))
                .firstMatch.exists,
            "a demo member must never be sent to a Review gate they cannot pass"
        )
        XCTAssertFalse(
            app.buttons
                .matching(NSPredicate(format: "identifier BEGINSWITH %@", "directory-add-"))
                .firstMatch.exists,
            "a member must not receive an organizer-only Add control"
        )

        for phrase in ["Built in", "From your communities", "Share with this community"] {
            XCTAssertEqual(
                app.descendants(matching: .any)
                    .matching(NSPredicate(format: "label CONTAINS[c] %@", phrase))
                    .count,
                0,
                "Tools must not expose “\(phrase)”"
            )
        }

        let toolsScreenshot = XCTAttachment(screenshot: app.screenshot())
        toolsScreenshot.name = "community-scoped-tools-enabled-member"
        toolsScreenshot.lifetime = .keepAlways
        add(toolsScreenshot)
        try app.performAccessibilityAudit(
            for: [.hitRegion, .sufficientElementDescription, .trait]
        ) { issue in
            issue.element?.label != "Riverside Tenants Union"
        }

        // Opening it actually serves the tool's pages.
        let openAfterAudit = app.descendants(matching: .any)[open.identifier]
        for _ in 0..<8 where !openAfterAudit.isHittable {
            app.swipeUp()
        }
        XCTAssertTrue(openAfterAudit.isHittable)
        openAfterAudit.tap()
        XCTAssertTrue(
            app.webViews.firstMatch.waitForExistence(timeout: 10),
            "the organizer-trusted tool opens and serves its page"
        )

        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }
}
