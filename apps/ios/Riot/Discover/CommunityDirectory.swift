import Foundation
import SwiftUI

// MARK: - The Discover front door — find a community you don't already have a link for.
//
// Today the only ways into a community are: create one, paste a link/QR, or
// find-nearby (Bluetooth). None of those help a person who wants to BROWSE and
// discover a community they have no link for. This is that front door.
//
// The surface reads communities only through the `CommunityDirectory` seam, so
// the search + category filtering is pure and runs entirely in unit tests with
// no FFI behind it. The seeded source below is a clearly-marked stand-in.
//
// FOLLOW-UP (out of scope here): the real long-term source is "plural signed
// directory feeds" published on the anchor relay. That backend is unbuilt. When
// it lands, a `RelayDirectoryFeedSource: CommunityDirectory` replaces
// `SeededCommunityDirectory` with verified feed rows — the surface and the view
// model do not change, because they only ever touch the protocol.

/// The four ways a person browses "what do I need right now", mirroring the
/// prototype's chips: Events / Help & recs / Info & updates / Guides.
public enum DiscoverCategory: String, CaseIterable, Identifiable, Sendable {
    case events
    case help
    case info
    case guides

    public var id: String { rawValue }

    /// Plain-language chip label — never a raw enum name.
    public var label: String {
        switch self {
        case .events: return "Events"
        case .help: return "Help & recs"
        case .info: return "Info & updates"
        case .guides: return "Guides"
        }
    }

    /// SF Symbol for the chip. Chosen to read at a glance without color alone.
    public var systemImage: String {
        switch self {
        case .events: return "megaphone"
        case .help: return "hands.sparkles"
        case .info: return "newspaper"
        case .guides: return "book.closed"
        }
    }
}

/// One community as the Discover surface shows it, before the person is a member:
/// what it is, who keeps it, how alive it is, and whether it is open to walk into
/// or invite-only. Every string is already in the plain language the surface
/// renders, so the row is the whole product decision and a pure function builds
/// it — the same shape as `RiotDirectoryRow` for apps.
public struct DiscoverableCommunity: Identifiable, Equatable, Hashable, Sendable {
    public let id: String
    public let name: String
    /// One line, plain language — "Neighbors defending each other from eviction."
    public let about: String
    public let category: DiscoverCategory
    public let stewardName: String
    /// People count as a browsing hint, never a precise live figure for a seed.
    public let peopleCount: Int
    /// "12 active today", "quiet" — a human liveness hint, not a transport receipt.
    public let activityHint: String
    /// Open to walk into vs invite-only (vouched). Drives the open/invite tag.
    public let isOpen: Bool
    /// A bounded "what's new" glimpse for the Preview screen — never an endless
    /// scroll. A handful of one-line items.
    public let whatsNew: [String]
    /// True when this row is a clearly-marked seed, not a real live community.
    /// The surface stamps a "Sample" badge from this so nothing is ever presented
    /// as a genuine live directory listing.
    public let isSeed: Bool
    /// The `riot://newswire/join/v1/...` reference this community routes into, or
    /// nil when there is none yet. A seed has none (it is not a live community);
    /// a live feed row will carry one and route straight into the existing
    /// commit-join flow. See `CommunityJoinRoute`.
    public let joinReference: String?

    public init(
        id: String,
        name: String,
        about: String,
        category: DiscoverCategory,
        stewardName: String,
        peopleCount: Int,
        activityHint: String,
        isOpen: Bool,
        whatsNew: [String],
        isSeed: Bool,
        joinReference: String?
    ) {
        self.id = id
        self.name = name
        self.about = about
        self.category = category
        self.stewardName = stewardName
        self.peopleCount = peopleCount
        self.activityHint = activityHint
        self.isOpen = isOpen
        self.whatsNew = whatsNew
        self.isSeed = isSeed
        self.joinReference = joinReference
    }

    /// Two-letter monogram for the card badge (initials of the first two words).
    public var monogram: String {
        let words = name.split(separator: " ")
        let letters = words.prefix(2).compactMap { $0.first }
        let joined = String(letters).uppercased()
        return joined.isEmpty ? "•" : joined
    }

    /// The open/invite tag text.
    public var accessLabel: String { isOpen ? "Open" : "Invite-only" }
}

/// How a person browses discoverable communities. The Discover surface reaches
/// its source ONLY through this protocol, so every code path is provable without
/// FFI and the seeded source swaps for a live signed-directory feed later with no
/// change above it. The twin of `DirectoryPorting` for apps.
public protocol CommunityDirectory: AnyObject {
    func discoverableCommunities() -> [DiscoverableCommunity]
}

/// A clearly-marked, in-memory seed source that mirrors the design prototype's
/// activist communities. NOT a live directory — every row is `isSeed: true` and
/// carries no join reference, so the surface can be real and navigable now while
/// the signed-directory-feed backend is still unbuilt.
public final class SeededCommunityDirectory: CommunityDirectory {
    public init() {}

