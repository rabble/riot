import XCTest
@testable import RiotKit

final class CompactReactionBarTests: XCTestCase {
    func testClosedReactionPresentationTableUsesApprovedGlyphsAndNames() {
        XCTAssertEqual(ReactionKind.allCases.map(\.glyph), ["♥", "✊︎", "!", "◌"])
        XCTAssertEqual(
            ReactionKind.allCases.map(\.label),
            ["Support", "Solidarity", "Important", "Grief"])
    }

    func testCountFormatterAlwaysShowsZeroAndCapsLargeCounts() {
        XCTAssertEqual(ReactionCountFormatter.string(0), "0")
        XCTAssertEqual(ReactionCountFormatter.string(42), "42")
        XCTAssertEqual(ReactionCountFormatter.string(999), "999")
        XCTAssertEqual(ReactionCountFormatter.string(1_000), "999+")
        XCTAssertEqual(ReactionCountFormatter.string(Int.max), "999+")
    }

    func testCompactMetricsKeepAFullHitTargetAndFixedVisualWidth() {
        XCTAssertEqual(CompactReactionMetrics.minimumTarget, 44)
        XCTAssertEqual(CompactReactionMetrics.normalVisualWidth, 72)
        XCTAssertEqual(CompactReactionMetrics.normalVisualHeight, 32)
        XCTAssertEqual(CompactReactionMetrics.spacing, 8)
        XCTAssertEqual(CompactReactionMetrics.gridRowSpacing, 8)
        XCTAssertEqual(CompactReactionMetrics.singleRowBreakpoint, 312)
    }

    func testTypographyAndLegendCopyMatchTheClosedDesign() {
        XCTAssertEqual(CompactReactionMetrics.glyphPointSize, 15)
        XCTAssertEqual(CompactReactionMetrics.countPointSize, 13)
        XCTAssertEqual(CompactReactionMetrics.contentSpacing, 6)
        XCTAssertEqual(
            ReactionLegendCopy.text,
            "Reactions: ♥ Support · ✊︎ Solidarity · ! Important · ◌ Grief.")
    }

    func testReactionSemanticColorsMatchTokensAndMeetTextContrast() {
        XCTAssertEqual(RiotTheme.onReactionAccentHex(for: .light), 0xF6F2E9)
        XCTAssertEqual(RiotTheme.onReactionAccentHex(for: .dark), 0x131209)
        XCTAssertEqual(RiotTheme.dangerHex(for: .light), 0xB3261E)
        XCTAssertEqual(RiotTheme.dangerHex(for: .dark), 0xFFB4AB)

        XCTAssertGreaterThanOrEqual(
            contrast(0xF6F2E9, 0xD1216E),
            4.5)
        XCTAssertGreaterThanOrEqual(
            contrast(0x131209, 0xFF5F9E),
            4.5)
        XCTAssertGreaterThanOrEqual(
            contrast(0xB3261E, 0xFCFAF4),
            4.5)
        XCTAssertGreaterThanOrEqual(
            contrast(0xFFB4AB, 0x201E16),
            4.5)
        XCTAssertGreaterThanOrEqual(contrast(0x4A473B, 0xFCFAF4), 3)
        XCTAssertGreaterThanOrEqual(contrast(0xBEB69E, 0x201E16), 3)
    }

    func testStateTreatmentKeepsSelectionDuringPendingAndSeparatesAuthorityDisabled() {
        let selectedPending = ReactionControlVisualState(
            selected: true,
            pending: true,
            failed: false,
            authorityDisabled: false,
            hovered: false,
            pressed: false).presentation
        XCTAssertEqual(selectedPending.fill, .pink)
        XCTAssertEqual(selectedPending.foreground, .onReactionAccent)
        XCTAssertTrue(selectedPending.showsSpinner)
        XCTAssertTrue(selectedPending.showsCheck)
        XCTAssertEqual(selectedPending.opacity, 1)

        let authorityDisabled = ReactionControlVisualState(
            selected: true,
            pending: false,
            failed: false,
            authorityDisabled: true,
            hovered: true,
            pressed: true).presentation
        XCTAssertEqual(authorityDisabled.opacity, 0.5)
        XCTAssertEqual(authorityDisabled.scale, 1)
        XCTAssertEqual(authorityDisabled.fill, .pink)
    }

