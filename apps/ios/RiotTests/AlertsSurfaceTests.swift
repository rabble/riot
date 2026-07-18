import XCTest
@testable import RiotKit

/// Unit 3 — the Alerts surface, tested in isolation (no store, no FFI). Contracts:
/// the card shows ONLY the active community's alerts; the organizer flag + the
/// organizer-first order come ONLY from the core coordinate rule
/// (signerID == namespaceID), never from a self-claimed name; the signer shown is
/// the core-verified id, and an empty community shows an actionable empty state.
final class AlertsSurfaceTests: XCTestCase {
    private static let activeNS = String(repeating: "a", count: 64)
    private static let otherNS  = String(repeating: "b", count: 64)

    private static func entry(
        _ headline: String,
        entryID: String = String(repeating: "e", count: 64),
        namespaceID: String = activeNS,
        signerID: String,
        createdAt: UInt64 = 100,
        expiresAt: UInt64 = 1_000,
        aiAssisted: Bool = false
    ) -> RiotEntry {
        RiotEntry(entryID: entryID, namespaceID: namespaceID, signerID: signerID,
                  headline: headline, createdAt: createdAt, validFrom: nil,
                  expiresAt: expiresAt, aiAssisted: aiAssisted)
    }

    // MARK: - Per-community scoping (defense in depth)

    func testForeignCommunityAlertsAreFilteredOut() {
        // model.entries is already active-scoped at the FFI, but the Swift filter is
        // the belt-and-suspenders guarantee: a planted foreign-namespace row never shows.
        let mine    = Self.entry("Road closed", signerID: Self.activeNS)          // organizer of active
        let foreign = Self.entry("Not mine", namespaceID: Self.otherNS, signerID: Self.otherNS)
        guard case let .populated(rows) = AlertsListState.from([mine, foreign], activeNamespaceID: Self.activeNS) else {
            return XCTFail("expected populated")
        }
        XCTAssertEqual(rows.map(\.headline), ["Road closed"])
        XCTAssertFalse(rows.contains { $0.namespaceID == Self.otherNS })
    }

    // MARK: - Organizer flag + order from the coordinate, never a name

    func testOrganizerFlagComesFromCoordinateNotName() {
        // signerID == namespaceID ⟺ organizer. A member who NAMES themselves the
        // organizer's hex in the headline is still a member.
        let organizer = Self.entry("A", signerID: Self.activeNS)                       // subspace == namespace
        let member    = Self.entry("B", signerID: String(repeating: "c", count: 64))   // subspace != namespace
        let orgRow = AlertRow(organizer, activeNamespaceID: Self.activeNS)
        let memRow = AlertRow(member, activeNamespaceID: Self.activeNS)
        XCTAssertTrue(orgRow.isOrganizer)
        XCTAssertFalse(memRow.isOrganizer)
    }

    func testOrganizerFirstThenNewestOrdering() {
        let orgOld  = Self.entry("org-old",  entryID: String(repeating: "1", count: 64), signerID: Self.activeNS, createdAt: 10)
        let memNew  = Self.entry("mem-new",  entryID: String(repeating: "2", count: 64), signerID: String(repeating: "c", count: 64), createdAt: 50)
        let orgNew  = Self.entry("org-new",  entryID: String(repeating: "3", count: 64), signerID: Self.activeNS, createdAt: 40)
        guard case let .populated(rows) = AlertsListState.from([memNew, orgOld, orgNew], activeNamespaceID: Self.activeNS) else {
            return XCTFail("expected populated")
        }
        // Organizers first (newest organizer before older organizer), then members newest-first.
        XCTAssertEqual(rows.map(\.headline), ["org-new", "org-old", "mem-new"])
        XCTAssertEqual(rows.map(\.isOrganizer), [true, true, false])
    }

    // MARK: - Core-verified signer identity, never the raw hex as the display

    func testRowCarriesVerifiedSignerTagAndPlainHeadline() {
        let e = Self.entry("**not** a link", signerID: Self.activeNS)
        let row = AlertRow(e, activeNamespaceID: Self.activeNS)
        // Short signer tag is the core-verified id (first 8 hex), not the full id.
        XCTAssertEqual(row.signerTag, String(Self.activeNS.prefix(8)))
        XCTAssertEqual(row.signerID, Self.activeNS)      // full id retained for detail/pinning only
        // Headline is carried verbatim (rendered as plain Text by the view — no markdown auto-link).
        XCTAssertEqual(row.headline, "**not** a link")
    }

    // MARK: - Actionable / benign empty state

    func testNoAlertsIsABenignEmptyStateNotABlankList() {
        guard case let .empty(empty) = AlertsListState.from([], activeNamespaceID: Self.activeNS) else {
            return XCTFail("an empty community must show the empty state, not .populated([])")
        }
        XCTAssertEqual(empty.title, AlertsStrings.emptyTitle)
        XCTAssertFalse(empty.message.isEmpty)
    }