    public func discoverableCommunities() -> [DiscoverableCommunity] {
        [
            DiscoverableCommunity(
                id: "seed-river-city-wire",
                name: "River City Wire",
                about: "Dispatches from the streets — protests, council votes, and mutual aid, in the community's own words.",
                category: .info,
                stewardName: "Ada Booker",
                peopleCount: 34,
                activityHint: "12 active today",
                isOpen: true,
                whatsNew: ["3 asks open", "1 alert", "2 reports since yesterday"],
                isSeed: true,
                joinReference: nil
            ),
            DiscoverableCommunity(
                id: "seed-eastside-tenant-union",
                name: "Eastside Tenant Union",
                about: "Neighbors defending each other from eviction. Know your rights, find your block captain.",
                category: .help,
                stewardName: "Mira Koss",
                peopleCount: 81,
                activityHint: "5 active",
                isOpen: false,
                whatsNew: ["Block captain sign-up: 6 of 14 blocks", "Legal clinic Saturday 10am"],
                isSeed: true,
                joinReference: nil
            ),
            DiscoverableCommunity(
                id: "seed-harbor-mutual-aid",
                name: "Harbor Mutual Aid",
                about: "Rides, meals, childcare, and repairs — neighbors sharing what they have.",
                category: .help,
                stewardName: "Sol Ortega",
                peopleCount: 128,
                activityHint: "quiet",
                isOpen: true,
                whatsNew: ["Van + 6 seats offered Thursday", "Pantry restock this weekend"],
                isSeed: true,
                joinReference: nil
            ),
            DiscoverableCommunity(
                id: "seed-climate-convergence",
                name: "Climate Justice Convergence",
                about: "Where the city's climate groups coordinate actions, marches, and teach-ins.",
                category: .events,
                stewardName: "Teo Salas",
                peopleCount: 210,
                activityHint: "18 active today",
                isOpen: true,
                whatsNew: ["March route posted for the 12th", "Teach-in Wednesday evening"],
                isSeed: true,
                joinReference: nil
            ),
            DiscoverableCommunity(
                id: "seed-know-your-rights",
                name: "Know Your Rights Collective",
                about: "Plain-language guides for stops, searches, and protests — kept current, in several languages.",
                category: .guides,
                stewardName: "Nadia Rahman",
                peopleCount: 64,
                activityHint: "3 active",
                isOpen: true,
                whatsNew: ["Updated: what to do at a checkpoint", "New Spanish translation"],
                isSeed: true,
                joinReference: nil
            ),
            DiscoverableCommunity(
                id: "seed-backwoods-collective",
                name: "Backwoods Collective",
                about: "A rural network that stays reachable off-grid — sync catches up when you're near a member.",
                category: .info,
                stewardName: "June Alder",
                peopleCount: 22,
                activityHint: "quiet",
                isOpen: false,
                whatsNew: ["Trail check-in schedule for spring"],
                isSeed: true,
                joinReference: nil
            ),
        ]
    }
}

/// The two ways the Preview screen's join action can go. A community carrying a
/// real reference (a live feed row) routes straight into the existing
/// commit-join flow; a seed with no reference falls back to the honest paste/QR
/// sheet — Riot never fabricates a joinable coordinate for a sample row.
public enum CommunityJoinRoute: Equatable {
    case commitReference(String)
    case pasteOrScan

    public static func route(for community: DiscoverableCommunity) -> CommunityJoinRoute {
        if let reference = community.joinReference {
            return .commitReference(reference)
        }
        return .pasteOrScan
    }
}

/// Discover surface logic with no FFI of its own — it reads communities only
/// through `CommunityDirectory`, and its search + category filtering is a pure
/// static function the tests call directly. The twin of `RiotDirectoryModel` for
/// community discovery.
@MainActor
public final class CommunityDiscoveryModel: ObservableObject {
    /// The live search text. Publishing it re-derives `results`.
    @Published public var searchText: String = "" { didSet { recompute() } }
    /// The selected category chip, or nil for "all". Re-derives `results`.
    @Published public var selectedCategory: DiscoverCategory? { didSet { recompute() } }
    @Published public private(set) var results: [DiscoverableCommunity] = []

    private var all: [DiscoverableCommunity] = []
    private let source: CommunityDirectory

    public init(source: CommunityDirectory = SeededCommunityDirectory()) {
        self.source = source
    }

    /// Re-reads the directory and re-applies the current search + category. The
    /// only way the surface learns of a new listing (today a no-op reseed; with a
    /// live feed, whatever most recently arrived).
    public func refresh() {
        all = source.discoverableCommunities()
        recompute()
    }

    /// Toggles a category chip: tapping the selected one clears it.
    public func toggle(_ category: DiscoverCategory) {
        selectedCategory = (selectedCategory == category) ? nil : category
    }

    private func recompute() {
        results = Self.filter(all, search: searchText, category: selectedCategory)
    }

    /// Pure search + category filter. Search matches name, one-line about, and
    /// steward, case- and diacritic-insensitively; a blank search returns
    /// everything. Category, when set, restricts to that category. Composed with
    /// AND. Kept static and pure so the whole filtering contract is provable
    /// without a live model.
    public static func filter(
        _ all: [DiscoverableCommunity],
        search: String,
        category: DiscoverCategory?
    ) -> [DiscoverableCommunity] {
        let needle = search.trimmingCharacters(in: .whitespacesAndNewlines)
        return all.filter { community in
            if let category, community.category != category { return false }
            guard !needle.isEmpty else { return true }
            let haystack = [community.name, community.about, community.stewardName]
                .joined(separator: "\n")
            return haystack.range(
                of: needle,
                options: [.caseInsensitive, .diacriticInsensitive]
            ) != nil
        }
    }
}
