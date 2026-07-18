import SwiftUI

public enum AlertsStrings {
    public static let title = "Alerts"
    public static let organizerBadge = "Organizer"
    public static let emptyTitle = "No alerts yet"
    public static let emptyMessage = "Signed alerts from this community will appear here."
    public static let expired = "Expired"
    public static let active = "Active"
    public static func expires(inSeconds: Int64) -> String {
        // Human phrase, never a raw epoch. Coarse buckets are enough for a board row.
        let mins = inSeconds / 60
        if mins < 60 { return "Expires in \(max(mins, 1))m" }
        let hours = mins / 60
        if hours < 24 { return "Expires in \(hours)h" }
        return "Expires in \(hours / 24)d"
    }
}

/// Freshness as a human phrase derived from the alert's validity window — a pure
/// function of the entry + now, mirroring `CommunityRelativeTime.syncFreshness`.
public enum AlertRelativeTime {
    public static func freshness(_ entry: RiotEntry, now: Date = Date()) -> String {
        let nowSecs = Int64(now.timeIntervalSince1970)
        let remaining = Int64(entry.expiresAt) - nowSecs
        if remaining <= 0 { return AlertsStrings.expired }
        return AlertsStrings.expires(inSeconds: remaining)
    }
}

/// One alert row. `isOrganizer` and the ordering come ONLY from the core-verified
/// coordinate rule (signer subspace == namespace id); the display name is never
/// consulted for either. The full `signerID`/`entry` are retained for the detail
/// sheet and pinning, never as the row's display string.
public struct AlertRow: Equatable, Identifiable, Sendable {
    public let entry: RiotEntry
    public var id: String { entry.entryID }
    public var headline: String { entry.headline }
    public var namespaceID: String { entry.namespaceID }
    public var signerID: String { entry.signerID }
    public var aiAssisted: Bool { entry.aiAssisted }
    public var signerTag: String { String(entry.signerID.prefix(8)) }
    public let isOrganizer: Bool
    public let freshness: String

    public init(_ entry: RiotEntry, activeNamespaceID: String, now: Date = Date()) {
        self.entry = entry
        // Coordinate rule: an organizer signs with the author subspace that equals
        // the space namespace id (both fields are core-verified identity).
        self.isOrganizer = entry.signerID.lowercased() == entry.namespaceID.lowercased()
        self.freshness = AlertRelativeTime.freshness(entry, now: now)
    }
}

public struct AlertsEmpty: Equatable, Sendable {
    public let title: String
    public let message: String
    public static let noAlerts = AlertsEmpty(title: AlertsStrings.emptyTitle,
                                             message: AlertsStrings.emptyMessage)
}

public enum AlertsListState: Equatable, Sendable {
    case empty(AlertsEmpty)
    case populated([AlertRow])

    /// Maps the app's (already active-scoped) entries into organizer-first rows.
    /// The `namespaceID == activeNamespaceID` filter is defense in depth: the FFI
    /// `list_current_entries` already scopes to the active namespace, but a Swift
    /// filter guarantees a future FFI regression can never leak a foreign alert.
    public static func from(_ entries: [RiotEntry], activeNamespaceID: String, now: Date = Date()) -> AlertsListState {
        let scoped = entries.filter { $0.namespaceID.lowercased() == activeNamespaceID.lowercased() }
        guard !scoped.isEmpty else { return .empty(.noAlerts) }
        let rows = scoped
            .map { AlertRow($0, activeNamespaceID: activeNamespaceID, now: now) }
            .sorted { lhs, rhs in
                if lhs.isOrganizer != rhs.isOrganizer { return lhs.isOrganizer } // organizers first
                if lhs.entry.createdAt != rhs.entry.createdAt {
                    return lhs.entry.createdAt > rhs.entry.createdAt              // then newest first
                }
                return lhs.entry.entryID < rhs.entry.entryID                     // stable tiebreak
            }
        return .populated(rows)
    }
}