    func testHoverAndFailureUseExactSemanticRoles() {
        let hover = ReactionControlVisualState(
            selected: false,
            pending: false,
            failed: false,
            authorityDisabled: false,
            hovered: true,
            pressed: false).presentation
        XCTAssertEqual(hover.fill, .paper2)
        XCTAssertEqual(hover.outline, .ink)
        XCTAssertEqual(hover.outlineWidth, 2)

        let failedSelected = ReactionControlVisualState(
            selected: true,
            pending: false,
            failed: true,
            authorityDisabled: false,
            hovered: false,
            pressed: false).presentation
        XCTAssertEqual(failedSelected.fill, .pink)
        XCTAssertEqual(failedSelected.outline, .danger)
        XCTAssertEqual(failedSelected.outlineWidth, 2)
        XCTAssertTrue(failedSelected.showsCheck)
    }

    func testInteractionDisabledFailureIsNotStyledAsAuthorityUnavailable() {
        let capacityFailure = ReactionControlPresentation(
            kind: .support,
            count: 2,
            selected: false,
            pending: false,
            failed: true,
            disabled: true,
            authorityDisabled: false)
        let failedVisual = capacityFailure.visualState(
            hovered: false,
            pressed: false).presentation

        XCTAssertTrue(capacityFailure.disabled)
        XCTAssertEqual(failedVisual.outline, .danger)
        XCTAssertEqual(failedVisual.outlineWidth, 2)
        XCTAssertEqual(failedVisual.opacity, 1)

        let authorityLostWhilePending = ReactionControlPresentation(
            kind: .grief,
            count: 1,
            selected: true,
            pending: true,
            failed: false,
            disabled: true,
            authorityDisabled: true)
        let authorityVisual = authorityLostWhilePending.visualState(
            hovered: true,
            pressed: true).presentation

        XCTAssertEqual(authorityVisual.opacity, 0.5)
        XCTAssertFalse(authorityVisual.showsSpinner)
        XCTAssertEqual(authorityVisual.scale, 1)
        XCTAssertTrue(authorityLostWhilePending.contentPresentation.showsGlyph)
        XCTAssertFalse(authorityLostWhilePending.contentPresentation.showsSpinner)
        XCTAssertTrue(authorityLostWhilePending.contentPresentation.showsCheck)
    }

    func testLayoutUsesTwoByTwoAt288AndOneByFourAt500() {
        XCTAssertEqual(CompactReactionMetrics.columns(for: 288), 2)
        XCTAssertEqual(CompactReactionMetrics.columns(for: 311), 2)
        XCTAssertEqual(CompactReactionMetrics.columns(for: 312), 4)
        XCTAssertEqual(CompactReactionMetrics.columns(for: 500), 4)
    }

    func testAccessibility3RetainsFixedSlotsAndGrowsTheTarget() {
        let normal = CompactReactionMetrics.presentation(for: .normal)
        let accessibility = CompactReactionMetrics.presentation(for: .accessibility3)

        XCTAssertEqual(normal.visualWidth, accessibility.visualWidth)
        XCTAssertEqual(normal.glyphSlotWidth, accessibility.glyphSlotWidth)
        XCTAssertEqual(normal.statusSlotWidth, accessibility.statusSlotWidth)
        XCTAssertGreaterThan(accessibility.visualHeight, normal.visualHeight)
        XCTAssertGreaterThanOrEqual(accessibility.targetHeight, accessibility.visualHeight)
    }

    func testControlPresentationNamesCountSelectionAndStatusWithoutIdentifiers() {
        let presentation = ReactionControlPresentation(
            kind: .solidarity,
            count: 3,
            selected: true,
            pending: false,
            failed: false)

        XCTAssertEqual(presentation.accessibilityLabel, "Solidarity")
        XCTAssertEqual(presentation.accessibilityValue, "3 reactions, selected")
        XCTAssertEqual(presentation.accessibilityHint, "Removes your reaction")
        XCTAssertEqual(presentation.help, "Solidarity")
        XCTAssertFalse(presentation.stableIdentifier.contains("post"))
        XCTAssertEqual(presentation.stableIdentifier, "reaction-solidarity")
    }

    func testPendingAndFailureKeepTheSameReservedWidthAndExposeState() {
        let idle = ReactionControlPresentation(
            kind: .support, count: 0, selected: false, pending: false, failed: false)
        let pending = ReactionControlPresentation(
            kind: .support, count: 0, selected: false, pending: true, failed: false)
        let failed = ReactionControlPresentation(
            kind: .support, count: 0, selected: false, pending: false, failed: true)

        XCTAssertEqual(idle.reservedWidth, pending.reservedWidth)
        XCTAssertEqual(pending.reservedWidth, failed.reservedWidth)
        XCTAssertEqual(idle.accessibilityValue, "No reactions, not selected")
        XCTAssertEqual(pending.accessibilityValue, "No reactions, saving")
        XCTAssertEqual(failed.accessibilityValue, "No reactions, save failed")
    }

