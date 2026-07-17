import XCTest
@testable import RiotKit

/// Unit 1 — iOS surface, join by link / QR.
///
/// `JoinReferenceModel` is the pure, camera-free seam that decodes and validates a
/// pasted or scanned `riot://newswire/join/v1/...` reference, produces an HONEST
/// preview (namespace only — the reference carries no community name), and answers
/// the duplicate-join question. The live capture (`QRScannerView`) is not unit
/// tested; this model is, so paste/scan validation and the honest no-title preview
/// are provable without a device.
final class JoinReferenceTests: XCTestCase {
    /// A real reference minted by the FFI decodes to its coordinates, and the
    /// preview leads with the namespace — never a fabricated title, because the
    /// share reference carries none (anti-spoof).
    func testDecodeProducesHonestPreviewWithNoTitle() throws {
        let profile = try openLocalProfile()
        let space = try profile.createNewswireSpace(input: NewswireSpaceInput(
            name: "Riverside",
            summary: "s",
            languages: ["en"],
            geographicTags: [],
            topicTags: [],
            editorialRoster: []
        ))
        let ref = try profile.newswireShareReference(spaceDescriptorEntryId: space.entryId)

        let model = JoinReferenceModel()
        let preview = try model.preview(fromPastedString: ref.encoded)
        XCTAssertEqual(preview.namespaceIdHex, ref.namespaceId)
        XCTAssertNil(preview.title, "share ref carries no title; UI must not fabricate one")
        XCTAssertFalse(preview.shortNamespace.isEmpty)
        XCTAssertEqual(preview.encoded, ref.encoded)
    }

    /// A non-reference string, or a canonically-shaped but malformed reference, is
    /// refused rather than silently interpreted.
    func testMalformedStringSurfacesActionableError() {
        let model = JoinReferenceModel()
        XCTAssertThrowsError(try model.preview(fromPastedString: "https://example.com/nope"))
        XCTAssertThrowsError(try model.preview(fromPastedString: "riot://newswire/join/v1/abc"))
    }

    /// The scan path is fed hostile QR input, so it enforces the `riot://` scheme
    /// BEFORE handing anything to the decoder.
    func testNonRiotScannedPayloadRejected() {
        let model = JoinReferenceModel()
        XCTAssertThrowsError(try model.preview(fromScannedString: "WIFI:S:foo;;")) { error in
            XCTAssertEqual(error as? JoinReferenceError, .notARiotJoinLink)
        }
    }

    /// A too-long payload (either path) is refused before any work — a QR code (or a
    /// pasted blob) cannot make the app decode an unbounded string.
    func testTooLongPayloadRejected() {
        let model = JoinReferenceModel()
        let huge = "riot://newswire/join/v1/" + String(repeating: "a", count: 5000)
        XCTAssertThrowsError(try model.preview(fromScannedString: huge)) { error in
            XCTAssertEqual(error as? JoinReferenceError, .tooLong)
        }
        XCTAssertThrowsError(try model.preview(fromPastedString: huge)) { error in
            XCTAssertEqual(error as? JoinReferenceError, .tooLong)
        }
    }

    /// The duplicate-join guard: a namespace already held routes to a switch instead
    /// of a second row.
    func testDuplicateJoinIsDetected() {
        let model = JoinReferenceModel()
        let existing = [CommunityRowStub(namespaceId: "abc123")]
        XCTAssertTrue(model.isAlreadyJoined(namespaceIdHex: "abc123", within: existing.map(\.namespaceId)))
        XCTAssertFalse(model.isAlreadyJoined(namespaceIdHex: "zzz999", within: existing.map(\.namespaceId)))
    }
}

/// A minimal stand-in for a held-community row: the duplicate check only needs the
/// namespace id, so the test does not construct a full `CommunityRow`.
private struct CommunityRowStub {
    let namespaceId: String
}
