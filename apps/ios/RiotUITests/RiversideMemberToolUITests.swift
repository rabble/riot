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
final class RiversideMemberToolUITests: XCTestCase {
    func testDemoMemberOpensAnOrganizerTrustedToolWithoutAReviewGate() {
        let app = XCUIApplication()
        app.launch()
        if app.alerts.firstMatch.waitForExistence(timeout: 2) {
            app.alerts.firstMatch.buttons.firstMatch.tap()
        }

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

        // As a member of an organizer-shaped space, the Checklist must be OPENABLE
        // straight away…
        let open = app.buttons["directory-open-Checklist"]
        XCTAssertTrue(
            open.waitForExistence(timeout: 10),
            "an organizer-trusted tool must be openable by a demo member"
        )

        // …and there must be NO Review gate. A member cannot approve, so a
        // Review affordance here would be the exact dead end 0B removes.
        XCTAssertFalse(
            app.buttons["directory-review-Checklist"].exists,
            "a demo member must never be sent to a Review gate they cannot pass"
        )

        // Opening it actually serves the tool's pages.
        open.tap()
        XCTAssertTrue(
            app.webViews.firstMatch.waitForExistence(timeout: 10),
            "the organizer-trusted tool opens and serves its page"
        )

        let screenshot = XCTAttachment(screenshot: app.screenshot())
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }
}
