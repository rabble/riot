import XCTest

/// Drives ONE phone of a two-phone rehearsal. Run this concurrently against two
/// simulators: ROLE=organizer creates the space and approves the checklist;
/// ROLE=member starts fresh and just searches. Both then tap "Find nearby
/// devices" and we capture what each phone actually shows.
final class TwoPhoneRehearsalUITests: XCTestCase {
    private func shot(_ app: XCUIApplication, _ name: String) {
        let s = XCTAttachment(screenshot: app.screenshot())
        s.name = name
        s.lifetime = .keepAlways
        add(s)
    }

    func testOrganizerPhone() { rehearse(role: "organizer") }
    func testMemberPhone() { rehearse(role: "member") }

    private func rehearse(role: String) {
        let app = XCUIApplication()
        app.launch()
        if app.alerts.firstMatch.waitForExistence(timeout: 2) {
            app.alerts.firstMatch.buttons.firstMatch.tap()
        }

        if role == "organizer" {
            let create = app.buttons["CREATE PUBLIC SPACE"]
            if create.waitForExistence(timeout: 5) { create.tap() }
            // Approve the checklist so the member can inherit it.
            app.buttons["Spaces"].tap()
            let review = app.buttons["review-Checklist"]
            if review.waitForExistence(timeout: 5) {
                review.tap()
                let approve = app.buttons["approve-app"]
                if approve.waitForExistence(timeout: 5) { approve.tap() }
            }
            shot(app, "organizer-after-approve")
        } else {
            shot(app, "member-fresh")
        }

        // Both phones: search for each other.
        app.buttons["Connect"].tap()
        let buttons = app.buttons.allElementsBoundByIndex.prefix(12).map { $0.label }
        let labels = app.staticTexts.allElementsBoundByIndex.prefix(12).map { $0.label }
        print("REHEARSAL[\(role)] connect-screen buttons: \(buttons)")
        print("REHEARSAL[\(role)] connect-screen texts: \(labels)")
        let find = app.buttons["FIND NEARBY DEVICES"]
        guard find.waitForExistence(timeout: 5) else {
            shot(app, "\(role)-no-find-button")
            XCTFail("\(role): no Find nearby devices button — buttons were \(buttons)")
            return
        }
        find.tap()
        shot(app, "\(role)-searching")

        // Give discovery real time, then record what the phone says.
        let deadline = Date().addingTimeInterval(45)
        var lastSeen = ""
        while Date() < deadline {
            let texts = app.staticTexts.allElementsBoundByIndex.prefix(12).map { $0.label }
            lastSeen = texts.joined(separator: " | ")
            if lastSeen.contains("Connect with") || lastSeen.contains("Getting the latest")
                || lastSeen.contains("caught up") || lastSeen.contains("up to date") {
                break
            }
            Thread.sleep(forTimeInterval: 2)
        }
        shot(app, "\(role)-after-search")
        print("REHEARSAL[\(role)] screen: \(lastSeen)")
    }
}