    // MARK: - Freshness is a human phrase, never a raw epoch

    func testFreshnessDescribesExpiryWithoutRawTimestamps() {
        let now = Date(timeIntervalSince1970: 500)
        let live = Self.entry("live", signerID: Self.activeNS, createdAt: 100, expiresAt: 1_000)
        let dead = Self.entry("dead", signerID: Self.activeNS, createdAt: 100, expiresAt: 200)
        XCTAssertFalse(AlertRelativeTime.freshness(live, now: now).contains("1000"))
        XCTAssertEqual(AlertRelativeTime.freshness(dead, now: now), AlertsStrings.expired)
    }

    func testExpiredAndForeignAlertsAreOmittedFromActivePresentation() {
        let now = Date(timeIntervalSince1970: 100)
        let expired = Self.entry(
            "expired", namespaceID: Self.activeNS, signerID: Self.activeNS, expiresAt: 100
        )
        let foreign = Self.entry(
            "foreign", namespaceID: Self.otherNS, signerID: Self.otherNS, expiresAt: 200
        )

        XCTAssertEqual(
            ActiveAlertsPresentation.from(
                [expired, foreign],
                activeNamespaceID: Self.activeNS,
                now: now
            ),
            .hidden
        )
    }

    func testThreeActiveAlertsCapAtTwoWithCountedOverflow() {
        let entries = (1...3).map {
            Self.entry(
                "alert-\($0)",
                entryID: String(repeating: "\($0)", count: 64),
                signerID: Self.activeNS,
                createdAt: UInt64($0),
                expiresAt: 200
            )
        }
        let state = ActiveAlertsPresentation.from(
            entries,
            activeNamespaceID: Self.activeNS,
            now: Date(timeIntervalSince1970: 100)
        )

        guard case let .visible(rows, allRows) = state else {
            return XCTFail("expected visible alerts")
        }
        XCTAssertEqual(rows.count, 2)
        XCTAssertEqual(allRows.count, 3)
        XCTAssertEqual(state.overflowLabel, "View all 3 active alerts")
    }

    func testTwoActiveAlertsHaveNoOverflowAndUseOrganizerNewestOrder() {
        let member = Self.entry(
            "new member",
            entryID: String(repeating: "1", count: 64),
            signerID: String(repeating: "c", count: 64),
            createdAt: 200,
            expiresAt: 300
        )
        let organizer = Self.entry(
            "older organizer",
            entryID: String(repeating: "2", count: 64),
            signerID: Self.activeNS,
            createdAt: 100,
            expiresAt: 300
        )
        let state = ActiveAlertsPresentation.from(
            [member, organizer],
            activeNamespaceID: Self.activeNS,
            now: Date(timeIntervalSince1970: 50)
        )

        guard case let .visible(rows, allRows) = state else {
            return XCTFail("expected visible alerts")
        }
        XCTAssertEqual(rows.map(\.headline), ["older organizer", "new member"])
        XCTAssertEqual(allRows, rows)
        XCTAssertNil(state.overflowLabel)
    }

    func testHomePresentationPinsCompactSectionOrderAndActivePredicate() {
        let active = Self.entry(
            "active", signerID: Self.activeNS, expiresAt: 101
        )
        let hidden = HomePresentation.sections(
            wireHasPosts: true,
            alerts: .hidden,
            hasTools: true
        )
        XCTAssertEqual(hidden, [.post, .newswire, .tools])

        let visible = ActiveAlertsPresentation.from(
            [active],
            activeNamespaceID: Self.activeNS,
            now: Date(timeIntervalSince1970: 100)
        )
        XCTAssertEqual(
            HomePresentation.sections(
                wireHasPosts: true,
                alerts: visible,
                hasTools: true
            ),
            [.activeAlerts, .post, .newswire, .tools]
        )
    }

    func testLastActiveAlertDisappearsAtItsExactExpiry() {
        let entry = Self.entry(
            "brief", signerID: Self.activeNS, expiresAt: 101
        )
        let before = ActiveAlertsPresentation.from(
            [entry],
            activeNamespaceID: Self.activeNS,
            now: Date(timeIntervalSince1970: 100)
        )
        XCTAssertEqual(before.nextExpiryDate, Date(timeIntervalSince1970: 101))

        let atExpiry = ActiveAlertsPresentation.from(
            [entry],
            activeNamespaceID: Self.activeNS,
            now: Date(timeIntervalSince1970: 101)
        )
        XCTAssertEqual(atExpiry, .hidden)
    }

