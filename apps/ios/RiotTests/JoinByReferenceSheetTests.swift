import XCTest
@testable import RiotKit

/// Unit 1 — the join-by-reference commit path the `JoinByReferenceSheet` drives.
///
/// The sheet decodes a pasted/scanned string with `JoinReferenceModel`, shows an
/// honest namespace-only preview, and on confirm calls `RiotAppModel.commitJoin`.
/// That method is what these tests exercise against the real FFI: a first join lands
/// a PENDING member community (no fabricated title), and re-committing an
/// already-held reference SWITCHES to it rather than minting a duplicate row.
@MainActor
final class JoinByReferenceSheetTests: XCTestCase {
    func testJoiningAReferenceCreatesAPendingMemberCommunity() throws {
        let dir = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: dir) }
        let model = RiotAppModel()
        model.bootstrap(storageDirectory: dir, keyStore: TestWrappingKeyStore(), starterPacks: [])

        model.createSpace(title: "Community A")
        XCTAssertEqual(model.community?.name, "Community A")

        // A second namespace to follow, minted by a throwaway profile.
        let origin = try openLocalProfile()
        let b = try origin.createPublicSpace(title: "Community B")
        let preview = try JoinReferenceModel().preview(
            fromPastedString: Self.shareReference(forNamespace: b.namespaceId)
        )
        // The preview is honest: coordinates only, never a name.
        XCTAssertNil(preview.title)

        model.commitJoin(preview: preview)

        XCTAssertNil(model.errorMessage, "a valid reference joins without error")
        XCTAssertEqual(model.communities.count, 2, "both communities are held")
        XCTAssertEqual(model.community?.namespaceID, b.namespaceId, "the shell is now on the joined community")
        XCTAssertFalse(model.community?.isOrganizer ?? true, "the joiner is a member, not an organizer")

        // No fabricated name pre-sync: the joined row carries the provisional
        // placeholder, not a real community name (which arrives on first sync).
        let joined = model.communities.first { $0.namespaceID == b.namespaceId }
        XCTAssertTrue(
            joined?.name.hasPrefix("New community") ?? false,
            "pre-sync, the joined community shows the honest provisional label, not a real name"
        )
    }

    func testJoiningAnAlreadyJoinedReferenceSwitchesInsteadOfDuplicating() throws {
        let dir = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: dir) }
        let model = RiotAppModel()
        model.bootstrap(storageDirectory: dir, keyStore: TestWrappingKeyStore(), starterPacks: [])

        model.createSpace(title: "Community A")

        let origin = try openLocalProfile()
        let b = try origin.createPublicSpace(title: "Community B")
        let preview = try JoinReferenceModel().preview(
            fromPastedString: Self.shareReference(forNamespace: b.namespaceId)
        )

        model.commitJoin(preview: preview)
        XCTAssertEqual(model.communities.count, 2, "the first join adds B")

        // Move off B, then re-commit the SAME reference.
        model.switchCommunity(namespaceID: model.communities.first { $0.name == "Community A" }!.namespaceID)
        XCTAssertEqual(model.community?.name, "Community A")

        model.commitJoin(preview: preview)

        XCTAssertEqual(model.communities.count, 2, "re-committing a held reference does NOT add a second row")
        XCTAssertEqual(model.community?.namespaceID, b.namespaceId, "it switches to the already-held community instead")
    }

    // MARK: - Helpers

    private static func shareReference(forNamespace namespace: String) throws -> String {
        try newswireEncodeShareReference(
            namespaceId: namespace,
            descriptorEntryId: String(repeating: "1", count: 64),
            contentDigest: String(repeating: "2", count: 64)
        )
    }

    private static func temporaryProfileDirectory() throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("riot-join-sheet-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }
}

private final class TestWrappingKeyStore: WrappingKeyStore {
    private var key = Data(repeating: 0x42, count: 32)
    func loadOrCreateWrappingKey() throws -> Data { key }
}
