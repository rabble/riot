import XCTest
@testable import RiotKit

/// Unit 2 — the wiring the `ShareCommunitySheet` drives. The sheet reads the
/// active community's `newswireDescriptorEntryID` and asks the repository to mint
/// the canonical share reference. A held descriptor yields a shareable link that
/// round-trips through the core decoder; a community whose descriptor id is `nil`
/// (joined, before first sync) yields the honest `.unavailable` state — never a
/// fabricated link, never a crash, and the resolver is never consulted.
@MainActor
final class ShareCommunitySheetTests: XCTestCase {
    func testHeldDescriptorSheetIsShareableAndRoundTrips() throws {
        let profile = try openLocalProfile()
        let space = try profile.createNewswireSpace(input: NewswireSpaceInput(
            name: "Harbor Assembly", summary: "A community newswire.",
            languages: ["en"], geographicTags: ["harbor"], topicTags: ["local"],
            editorialRoster: []))

        let community = CommunityContext(
            name: "Harbor Assembly",
            namespaceID: "harbor-namespace",
            newswireDescriptorEntryID: space.entryId,
            isOrganizer: true)

        let sheet = ShareCommunitySheet(
            community: community,
            resolveEncoded: { id in
                try profile.newswireShareReference(spaceDescriptorEntryId: id).encoded
            },
            onClose: {})

        guard case let .shareable(link) = sheet.content else {
            return XCTFail("a held descriptor must be shareable from the sheet")
        }
        XCTAssertTrue(link.hasPrefix("riot://newswire/join/v1/"))
        // The link the sheet shares is the same one Unit 1's decoder accepts.
        XCTAssertEqual(try newswireDecodeShareReference(encoded: link).encoded, link)
        // And it renders a QR locally (no network, no fabricated image).
        XCTAssertNotNil(QRImageRenderer.makeQRCode(from: link))
    }

    func testNilDescriptorSheetIsUnavailableAndNeverCallsResolver() {
        let community = CommunityContext(
            name: "Pending Community",
            namespaceID: "pending-namespace",
            newswireDescriptorEntryID: nil,
            isOrganizer: false)

        let sheet = ShareCommunitySheet(
            community: community,
            resolveEncoded: { _ in
                XCTFail("resolver must not be consulted when the descriptor id is nil")
                return ""
            },
            onClose: {})

        XCTAssertEqual(sheet.content, .unavailable)
    }
}