    func testIdleExpiryRefreshUsesInjectedClock() async throws {
        let expiry = Date(timeIntervalSince1970: 101)
        let refreshed = Date(timeIntervalSince1970: 101.25)
        let entry = Self.entry(
            "brief", signerID: Self.activeNS, expiresAt: 101
        )
        let recorder = ExpiryRecorder()
        let clock = ActiveAlertsClock(
            now: { refreshed },
            sleepUntil: { date in await recorder.record(date) }
        )
        XCTAssertNotEqual(
            ActiveAlertsPresentation.from(
                [entry],
                activeNamespaceID: Self.activeNS,
                now: Date(timeIntervalSince1970: 100)
            ),
            .hidden
        )

        let result = try await ActiveAlertsExpiryRefresh.wait(
            until: expiry,
            clock: clock
        )

        XCTAssertEqual(result, refreshed)
        XCTAssertEqual(
            ActiveAlertsPresentation.from(
                [entry],
                activeNamespaceID: Self.activeNS,
                now: result
            ),
            .hidden
        )
        let recorded = await recorder.recorded()
        XCTAssertEqual(recorded, [expiry])
    }

    func testExpiryRefreshPropagatesCancellationFromKeyedShellTask() async {
        let task = Task {
            try await ActiveAlertsExpiryRefresh.wait(
                until: Date(timeIntervalSince1970: 500),
                clock: ActiveAlertsClock(
                    now: { Date(timeIntervalSince1970: 500) },
                    sleepUntil: { _ in
                        try await Task.sleep(for: .seconds(30))
                    }
                )
            )
        }
        task.cancel()

        do {
            _ = try await task.value
            XCTFail("a removed keyed shell must not refresh its replacement")
        } catch is CancellationError {
            // Expected: SwiftUI cancellation reaches the injected wait.
        } catch {
            XCTFail("expected CancellationError, got \(error)")
        }
    }

    func testAlertActionsExposeStableAccessibilityIdentifiers() {
        XCTAssertEqual(AlertsAccessibility.viewAll, "active-alerts-view-all")
        XCTAssertEqual(AlertsAccessibility.done, "active-alerts-done")
    }
}

private actor ExpiryRecorder {
    private var dates: [Date] = []
    func record(_ date: Date) { dates.append(date) }
    func recorded() -> [Date] { dates }
}

// MARK: - Task 2: AlertDetailSheet renders the AlertDetail value model

extension AlertsSurfaceTests {
    func testAlertDetailModelDrivesTheSheetContent() {
        let e = Self.entry("Bridge out on 5th", signerID: Self.activeNS, aiAssisted: true)
        let detail = AlertDetail(entry: e)
        XCTAssertEqual(detail.headline, "Bridge out on 5th")
        XCTAssertTrue(detail.aiAssisted)
        // Summary is the act-on-it window; the 64-hex ids live only under technical.
        XCTAssertTrue(detail.summary.contains { $0.label == "Expires" })
        XCTAssertTrue(detail.technical.contains { $0.label == "Signer" && $0.value == Self.activeNS })
        XCTAssertFalse(detail.summary.contains { $0.value == Self.activeNS }, "full ids never lead the sheet")
    }

    func testSheetTechnicalDisclosureStartsClosedByContract() {
        // The sheet binds its DisclosureGroup to this default; a full id must never
        // be visible until a person opts in (navigation accessibility contract).
        XCTAssertFalse(AlertDetailSheet.technicalStartsExpanded)
        XCTAssertEqual(AlertDetail.technicalDisclosureTitle, "Technical details")
    }

    func testHeadlineIsCarriedVerbatimForPlainTextRendering() {
        // Anti-injection: a markdown-looking headline is preserved literally; the
        // view renders it as plain Text (verbatim:), never AttributedString auto-link.
        let e = Self.entry("[tap here](http://evil.example)", signerID: Self.activeNS)
        XCTAssertEqual(AlertDetail(entry: e).headline, "[tap here](http://evil.example)")
        XCTAssertEqual(AlertRow(e, activeNamespaceID: Self.activeNS).headline, "[tap here](http://evil.example)")
    }
}

// MARK: - Task 3: Home card wiring (single entry point)

extension AlertsSurfaceTests {
    func testAlertsListStateIsBuiltFromActiveModelEntries() {
        // The Home card feeds AlertsListState.from(model.entries, model.space?.namespaceID)
        // — the exact call the view makes — so a green state here is the card's content.
        let e = Self.entry("Water main break", signerID: Self.activeNS)
        guard case let .populated(rows) = AlertsListState.from([e], activeNamespaceID: Self.activeNS) else {
            return XCTFail("expected populated")
        }
        XCTAssertEqual(rows.first?.headline, "Water main break")
        XCTAssertEqual(rows.first.map { AlertDetail(entry: $0.entry).headline }, "Water main break")
    }

    func testNoActiveSpaceYieldsEmptyStateNotACrash() {
        // Home renders the card with activeNamespaceID = "" before a community is joined.
        guard case .empty = AlertsListState.from([], activeNamespaceID: "") else {
            return XCTFail("no active space must be the benign empty state")
        }
    }
}
