import Foundation

// MARK: - What's new / unread (per-device seen state)

/// Whether a reader has caught up on a community's wire is a per-DEVICE fact, not
/// a signed record — it says nothing about the collective and must never touch the
/// Willow store or cross the FFI. It lives in UserDefaults, keyed per community.
///
/// The unread math is pure over two inputs: the posts a projection is showing
/// (each a `SeenPostRef`) and the highest order key this device has already seen
/// (the cursor). Everything that draws a badge, a delta chip, or a per-row "new"
/// dot reads from `NewswireUnread`, which is a pure function of those two.

/// The minimum a post contributes to the unread computation: its stable entry id
/// and the Willow order key (`tai_j2000_micros`, newest-first) the wire is sorted
/// by. Deliberately narrower than a `NewswirePostRow` so the math is testable
/// without constructing an FFI projection.
public struct SeenPostRef: Equatable, Sendable {
    public let entryID: String
    public let taiJ2000Micros: UInt64

    public init(entryID: String, taiJ2000Micros: UInt64) {
        self.entryID = entryID
        self.taiJ2000Micros = taiJ2000Micros
    }
}

/// The backing store `SeenCursorStore` persists to. `UserDefaults` conforms in
/// production; tests inject an in-memory double so the cursor logic is verified
/// without touching the real defaults database. The value is a decimal string so
/// the full `UInt64` order key round-trips without the `Int`/`Double` precision
/// loss `UserDefaults`'s numeric accessors would impose.
public protocol SeenStateStoring: AnyObject {
    func seenValue(forKey key: String) -> String?
    func setSeenValue(_ value: String?, forKey key: String)
}

extension UserDefaults: SeenStateStoring {
    public func seenValue(forKey key: String) -> String? { string(forKey: key) }

    public func setSeenValue(_ value: String?, forKey key: String) {
        if let value {
            set(value, forKey: key)
        } else {
            removeObject(forKey: key)
        }
    }
}

/// Persists, per community, the highest order key this device has viewed. The
/// cursor only ever moves FORWARD: `advance` never lowers it, so a stale reload or
/// an out-of-order call can never resurrect already-seen posts as unread.
///
/// The community key is the newswire space descriptor entry id (its namespace) —
/// stable for the life of the community and distinct per community, which is what
/// keeps one community's seen state from bleeding into another's. An empty key
/// (a community with no descriptor yet) is inert: there is nothing to track until
/// the wire has an identity.
public final class SeenCursorStore {
    private let store: SeenStateStoring
    private let keyPrefix: String

    public init(store: SeenStateStoring = UserDefaults.standard,
                keyPrefix: String = "riot.newswire.seen.") {
        self.store = store
        self.keyPrefix = keyPrefix
    }

    private func storageKey(forCommunity community: String) -> String? {
        community.isEmpty ? nil : keyPrefix + community
    }

    /// The highest order key this device has marked seen for the community, or
    /// `nil` when it has never looked (a fresh community — everything is unread).
    public func cursor(forCommunity community: String) -> UInt64? {
        guard let key = storageKey(forCommunity: community),
              let raw = store.seenValue(forKey: key) else { return nil }
        return UInt64(raw)
    }

    /// Move the community's cursor up to `value`. A no-op when `value` is not
    /// strictly greater than the stored cursor (monotonic) or the community has no
    /// key yet, so this is always safe to call with the max of whatever is on
    /// screen — it can never mark seen something older than the reader saw.
    public func advance(community: String, to value: UInt64) {
        guard let key = storageKey(forCommunity: community) else { return }
        if let existing = cursor(forCommunity: community), existing >= value { return }
        store.setSeenValue(String(value), forKey: key)
    }
}

/// The unread state for one community's wire: how many posts are newer than the
/// seen cursor, which specific posts those are (so a row can draw its "new"
/// marker), and the newest order key currently shown (what a mark-all-seen would
/// advance the cursor to). A pure value derived from the shown posts and the
/// stored cursor — no side effects, no persistence.
public struct NewswireUnread: Equatable, Sendable {
    /// How many shown posts are newer than the cursor.
    public let count: Int
    /// The entry ids of exactly those unread posts.
    public let newEntryIDs: Set<String>
    /// The newest order key among the shown posts, or `nil` when none are shown.
    /// Marking all seen advances the cursor to this — never past a post the reader
    /// has not loaded.
    public let latestTimestamp: UInt64?

    public init(posts: [SeenPostRef], cursor: UInt64?) {
        let unread = posts.filter { post in
            guard let cursor else { return true }
            return post.taiJ2000Micros > cursor
        }
        self.count = unread.count
        self.newEntryIDs = Set(unread.map(\.entryID))
        self.latestTimestamp = posts.map(\.taiJ2000Micros).max()
    }

    /// The empty unread state — nothing shown, nothing new.
    public static let none = NewswireUnread(posts: [], cursor: nil)

    /// Whether the given post is one of the unread ones (drives the per-row dot).
    public func isNew(_ entryID: String) -> Bool { newEntryIDs.contains(entryID) }

    /// Whether there is anything new to announce (drives the delta chip + badge).
    public var hasUnread: Bool { count > 0 }
}
