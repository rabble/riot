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
