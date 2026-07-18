import XCTest
@testable import RiotKit

/// An in-memory `SeenStateStoring` so the cursor logic is exercised without the
/// real UserDefaults database — and so "persists across reload" can be tested by
/// building a second `SeenCursorStore` over the SAME backing dictionary.
private final class MemorySeenStore: SeenStateStoring {
    private var values: [String: String] = [:]
    func seenValue(forKey key: String) -> String? { values[key] }
    func setSeenValue(_ value: String?, forKey key: String) {
        if let value { values[key] = value } else { values[key] = nil }
    }
}

private func ref(_ id: String, _ tai: UInt64) -> SeenPostRef {
    SeenPostRef(entryID: id, taiJ2000Micros: tai)
}

final class WhatsNewTests: XCTestCase {
    // MARK: - Unread computation

    func testFreshCommunityWithNoCursorMarksEveryPostUnread() {
        let posts = [ref("a", 30), ref("b", 20), ref("c", 10)]
        let unread = NewswireUnread(posts: posts, cursor: nil)
        XCTAssertEqual(unread.count, 3)
        XCTAssertTrue(unread.hasUnread)
        XCTAssertTrue(unread.isNew("a"))
        XCTAssertTrue(unread.isNew("c"))
        XCTAssertEqual(unread.latestTimestamp, 30)
    }

    func testCursorAtLatestMarksNothingUnread() {
        let posts = [ref("a", 30), ref("b", 20)]
        let unread = NewswireUnread(posts: posts, cursor: 30)
        XCTAssertEqual(unread.count, 0)
        XCTAssertFalse(unread.hasUnread)
        XCTAssertFalse(unread.isNew("a"))
    }

    func testOnlyPostsStrictlyNewerThanCursorAreUnread() {
        let posts = [ref("new", 40), ref("seen", 25), ref("boundary", 25)]
        let unread = NewswireUnread(posts: posts, cursor: 25)
        XCTAssertEqual(unread.count, 1)
        XCTAssertTrue(unread.isNew("new"))
        XCTAssertFalse(unread.isNew("seen"))
        XCTAssertFalse(unread.isNew("boundary"))
    }

    func testExactlyOneNewerPostReadsAsUnreadOne() {
        let posts = [ref("newest", 100), ref("old", 50)]
        let unread = NewswireUnread(posts: posts, cursor: 60)
        XCTAssertEqual(unread.count, 1)
    }

    func testLatestTimestampIsNilWhenNoPosts() {
        let unread = NewswireUnread(posts: [], cursor: nil)
        XCTAssertEqual(unread.count, 0)
        XCTAssertNil(unread.latestTimestamp)
        XCTAssertFalse(unread.hasUnread)
    }

    // MARK: - Seen cursor persistence

    func testCursorIsNilBeforeAnythingIsMarkedSeen() {
        let store = SeenCursorStore(store: MemorySeenStore())
        XCTAssertNil(store.cursor(forCommunity: "space-1"))
    }

    func testAdvanceThenReadReturnsTheAdvancedValue() {
        let store = SeenCursorStore(store: MemorySeenStore())
        store.advance(community: "space-1", to: 42)
        XCTAssertEqual(store.cursor(forCommunity: "space-1"), 42)
    }

    func testAdvanceNeverMovesTheCursorBackward() {
        let store = SeenCursorStore(store: MemorySeenStore())
        store.advance(community: "space-1", to: 100)
        store.advance(community: "space-1", to: 40)
        XCTAssertEqual(store.cursor(forCommunity: "space-1"), 100)
    }

    func testCursorPersistsAcrossStoreReload() {
        let backing = MemorySeenStore()
        SeenCursorStore(store: backing).advance(community: "space-1", to: 77)
        // A fresh store over the same backing is a new app launch.
        let reopened = SeenCursorStore(store: backing)
        XCTAssertEqual(reopened.cursor(forCommunity: "space-1"), 77)
    }

    func testOneCommunitysCursorDoesNotAffectAnother() {
        let store = SeenCursorStore(store: MemorySeenStore())
        store.advance(community: "space-A", to: 500)
        XCTAssertEqual(store.cursor(forCommunity: "space-A"), 500)
        XCTAssertNil(store.cursor(forCommunity: "space-B"))
        store.advance(community: "space-B", to: 10)
        XCTAssertEqual(store.cursor(forCommunity: "space-A"), 500)
        XCTAssertEqual(store.cursor(forCommunity: "space-B"), 10)
    }

    func testEmptyCommunityKeyIsInert() {
        let store = SeenCursorStore(store: MemorySeenStore())
        store.advance(community: "", to: 99)
        XCTAssertNil(store.cursor(forCommunity: ""))
    }

    func testLargeOrderKeyRoundTripsWithoutPrecisionLoss() {
        let store = SeenCursorStore(store: MemorySeenStore())
        let big: UInt64 = 9_000_000_000_000_000_123
        store.advance(community: "space-1", to: big)
        XCTAssertEqual(store.cursor(forCommunity: "space-1"), big)
    }

    // MARK: - End-to-end: mark seen zeroes the next unread computation

    func testMarkSeenThenRecomputeShowsZeroUnread() {
        let store = SeenCursorStore(store: MemorySeenStore())
        let posts = [ref("a", 30), ref("b", 20), ref("c", 10)]

        let firstVisit = NewswireUnread(posts: posts, cursor: store.cursor(forCommunity: "s"))
        XCTAssertEqual(firstVisit.count, 3)

        // Marking all seen advances the cursor to the newest shown post.
        store.advance(community: "s", to: firstVisit.latestTimestamp ?? 0)

        let secondVisit = NewswireUnread(posts: posts, cursor: store.cursor(forCommunity: "s"))
        XCTAssertEqual(secondVisit.count, 0)

        // A newer post arriving after mark-seen is the only thing that reads unread.
        let withNewer = posts + [ref("d", 45)]
        let thirdVisit = NewswireUnread(posts: withNewer, cursor: store.cursor(forCommunity: "s"))
        XCTAssertEqual(thirdVisit.count, 1)
        XCTAssertTrue(thirdVisit.isNew("d"))
    }
}
