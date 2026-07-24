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
        treatment: NewswirePostTreatment = .ordinary,
        expires: UInt64? = nil
    ) -> NewswireProjectedPost {
        NewswireProjectedPost(
            entryId: id, author: author(authorID), taiJ2000Micros: tai,
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

    // MARK: - Recent activity: an HONEST recency, in the ordering domain

    // A projected post carries no wall-clock "posted at" — only the signed Willow
    // ordering value (TAI/J2000 µs). Recency is therefore expressed in that same
    // domain: how far a person's newest post sits behind the freshest update
    // visible in the community. A pure function of two ordering values — no epoch
    // conversion, no invented timestamp, deterministic under test.

    private static let hourMicros: UInt64 = 3_600 * 1_000_000
    private static let dayMicros: UInt64 = 86_400 * 1_000_000

    func testHeaderRecencyReflectsTheGapFromNewestPostToTheLatestUpdate() {
        // Person's newest post is three hours behind the community's freshest one.
        let person: UInt64 = 1_000_000_000
        let community = person + 3 * Self.hourMicros
        XCTAssertEqual(
            PersonActivity.headerRecency(personNewestMicros: person, communityNewestMicros: community),
            "Last posted 3 hours before the latest update"
        )
    }

    func testHeaderRecencyReadsAsLatestWhenThePersonHoldsTheFreshestUpdate() {
        let micros: UInt64 = 42_000_000
        XCTAssertEqual(
            PersonActivity.headerRecency(personNewestMicros: micros, communityNewestMicros: micros),
            PeopleStrings.mostRecentToPost
        )
    }

    func testHeaderRecencyIsNilWhenThePersonHasNoPosts() {
        // No posts → NO activity line at all. Absence is never dressed up as a
        // fabricated "active now".
        XCTAssertNil(
            PersonActivity.headerRecency(personNewestMicros: nil, communityNewestMicros: 10)
        )
    }

    func testRowRecencyLabelsTheFreshestRowAsTheLatestUpdate() {
        XCTAssertEqual(PersonActivity.rowRecency(rowMicros: 500, communityNewestMicros: 500), "Latest update")
        XCTAssertEqual(
            PersonActivity.rowRecency(rowMicros: 500, communityNewestMicros: 500 + Self.dayMicros),
            "1 day before the latest update"
        )
    }

    @MainActor
    func testModelExposesRecentActivityDerivedFromTheNewestPostOnLoad() {
        // Alice's newest post (tai 1_000_000_000) trails Bob's newer one by 2 hours.
        let aliceNewest: UInt64 = 1_000_000_000
        let bobNewest = aliceNewest + 2 * Self.hourMicros
        let projection = Self.projection(open: [
            Self.post(id: "a1", authorID: Self.alice, tai: aliceNewest),
            Self.post(id: "b1", authorID: Self.bob, tai: bobNewest),
        ])
        let model = PersonDetailModel(
            person: PersonRow(Self.contributor(id: Self.alice)),
            projector: FixedProjector(projection: projection),
            spaceDescriptorEntryID: "d"
        )
        model.load()
        XCTAssertEqual(model.recentActivity, "Last posted 2 hours before the latest update")
        XCTAssertEqual(model.communityNewestMicros, bobNewest)
    }

    @MainActor
    func testModelRecentActivityIsLatestWhenThePersonHoldsTheFreshestPost() {
        let projection = Self.projection(open: [
            Self.post(id: "a1", authorID: Self.alice, tai: 900),
            Self.post(id: "a2", authorID: Self.alice, tai: 1_000),
        ])
        let model = PersonDetailModel(
            person: PersonRow(Self.contributor(id: Self.alice)),
            projector: FixedProjector(projection: projection),
            spaceDescriptorEntryID: "d"
        )
        model.load()
        XCTAssertEqual(model.recentActivity, PeopleStrings.mostRecentToPost)
    }

    @MainActor
    func testModelHasNoRecentActivityLineWhenThePersonHasNoPosts() {
        // Honest empty state: a person known only through replies/editorial shows
        // NO recency line — never a fabricated one.
        let projection = Self.projection(open: [Self.post(id: "2", authorID: Self.bob)])
        let model = PersonDetailModel(
            person: PersonRow(Self.contributor(id: Self.alice, count: 3)),
            projector: FixedProjector(projection: projection),
            spaceDescriptorEntryID: "d"
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
            spaceDescriptorEntryID: "d"
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
}
