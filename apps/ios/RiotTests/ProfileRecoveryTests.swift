import XCTest
@testable import RiotKit

/// Phase 1 of the self-healing recovery system: applying the reusable core
/// (`RecoveryQuarantine` + `recovering` + `RecoveryReport`) to
/// `ProfileRepository.open`. A phone that upgraded from an older build, took a
/// partial write, or synced a bundle the new core rejects used to dead-end at
/// "Opening your profile…" with a RETRY that re-failed forever. `open` must now
/// DEGRADE (drop the space, skip a bad alert) or, when the profile can't be
/// opened at all, QUARANTINE the persisted state aside (never delete it) and open
/// fresh — so the user always lands in a usable app, the bad data is preserved on
/// disk with a manifest, and the `RecoveryReport` names what was dropped.
final class ProfileRecoveryTests: XCTestCase {
    private func uniqueURL(_ label: String) -> URL {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("recovery-\(label)-\(UUID().uuidString).json")
    }

    private func snapshot(_ url: URL) throws -> [String: Any] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
    }

    private func writeSnapshot(_ object: [String: Any], to url: URL) throws {
        try JSONSerialization.data(withJSONObject: object).write(to: url, options: .atomic)
    }

    // MARK: - Deepest recovery: the profile can't be opened at all

    /// A persisted sealed identity the core rejects (the real-device upgrade
    /// case): `open` must not throw. It quarantines the persisted snapshot aside
    /// (with a manifest) and opens a FRESH profile — the user lands in onboarding,
    /// not a dead RETRY — and the corrupt data is preserved on disk, not erased.
    func testCorruptSealedIdentityQuarantinesAndRecovers() throws {
        let url = uniqueURL("sealed")
        let keyStore = TestWrappingKeyStore()
        let corrupt = Data(repeating: 0, count: 112).base64EncodedString()

        let first = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        _ = try first.createPublicSpace(title: "Berlin Mutual Aid")
        XCTAssertNil(first.recovery, "a clean first open carries no recovery signal")

        // Corrupt the sealed identity to 112 bytes the core's unseal will reject.
        var object = try snapshot(url)
        object["sealedIdentity"] = corrupt
        try writeSnapshot(object, to: url)

        // Reopen: recovers instead of throwing.
        let recovered = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        let report = try XCTUnwrap(recovered.recovery, "recovery must be surfaced, not silent")
        XCTAssertTrue(report.quarantinedProfile, "the whole profile was healed by quarantine + fresh open")
        XCTAssertNil(recovered.currentSpace, "a fresh profile lands with no community")
        // The fresh profile is usable: it can name itself and open a new space.
        _ = try recovered.me()
        _ = try recovered.createPublicSpace(title: "Fresh")

        // A quarantine ref with a manifest was recorded (never a silent heal).
        let ref = try XCTUnwrap(report.quarantined.first, "the quarantine is named in the report")
        XCTAssertEqual(ref.manifest.reason, "profile-open")
        XCTAssertNotNil(ref.manifest.error, "the manifest records WHY (the underlying error)")
        XCTAssertTrue(ref.manifest.artifacts.contains(url.lastPathComponent),
            "the manifest lists the relocated snapshot: \(ref.manifest.artifacts)")
        XCTAssertTrue(FileManager.default.fileExists(atPath: ref.manifestURL.path),
            "manifest.json is on disk beside the quarantined data")

        // The bad data was PRESERVED, not deleted: the moved snapshot survives in
        // the quarantine dir with the corrupt sealedIdentity we wrote.
        let quarantinedSnapshot = ref.directory.appendingPathComponent(url.lastPathComponent)
        let preserved = try XCTUnwrap(
            JSONSerialization.jsonObject(with: Data(contentsOf: quarantinedSnapshot)) as? [String: Any]
        )
        XCTAssertEqual(preserved["sealedIdentity"] as? String, corrupt,
            "the corrupt identity is preserved verbatim in quarantine")
    }

    /// With a durable SQLite database, the quarantine sweeps the database file
    /// aside too — a partial DB write is exactly what can wedge a core open — and
    /// the fresh profile opens on a clean database.
    func testCorruptSealedIdentityQuarantinesDatabaseFile() throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("recovery-db-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let url = dir.appendingPathComponent("riot-profile.json")
        let dbPath = dir.appendingPathComponent("riot.db").path
        let keyStore = TestWrappingKeyStore()

        let first = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore,
            databasePath: dbPath
        )
        _ = try first.createPublicSpace(title: "Berlin Mutual Aid")
        XCTAssertTrue(FileManager.default.fileExists(atPath: dbPath), "the db was created")

        var object = try snapshot(url)
        object["sealedIdentity"] = Data(repeating: 0, count: 112).base64EncodedString()
        try writeSnapshot(object, to: url)

        let recovered = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore,
            databasePath: dbPath
        )
        let report = try XCTUnwrap(recovered.recovery)
        XCTAssertTrue(report.quarantinedProfile)
        let ref = try XCTUnwrap(report.quarantined.first)
        XCTAssertTrue(ref.manifest.artifacts.contains("riot-profile.json"),
            "snapshot quarantined: \(ref.manifest.artifacts)")
        XCTAssertTrue(ref.manifest.artifacts.contains("riot.db"),
            "database quarantined: \(ref.manifest.artifacts)")
        XCTAssertTrue(
            FileManager.default.fileExists(atPath: ref.directory.appendingPathComponent("riot.db").path),
            "the quarantined db is preserved on disk")
        // Fresh profile is usable on a clean db.
        _ = try recovered.createPublicSpace(title: "Fresh")
    }

    // MARK: - Degrade: a space that won't restore

    /// A space the core refuses to rejoin (here: a non-communal namespace) must
    /// not brick the launch. The identity is kept, the space is dropped, the
    /// pre-drop snapshot is preserved in quarantine, and the user lands in a
    /// usable app.
    func testBadSpaceOpensWithoutItKeepingIdentity() throws {
        let url = uniqueURL("space")
        let keyStore = TestWrappingKeyStore()

        let first = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        _ = try first.createPublicSpace(title: "Berlin Mutual Aid")
        let originalID = try first.me().id

        // Replace the persisted namespace with a non-communal one core refuses to
        // join (odd last byte → NamespaceNotCommunal → InvalidInput).
        var object = try snapshot(url)
        var space = try XCTUnwrap(object["space"] as? [String: Any])
        space["namespaceID"] = String(repeating: "00", count: 31) + "01"
        object["space"] = space
        try writeSnapshot(object, to: url)

        let recovered = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        let report = try XCTUnwrap(recovered.recovery, "dropping a space is a recovery worth surfacing")
        XCTAssertTrue(report.spaceDropped)
        XCTAssertFalse(report.quarantinedProfile, "the identity opened — only the space was dropped")
        XCTAssertNil(recovered.currentSpace, "the unrestorable space is dropped from the working state")
        XCTAssertEqual(try recovered.me().id, originalID, "the identity is kept — same person, no churn")

        // The pre-drop snapshot is preserved as a quarantine blob (never deleted).
        let ref = try XCTUnwrap(report.quarantined.first)
        XCTAssertEqual(ref.manifest.reason, "space")
        let blob = ref.directory.appendingPathComponent("profile-snapshot.json")
        XCTAssertTrue(FileManager.default.fileExists(atPath: blob.path),
            "the pre-drop snapshot bytes are preserved aside")
    }

    // MARK: - Degrade: one bad alert among good ones

    /// One alert bundle the core rejects must be skipped and quarantined, not
    /// fatal — the other alerts still come back. Mirrors the existing
    /// app-data/trust loops, which already skip a single bad element.
    func testBadAlertIsSkippedOthersRestored() throws {
        let url = uniqueURL("alert")
        let keyStore = TestWrappingKeyStore()

        let first = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        let space = try first.createPublicSpace(title: "Berlin Mutual Aid")
        let future = UInt64(Date().timeIntervalSince1970) + 3600
        _ = try first.signAlert(in: space, draft: AlertDraft(
            expiresAt: future, headline: "One", description: "the first alert",
            sourceClaims: ["a source"], aiAssisted: false))
        _ = try first.signAlert(in: space, draft: AlertDraft(
            expiresAt: future, headline: "Two", description: "the second alert",
            sourceClaims: ["a source"], aiAssisted: false))
        XCTAssertEqual(try first.currentEntries().count, 2)

        // Corrupt the FIRST alert's bundle so its replay throws; the second stays
        // valid.
        var object = try snapshot(url)
        var alerts = try XCTUnwrap(object["alerts"] as? [[String: Any]])
        alerts[0]["bundle"] = Data("not a bundle".utf8).base64EncodedString()
        object["alerts"] = alerts
        try writeSnapshot(object, to: url)

        let recovered = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        let report = try XCTUnwrap(recovered.recovery)
        XCTAssertEqual(report.alertsSkipped, 1, "exactly the one bad alert was skipped")
        XCTAssertFalse(report.quarantinedProfile)
        XCTAssertNotNil(recovered.currentSpace, "the space still restored")
        XCTAssertEqual(try recovered.currentEntries().count, 1, "the healthy alert still came back")

        // The rejected bundle was preserved (quarantined), not silently dropped.
        let ref = try XCTUnwrap(report.quarantined.first { $0.manifest.reason == "alert-replay" })
        XCTAssertTrue(
            FileManager.default.fileExists(atPath: ref.directory.appendingPathComponent("alert.bundle").path),
            "the bad alert bundle is preserved aside")
    }

    // MARK: - No false positives

    func testCleanReopenHasNoRecoverySignal() throws {
        let url = uniqueURL("clean")
        let keyStore = TestWrappingKeyStore()

        let first = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        let space = try first.createPublicSpace(title: "Berlin Mutual Aid")
        _ = try first.signAlert(in: space, draft: AlertDraft(
            expiresAt: UInt64(Date().timeIntervalSince1970) + 3600,
            headline: "One", description: "the first alert",
            sourceClaims: ["a source"], aiAssisted: false))

        let reopened = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        XCTAssertNil(reopened.recovery, "a healthy reopen recovers nothing")
        XCTAssertNotNil(reopened.currentSpace)
        XCTAssertEqual(try reopened.currentEntries().count, 1)
    }
}

/// Duplicated per the project convention (each test file keeps its own `private`
/// copy); a fixed 32-byte key so sealed identities round-trip across reopen.
private final class TestWrappingKeyStore: WrappingKeyStore {
    private var key: Data?

    func loadOrCreateWrappingKey() throws -> Data {
        if let key { return key }
        let created = Data(repeating: 0x5a, count: 32)
        key = created
        return created
    }
}
