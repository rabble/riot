import XCTest
@testable import RiotKit

/// Unit 1E — newswire merge & share, iOS half.
///
/// The committed cross-platform golden vector (`fixtures/newswire/newswire-golden-1.json`,
/// registered in this hostless test bundle's Resources phase and read via
/// `Bundle(for:)`) is the byte-identity anchor. iOS runs the SAME `riot-core`
/// encoder that Rust does — linked into `RiotKit` — so reproducing the fixture's
/// content digest and share-reference string here proves iOS and Rust encode the
/// identical record, not merely that the fixture is well-formed.
///
/// It also exercises the real share flow: a held descriptor mints a digest-bound
/// reference whose `content_digest` binds the descriptor's canonical bytes, so a
/// substituted community name or roster is detectable.
@MainActor
final class NewswireShareTests: XCTestCase {
    private func golden() throws -> [String: Any] {
        let url = try XCTUnwrap(
            Bundle(for: Self.self).url(forResource: "newswire-golden-1", withExtension: "json"),
            "newswire-golden-1.json is not bundled into the test resources"
        )
        let data = try Data(contentsOf: url)
        return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    private func descriptorInput(_ doc: [String: Any]) throws -> (NewswireSpaceInput, String) {
        let d = try XCTUnwrap(doc["descriptor"] as? [String: Any])
        let input = NewswireSpaceInput(
            name: try XCTUnwrap(d["name"] as? String),
            summary: try XCTUnwrap(d["summary"] as? String),
            languages: try XCTUnwrap(d["languages"] as? [String]),
            geographicTags: try XCTUnwrap(d["geographic_tags"] as? [String]),
            topicTags: try XCTUnwrap(d["topic_tags"] as? [String]),
            editorialRoster: try XCTUnwrap(d["editorial_roster_hex"] as? [String])
        )
        return (input, try XCTUnwrap(d["namespace_id_hex"] as? String))
    }

    /// iOS's native encoder reproduces the committed descriptor content digest
    /// byte-for-byte — a shared WILLIAM3 digest implies shared canonical CBOR, so
    /// iOS and Rust encode the identical descriptor record.
    func testGoldenDescriptorContentDigestMatchesFixture() throws {
        let doc = try golden()
        let (input, namespaceId) = try descriptorInput(doc)
        let digest = try newswireDescriptorContentDigest(input: input, namespaceId: namespaceId)
        let expected = try XCTUnwrap(
            (doc["descriptor"] as? [String: Any])?["content_digest_hex"] as? String
        )
        XCTAssertEqual(digest, expected)
    }

    /// iOS reproduces the committed share-reference string, and decodes it back to
    /// the same coordinates.
    func testGoldenShareReferenceStringMatchesFixture() throws {
        let doc = try golden()
        let s = try XCTUnwrap(doc["share_reference"] as? [String: Any])
        let namespaceId = try XCTUnwrap(s["namespace_id_hex"] as? String)
        let entryId = try XCTUnwrap(s["descriptor_entry_id_hex"] as? String)
        let digest = try XCTUnwrap(s["content_digest_hex"] as? String)
        let expected = try XCTUnwrap(s["encoded"] as? String)

        let encoded = try newswireEncodeShareReference(
            namespaceId: namespaceId,
            descriptorEntryId: entryId,
            contentDigest: digest
        )
        XCTAssertEqual(encoded, expected)

        let decoded = try newswireDecodeShareReference(encoded: encoded)
        XCTAssertEqual(decoded.namespaceId, namespaceId)
        XCTAssertEqual(decoded.descriptorEntryId, entryId)
        XCTAssertEqual(decoded.contentDigest, digest)
        XCTAssertEqual(decoded.encoded, expected)
    }

    /// The real share flow: create a community, mint its digest-bound reference,
    /// and round-trip the encoded string. Two distinct communities never collide
    /// on the content digest — the anti-substitution binding.
    func testHeldDescriptorMintsVerifiableShareReference() throws {
        let profile = try openLocalProfile()
        let space = try profile.createNewswireSpace(input: NewswireSpaceInput(
            name: "Riverside Commons",
            summary: "A community newswire.",
            languages: ["en"],
            geographicTags: ["riverside"],
            topicTags: ["local"],
            editorialRoster: []
        ))

        let reference = try profile.newswireShareReference(spaceDescriptorEntryId: space.entryId)
        XCTAssertEqual(reference.descriptorEntryId, space.entryId)
        XCTAssertEqual(reference.contentDigest.count, 64)
        XCTAssertTrue(reference.encoded.hasPrefix("riot://newswire/join/v1/"))

        let decoded = try newswireDecodeShareReference(encoded: reference.encoded)
        XCTAssertEqual(decoded.namespaceId, reference.namespaceId)
        XCTAssertEqual(decoded.descriptorEntryId, reference.descriptorEntryId)
        XCTAssertEqual(decoded.contentDigest, reference.contentDigest)

        let other = try profile.createNewswireSpace(input: NewswireSpaceInput(
            name: "Harbor Commons",
            summary: "A different community newswire.",
            languages: ["en"],
            geographicTags: ["harbor"],
            topicTags: ["local"],
            editorialRoster: []
        ))
        let otherReference = try profile.newswireShareReference(
            spaceDescriptorEntryId: other.entryId
        )
        XCTAssertNotEqual(otherReference.contentDigest, reference.contentDigest)
        XCTAssertNotEqual(otherReference.descriptorEntryId, reference.descriptorEntryId)
    }

    /// A mangled or foreign-scheme string is rejected, never silently decoded.
    func testDecodeRejectsMalformedReference() {
        XCTAssertThrowsError(
            try newswireDecodeShareReference(encoded: "https://example.com/not-a-reference")
        )
        XCTAssertThrowsError(
            try newswireDecodeShareReference(encoded: "riot://newswire/join/v1/abc")
        )
    }
}
