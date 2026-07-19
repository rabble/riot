import SwiftUI

// MARK: - Composite-site read surface (Unit 6, WU-006 Tasks 1-3)
//
// Renders Unit 4's `ResolvedCompositeSite` view model with NO business logic —
// every visible decision below traces to a core-resolved field
// (`SiteTrustTier`, `SiteItemTreatment`, `SiteDegradation`). SwiftUI view
// bodies are not directly unit-testable, so the decisions live in pure,
// deterministic value types (mirroring `ConferenceShellView`'s
// `ShellRecoveryState`/`CatalogFailureView` convention and
// `CommunityChooser`'s `CommunityRelativeTime` pattern); `CompositeSiteSurfaceView`
// itself does nothing but render them.

// MARK: - Task 2: trust-tier styling (anti-impersonation)

/// The visual identity of a resolved item's trust tier. This is a
/// SECURITY-relevant UI type: an open-wire item must never carry editorial
/// styling, so `for(.editorial)` and `for(.openWire)` are required to produce
/// distinct badge labels, role tokens, and symbols — see
/// `CompositeSiteSurfaceTests.testEditorialAndOpenWireProduceDistinctStyles`.
public struct CompositeSiteItemStyle: Equatable, Hashable, Sendable {
    /// The short label shown on the item's trust badge.
    public let badgeLabel: String
    /// A stable, machine-checkable token identifying the tier (also used as an
    /// accessibility identifier suffix) — distinct per tier by construction.
    public let roleToken: String
    /// The SF Symbol drawn next to the badge.
    public let symbolName: String

    public static func `for`(_ tier: SiteTrustTier) -> CompositeSiteItemStyle {
        switch tier {
        case .editorial:
            CompositeSiteItemStyle(
                badgeLabel: "Editorial", roleToken: "editorial",
                symbolName: "checkmark.seal.fill")
        case .openWire:
            CompositeSiteItemStyle(
                badgeLabel: "Open wire", roleToken: "open-wire",
                symbolName: "antenna.radiowaves.left.and.right")
        case .comment:
            CompositeSiteItemStyle(
                badgeLabel: "Comment", roleToken: "comment",
                symbolName: "bubble.left.fill")
        }
    }
}

// MARK: - Task 1: accountable placeholders for moderated items

/// How one resolved item's moderation `treatment` renders. An ordinary item
/// shows its own content (`isPlaceholder == false`); a hidden or tombstoned
/// item is PRESENT but shows accountable placeholder copy instead of its
/// content — moderation is disclosed, never a silent disappearance.
public struct CompositeSiteItemPlaceholder: Equatable, Sendable {
    public let isPlaceholder: Bool
    public let text: String

    public static func `for`(_ treatment: SiteItemTreatment) -> CompositeSiteItemPlaceholder {
        switch treatment {
        case .ordinary:
            CompositeSiteItemPlaceholder(isPlaceholder: false, text: "")
        case .hidden:
            CompositeSiteItemPlaceholder(
                isPlaceholder: true,
                text: "This post was hidden by a site editor.")
        case .tombstoned:
            CompositeSiteItemPlaceholder(
                isPlaceholder: true,
                text: "This post was removed by a site editor.")
        }
    }
}

// MARK: - Task 3: degradation copy + next-step

/// Designed copy for each of Unit 4's `SiteDegradation` states: a title plus a
/// concrete next-step, matching the existing honest-degradation convention
/// (`ShellRecoveryView` / `CatalogFailureView` in `ConferenceShellView.swift`).
/// `.none` has no copy (nothing to say); every other state has a non-empty
/// title AND next-step — never a blank screen, never an infinite spinner.
/// `transportBlocked` in particular states plainly that the site is
/// unavailable in this version, never a false "connecting…".
public enum CompositeSiteDegradation {
    public static func copy(for degradation: SiteDegradation) -> (title: String, nextStep: String)? {
        switch degradation {
        case .none:
            nil
        case .memberUnverified:
            (
                "A section of this site couldn't be verified",
                "This clears on its own once the missing signatures finish syncing."
            )
        case .editorialOnly:
            (
                "Showing editorial content only",
                "Comments and the open wire are still syncing in — check back shortly."
            )
        case .moderationLoading:
            (
                "Moderation list is loading",
                "Posts stay held until this site's moderation list catches up."
            )
        case .transportBlocked:
            (
                "This site requires Tor",
                "This site's transport isn't available in this version, so it can't be reached right now."
            )
        case .manifestRollbackAlarm:
            (
                "This site's configuration looks rolled back",
                "Content is held until an organizer confirms the site's manifest."
            )
        case .equivocationAlarm:
            (
                "This site has conflicting owner signatures",
                "Content is held until the conflicting signatures are resolved."
            )
        case .manifestInvalid:
            (
                "This site couldn't be verified",
                "Content is held until a valid signature syncs."
            )
        }
    }
}

// MARK: - The thin view

/// Renders Unit 4's `ResolvedCompositeSite`: a degradation banner (when
/// `degradation != .none`) above the resolved items, each grouped/labeled by
/// its trust tier and rendered as its own content or an accountable
/// placeholder. No business logic — every decision above is looked up from
/// the core-resolved `trustTier` / `treatment` / `degradation` fields.
public struct CompositeSiteSurfaceView: View {
    public let site: ResolvedCompositeSite

    public init(site: ResolvedCompositeSite) {
        self.site = site
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if let copy = CompositeSiteDegradation.copy(for: site.degradation) {
                    degradationBanner(copy)
                }
                ForEach(site.items, id: \.entryId) { item in
                    itemRow(item)
                }
            }
            .padding(16)
        }
        .accessibilityIdentifier("composite-site-surface")
    }

    @ViewBuilder
    private func degradationBanner(_ copy: (title: String, nextStep: String)) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(copy.title)
                .font(.headline)
            Text(copy.nextStep)
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(12)
        .background(Color.secondary.opacity(0.12))
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .accessibilityIdentifier("composite-site-degradation-banner")
    }

    @ViewBuilder
    private func itemRow(_ item: ResolvedSiteItem) -> some View {
        let style = CompositeSiteItemStyle.for(item.trustTier)
        let placeholder = CompositeSiteItemPlaceholder.for(item.treatment)
        HStack(alignment: .top, spacing: 8) {
            Label(style.badgeLabel, systemImage: style.symbolName)
                .font(.caption)
                .labelStyle(.titleAndIcon)
                .accessibilityIdentifier("composite-site-item-badge-\(style.roleToken)")
            if placeholder.isPlaceholder {
                Text(placeholder.text)
                    .font(.body)
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("composite-site-item-placeholder")
            } else {
                Text(item.entryId)
                    .font(.body)
                    .accessibilityIdentifier("composite-site-item-content")
            }
        }
        .accessibilityIdentifier("composite-site-item-\(item.entryId)")
    }
}
