import XCTest
@testable import RiotKit

/// Repository-layer tests for the signed-JS-apps surface: starter install on
/// open, host-side trust gating, trust and app-data persistence across reopen,
/// resource serving, and display names.
///
/// App-data now survives relaunch via `appDataPutWithReceipt` receipts persisted
/// in the snapshot and replayed on open (Task 10's relaunch-persistence
/// contract), so the previously-gated reopen test is exercised for real here.
final class AppRepositoryTests: XCTestCase {
    // MARK: - Fixtures

    /// Repo root derived from this file at `apps/ios/RiotTests/…` (four levels
    /// up), so the frozen starter artifacts load without a bundle resource.
    private static func repoRoot(file: StaticString = #filePath) -> URL {
        URL(fileURLWithPath: "\(file)")
            .deletingLastPathComponent() // RiotTests
            .deletingLastPathComponent() // ios
            .deletingLastPathComponent() // apps
            .deletingLastPathComponent() // repo root
    }

    private func starterPacks() throws -> [(manifest: Data, bundle: Data)] {
        let apps = Self.repoRoot().appendingPathComponent("fixtures/apps")
        let manifest = try Data(contentsOf: apps.appendingPathComponent("checklist.manifest.cbor"))
        let bundle = try Data(contentsOf: apps.appendingPathComponent("checklist.bundle.cbor"))
        return [(manifest: manifest, bundle: bundle)]
    }

    private func makeStorage(_ label: String) throws -> ProtectedProfileStorage {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("app-repo-\(label)-\(UUID().uuidString).json")
        return try ProtectedProfileStorage(fileURL: url)
    }

    // MARK: - Listing + trust

    func testStarterChecklistListsPendingThenTrusts() throws {
        let repository = try RiotProfileRepository.open(
            storage: try makeStorage("list"),
            keyStore: TestWrappingKeyStore(),
            starterPacks: try starterPacks()
        )
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")

        let before = try repository.spaceApps()
        XCTAssertEqual(before.count, 1)
        XCTAssertEqual(before[0].name, "Checklist")
        XCTAssertEqual(before[0].version, "1.0.0")
        XCTAssertFalse(before[0].permissions.isEmpty)
        XCTAssertFalse(before[0].trusted)

        try repository.trustApp(appID: before[0].appIDHex)
        let after = try repository.spaceApps()
        XCTAssertTrue(after[0].trusted)
    }

    func testSpaceAppsEmptyBeforeJoiningSpace() throws {
        let repository = try RiotProfileRepository.open(
            storage: try makeStorage("no-space"),
            keyStore: TestWrappingKeyStore(),
            starterPacks: try starterPacks()
        )
        XCTAssertEqual(try repository.spaceApps(), [])
    }

    func testTrustSurvivesReopen() throws {
        let storage = try makeStorage("reopen")
        let keyStore = TestWrappingKeyStore()
        let packs = try starterPacks()

        let first = try RiotProfileRepository.open(
            storage: storage,
            keyStore: keyStore,
            starterPacks: packs
        )
        _ = try first.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try first.spaceApps()[0].appIDHex
        try first.trustApp(appID: appID)

        // A fresh Rust profile (trust is in-memory there); the repository must
        // re-apply the persisted trust decision after re-installing the packs.
        let second = try RiotProfileRepository.open(
            storage: storage,
            keyStore: keyStore,
            starterPacks: packs
        )
        XCTAssertTrue(try second.spaceApps()[0].trusted)
    }

    // MARK: - Turning an app off

