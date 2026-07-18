import XCTest
@testable import RiotKit

/// A controllable `SystemNotificationScheduling` double so the notifier's effect
/// layer is exercised without touching the real notification center: the test
/// sets the authorization it should report and records every scheduled request.
@MainActor
private final class SpyScheduler: SystemNotificationScheduling {
    var authorization: NotifierAuthorization
    var requestCount = 0
    private(set) var scheduled: [(identifier: String, title: String, body: String, communityID: String)] = []

    init(authorization: NotifierAuthorization) { self.authorization = authorization }

    func currentAuthorization() async -> NotifierAuthorization { authorization }

    func requestAuthorization() async -> NotifierAuthorization {
        requestCount += 1
        // A grant flips the reported status, mirroring the real center.
        if authorization == .notDetermined { authorization = .authorized }
        return authorization
    }

    func schedule(identifier: String, title: String, body: String, communityID: String) {
        scheduled.append((identifier, title, body, communityID))
    }
}

private func ref(_ id: String, _ tai: UInt64) -> SeenPostRef {
    SeenPostRef(entryID: id, taiJ2000Micros: tai)
}

private func unread(_ posts: [SeenPostRef], cursor: UInt64?) -> NewswireUnread {
    NewswireUnread(posts: posts, cursor: cursor)
}

@MainActor
final class LocalNotifierTests: XCTestCase {
    // MARK: - Pure decision logic

    func testBackgroundedNewContentAuthorizedSystemNotifies() {
        let state = unread([ref("a", 30), ref("b", 20)], cursor: 10)
        let decision = NotificationDecision.decide(
            unread: state, lastNotifiedUpTo: nil, phase: .background, authorization: .authorized)
        XCTAssertEqual(decision, .systemNotify(count: 2, upTo: 30))
    }

    func testForegroundedNewContentBannersNotSystemNotify() {
        let state = unread([ref("a", 30)], cursor: nil)
        let decision = NotificationDecision.decide(
            unread: state, lastNotifiedUpTo: nil, phase: .active, authorization: .authorized)
        XCTAssertEqual(decision, .inAppBanner(count: 1, upTo: 30))
    }

    func testDeniedAuthorizationWhileBackgroundedDoesNothing() {
        let state = unread([ref("a", 30)], cursor: nil)
        let decision = NotificationDecision.decide(
            unread: state, lastNotifiedUpTo: nil, phase: .background, authorization: .denied)
        XCTAssertEqual(decision, .nothing)
    }

    func testNotDeterminedAuthorizationWhileBackgroundedDoesNothing() {
        let state = unread([ref("a", 30)], cursor: nil)
        let decision = NotificationDecision.decide(
            unread: state, lastNotifiedUpTo: nil, phase: .background, authorization: .notDetermined)
        XCTAssertEqual(decision, .nothing)
    }

    func testNoUnreadDoesNothing() {
        let state = unread([ref("a", 30)], cursor: 30)
        let decision = NotificationDecision.decide(
            unread: state, lastNotifiedUpTo: nil, phase: .background, authorization: .authorized)
        XCTAssertEqual(decision, .nothing)
    }

    func testSameBatchAlreadyNotifiedDoesNothing() {
        // Latest shown is 30; we already announced up to 30 → a projection refresh
        // that re-shows the same posts must stay silent (de-spam).
        let state = unread([ref("a", 30), ref("b", 20)], cursor: 10)
        let decision = NotificationDecision.decide(
            unread: state, lastNotifiedUpTo: 30, phase: .background, authorization: .authorized)
        XCTAssertEqual(decision, .nothing)
    }

    func testStrictlyNewerBatchThanLastNotifiedNotifies() {
        let state = unread([ref("c", 40), ref("a", 30)], cursor: 10)
        let decision = NotificationDecision.decide(
            unread: state, lastNotifiedUpTo: 30, phase: .background, authorization: .authorized)
        XCTAssertEqual(decision, .systemNotify(count: 2, upTo: 40))
    }

    // MARK: - Effect layer (de-spam + phase routing through the scheduler)

