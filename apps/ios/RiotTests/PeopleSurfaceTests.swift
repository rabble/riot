import XCTest
@testable import RiotKit

/// Unit 1C — the People surface (Known contributors), tested in isolation (no
/// store, no FFI: the projector seam is stubbed). The contracts: this is
/// CONTRIBUTORS, never a membership roster or presence; every author is shown
/// RENDERED as `name · tag`, never a raw hex id; the organizer flag comes ONLY
/// from the core's coordinate rule, never from a name; an empty community shows
/// an actionable empty state; and a projection failure never leaks a raw error.
final class PeopleSurfaceTests: XCTestCase {
    // MARK: - Test doubles

    private final class StubProjector: NewswireContributorProjecting {
        var contributors: [NewswireContributor] = []
        var error: Error?
        private(set) var callCount = 0

        func projectNewswireContributors(
            spaceDescriptorEntryID: String
        ) throws -> [NewswireContributor] {
            callCount += 1
            if let error { throw error }
            return contributors
        }
    }

    private struct RawInternalError: Error {
        let description = "Internal { store: \"corrupt row 0x41\" }"
    }

    // MARK: - Fixtures

    private static func author(_ name: String, tag: String, id: String) -> NewswireAuthor {
        NewswireAuthor(id: id, displayName: name, tag: tag, rendered: "\(name) · \(tag)")
    }

    private static func contributor(
        _ name: String,
        tag: String = "a3f91122",
        id: String = String(repeating: "1", count: 64),
        organizer: Bool = false,
        count: UInt32 = 1
    ) -> NewswireContributor {
        NewswireContributor(
            author: author(name, tag: tag, id: id),
            isOrganizer: organizer,
            contributionCount: count
        )
    }

    @MainActor
    private func makeModel(
        projector: StubProjector,
        space: String = String(repeating: "d", count: 64)
    ) -> PeopleSurfaceModel {
        PeopleSurfaceModel(projector: projector, spaceDescriptorEntryID: space)
    }

    // MARK: - Contributors, not membership or presence

    func testSurfaceVocabularyNeverLabelsMembershipOrPresence() {
        // The surface's own fixed strings — never a rendered NAME, which may
        // legitimately fall back to "member · tag". A People surface that called
        // its rows "members" or showed "online" would be a roster, not the known
        // contributors this is.
        let vocabulary = [
            PeopleStrings.title,
            PeopleStrings.organizerBadge,
            PeopleStrings.emptyTitle,
            PeopleStrings.emptyMessage,
            PeopleStrings.emptyActionLabel,
            PeopleStrings.unavailableMessage,
            PeopleStrings.contributions(1),
            PeopleStrings.contributions(4),
        ]
        let forbidden = ["member", "online", "present", "presence", "roster", "logged in", "active now"]
        for text in vocabulary {
            let lowered = text.lowercased()
            for word in forbidden {
                XCTAssertFalse(
                    lowered.contains(word),
                    "People vocabulary must not imply membership/presence: \(text) contains \(word)"
                )
            }
        }
        // And it names itself for what it is.
        XCTAssertEqual(PeopleStrings.title, "Contributors")
    }

    func testStateHasNoPresenceConcept() {
        // A structural guarantee: a PersonRow simply has no online/last-seen
        // field to populate. The count is content-derived, singular-aware.
        let row = PersonRow(Self.contributor("Ana", count: 1))
        XCTAssertEqual(PeopleStrings.contributions(row.contributionCount), "1 contribution")
        XCTAssertEqual(PeopleStrings.contributions(4), "4 contributions")
    }

    // MARK: - Rendered names, never raw hex

    func testRowShowsRenderedNameAndTagNotRawId() {
        let id = String(repeating: "b", count: 64)
        let row = PersonRow(Self.contributor("Ana", tag: "a3f91122", id: id))
        XCTAssertEqual(row.rendered, "Ana · a3f91122")
        XCTAssertEqual(row.displayName, "Ana")
        XCTAssertEqual(row.tag, "a3f91122")
        // The raw id is carried for pinning/Technical details only — it is never
        // the display string.
        XCTAssertEqual(row.id, id)
        XCTAssertNotEqual(row.rendered, row.id)
        XCTAssertFalse(row.rendered.contains(id))
    }

