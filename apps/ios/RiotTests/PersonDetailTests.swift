import XCTest
@testable import RiotKit

/// The Person detail surface — a contributor's path to the content they posted.
/// Tested in isolation (no store, no FFI: the newswire projector seam is stubbed).
///
/// The contract: a person's page shows exactly the posts THAT person authored in
/// this community, newest first, drawn from the SAME collective projection the
/// Home wire draws — filtered by the author's stable id, never re-decided. A
/// featured post that appears on both the front page and the open wire is shown
/// once. Expired ("earlier") posts still count as their content. A projection
/// failure never leaks a raw error.
final class PersonDetailTests: XCTestCase {
    // MARK: - Fixtures

    private static let alice = String(repeating: "a", count: 64)
    private static let bob = String(repeating: "b", count: 64)

    private static func author(_ id: String, name: String = "Ana") -> NewswireAuthor {
        NewswireAuthor(
            id: id, displayName: name, tag: String(id.prefix(8)),
            rendered: "\(name) · \(id.prefix(8))"
        )
    }

    private static func post(
        id: String,
        authorID: String,
        headline: String? = "Headline",
        tai: UInt64 = 1,
        created: UInt64? = nil,
        treatment: NewswirePostTreatment = .ordinary,
        expires: UInt64? = nil
    ) -> NewswireProjectedPost {
        NewswireProjectedPost(
            entryId: id, author: author(authorID), taiJ2000Micros: tai,
            createdAtUnixSeconds: created,
            headline: headline, body: "body", language: "en",
            coarseLocation: nil, eventTimeUnixSeconds: nil, expiresAtUnixSeconds: expires,
            sourceClaims: [], operationalProfile: nil, aiAssisted: false,
            verificationIds: [], correctionIds: [], treatment: treatment, reactions: []
        )
    }

    private static func projection(
        open: [NewswireProjectedPost] = [],
        front: [NewswireProjectedPost] = [],
        earlier: [NewswireProjectedPost] = []
    ) -> NewswireProjectionView {
        NewswireProjectionView(
            openWire: open, frontPage: front, earlier: earlier,
            comments: [], editorialHistory: [], futureQuarantine: []
        )
    }

    private static func contributor(
        id: String, name: String = "Ana", organizer: Bool = false, count: UInt32 = 1
    ) -> NewswireContributor {
        NewswireContributor(
            author: author(id, name: name), isOrganizer: organizer, contributionCount: count
        )
    }

    private struct FixedProjector: NewswireProjecting {
        let projection: NewswireProjectionView
        func projectNewswire(spaceDescriptorEntryID: String) throws -> NewswireProjectionView {
            projection
        }
    }

    private struct ThrowingProjector: NewswireProjecting {
        struct RawInternalError: Error { let description = "Internal { store: \"corrupt 0x41\" }" }
        func projectNewswire(spaceDescriptorEntryID: String) throws -> NewswireProjectionView {
            throw RawInternalError()
        }
    }

    // MARK: - The filter: this person's posts, no one else's

    func testAuthoredReturnsOnlyThatPersonsPostsNewestFirst() {
        let projection = Self.projection(open: [
            Self.post(id: "1", authorID: Self.alice, tai: 10),
            Self.post(id: "2", authorID: Self.bob, tai: 20),
            Self.post(id: "3", authorID: Self.alice, tai: 30),
        ])
        let rows = PersonPosts.authored(by: Self.alice, in: projection)
        // Alice's two posts, newest ordering value first; Bob's post excluded.
        XCTAssertEqual(rows.map(\.id), ["3", "1"])
        XCTAssertTrue(rows.allSatisfy { $0.authorKeyHex == Self.alice })
        XCTAssertFalse(rows.contains { $0.authorKeyHex == Self.bob })
    }

    func testAuthoredDedupesAPostFeaturedOnBothFrontPageAndOpenWire() {
        let featured = Self.post(id: "1", authorID: Self.alice, tai: 5)
        let projection = Self.projection(open: [featured], front: [featured])
        let rows = PersonPosts.authored(by: Self.alice, in: projection)
        XCTAssertEqual(rows.map(\.id), ["1"], "a featured post is shown once, not twice")
    }

