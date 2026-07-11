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

    // MARK: - Display name

    func testDisplayNameComesFromProfileNotPlaceholder() throws {
        let repository = try RiotProfileRepository.open(
            storage: try makeStorage("displayname"),
            keyStore: TestWrappingKeyStore(),
            starterPacks: try starterPacks()
        )
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try repository.spaceApps()[0].appIDHex
        try repository.trustApp(appID: appID)
        let bridge = try XCTUnwrap(repository.appDataBridge(appID: appID))

        let name = bridge.displayName()
        XCTAssertTrue(name.hasPrefix("member-"), "expected an FFI-derived name, got \(name)")
        XCTAssertNotEqual(name, "member")
    }

    // MARK: - Snapshot helpers

    private func removeTrustedAppIDs(from snapshotURL: URL) throws {
        try removeKey("trustedAppIDs", from: snapshotURL)
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