    func testLegendDismissalPersistsAcrossStoreRecreation() {
        let suiteName = "CompactReactionBarTests.\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            return XCTFail("Unable to create isolated defaults")
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }

        XCTAssertTrue(ReactionLegendStore(defaults: defaults).shouldShow)
        ReactionLegendStore(defaults: defaults).dismiss()
        XCTAssertFalse(ReactionLegendStore(defaults: defaults).shouldShow)
    }

    func testFailureOrderChoosesLatestFailureForSurfaceAndPost() {
        var order = ReactionFailureOrder()
        let support = ReactionKey(postID: "private-post-id", kind: .support)
        let grief = ReactionKey(postID: "private-post-id", kind: .grief)
        order.record(support)
        order.record(grief)

        XCTAssertEqual(order.latestKey(forPostID: "private-post-id"), grief)
        order.remove(grief)
        XCTAssertEqual(order.latestKey(forPostID: "private-post-id"), support)
    }

    func testAnnouncementCursorConsumesEachSurfaceSequenceOnlyOnce() {
        var cursor = ReactionAnnouncementCursor()
        XCTAssertTrue(cursor.shouldAnnounce(sequence: 4))
        XCTAssertFalse(cursor.shouldAnnounce(sequence: 4))
        XCTAssertFalse(cursor.shouldAnnounce(sequence: 3))
        XCTAssertTrue(cursor.shouldAnnounce(sequence: 5))
    }

    func testModelScopedAnnouncementConsumptionSurvivesHostRecreation() {
        let announcements = [
            ReactionAnnouncement(sequence: 4, surface: .openWire, message: "Saved"),
        ]
        var consumption = ReactionAnnouncementConsumption()
        XCTAssertEqual(
            consumption.consumeLatest(in: announcements, surface: .openWire)?.message,
            "Saved")
        XCTAssertNil(consumption.consumeLatest(in: announcements, surface: .openWire))
        XCTAssertNil(consumption.consumeLatest(in: announcements, surface: .frontPage))
    }

    func testFailureCopyPriorityIsCommittedThenRetryableThenRecency() {
        let support = ReactionKey(postID: "post", kind: .support)
        let grief = ReactionKey(postID: "post", kind: .grief)
        let important = ReactionKey(postID: "post", kind: .important)
        var order = ReactionFailureOrder()
        order.record(support)
        order.record(grief)
        order.record(important)
        let failures = [
            support: failure(.authorityOrInput, feedback: .rejected, message: "authority"),
            grief: failure(.retryablePersistence, feedback: .rejected, message: "retryable"),
            important: failure(
                .retryablePersistence,
                feedback: .committedNeedsRefresh,
                message: "committed"),
        ]

        XCTAssertEqual(
            ReactionFailureSelector.latestVisible(
                failures: failures,
                order: order,
                postID: "post",
                surface: .openWire)?.message,
            "committed")

        var withoutCommitted = failures
        withoutCommitted.removeValue(forKey: important)
        XCTAssertEqual(
            ReactionFailureSelector.latestVisible(
                failures: withoutCommitted,
                order: order,
                postID: "post",
                surface: .openWire)?.message,
            "retryable")

        withoutCommitted.removeValue(forKey: grief)
        XCTAssertEqual(
            ReactionFailureSelector.latestVisible(
                failures: withoutCommitted,
                order: order,
                postID: "post",
                surface: .openWire)?.message,
            "authority")
    }

    private func contrast(_ first: UInt32, _ second: UInt32) -> Double {
        let lighter = max(luminance(first), luminance(second))
        let darker = min(luminance(first), luminance(second))
        return (lighter + 0.05) / (darker + 0.05)
    }

    private func failure(
        _ kind: ReactionFailureKind,
        feedback: ReactionFeedbackKind,
        message: String
    ) -> ReactionFailurePresentation {
        ReactionFailurePresentation(
            failure: ReactionFailure(
                kind: kind,
                publicCode: "test",
                message: message),
            surface: .openWire,
            kind: feedback)
    }

    private func luminance(_ hex: UInt32) -> Double {
        let components = [
            Double((hex >> 16) & 0xFF) / 255,
            Double((hex >> 8) & 0xFF) / 255,
            Double(hex & 0xFF) / 255,
        ]
        return components
            .map { $0 <= 0.03928 ? $0 / 12.92 : pow(($0 + 0.055) / 1.055, 2.4) }
            .enumerated()
            .reduce(0) { partial, item in
                partial + item.element * [0.2126, 0.7152, 0.0722][item.offset]
            }
    }
}