    func testAuthoredIncludesEarlierExpiredPosts() {
        let projection = Self.projection(
            open: [Self.post(id: "1", authorID: Self.alice, tai: 10)],
            earlier: [Self.post(id: "0", authorID: Self.alice, tai: 1, expires: 5)]
        )
        let rows = PersonPosts.authored(by: Self.alice, in: projection)
        XCTAssertEqual(rows.map(\.id), ["1", "0"], "an expired post is still this person's content")
    }

    func testAuthoredIsEmptyWhenThePersonHasNoPosts() {
        let projection = Self.projection(open: [Self.post(id: "2", authorID: Self.bob)])
        XCTAssertTrue(PersonPosts.authored(by: Self.alice, in: projection).isEmpty)
    }

    // MARK: - The model: load, empty, and never a raw error

    @MainActor
    func testModelLoadsThePersonsPosts() {
        let projection = Self.projection(open: [
            Self.post(id: "1", authorID: Self.alice, tai: 10),
            Self.post(id: "2", authorID: Self.bob, tai: 20),
        ])
        let model = PersonDetailModel(
            person: PersonRow(Self.contributor(id: Self.alice)),
            projector: FixedProjector(projection: projection),
            spaceDescriptorEntryID: "d"
        )
        model.load()
        guard case let .posts(rows) = model.state else {
            return XCTFail("expected .posts")
        }
        XCTAssertEqual(rows.map(\.id), ["1"])
    }

    @MainActor
    func testModelIsEmptyWhenThePersonHasNoPosts() {
        // A contributor known only through editorial actions or replies has no
        // posts to show — an honest empty state, never a fabricated row.
        let projection = Self.projection(open: [Self.post(id: "2", authorID: Self.bob)])
        let model = PersonDetailModel(
            person: PersonRow(Self.contributor(id: Self.alice, count: 3)),
            projector: FixedProjector(projection: projection),
            spaceDescriptorEntryID: "d"
        )
        model.load()
        XCTAssertEqual(model.state, .empty)
    }

    @MainActor
    func testModelProjectionFailureShowsFixedMessageNeverRawError() {
        let model = PersonDetailModel(
            person: PersonRow(Self.contributor(id: Self.alice)),
            projector: ThrowingProjector(),
            spaceDescriptorEntryID: "d"
        )
        model.load()
        guard case let .unavailable(message) = model.state else {
            return XCTFail("a failure must map to the fixed unavailable state")
        }
        XCTAssertEqual(message, PeopleStrings.personUnavailableMessage)
        XCTAssertFalse(message.contains("corrupt"))
        XCTAssertFalse(message.contains("Internal"))
    }

    // MARK: - The person carries their identity onto the page

    @MainActor
    func testModelCarriesThePersonIdentityForTheHeader() {
        let model = PersonDetailModel(
            person: PersonRow(Self.contributor(id: Self.alice, name: "Ana", organizer: true, count: 4)),
            projector: FixedProjector(projection: Self.projection()),
            spaceDescriptorEntryID: "d"
        )
        XCTAssertEqual(model.person.displayName, "Ana")
        XCTAssertTrue(model.person.isOrganizer)
        XCTAssertEqual(PeopleStrings.contributions(model.person.contributionCount), "4 contributions")
    }

    // MARK: - Recent activity: a true wall-clock "ago"

    // A projected post now carries a real creation instant (createdAtUnixSeconds,
    // recovered by core from the entry timestamp), so recency is a true "N ago"
    // against the current clock. Every function takes an explicit `now`, so it
    // stays deterministic under test — no fabricated instant, no reliance on the
    // real wall clock.

    // A fixed "now": 2001-09-09T01:46:40Z (Unix 1_000_000_000), the anchor the
    // post creation times below are measured back from.
    private static let fixedNow = Date(timeIntervalSince1970: 1_000_000_000)

    func testHeaderRecencyReadsTheNewestPostsRealCreationTimeAsAgo() {
        // Newest post created three hours before `now`.
        let created: UInt64 = 1_000_000_000 - 3 * 3_600
        XCTAssertEqual(
            PersonActivity.headerRecency(
                personNewestCreatedUnixSeconds: created, now: Self.fixedNow),
            "Last posted 3h ago"
        )
    }

