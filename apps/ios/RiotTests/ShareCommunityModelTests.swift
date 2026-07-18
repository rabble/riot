import XCTest
@testable import RiotKit

@MainActor
final class ShareCommunityModelTests: XCTestCase {
    private struct ResolverFailed: Error {}

    func testHeldCommunityProducesAShareableRiotLink() throws {
        let profile = try openLocalProfile()
        let space = try profile.createNewswireSpace(input: NewswireSpaceInput(
            name: "Riverside Commons", summary: "A community newswire.",
            languages: ["en"], geographicTags: ["riverside"], topicTags: ["local"],
            editorialRoster: []))

        let model = ShareCommunityModel()
        let content = model.content(descriptorEntryID: space.entryId) { id in
            try profile.newswireShareReference(spaceDescriptorEntryId: id).encoded
        }

        guard case let .shareable(link) = content else {
            return XCTFail("a held descriptor must be shareable")
        }
        XCTAssertTrue(link.hasPrefix("riot://newswire/join/v1/"))
        // The shared link round-trips through the core decoder (anti-fabrication:
        // the string we hand out is the same one Unit 1 decodes back).
        XCTAssertEqual(try newswireDecodeShareReference(encoded: link).encoded, link)
        // And it renders a QR locally.
        XCTAssertNotNil(QRImageRenderer.makeQRCode(from: link))
    }

    func testUnknownDescriptorIsUnavailableNotAFabricatedLink() {
        let model = ShareCommunityModel()
        // nil descriptor id (a joined community before its descriptor is known)
        XCTAssertEqual(
            model.content(descriptorEntryID: nil, resolveEncoded: { _ in "x" }),
            .unavailable)
        // resolver throws (profile closed / descriptor not held) => unavailable, no crash
        XCTAssertEqual(
            model.content(descriptorEntryID: "deadbeef", resolveEncoded: { _ in throw ResolverFailed() }),
            .unavailable)
        // a resolver that returns a non-riot string is rejected (never shared)
        XCTAssertEqual(
            model.content(descriptorEntryID: "deadbeef", resolveEncoded: { _ in "https://evil.example/x" }),
            .unavailable)
    }
}