    func testEvaluateBackgroundedSchedulesOnceThenDeSpams() async {
        let scheduler = SpyScheduler(authorization: .authorized)
        let notifier = LocalNotifier(scheduler: scheduler)
        let state = unread([ref("a", 30), ref("b", 20)], cursor: 10)

        await notifier.evaluate(communityID: "ns1", communityName: "Springfield", unread: state, phase: .background)
        XCTAssertEqual(scheduler.scheduled.count, 1)
        XCTAssertEqual(scheduler.scheduled.first?.title, "Springfield")
        XCTAssertEqual(scheduler.scheduled.first?.communityID, "ns1")

        // The very same projection arriving again must not re-notify.
        await notifier.evaluate(communityID: "ns1", communityName: "Springfield", unread: state, phase: .background)
        XCTAssertEqual(scheduler.scheduled.count, 1)
    }

    func testEvaluateNewerBatchNotifiesAgain() async {
        let scheduler = SpyScheduler(authorization: .authorized)
        let notifier = LocalNotifier(scheduler: scheduler)

        await notifier.evaluate(
            communityID: "ns1", communityName: "Springfield",
            unread: unread([ref("a", 30)], cursor: 10), phase: .background)
        await notifier.evaluate(
            communityID: "ns1", communityName: "Springfield",
            unread: unread([ref("b", 55), ref("a", 30)], cursor: 10), phase: .background)
        XCTAssertEqual(scheduler.scheduled.count, 2)
    }

    func testEvaluateForegroundedBannersNotSchedules() async {
        let scheduler = SpyScheduler(authorization: .authorized)
        let notifier = LocalNotifier(scheduler: scheduler)

        await notifier.evaluate(
            communityID: "ns1", communityName: "Springfield",
            unread: unread([ref("a", 30)], cursor: 10), phase: .active)

        XCTAssertTrue(scheduler.scheduled.isEmpty)
        XCTAssertEqual(notifier.banner?.communityName, "Springfield")
        XCTAssertEqual(notifier.banner?.count, 1)
        XCTAssertEqual(notifier.banner?.text, "1 new in Springfield")
    }

    func testEvaluateEmptyCommunityIDIsInert() async {
        let scheduler = SpyScheduler(authorization: .authorized)
        let notifier = LocalNotifier(scheduler: scheduler)

        await notifier.evaluate(
            communityID: "", communityName: "Springfield",
            unread: unread([ref("a", 30)], cursor: 10), phase: .background)

        XCTAssertTrue(scheduler.scheduled.isEmpty)
        XCTAssertNil(notifier.banner)
    }

    func testEvaluateDeniedWhileBackgroundedSchedulesNothing() async {
        let scheduler = SpyScheduler(authorization: .denied)
        let notifier = LocalNotifier(scheduler: scheduler)

        await notifier.evaluate(
            communityID: "ns1", communityName: "Springfield",
            unread: unread([ref("a", 30)], cursor: 10), phase: .background)

        XCTAssertTrue(scheduler.scheduled.isEmpty)
    }

    func testRequestAuthorizationOnlyWhenNotDetermined() async {
        let undetermined = SpyScheduler(authorization: .notDetermined)
        let notifier = LocalNotifier(scheduler: undetermined)
        await notifier.requestAuthorizationIfNeeded()
        await notifier.requestAuthorizationIfNeeded()
        XCTAssertEqual(undetermined.requestCount, 1)

        // Already decided → never prompt again.
        let denied = SpyScheduler(authorization: .denied)
        let notifier2 = LocalNotifier(scheduler: denied)
        await notifier2.requestAuthorizationIfNeeded()
        XCTAssertEqual(denied.requestCount, 0)
    }

    func testBannerDismissClears() async {
        let scheduler = SpyScheduler(authorization: .authorized)
        let notifier = LocalNotifier(scheduler: scheduler)
        await notifier.evaluate(
            communityID: "ns1", communityName: "Springfield",
            unread: unread([ref("a", 30)], cursor: 10), phase: .active)
        XCTAssertNotNil(notifier.banner)
        notifier.dismissBanner()
        XCTAssertNil(notifier.banner)
    }
}
