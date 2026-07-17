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

    // MARK: - Phase 5: the protected-storage blob load itself

    /// The earliest boundary — before open even begins. If the persisted bytes
    /// will not DECODE at all (a truncated write, a foreign file, a schema jump
    /// the JSON can't survive), `open` must not throw: it quarantines the raw blob
    /// aside (never deletes it) and starts from an EMPTY profile, landing the user
    /// in a usable app. This is distinct from `.profileOpen` (a decodable snapshot
    /// the CORE rejects) — the reason recorded is `storage-blob`.
    func testUndecodableStorageBlobQuarantinesAndStartsEmpty() throws {
        let url = uniqueURL("storage-blob")
        let keyStore = TestWrappingKeyStore()

        // Bytes that are not JSON at all — `JSONDecoder` throws before `open` runs.
        let garbage = Data("this is not a profile snapshot".utf8)
        try garbage.write(to: url, options: .atomic)

        let recovered = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        let report = try XCTUnwrap(recovered.recovery, "an undecodable blob is a recovery worth surfacing")
        XCTAssertTrue(report.storageBlobQuarantined, "the raw blob was set aside and the open started empty")
        XCTAssertFalse(report.quarantinedProfile, "this is the earlier storage-blob boundary, not profile-open")
        XCTAssertNil(recovered.currentSpace, "an empty start lands with no community")
        // The empty-started profile is fully usable.
        _ = try recovered.me()
        _ = try recovered.createPublicSpace(title: "Fresh")

        let ref = try XCTUnwrap(report.quarantined.first { $0.manifest.reason == "storage-blob" })
        XCTAssertNotNil(ref.manifest.error, "the manifest records WHY the decode failed")
        XCTAssertTrue(FileManager.default.fileExists(atPath: ref.manifestURL.path),
            "manifest.json is on disk beside the quarantined blob")
        // The undecodable bytes are PRESERVED verbatim, never destroyed.
        let preserved = ref.directory.appendingPathComponent(url.lastPathComponent)
        XCTAssertEqual(try Data(contentsOf: preserved), garbage,
            "the raw undecodable blob is preserved in quarantine")
    }

    // MARK: - Phase 4: a bad carried app pack / app-data bundle

    /// A neighbour-carried app pack whose bytes no longer install must be set
    /// aside + RECORDED (not silently `try?`-skipped) and the rest of the open
    /// continues — the identity and space still come back.
    func testBadCarriedAppPackQuarantinedOthersSurvive() throws {
        let url = uniqueURL("app-pack")
        let keyStore = TestWrappingKeyStore()

        let first = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        _ = try first.createPublicSpace(title: "Berlin Mutual Aid")
        let originalID = try first.me().id

        // Inject a carried app whose manifest/bundle are garbage the installer rejects.
        var object = try snapshot(url)
        object["carriedApps"] = [[
            "appIDHex": String(repeating: "ab", count: 32),
            "manifest": Data("not a manifest".utf8).base64EncodedString(),
            "bundle": Data("not a bundle".utf8).base64EncodedString(),
        ]]
        try writeSnapshot(object, to: url)

        let recovered = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        let report = try XCTUnwrap(recovered.recovery)
        XCTAssertEqual(report.appPacksSkipped, 1, "exactly the one bad pack was set aside")
        XCTAssertFalse(report.quarantinedProfile, "only the pack failed — the profile opened")
        XCTAssertNotNil(recovered.currentSpace, "the space still restored")
        XCTAssertEqual(try recovered.me().id, originalID, "the identity is kept")

        let ref = try XCTUnwrap(report.quarantined.first { $0.manifest.reason == "app-pack" })
        XCTAssertFalse(ref.manifest.artifacts.isEmpty,
            "the bad pack's bytes are preserved aside: \(ref.manifest.artifacts)")
    }

    /// A committed app-data bundle the core rejects on replay must be set aside +
    /// recorded, not silently `try?`-skipped, and the open continues.
    func testBadAppDataBundleQuarantinedOthersSurvive() throws {
        let url = uniqueURL("app-data")
        let keyStore = TestWrappingKeyStore()

        let first = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        _ = try first.createPublicSpace(title: "Berlin Mutual Aid")

        var object = try snapshot(url)
        object["appDataBundles"] = [Data("not an app-data bundle".utf8).base64EncodedString()]
        try writeSnapshot(object, to: url)

        let recovered = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        let report = try XCTUnwrap(recovered.recovery)
        XCTAssertEqual(report.appDataSkipped, 1, "exactly the one bad app-data bundle was set aside")
        XCTAssertNotNil(recovered.currentSpace, "the space still restored")

        let ref = try XCTUnwrap(report.quarantined.first { $0.manifest.reason == "app-data" })
        XCTAssertTrue(
            FileManager.default.fileExists(atPath: ref.directory.appendingPathComponent("app-data.bundle").path),
            "the bad app-data bundle is preserved aside")
    }

    // MARK: - Phase 3: a bad SYNCED bundle among good local alerts

    /// A bundle that arrived over SYNC (provenance `fromSync`) and the core now
    /// rejects on replay is quarantined under the distinct `sync-import` reason,
    /// never partially applied, and the replay loop CONTINUES — a locally authored
    /// alert in the same snapshot still comes back.
    func testBadSyncedBundleQuarantinedUnderSyncImport() throws {
        let url = uniqueURL("sync-import")
        let keyStore = TestWrappingKeyStore()

        let first = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        let space = try first.createPublicSpace(title: "Berlin Mutual Aid")
        _ = try first.signAlert(in: space, draft: AlertDraft(
            expiresAt: UInt64(Date().timeIntervalSince1970) + 3600,
            headline: "Local", description: "a locally authored alert",
            sourceClaims: ["a source"], aiAssisted: false))
        XCTAssertEqual(try first.currentEntries().count, 1)

        // Append a SYNC-origin bundle the core rejects. The existing local alert
        // stays as-is (fromSync absent → treated as local).
        var object = try snapshot(url)
        var alerts = try XCTUnwrap(object["alerts"] as? [[String: Any]])
        alerts.append([
            "bundle": Data("not a synced bundle".utf8).base64EncodedString(),
            "fromSync": true,
        ])
        object["alerts"] = alerts
        try writeSnapshot(object, to: url)

        let recovered = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        let report = try XCTUnwrap(recovered.recovery)
        XCTAssertEqual(report.syncImportsSkipped, 1, "the bad synced bundle was set aside as a sync import")
        XCTAssertEqual(report.alertsSkipped, 0, "the local alert was NOT the one that failed")
        XCTAssertNotNil(recovered.currentSpace)
        XCTAssertEqual(try recovered.currentEntries().count, 1, "the healthy local alert still came back")

        let ref = try XCTUnwrap(report.quarantined.first { $0.manifest.reason == "sync-import" })
        XCTAssertTrue(
            FileManager.default.fileExists(atPath: ref.directory.appendingPathComponent("synced.bundle").path),
            "the rejected synced bundle is preserved aside")
    }

    // MARK: - Phase 2: a community that will not open/reproject

    /// Switching to a community the registry cannot open (its at-rest author was
    /// quarantined by the core, or it is simply not openable) must NOT brick: it
    /// surfaces `CommunityUnavailable`, leaves the CURRENT community untouched, and
    /// records the event through the recovery core (never a silent heal). From the
    /// Swift boundary a corrupt author and an unopenable target are the same
    /// `CommunityUnavailable` signal, so this exercises the resilient wrapper
    /// without corrupting a live database.
    func testSwitchToUnavailableCommunityRecordsAndDoesNotBrick() throws {
        let url = uniqueURL("community")
        let keyStore = TestWrappingKeyStore()

        let repo = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url), keyStore: keyStore
        )
        let space = try repo.createPublicSpace(title: "Berlin Mutual Aid")
        XCTAssertNil(repo.recovery, "a clean open carries no recovery signal")

        // A well-formed but unheld namespace — core answers CommunityUnavailable.
        let unheld = String(repeating: "cd", count: 32)
        XCTAssertThrowsError(try repo.switchToCommunity(namespaceID: unheld),
            "switching to a community that cannot open surfaces, it does not silently succeed")

        // The current community is untouched — a failed switch never leaves it.
        XCTAssertEqual(repo.currentSpace?.namespaceID, space.namespaceID,
            "the working community survives the failed switch")

        // The event is recorded through the recovery core, not swallowed.
        let report = try XCTUnwrap(repo.recovery, "an unavailable community is a recovery worth surfacing")
        XCTAssertTrue(report.communityUnavailable)
        let ref = try XCTUnwrap(report.quarantined.first { $0.manifest.reason == "community" })
        XCTAssertNotNil(ref.manifest.error, "the manifest records WHY the community could not open")
        XCTAssertTrue(FileManager.default.fileExists(atPath: ref.manifestURL.path),
            "a manifest is on disk for the unavailable community")
        XCTAssertFalse(ref.manifest.artifacts.isEmpty,
            "a record of the unavailable community is preserved: \(ref.manifest.artifacts)")
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
