import Foundation
import SwiftUI

// MARK: - Multiple communities (Unit 3B): the "Your communities" chooser

/// Plain-language relationship, from core's derived `CommunityRelationship`.
/// The chooser leads with what a person IS here, never a namespace id.
public extension CommunityRelationship {
    var plainLabel: String {
        switch self {
        case .organizer: return "Organizer"
        case .member: return "Member"
        case .publicReader: return "Public reader"
        }
    }
}

/// Plain relative-time rendering for the chooser's "recent activity" and "sync
/// freshness" — human phrases, never a raw timestamp. A pure function of the
/// signed second and a supplied `now`, so it is deterministic under test.
public enum CommunityRelativeTime {
    public static func recentActivity(_ unixSeconds: UInt64?, now: Date = Date()) -> String {
        guard let unixSeconds else { return "No activity yet" }
        return "Active \(phrase(unixSeconds, now: now))"
    }

    public static func syncFreshness(_ unixSeconds: UInt64?, now: Date = Date()) -> String {
        guard let unixSeconds else { return "Not synced yet" }
        return "Synced \(phrase(unixSeconds, now: now))"
    }

    static func phrase(_ unixSeconds: UInt64, now: Date) -> String {
        let then = Date(timeIntervalSince1970: TimeInterval(unixSeconds))
        let seconds = max(0, now.timeIntervalSince(then))
        switch seconds {
        case ..<60: return "just now"
        case ..<3_600:
            let minutes = Int(seconds / 60)
            return "\(minutes) minute\(minutes == 1 ? "" : "s") ago"
        case ..<86_400:
            let hours = Int(seconds / 3_600)
            return "\(hours) hour\(hours == 1 ? "" : "s") ago"
        default:
            let days = Int(seconds / 86_400)
            return "\(days) day\(days == 1 ? "" : "s") ago"
        }
    }
}

/// One row in the "Your communities" chooser, in plain language only. The
/// namespace id is carried for addressing (switch, recovery) but is NEVER the
/// leading display — name and relationship are (nav design: "no technical ids
/// dominating"). Built from core's `CommunityRow` so the derived relationship
/// and availability come from core, not a UI guess.
public struct CommunityChooserRow: Equatable, Identifiable, Sendable {
    public let namespaceID: String
    public let name: String
    public let relationshipLabel: String
    public let recentActivity: String
    public let syncFreshness: String
    /// Can be opened right now — the switch target. False → recovery, never dropped.
    public let available: Bool
    public let archived: Bool
    /// A corrupt at-rest author was preserved for recovery; opening needs repair.
    public let quarantined: Bool

    public var id: String { namespaceID }

    /// A stable accessibility identifier — the namespace is fine HERE (a11y
    /// handle), never in the visible label.
    public var accessibilityID: String { "community-row-\(namespaceID)" }

    public init(
        namespaceID: String,
        name: String,
        relationshipLabel: String,
        recentActivity: String,
        syncFreshness: String,
        available: Bool,
        archived: Bool,
        quarantined: Bool
    ) {
        self.namespaceID = namespaceID
        self.name = name
        self.relationshipLabel = relationshipLabel
        self.recentActivity = recentActivity
        self.syncFreshness = syncFreshness
        self.available = available
        self.archived = archived
        self.quarantined = quarantined
    }

    public static func from(_ row: CommunityRow, now: Date = Date()) -> CommunityChooserRow {
        CommunityChooserRow(
            namespaceID: row.namespaceId,
            name: row.title,
            relationshipLabel: row.relationship.plainLabel,
            recentActivity: CommunityRelativeTime.recentActivity(row.recentActivityUnixSeconds, now: now),
            syncFreshness: CommunityRelativeTime.syncFreshness(row.syncFreshnessUnixSeconds, now: now),
            available: row.available,
            archived: row.archived,
            quarantined: row.quarantined
        )
    }
}

// MARK: - Returning-opens-last-available

/// What the shell does on return, given the last-active community and the full
/// held set (nav design Slice 3: "Returning opens the last available community
/// directly. If the last community is unavailable, Riot opens the chooser and
/// preserves its record with recovery actions.").
public enum CommunityReturnOutcome: Equatable, Sendable {
    /// The last-active community is available — open its Home directly.
    case openCommunity(namespaceID: String)
    /// The last-active community cannot open — its record is preserved and the
    /// chooser opens with in-place recovery.
    case unavailable(CommunityUnavailable)
    /// No active community, but the person holds selectable ones — show the chooser.
    case chooser
    /// No held community at all.
    case noCommunity

    /// The decision. `active` is core's `activeCommunity()`; `all` is
    /// `listCommunities()`. Archived communities do not count as a selectable set.
    public static func decide(active: CommunityRow?, all: [CommunityRow]) -> CommunityReturnOutcome {
        if let active {
            if active.available && !active.archived {
                return .openCommunity(namespaceID: active.namespaceId)
            }
            // Last community can't open — preserve its record, recover in place.
            return .unavailable(CommunityUnavailable(name: active.title))
        }
        let selectable = all.filter { !$0.archived }
        return selectable.isEmpty ? .noCommunity : .chooser
    }
}

