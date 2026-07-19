import XCTest
@testable import RiotKit

/// Composite-site Unit 6 (WU-006 Tasks 1-3) — the iOS read surface for Unit 4's
/// `ResolvedCompositeSite` view model. SwiftUI view bodies are not directly
/// unit-testable, so every visible DECISION is extracted into a pure,
/// deterministic value type that this suite exercises directly; the view itself
/// (`CompositeSiteSurfaceView`) does nothing but render these values.
///
/// Two properties are safety-relevant, not cosmetic:
///  1. **Anti-impersonation (Task 2)** — an open-wire item must never carry
///     editorial styling. `CompositeSiteItemStyle.for(_:)` must produce visibly
///     DISTINCT values per trust tier.
///  2. **Honest degradation (Task 3)** — every non-`.none` `SiteDegradation` has
///     a non-empty title AND a non-empty next-step, and the fail-closed
///     `transportBlocked` case names its unavailability plainly — never a false
///     "connecting…".
final class CompositeSiteSurfaceTests: XCTestCase {

    // MARK: - Task 2: trust-tier styling (anti-impersonation)

    func testEditorialAndOpenWireProduceDistinctStyles() {
        let editorial = CompositeSiteItemStyle.for(.editorial)
        let openWire = CompositeSiteItemStyle.for(.openWire)

        XCTAssertNotEqual(editorial, openWire, "open-wire must not be styled like editorial")
        XCTAssertNotEqual(
            openWire.badgeLabel, editorial.badgeLabel,
            "an open-wire item must not carry the editorial badge label")
        XCTAssertNotEqual(
            openWire.roleToken, editorial.roleToken,
            "an open-wire item must not carry the editorial role token")
        XCTAssertNotEqual(
            openWire.symbolName, editorial.symbolName,
            "an open-wire item must not carry the editorial symbol")
    }

    func testAllThreeTrustTiersProduceDistinctStyles() {
        let styles = [
            CompositeSiteItemStyle.for(.editorial),
            CompositeSiteItemStyle.for(.openWire),
            CompositeSiteItemStyle.for(.comment),
        ]
        XCTAssertEqual(Set(styles).count, 3, "each trust tier must have a visually distinct style")
        XCTAssertEqual(
            Set(styles.map(\.badgeLabel)).count, 3, "each trust tier must have a distinct badge label")
    }

    func testCommentStyleIsNotEditorial() {
        let editorial = CompositeSiteItemStyle.for(.editorial)
        let comment = CompositeSiteItemStyle.for(.comment)
        XCTAssertNotEqual(comment, editorial, "a comment must not be styled like editorial")
    }

    // MARK: - Task 1: accountable placeholders for moderated items

    func testHiddenItemYieldsANonEmptyAccountablePlaceholder() {
        let placeholder = CompositeSiteItemPlaceholder.for(.hidden)
        XCTAssertTrue(placeholder.isPlaceholder)
        XCTAssertFalse(placeholder.text.isEmpty, "a hidden item must render accountable copy, not silence")
    }

    func testTombstonedItemYieldsANonEmptyAccountablePlaceholder() {
        let placeholder = CompositeSiteItemPlaceholder.for(.tombstoned)
        XCTAssertTrue(placeholder.isPlaceholder)
        XCTAssertFalse(
            placeholder.text.isEmpty, "a tombstoned item must render accountable copy, not silence")
    }

    func testOrdinaryItemIsNotAPlaceholder() {
        let placeholder = CompositeSiteItemPlaceholder.for(.ordinary)
        XCTAssertFalse(placeholder.isPlaceholder, "an ordinary item renders its own content, not a placeholder")
    }

    func testHiddenAndTombstonedPlaceholdersAreDistinct() {
        let hidden = CompositeSiteItemPlaceholder.for(.hidden)
        let tombstoned = CompositeSiteItemPlaceholder.for(.tombstoned)
        XCTAssertNotEqual(
            hidden.text, tombstoned.text,
            "hidden and tombstoned are different accountable states and should say so distinctly")
    }

    // MARK: - Task 3: degradation copy + next-step

    func testNoDegradationHasNoCopy() {
        XCTAssertNil(CompositeSiteDegradation.copy(for: .none))
    }

    func testEveryOtherDegradationHasANonEmptyTitleAndNextStep() {
        let degradations: [SiteDegradation] = [
            .memberUnverified, .editorialOnly, .moderationLoading,
            .transportBlocked, .manifestRollbackAlarm, .equivocationAlarm, .manifestInvalid,
        ]
        for degradation in degradations {
            guard let copy = CompositeSiteDegradation.copy(for: degradation) else {
                XCTFail("\(degradation) must have degradation copy")
                continue
            }
            XCTAssertFalse(copy.title.isEmpty, "\(degradation) must have a non-empty title")
            XCTAssertFalse(copy.nextStep.isEmpty, "\(degradation) must have a non-empty next step")
        }
    }

    func testTransportBlockedIsHonestAndFailClosed() {
        guard let copy = CompositeSiteDegradation.copy(for: .transportBlocked) else {
            return XCTFail("transportBlocked must have degradation copy")
        }
        let combined = (copy.title + " " + copy.nextStep).lowercased()
        XCTAssertTrue(
            combined.contains("tor") || combined.contains("unavailable") || combined.contains("not available"),
            "transportBlocked must plainly name why it is unavailable (e.g. Tor / unavailable)")
        XCTAssertFalse(
            combined.contains("connecting"), "transportBlocked must never claim it is still connecting")
    }

    func testDegradationCopiesAreAllDistinct() {
        let degradations: [SiteDegradation] = [
            .memberUnverified, .editorialOnly, .moderationLoading,
            .transportBlocked, .manifestRollbackAlarm, .equivocationAlarm, .manifestInvalid,
        ]
        let titles = degradations.compactMap { CompositeSiteDegradation.copy(for: $0)?.title }
        XCTAssertEqual(Set(titles).count, degradations.count, "each degradation state needs its own copy")
    }

    // MARK: - View smoke test (construction only — bodies aren't unit-testable)

    @MainActor
    func testSurfaceViewConstructsForEveryDegradationWithoutCrashing() {
        for degradation in [
            SiteDegradation.none, .memberUnverified, .editorialOnly, .moderationLoading,
            .transportBlocked, .manifestRollbackAlarm, .equivocationAlarm, .manifestInvalid,
        ] {
            let site = ResolvedCompositeSite(
                root: "ab".repeatedHex(32),
                degradation: degradation,
                transportStatus: "available",
                items: [
                    ResolvedSiteItem(
                        entryId: "11".repeatedHex(32), authorSubspace: "cd".repeatedHex(32),
                        trustTier: .editorial, treatment: .ordinary),
                    ResolvedSiteItem(
                        entryId: "22".repeatedHex(32), authorSubspace: "ef".repeatedHex(32),
                        trustTier: .openWire, treatment: .hidden),
                    ResolvedSiteItem(
                        entryId: "33".repeatedHex(32), authorSubspace: "01".repeatedHex(32),
                        trustTier: .comment, treatment: .tombstoned),
                ],
                writerCapExpired: false)
            _ = CompositeSiteSurfaceView(site: site)
        }
    }
}

private extension String {
    /// Repeats a two-char hex unit `count` times — a readable full 32-byte id.
    func repeatedHex(_ count: Int) -> String { String(repeating: self, count: count) }
}
