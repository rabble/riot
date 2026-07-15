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
    /// Joined but never synced: a held member community with no activity and no
    /// sync exchange yet (Unit 3D, manual share-reference join). A distinct,
    /// HONEST state — the descriptor and content arrive on the first sync; until
    /// then the row says so rather than fabricating a name or a feed.
    public let pendingFirstSync: Bool

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
        quarantined: Bool,
        pendingFirstSync: Bool = false
    ) {
        self.namespaceID = namespaceID
        self.name = name
        self.relationshipLabel = relationshipLabel
        self.recentActivity = recentActivity
        self.syncFreshness = syncFreshness
        self.available = available
        self.archived = archived
        self.quarantined = quarantined
        self.pendingFirstSync = pendingFirstSync
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
            quarantined: row.quarantined,
            pendingFirstSync: Self.isPendingFirstSync(row)
        )
    }

    /// A community is "pending first sync" when it is a held, openable MEMBER
    /// space that has received nothing yet — no local activity and no sync
    /// exchange. An organizer's own space is never pending (its descriptor is
    /// local from creation), and any recorded activity or sync clears the state.
    /// Derived entirely from core's `CommunityRow`, never from a UI guess.
    static func isPendingFirstSync(_ row: CommunityRow) -> Bool {
        row.available
            && !row.archived
            && !row.quarantined
            && row.relationship != .organizer
            && row.recentActivityUnixSeconds == nil
            && row.syncFreshnessUnixSeconds == nil
    }
}

// MARK: - Manual multi-community join (Unit 3D)

/// The manual, share-reference join path. A person pastes a
/// `riot://newswire/join/v1/...` reference someone shared; Riot decodes it, joins
/// the named community as a fresh unlinkable member, and shows the community
/// "pending first sync" until its descriptor and content arrive over sync.
public enum CommunityShareJoin {
    /// The provisional local label a joined community carries BEFORE its signed
    /// descriptor arrives over sync and supplies the real community name. The
    /// reference carries only coordinates, never a name, so this is the honest
    /// placeholder; a short namespace prefix keeps two pending joins
    /// distinguishable without leading with a full technical id (nav design).
    public static func provisionalTitle(namespaceID: String) -> String {
        "New community · \(namespaceID.prefix(6))"
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
    @State private var isJoinSheetPresented = false
    @State private var pastedReference = ""

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
                    Button("Join another community") { isJoinSheetPresented = true }
                        .accessibilityIdentifier("chooser-join-another")
                }
            }
            .navigationTitle("Your communities")
            .toolbar {
                Button("Done") { model.dismissCommunityChooser() }
                    .accessibilityIdentifier("chooser-done")
            }
            .sheet(isPresented: $isJoinSheetPresented) {
                CommunityJoinSheet(model: model, pastedReference: $pastedReference) {
                    isJoinSheetPresented = false
                    pastedReference = ""
                }
            }
        }
    }
}

/// Paste-a-share-reference join (Unit 3D). Someone shares a
/// `riot://newswire/join/v1/...` link; you paste it here to follow that
/// community. It joins as a fresh unlinkable member and appears in the chooser
/// "pending first sync" until its content arrives over sync. A malformed
/// reference is refused with a plain-language message and changes nothing.
struct CommunityJoinSheet: View {
    @ObservedObject var model: RiotAppModel
    @Binding var pastedReference: String
    let onDone: () -> Void

    /// A share reference is never capitalized; the auto-capitalization modifier is
    /// iOS-only, so it is applied only there — macOS shares this source.
    private var joinReferenceField: some View {
        let field = TextField("Paste a community link", text: $pastedReference, axis: .vertical)
        #if os(iOS)
        return field.textInputAutocapitalization(.never)
        #else
        return field
        #endif
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    joinReferenceField
                        .autocorrectionDisabled()
                        .accessibilityIdentifier("join-reference-field")
                } footer: {
                    Text("Paste a link someone shared. You'll join right away; its updates arrive the next time you sync.")
                }
                if let error = model.errorMessage {
                    Text(error)
                        .foregroundStyle(.red)
                        .accessibilityIdentifier("join-error")
                }
            }
            .navigationTitle("Join a community")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Join") {
                        model.joinAdditionalCommunity(shareReference: pastedReference)
                        if model.errorMessage == nil { onDone() }
                    }
                    .disabled(pastedReference.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .accessibilityIdentifier("join-confirm")
                }
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel", action: onDone)
                        .accessibilityIdentifier("join-cancel")
                }
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
