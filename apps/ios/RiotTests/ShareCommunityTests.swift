import XCTest
@testable import RiotKit

/// Share-a-community: the logic behind the QR + link screen. The share flow's
/// value is that a real held descriptor becomes a scannable, copyable canonical
/// link; these tests pin the three seams that carry that without a device —
/// the QR renders, the resolver surfaces the encoded link, and the missing /
/// failing descriptor paths stay honest rather than showing a broken code.
@MainActor
final class ShareCommunityTests: XCTestCase {
    /// A stub descriptor→reference minter, so the resolver and view model are
    /// provable without a live profile or the FFI.
    private struct StubReferencing: NewswireShareReferencing {
        var result: Result<NewswireShareReference, Error>
        func newswireShareReference(spaceDescriptorEntryID: String) throws -> NewswireShareReference {
            try result.get()
        }
    }

    private struct MintFailure: Error {}

    /// Wraps a live core `MobileProfile` behind the share seam, so the round-trip
    /// test exercises the REAL `riot-core` encoder (not a stub) through the exact
    /// protocol the screen consumes.
    private struct LiveReferencing: NewswireShareReferencing {
        let profile: MobileProfile
        func newswireShareReference(spaceDescriptorEntryID: String) throws -> NewswireShareReference {
            try profile.newswireShareReference(spaceDescriptorEntryId: spaceDescriptorEntryID)
        }
    }

    private func reference(encoded: String) -> NewswireShareReference {
        NewswireShareReference(
            namespaceId: "aa",
            descriptorEntryId: "bb",
            contentDigest: String(repeating: "0", count: 64),
            encoded: encoded
        )
    }

    // MARK: - QR generation (pure CoreImage)

    func testKnownShareStringProducesQRImage() throws {
        let link = "riot://newswire/join/v1/AAAA.BBBB.CCCC"
        let image = try XCTUnwrap(
            CommunityQRCode.cgImage(for: link),
            "a non-empty share link must render a QR image"
        )
        // A real QR is a square block grid, not a zero-sized image.
        XCTAssertGreaterThan(image.width, 0)
        XCTAssertEqual(image.width, image.height)
    }

    func testEmptyStringProducesNoQRImage() {
        XCTAssertNil(CommunityQRCode.cgImage(for: ""))
    }

    // MARK: - Resolver + view model

    func testResolverSurfacesEncodedLink() {
        let link = "riot://newswire/join/v1/AAAA.BBBB.CCCC"
        let outcome = ShareCommunityResolver.resolve(
            spaceDescriptorEntryID: "descriptor-1",
            referencing: StubReferencing(result: .success(reference(encoded: link)))
        )
        XCTAssertEqual(outcome, .ready(link: link))
    }

    func testViewModelSurfacesEncodedLink() {
        let link = "riot://newswire/join/v1/AAAA.BBBB.CCCC"
        let model = ShareCommunityModel(
            communityName: "Riverside Commons",
            spaceDescriptorEntryID: "descriptor-1",
            referencing: StubReferencing(result: .success(reference(encoded: link)))
        )
        XCTAssertEqual(model.communityName, "Riverside Commons")
        XCTAssertEqual(model.outcome, .ready(link: link))
        XCTAssertEqual(model.shareLink, link)
    }

    // MARK: - Honest error paths

    func testMissingDescriptorIsUnavailableWithoutCallingMinter() {
        // A blank descriptor id (a joined community pending first sync) must not
        // even attempt to mint — it can't be shared yet, honestly.
        let outcome = ShareCommunityResolver.resolve(
            spaceDescriptorEntryID: "   ",
            referencing: StubReferencing(result: .failure(MintFailure()))
        )
        XCTAssertEqual(outcome, .unavailable(message: ShareCommunityResolver.missingDescriptorMessage))
    }

    func testMintFailureIsUnavailableWithFixedCopy() {
        let model = ShareCommunityModel(
            communityName: "Riverside Commons",
            spaceDescriptorEntryID: "descriptor-1",
            referencing: StubReferencing(result: .failure(MintFailure()))
        )
        XCTAssertEqual(model.outcome, .unavailable(message: ShareCommunityResolver.mintFailureMessage))
        XCTAssertNil(model.shareLink)
    }

    /// The end-to-end anchor: a real held descriptor mints a reference whose
    /// canonical `encoded` link both renders as a QR and is what the view model
    /// hands to Copy/Share. Uses the live `riot-core` encoder linked into RiotKit,
    /// not a stub, so the screen shares exactly what core signed.
    func testRealDescriptorRoundTripsThroughShareModel() throws {
        let profile = try openLocalProfile()
        let space = try profile.createNewswireSpace(input: NewswireSpaceInput(
            name: "Riverside Commons",
            summary: "A community newswire.",
            languages: ["en"],
            geographicTags: ["riverside"],
            topicTags: ["local"],
            editorialRoster: []
        ))
        let model = ShareCommunityModel(
            communityName: "Riverside Commons",
            spaceDescriptorEntryID: space.entryId,
            referencing: LiveReferencing(profile: profile)
        )
        let link = try XCTUnwrap(model.shareLink)
        XCTAssertTrue(link.hasPrefix("riot://newswire/join/v1/"))
        XCTAssertNotNil(CommunityQRCode.cgImage(for: link))
    }
}