    func testHeaderRecencyReadsJustNowForAFreshPost() {
        let created: UInt64 = 1_000_000_000 - 5 // five seconds ago
        XCTAssertEqual(
            PersonActivity.headerRecency(
                personNewestCreatedUnixSeconds: created, now: Self.fixedNow),
            "Last posted just now"
        )
    }

    func testHeaderRecencyIsNilWhenThePersonHasNoRecoverableTime() {
        // No posts (nil) or a 0/absent creation time → NO activity line at all.
        XCTAssertNil(
            PersonActivity.headerRecency(personNewestCreatedUnixSeconds: nil, now: Self.fixedNow)
        )
        XCTAssertNil(
            PersonActivity.headerRecency(personNewestCreatedUnixSeconds: 0, now: Self.fixedNow)
        )
    }

    func testRowRecencyIsATrueAgoOrNilWhenAbsent() {
        let created: UInt64 = 1_000_000_000 - 86_400 // one day ago
        XCTAssertEqual(
            PersonActivity.rowRecency(rowCreatedUnixSeconds: created, now: Self.fixedNow),
            "yesterday"
        )
        XCTAssertNil(
            PersonActivity.rowRecency(rowCreatedUnixSeconds: nil, now: Self.fixedNow)
        )
    }

    @MainActor
    func testModelExposesRecentActivityFromTheNewestPostsCreationTimeOnLoad() {
        // Alice's newest post (highest ordering value) was created two hours ago.
        let aliceNewestCreated: UInt64 = 1_000_000_000 - 2 * 3_600
        let projection = Self.projection(open: [
            Self.post(id: "a0", authorID: Self.alice, tai: 10, created: 1_000_000_000 - 5 * 3_600),
            Self.post(id: "a1", authorID: Self.alice, tai: 99, created: aliceNewestCreated),
            Self.post(id: "b1", authorID: Self.bob, tai: 200, created: 1_000_000_000 - 1),
        ])
        let model = PersonDetailModel(
            person: PersonRow(Self.contributor(id: Self.alice)),
            projector: FixedProjector(projection: projection),
            spaceDescriptorEntryID: "d",
            now: { Self.fixedNow }
        )
        model.load()
        XCTAssertEqual(model.recentActivity, "Last posted 2h ago")
        // communityNewestMicros still reports the freshest ordering value (Bob's).
        XCTAssertEqual(model.communityNewestMicros, 200)
    }

    @MainActor
    func testModelHasNoRecentActivityLineWhenThePersonHasNoPosts() {
        // Honest empty state: a person known only through replies/editorial shows
        // NO recency line — never a fabricated one.
        let projection = Self.projection(open: [Self.post(id: "2", authorID: Self.bob)])
        let model = PersonDetailModel(
            person: PersonRow(Self.contributor(id: Self.alice, count: 3)),
            projector: FixedProjector(projection: projection),
            spaceDescriptorEntryID: "d",
            now: { Self.fixedNow }
        )
        model.load()
        XCTAssertEqual(model.state, .empty)
        XCTAssertNil(model.recentActivity)
    }

    @MainActor
    func testModelHasNoRecentActivityLineOnProjectionFailure() {
        let model = PersonDetailModel(
            person: PersonRow(Self.contributor(id: Self.alice)),
            projector: ThrowingProjector(),
            spaceDescriptorEntryID: "d",
            now: { Self.fixedNow }
        )
        model.load()
        XCTAssertNil(model.recentActivity)
    }

    // MARK: - Contribution summary names the community (#4)

    func testContributionSummaryNamesTheCommunityWhenKnown() {
        XCTAssertEqual(PeopleStrings.contributions(12, in: "Rojava Solidarity"), "12 contributions in Rojava Solidarity")
        XCTAssertEqual(PeopleStrings.contributions(1, in: "Rojava Solidarity"), "1 contribution in Rojava Solidarity")
    }

    func testContributionSummaryFallsBackToBareCountWithoutACommunityName() {
        XCTAssertEqual(PeopleStrings.contributions(4, in: ""), "4 contributions")
        XCTAssertEqual(PeopleStrings.contributions(4, in: "   "), "4 contributions")
    }