    func testAccessibilityLabelSpeaksTheRenderedName() {
        let row = PersonRow(Self.contributor("Ana", tag: "a3f91122", organizer: true, count: 3))
        // The spoken line leads with the rendered name, states organizer as a
        // WORD (never color alone), and gives the content-derived count.
        XCTAssertEqual(row.accessibilityLabel, "Ana · a3f91122, Organizer, 3 contributions")
    }

    // MARK: - Organizer only from the coordinate

    func testOrganizerFlagIsTakenOnlyFromTheProjectedCoordinate() {
        // A contributor the core did NOT mark organizer stays a non-organizer
        // even if they named themselves "Organizer". The surface trusts the
        // coordinate flag, never the name.
        let impostor = PersonRow(Self.contributor("Organizer", organizer: false))
        XCTAssertFalse(impostor.isOrganizer)
        XCTAssertFalse(impostor.accessibilityLabel.contains(", \(PeopleStrings.organizerBadge),"))

        let recognized = PersonRow(Self.contributor("Ana", organizer: true))
        XCTAssertTrue(recognized.isOrganizer)
        XCTAssertTrue(recognized.accessibilityLabel.contains(PeopleStrings.organizerBadge))
    }

    func testStatePreservesProjectedOrderAndFlags() {
        // The core sorts organizer-first; the surface maps 1:1 and never
        // reorders or re-derives the flag.
        let contributors = [
            Self.contributor("Ana", id: String(repeating: "0", count: 64), organizer: true, count: 5),
            Self.contributor("Ben", id: String(repeating: "2", count: 64), organizer: false, count: 2),
            Self.contributor("Cal", id: String(repeating: "3", count: 64), organizer: false, count: 1),
        ]
        guard case let .populated(rows) = PeopleSurfaceState.from(contributors) else {
            return XCTFail("expected a populated surface")
        }
        XCTAssertEqual(rows.map(\.displayName), ["Ana", "Ben", "Cal"])
        XCTAssertEqual(rows.map(\.isOrganizer), [true, false, false])
        XCTAssertEqual(rows.filter(\.isOrganizer).count, 1)
    }

    // MARK: - Actionable empty state

    func testNoContributorsIsAnActionableEmptyStateNotABlankList() {
        guard case let .empty(empty) = PeopleSurfaceState.from([]) else {
            return XCTFail("an empty community must show the empty state, not .populated([])")
        }
        XCTAssertEqual(empty.title, "No contributors yet")
        XCTAssertFalse(empty.actionLabel.isEmpty)
        XCTAssertEqual(empty.actionLabel, "Post the first update")
        XCTAssertFalse(empty.message.isEmpty)
    }

    @MainActor
    func testModelLoadsEmptyProjectionAsTheActionableEmptyState() {
        let projector = StubProjector()
        let model = makeModel(projector: projector)
        model.load()
        XCTAssertEqual(projector.callCount, 1)
        XCTAssertEqual(model.state, .empty(.noContributors))
    }

    @MainActor
    func testModelLoadsProjectedContributors() {
        let projector = StubProjector()
        projector.contributors = [Self.contributor("Ana", organizer: true, count: 2)]
        let model = makeModel(projector: projector)
        model.load()
        guard case let .populated(rows) = model.state else {
            return XCTFail("expected populated")
        }
        XCTAssertEqual(rows.count, 1)
        XCTAssertEqual(rows[0].rendered, "Ana · a3f91122")
        XCTAssertTrue(rows[0].isOrganizer)
    }

    // MARK: - Never a raw internal error

    @MainActor
    func testProjectionFailureShowsFixedMessageNeverRawError() {
        let projector = StubProjector()
        projector.error = RawInternalError()
        let model = makeModel(projector: projector)
        model.load()
        guard case let .unavailable(message) = model.state else {
            return XCTFail("a failure must map to the fixed unavailable state")
        }
        XCTAssertEqual(message, PeopleStrings.unavailableMessage)
        XCTAssertFalse(message.contains("corrupt"))
        XCTAssertFalse(message.contains("Internal"))
    }
}