    /// The mirror of `testTrustSurvivesReopen`, and the reason a revoke has to
    /// reach DISK: Rust's trust is in-memory, so `open` re-applies whatever the
    /// snapshot still lists. A revoke that never left the session would be
    /// silently undone by the next launch — the app would come back on.
    func testUntrustDropsTheAppFromDiskAndItStaysOffAcrossARelaunch() throws {
        let snapshotURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("app-repo-untrust-\(UUID().uuidString).json")
        let keyStore = TestWrappingKeyStore()
        let packs = try starterPacks()

        let first = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: snapshotURL),
            keyStore: keyStore,
            starterPacks: packs
        )
        _ = try first.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try first.spaceApps()[0].appIDHex
        try first.trustApp(appID: appID)
        XCTAssertEqual(
            try trustedAppIDs(in: snapshotURL).map { $0.lowercased() },
            [appID.lowercased()],
            "the trust decision is on disk — the premise of this test"
        )

        try first.untrustApp(appID: appID)

        XCTAssertFalse(try first.spaceApps()[0].trusted, "the live session sees it off at once")
        XCTAssertEqual(try trustedAppIDs(in: snapshotURL), [], "and the decision left the disk")
        XCTAssertNil(
            first.appDataBridge(appID: appID),
            "a revoked app loses its data bridge — the launch gate closes with it"
        )

        // A fresh Rust profile. If the revoke had not reached disk, `open` would
        // re-trust the app here and it would be back on.
        let reopened = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: snapshotURL),
            keyStore: keyStore,
            starterPacks: packs
        )
        XCTAssertFalse(try reopened.spaceApps()[0].trusted, "a revoked app stays off across a relaunch")
    }

    /// Revoking an app nobody ever turned on is a no-op, not a write and not a
    /// throw: the snapshot is left exactly as it was.
    func testUntrustingAnAppThatWasNeverTrustedChangesNothing() throws {
        let snapshotURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("app-repo-untrust-noop-\(UUID().uuidString).json")
        let repository = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: snapshotURL),
            keyStore: TestWrappingKeyStore(),
            starterPacks: try starterPacks()
        )
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try repository.spaceApps()[0].appIDHex

        try repository.untrustApp(appID: appID)

        XCTAssertFalse(try repository.spaceApps()[0].trusted)
        XCTAssertEqual(try trustedAppIDs(in: snapshotURL), [])
    }

    // MARK: - Resource serving

    func testAppResourceServesEntryPointAndRefusesEscapes() throws {
        let repository = try RiotProfileRepository.open(
            storage: try makeStorage("resource"),
            keyStore: TestWrappingKeyStore(),
            starterPacks: try starterPacks()
        )
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try repository.spaceApps()[0].appIDHex

        let index = try repository.appResource(appID: appID, path: "index.html")
        XCTAssertEqual(index.contentType, "text/html")
        XCTAssertFalse(index.bytes.isEmpty)

        XCTAssertThrowsError(try repository.appResource(appID: appID, path: "../escape"))
        XCTAssertThrowsError(try repository.appResource(appID: "deadbeef", path: "index.html"))
    }

    // MARK: - App-data bridge trust gate

    func testAppDataBridgeGatedOnTrust() throws {
        let repository = try RiotProfileRepository.open(
            storage: try makeStorage("bridge"),
            keyStore: TestWrappingKeyStore(),
            starterPacks: try starterPacks()
        )
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try repository.spaceApps()[0].appIDHex

        XCTAssertNil(repository.appDataBridge(appID: appID))
        try repository.trustApp(appID: appID)
        XCTAssertNotNil(repository.appDataBridge(appID: appID))
    }

    // MARK: - Silent exclusion

    func testCorruptedStarterPairIsSilentlySkipped() throws {
        var (manifest, bundle) = try XCTUnwrap(try starterPacks().first)
        // Corrupt the bundle's outer CBOR map header so it no longer decodes as
        // a bundle. (Flipping a *content* byte would not work: app ids are
        // content-derived, so that just yields a different valid app.) Rust's
        // installApp rejects the structurally broken bytes and the pair is
        // silently excluded.
        bundle[bundle.startIndex] ^= 0xFF

        let repository = try RiotProfileRepository.open(
            storage: try makeStorage("corrupt"),
            keyStore: TestWrappingKeyStore(),
            starterPacks: [(manifest: manifest, bundle: bundle)]
        )
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")
        XCTAssertEqual(try repository.spaceApps(), [])
    }

    // MARK: - Codable backward compatibility

    func testOpensSnapshotWrittenBeforeTrustedAppIDsField() throws {
        let snapshotURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("app-repo-legacy-\(UUID().uuidString).json")
        let keyStore = TestWrappingKeyStore()
        let packs = try starterPacks()

        // First open writes a snapshot (with the new field); strip it to
        // emulate a pre-`trustedAppIDs` snapshot, then reopen.
        let first = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: snapshotURL),
            keyStore: keyStore,
            starterPacks: packs
        )
        _ = try first.createPublicSpace(title: "Berlin Mutual Aid")
        try removeTrustedAppIDs(from: snapshotURL)

        let reopened = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: snapshotURL),
            keyStore: keyStore,
            starterPacks: packs
        )
        // Decodes cleanly with an empty trust list; nothing trusted yet.
        XCTAssertFalse(try reopened.spaceApps()[0].trusted)
    }

    // MARK: - App-data persistence across reopen

    func testAppDataSurvivesReopen() throws {
        let storage = try makeStorage("appdata-reopen")
        let keyStore = TestWrappingKeyStore()
        let packs = try starterPacks()

        let first = try RiotProfileRepository.open(
            storage: storage, keyStore: keyStore, starterPacks: packs
        )
        _ = try first.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try first.spaceApps()[0].appIDHex
        try first.appDataPut(appID: appID, key: "note", valueJSON: "\"hello\"")

        // A fresh Rust session (app-data is in-memory there); the repository must
        // replay the persisted receipt on open so the value comes back.
        let second = try RiotProfileRepository.open(
            storage: storage, keyStore: keyStore, starterPacks: packs
        )
        XCTAssertEqual(try second.appDataGet(appID: appID, key: "note"), "\"hello\"")
    }

    func testBridgePutPersistsAcrossReopen() throws {
        let storage = try makeStorage("bridge-reopen")
        let keyStore = TestWrappingKeyStore()
        let packs = try starterPacks()

        let first = try RiotProfileRepository.open(
            storage: storage, keyStore: keyStore, starterPacks: packs
        )
        _ = try first.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try first.spaceApps()[0].appIDHex
        try first.trustApp(appID: appID)

        // Write through the WebView-facing bridge (its `onPut` closure routes to
        // the repository's persisting path), not the repository directly — this
        // is the wiring the app actually uses.
        let bridge = try XCTUnwrap(first.appDataBridge(appID: appID))
        try bridge.put(key: "note", valueJSON: "\"from-bridge\"")

        let second = try RiotProfileRepository.open(
            storage: storage, keyStore: keyStore, starterPacks: packs
        )
        XCTAssertEqual(try second.appDataGet(appID: appID, key: "note"), "\"from-bridge\"")
    }

    func testOpensSnapshotWrittenBeforeAppDataBundlesField() throws {
        let snapshotURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("app-repo-legacy-appdata-\(UUID().uuidString).json")
        let keyStore = TestWrappingKeyStore()
        let packs = try starterPacks()

        let first = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: snapshotURL),
            keyStore: keyStore,
            starterPacks: packs
        )
        _ = try first.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try first.spaceApps()[0].appIDHex
        try first.appDataPut(appID: appID, key: "note", valueJSON: "\"hi\"")
        // Emulate a pre-`appDataBundles` snapshot by stripping the field.
        try removeKey("appDataBundles", from: snapshotURL)

        // Decodes cleanly with an empty bundle list; nothing to replay, so the
        // value is simply absent rather than the open failing.
        let reopened = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: snapshotURL),
            keyStore: keyStore,
            starterPacks: packs
        )
        XCTAssertNil(try reopened.appDataGet(appID: appID, key: "note"))
    }

    func testCorruptPersistedBundleDoesNotPreventOpen() throws {
        let snapshotURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("app-repo-appdata-corrupt-\(UUID().uuidString).json")
        let keyStore = TestWrappingKeyStore()
        let packs = try starterPacks()

        let first = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: snapshotURL),
            keyStore: keyStore,
            starterPacks: packs
        )
        _ = try first.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try first.spaceApps()[0].appIDHex
        try first.appDataPut(appID: appID, key: "a", valueJSON: "\"first\"")
        try first.appDataPut(appID: appID, key: "b", valueJSON: "\"second\"")
        // Wedge a garbage bundle between the two healthy receipts.
        try insertGarbageBundle(into: snapshotURL, at: 1)

        // The corrupt element is skipped on replay; the healthy bundles on either
        // side of it still commit, so open succeeds and both values are present.
        let reopened = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: snapshotURL),
            keyStore: keyStore,
            starterPacks: packs
        )
        XCTAssertEqual(try reopened.appDataGet(appID: appID, key: "a"), "\"first\"")
        XCTAssertEqual(try reopened.appDataGet(appID: appID, key: "b"), "\"second\"")
    }

    // MARK: - Identity

    /// The contract an app depends on: `whoami()` hands over a STABLE id to
    /// store, and the name is only a currently-drawable claim beside it.
    func testWhoamiGivesAStableIDPlusTheTwoHalvesToDrawIt() throws {
        let repository = try RiotProfileRepository.open(
            storage: try makeStorage("displayname"),
            keyStore: TestWrappingKeyStore(),
            starterPacks: try starterPacks()
        )
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try repository.spaceApps()[0].appIDHex
        try repository.trustApp(appID: appID)
        let bridge = try XCTUnwrap(repository.appDataBridge(appID: appID))

        let me = bridge.whoami()
        XCTAssertEqual(me.idHex.count, 64, "the id an app stores is the 32-byte subspace id, in hex")
        XCTAssertTrue(me.idHex.allSatisfy { $0.isHexDigit && !$0.isUppercase }, "lowercase hex: \(me.idHex)")
        // No name claimed yet, so the fallback — and the tag is the id's own
        // first 8 hex, which is what makes the pair hard to impersonate.
        XCTAssertEqual(me.displayName, "member")
        XCTAssertEqual(me.tag, String(me.idHex.prefix(8)))
    }

    /// An author whose profile has not synced here yet is a normal peer, not a
    /// failure: the row still has to draw. Only a malformed id is an error.
    func testProfileFallsBackForUnknownIDsAndRejectsMalformedOnes() throws {
        let repository = try RiotProfileRepository.open(
            storage: try makeStorage("profile-unknown"),
            keyStore: TestWrappingKeyStore(),
            starterPacks: try starterPacks()
        )
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try repository.spaceApps()[0].appIDHex
        try repository.trustApp(appID: appID)
        let bridge = try XCTUnwrap(repository.appDataBridge(appID: appID))

        let stranger = String(repeating: "ab", count: 32)
        let unknown = try XCTUnwrap(bridge.profile(idHex: stranger), "an unsynced peer must still draw")
        XCTAssertEqual(unknown.displayName, "member")
        XCTAssertEqual(unknown.tag, "abababab")

        XCTAssertNil(bridge.profile(idHex: "not-hex"))
        XCTAssertNil(bridge.profile(idHex: "abcd"), "a wrong-length id is a caller bug")
        XCTAssertNil(bridge.profile(idHex: ""))
    }

    // MARK: - Naming yourself

    /// The whole point of claiming a name: it is still yours next time.
    ///
    /// Core's profile store is in-memory per session and `set_display_name` hands
    /// back no bundle to replay, so without the persisted claim this comes back
    /// `member · <tag>` — to this person and to everyone they sync with.
    func testDisplayNameSurvivesReopen() throws {
        let storage = try makeStorage("my-name")
        let keyStore = TestWrappingKeyStore()

        let first = try RiotProfileRepository.open(
            storage: storage, keyStore: keyStore, starterPacks: []
        )
        XCTAssertNil(first.claimedName, "nobody has claimed a name yet")
        XCTAssertEqual(try first.me().displayName, "member")

        try first.setDisplayName("Ana")
        let named = try first.me()
        XCTAssertEqual(named.rendered, "Ana · \(named.tag)", "the name is shown WITH the key-derived tag")

        let reopened = try RiotProfileRepository.open(
            storage: storage, keyStore: keyStore, starterPacks: []
        )
        XCTAssertEqual(try reopened.me().rendered, named.rendered, "a name claimed once is still yours after a relaunch")
        XCTAssertEqual(reopened.claimedName, "Ana", "and the field they typed it into starts where they left it")
    }

    /// Joining someone else's space REGENERATES the author, which orphans the
    /// profile card written under the old subspace. Unless the claim is
    /// re-asserted, the person who just named themselves walks into the space they
    /// joined as `member · <a different tag>` — nameless, on every row they sign.
    func testDisplayNameSurvivesJoiningSomeoneElsesSpace() throws {
        let host = try RiotProfileRepository.open(
            storage: try makeStorage("name-host"),
            keyStore: TestWrappingKeyStore(),
            starterPacks: []
        )
        let space = try host.createPublicSpace(title: "Berlin Mutual Aid")

        let storage = try makeStorage("name-joiner")
        let keyStore = TestWrappingKeyStore()
        let joiner = try RiotProfileRepository.open(
            storage: storage, keyStore: keyStore, starterPacks: []
        )
        try joiner.setDisplayName("Ana")
        let beforeJoin = try joiner.me()

        try joiner.joinSpace(space)

        let afterJoin = try joiner.me()
        XCTAssertNotEqual(afterJoin.id, beforeJoin.id, "the join regenerated the author — the premise of this test")
        XCTAssertEqual(afterJoin.displayName, "Ana", "the name follows the person through the join")
        XCTAssertEqual(afterJoin.rendered, "Ana · \(afterJoin.tag)")

        let reopened = try RiotProfileRepository.open(
            storage: storage, keyStore: keyStore, starterPacks: []
        )
        XCTAssertEqual(
            try reopened.me().rendered,
            afterJoin.rendered,
            "and it is still theirs after a relaunch inside the joined space"
        )
    }

    /// Core is the single enforcement point for what a name may be, and a name it
    /// refuses is never written to disk — so a relaunch cannot resurrect one.
    func testNameCoreRefusesIsNeverPersisted() throws {
        let storage = try makeStorage("bad-name")
        let keyStore = TestWrappingKeyStore()
        let repository = try RiotProfileRepository.open(
            storage: storage, keyStore: keyStore, starterPacks: []
        )

        XCTAssertThrowsError(try repository.setDisplayName(""), "core bounds the name; the empty one is refused there")
        XCTAssertNil(repository.claimedName, "Rust first, disk second: a refused name is never written")

        let reopened = try RiotProfileRepository.open(
            storage: storage, keyStore: keyStore, starterPacks: []
        )
        XCTAssertEqual(try reopened.me().displayName, "member", "and it did not come back on relaunch")
    }

    // MARK: - Snapshot helpers

    private func removeTrustedAppIDs(from snapshotURL: URL) throws {
        try removeKey("trustedAppIDs", from: snapshotURL)
    }

    /// The trust decisions as they actually sit on disk — read from the snapshot
    /// rather than from the repository, so a revoke that only cleared memory
    /// cannot pass.
    private func trustedAppIDs(in snapshotURL: URL) throws -> [String] {
        let object = try XCTUnwrap(
            JSONSerialization.jsonObject(with: Data(contentsOf: snapshotURL)) as? [String: Any]
        )
        return object["trustedAppIDs"] as? [String] ?? []
    }

    private func removeKey(_ key: String, from snapshotURL: URL) throws {
        var object = try XCTUnwrap(
            JSONSerialization.jsonObject(with: Data(contentsOf: snapshotURL)) as? [String: Any]
        )
        object.removeValue(forKey: key)
        try JSONSerialization.data(withJSONObject: object).write(to: snapshotURL, options: .atomic)
    }

    /// Inserts a structurally-valid-but-meaningless bundle into the persisted
    /// `appDataBundles` array. The element is valid base64 (so the snapshot still
    /// decodes to `Data`) but is not a signed bundle (so replay throws and the
    /// element is skipped).
    private func insertGarbageBundle(into snapshotURL: URL, at index: Int) throws {
        var object = try XCTUnwrap(
            JSONSerialization.jsonObject(with: Data(contentsOf: snapshotURL)) as? [String: Any]
        )
        var bundles = object["appDataBundles"] as? [Any] ?? []
        bundles.insert(Data("garbage".utf8).base64EncodedString(), at: index)
        object["appDataBundles"] = bundles
        try JSONSerialization.data(withJSONObject: object).write(to: snapshotURL, options: .atomic)
    }
}

/// Duplicated per the project convention (the copy in `BindingSemanticsTests`
/// is `private`); a fixed 32-byte key so sealed identities round-trip.
private final class TestWrappingKeyStore: WrappingKeyStore {
    private var key: Data?

    func loadOrCreateWrappingKey() throws -> Data {
        if let key { return key }
        let created = Data(repeating: 0x5a, count: 32)
        key = created
        return created
    }
}