/// The single Home entry point for a community's signed alerts. Renders
/// `AlertsListState` inside a `RiotCard`, organizer-first rows, each opening
/// `AlertDetailSheet`. Headline + signer render as plain `Text(verbatim:)` — no
/// markdown auto-link. The signer line leads with the core-verified `signerTag` +
/// organizer badge; the optional self-claimed display name is secondary decoration.
public struct AlertsListView: View {
    public let entries: [RiotEntry]
    public let activeNamespaceID: String
    /// Self-claimed display name for a signer, if known — decoration only.
    public let displayName: (String) -> String?
    @State private var selected: RiotEntry?
    @Environment(\.colorScheme) private var colorScheme

    public init(entries: [RiotEntry], activeNamespaceID: String,
                displayName: @escaping (String) -> String? = { _ in nil }) {
        self.entries = entries
        self.activeNamespaceID = activeNamespaceID
        self.displayName = displayName
    }

    public var body: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 12) {
                Text(AlertsStrings.title.uppercased())
                    .font(.riot(.mono, size: 12, relativeTo: .caption)).tracking(1)
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                switch AlertsListState.from(entries, activeNamespaceID: activeNamespaceID) {
                case .empty(let empty):
                    Text(empty.title).font(.riot(.body, size: 15, relativeTo: .callout))
                        .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    Text(empty.message).font(.riot(.body, size: 13, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                case .populated(let rows):
                    ForEach(rows) { row in
                        Button { selected = row.entry } label: { rowLabel(row) }
                            .buttonStyle(.riotSecondary)
                            .accessibilityIdentifier("alert-\(row.id)")
                    }
                }
            }
        }
        .accessibilityIdentifier("home-alerts-card")
        .sheet(item: $selected) { entry in
            AlertDetailSheet(detail: AlertDetail(entry: entry), onClose: { selected = nil })
        }
    }

    @ViewBuilder private func rowLabel(_ row: AlertRow) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(verbatim: row.headline).font(.riot(.body, size: 17, relativeTo: .body))
            HStack(spacing: 6) {
                if row.isOrganizer {
                    Text(AlertsStrings.organizerBadge).font(.riot(.mono, size: 11, relativeTo: .caption2))
                }
                Text(verbatim: displayName(row.signerID) ?? row.signerTag)
                    .font(.riot(.mono, size: 11, relativeTo: .caption2))
                Spacer()
                Text(row.freshness).font(.riot(.mono, size: 11, relativeTo: .caption2))
            }
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        }
    }
}

/// Renders the existing `AlertDetail` value model. Headline is plain
/// `Text(verbatim:)` (never markdown/AttributedString auto-link — anti-injection);
/// the 64-hex ids stay behind the closed **Technical details** disclosure until a
/// person opts in.
public struct AlertDetailSheet: View {
    /// The disclosure default, exposed for the contract test (full ids stay hidden until opt-in).
    public static let technicalStartsExpanded = false

    public let detail: AlertDetail
    public let onClose: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var showingTechnical = AlertDetailSheet.technicalStartsExpanded

    public init(detail: AlertDetail, onClose: @escaping () -> Void) {
        self.detail = detail
        self.onClose = onClose
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                // Plain Text(verbatim:) — never markdown/AttributedString auto-link (anti-injection).
                Text(verbatim: detail.headline)
                    .font(.riot(.body, size: 20, relativeTo: .title3))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    .accessibilityAddTraits(.isHeader)
                if detail.aiAssisted {
                    Text("AI-assisted")
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        .accessibilityIdentifier("alert-detail-ai-assisted")
                }
                ForEach(detail.summary, id: \.label) { row in
                    LabeledContent(row.label, value: row.value)
                }
                DisclosureGroup(AlertDetail.technicalDisclosureTitle, isExpanded: $showingTechnical) {
                    VStack(alignment: .leading, spacing: 6) {
                        ForEach(detail.technical, id: \.label) { row in
                            VStack(alignment: .leading, spacing: 2) {
                                Text(row.label).font(.riot(.mono, size: 11, relativeTo: .caption2))
                                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                                Text(verbatim: row.value).font(.riot(.mono, size: 12, relativeTo: .caption))
                                    .textSelection(.enabled)
                            }
                        }
                    }
                }
                .font(.riot(.mono, size: 12, relativeTo: .caption))
                .accessibilityIdentifier("alert-detail-technical")
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Alert", detail.headline)
        .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done", action: onClose) } }
    }
}