    @MainActor
    func testModelContributionSummaryUsesTheSuppliedCommunityName() {
        let model = PersonDetailModel(
            person: PersonRow(Self.contributor(id: Self.alice, count: 5)),
            projector: FixedProjector(projection: Self.projection()),
            spaceDescriptorEntryID: "d",
            communityName: "Rojava Solidarity"
        )
        XCTAssertEqual(model.contributionSummary, "5 contributions in Rojava Solidarity")
    }

    // MARK: - RelativeTime: the shared wall-clock "ago" formatter

    // A fixed anchor: Unix 1_000_000_000. Every case measures a creation instant
    // back from it, so the phrasing is deterministic (no reliance on Date()).
    private static let agoNow = Date(timeIntervalSince1970: 1_000_000_000)

    func testRelativeTimeCoversTheWholeAgoLadder() {
        func ago(_ secondsBack: Int) -> String? {
            RelativeTime.ago(unixSeconds: UInt64(1_000_000_000 - secondsBack), now: Self.agoNow)
        }
        XCTAssertEqual(ago(5), "just now")           // < 1 minute
        XCTAssertEqual(ago(59), "just now")
        XCTAssertEqual(ago(60), "1m ago")
        XCTAssertEqual(ago(2 * 3_600), "2h ago")
        XCTAssertEqual(ago(86_400), "yesterday")     // exactly one day
        XCTAssertEqual(ago(3 * 86_400), "3d ago")
        XCTAssertEqual(ago(10 * 86_400), "1w ago")   // weeks bucket
        // Older than a month falls back to an absolute date, never "38d ago".
        let old = RelativeTime.ago(unixSeconds: 1_000_000_000 - 60 * 86_400, now: Self.agoNow)
        XCTAssertNotNil(old)
        XCTAssertFalse(old!.hasSuffix("ago"), "a >1-month instant should read as a date, got \(old!)")
    }

    func testRelativeTimeIsNilForMissingOrZeroInstant() {
        XCTAssertNil(RelativeTime.ago(unixSeconds: nil, now: Self.agoNow))
        XCTAssertNil(RelativeTime.ago(unixSeconds: 0, now: Self.agoNow))
    }

    func testRelativeTimeClampsAFutureInstantToJustNow() {
        // Clock skew between peers can put a creation time slightly ahead of ours;
        // never print a negative age.
        XCTAssertEqual(
            RelativeTime.ago(unixSeconds: 1_000_000_000 + 500, now: Self.agoNow),
            "just now"
        )
    }

    // MARK: - Key-derived avatar initials (never all "ME")

    func testAvatarInitialsUseARealDisplayNameWhenPresent() {
        XCTAssertEqual(PersonAvatar.initials(displayName: "Ana Ng", keySeed: "deadbeef"), "AN")
        XCTAssertEqual(PersonAvatar.initials(displayName: "Rosa", keySeed: "deadbeef"), "RO")
    }

    func testNamelessAuthorsGetDistinctKeyDerivedInitialsNotAllME() {
        // Core's fallback name is the bare word "member" (surfaced as "Member").
        // Two nameless authors must NOT both read "ME" — the initials come from
        // their distinct keys.
        let a = PersonAvatar.initials(displayName: "member", keySeed: "a3f91122")
        let b = PersonAvatar.initials(displayName: "Member", keySeed: "7b02ccef")
        XCTAssertNotEqual(a, "ME")
        XCTAssertNotEqual(b, "ME")
        XCTAssertNotEqual(a, b, "two distinct keys must yield distinct initials")
        XCTAssertEqual(a, "A3")
        XCTAssertEqual(b, "7B")
    }

    func testAvatarInitialsBulletWhenNoNameAndKeyHasNoHexGlyphs() {
        // A nameless author whose key seed carries no hex digits (defensive: real
        // seeds are hex) gets a stable bullet, never a blank.
        XCTAssertEqual(PersonAvatar.initials(displayName: "", keySeed: ""), "•")
        XCTAssertEqual(PersonAvatar.initials(displayName: "member", keySeed: "----"), "•")
    }
}