// MARK: - Registry seam

/// The multi-community registry, as the shell reads and drives it. Wraps the
/// Unit-3 FFI; `RiotProfileRepository` conforms (using the profile wrapping key
/// for the keyed operations), and tests inject a stub so the chooser + switch
/// are provable without a live store. Switch and persist need the wrapping key
/// (they seal/unseal per-community authors); list/active/archive/restore do not.
public protocol CommunityRegistry {
    func listCommunities() throws -> [CommunityRow]
    func activeCommunity() throws -> CommunityRow?
    @discardableResult
    func switchToCommunity(namespaceID: String) throws -> CommunityRow
    func archiveCommunity(namespaceID: String) throws
    @discardableResult
    func restoreCommunity(namespaceID: String) throws -> CommunityRow
    func persistCommunities() throws
    func communityRegistryQuarantined() throws -> Bool
}

// MARK: - Command-K

/// Community selection is focused with Command-K beginning with Slice 3 (nav
/// design). Modeled as a value so the binding is testable without a live window,
/// and stamped as the keyboard shortcut on the chooser control in the shell.
public enum CommunitySelectionShortcut {
    public static let keyEquivalent: Character = "k"
    /// The accessibility identifier the chooser entry point carries.
    public static let accessibilityID = "open-community-chooser"
}

// MARK: - The "Your communities" chooser view

/// Level-1 "Your communities" (nav design Slice 3). Lists held communities in
/// plain language — name and relationship lead, never a namespace id — with an
/// available row switching on tap and an unavailable row offering recovery in
/// place, never dropped. Create / Find nearby are actions on the chooser. Bound
/// to the app model's registry-backed `communities`; `Command-K` opens it from
/// the shell.
public struct CommunityChooserView: View {
    @ObservedObject private var model: RiotAppModel
    private let onCreate: () -> Void
    private let onFindNearby: () -> Void

    public init(
        model: RiotAppModel,
        onCreate: @escaping () -> Void = {},
        onFindNearby: @escaping () -> Void = {}
    ) {
        self.model = model
        self.onCreate = onCreate
        self.onFindNearby = onFindNearby
    }

    public var body: some View {
        NavigationStack {
            List {
                Section {
                    ForEach(model.communities) { row in
                        CommunityChooserRowView(row: row) {
                            model.switchCommunity(namespaceID: row.namespaceID)
                        }
                    }
                    if model.communities.isEmpty {
                        Text("You're not in a community yet.")
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("chooser-empty")
                    }
                }
                Section {
                    Button("Create a community", action: onCreate)
                        .accessibilityIdentifier("chooser-create")
                    Button("Find one nearby", action: onFindNearby)
                        .accessibilityIdentifier("chooser-find-nearby")
                }
            }
            .navigationTitle("Your communities")
            .toolbar {
                Button("Done") { model.dismissCommunityChooser() }
                    .accessibilityIdentifier("chooser-done")
            }
        }
    }
}

/// One plain-language chooser row. Name leads; relationship, recent activity, and
/// sync freshness are secondary; the namespace id appears nowhere on screen. An
/// unavailable row is dimmed and offers recovery rather than switching.
struct CommunityChooserRowView: View {
    let row: CommunityChooserRow
    let onSelect: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(row.name)
                .font(.headline)
            HStack(spacing: 8) {
                Text(row.relationshipLabel)
                Text("·")
                Text(row.recentActivity)
            }
            .font(.subheadline)
            .foregroundStyle(.secondary)
            Text(row.syncFreshness)
                .font(.caption)
                .foregroundStyle(.secondary)
            if !row.available {
                // Recovery in place — never a dead row. Retry re-attempts the
                // switch, which re-tries the unseal and recovers a community that a
                // transient read once quarantined (it is never permanently dead).
                HStack {
                    Text(row.quarantined
                        ? "Needs recovery before it can open."
                        : "Not available on this device yet.")
                        .font(.caption)
                        .foregroundStyle(.orange)
                        .accessibilityIdentifier("community-row-recovery-\(row.namespaceID)")
                    Spacer()
                    Button("Retry", action: onSelect)
                        .font(.caption)
                        .accessibilityIdentifier("community-row-retry-\(row.namespaceID)")
                }
            }
        }
        // An available row switches on tap; an unavailable row is not tappable as a
        // whole (its Retry button is the only action), so a stray tap can't switch.
        .contentShape(Rectangle())
        .onTapGesture { if row.available { onSelect() } }
        .accessibilityIdentifier(row.accessibilityID)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(row.name), \(row.relationshipLabel), \(row.recentActivity)")
    }
}
