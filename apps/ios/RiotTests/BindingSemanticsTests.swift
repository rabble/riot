import XCTest
@testable import RiotKit

final class BindingSemanticsTests: XCTestCase {
    func testEmptyProtectedProfileOpensWithAnEmptyBoard() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let storage = try ProtectedProfileStorage(fileURL: directory.appendingPathComponent("profile.json"))

        let repository = try RiotProfileRepository.open(storage: storage, keyStore: TestWrappingKeyStore())

        XCTAssertNil(repository.currentSpace)
        XCTAssertEqual(try repository.currentEntries(), [])
    }

    func testSignedAlertSurvivesProtectedOfflineReloadWithFullIdentityMetadata() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let snapshotURL = directory.appendingPathComponent("profile.json")
        let storage = try ProtectedProfileStorage(fileURL: snapshotURL)

        let keys = TestWrappingKeyStore()
        let first = try RiotProfileRepository.open(storage: storage, keyStore: keys)
        let space = try first.createPublicSpace(title: "Berlin Mutual Aid")
        let expiresAt = UInt64(Date().timeIntervalSince1970) + 3_600
        let signed = try first.signAlert(
            in: space,
            draft: AlertDraft(
                expiresAt: expiresAt,
                headline: "Water available at the east entrance",
                description: "Bring a bottle. Volunteers are refilling the tank.",
                sourceClaims: ["Two on-site volunteers"],
                aiAssisted: true
            )
        )

        XCTAssertEqual(signed.entryID.count, 64)
        XCTAssertEqual(signed.namespaceID.count, 64)
        XCTAssertEqual(signed.signerID.count, 64)

        // Reopening creates a fresh in-memory Rust profile and must rehydrate it
        // only from the protected local snapshot. No network transport is used.
        let reloaded = try RiotProfileRepository.open(storage: storage, keyStore: keys)
        let entries = try reloaded.currentEntries()

        XCTAssertEqual(entries.count, 1)
        XCTAssertEqual(entries[0].entryID, signed.entryID)
        XCTAssertEqual(entries[0].namespaceID, signed.namespaceID)
        XCTAssertEqual(entries[0].signerID, signed.signerID)
        XCTAssertEqual(entries[0].createdAt, signed.createdAt)
        XCTAssertEqual(entries[0].expiresAt, expiresAt)
        XCTAssertTrue(entries[0].aiAssisted)
    }

    func testSealedSignerSurvivesProcessRestartAndRestoresContentOffline() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let snapshotURL = directory.appendingPathComponent("profile.json")
        let storage = try ProtectedProfileStorage(fileURL: snapshotURL)
        let keys = TestWrappingKeyStore()

        var firstProcess: RiotProfileRepository? = try RiotProfileRepository.open(
            storage: storage,
            keyStore: keys
        )
        let space = try XCTUnwrap(firstProcess).createPublicSpace(title: "Durable Berlin Mutual Aid")
        let first = try XCTUnwrap(firstProcess).signAlert(
            in: space,
            draft: restartDraft(headline: "First signer continuity alert")
        )
        let sealedIdentity = try sealedIdentityBytes(in: snapshotURL)
        XCTAssertEqual(sealedIdentity.count, 112)
        firstProcess = nil

        let secondProcess = try RiotProfileRepository.open(storage: storage, keyStore: keys)
        let restoredBeforeSecondSign = try secondProcess.currentEntries()
        let second = try secondProcess.signAlert(
            in: try XCTUnwrap(secondProcess.currentSpace),
            draft: restartDraft(headline: "Second signer continuity alert")
        )

        XCTAssertEqual(restoredBeforeSecondSign.map(\.entryID), [first.entryID])
        XCTAssertEqual(first.signerID.count, 64)
        XCTAssertEqual(second.signerID, first.signerID)
        XCTAssertEqual(try secondProcess.currentEntries().count, 2)
    }

    /// A member's own alert must still be on the board after a relaunch.
    ///
    /// The creator's case is covered above, and it is not the same case: the
    /// creator's author namespace IS the space, while a joiner's author was
    /// regenerated INTO someone else's namespace. `open` re-joins before it
    /// replays, so this pins that a joiner's replay still lands on the board.
    /// `testJoinersSubspaceIdIsIdenticalAfterReopening` does not pin it — it
    /// asserts one distinct signer, which still holds if the pre-restart alert
    /// vanished entirely.
    func testAlertSignedAfterJoiningSomeoneElsesSpaceSurvivesReopen() throws {
        let organizer = try openRepository()
        let space = try organizer.repository.createPublicSpace(title: "Riverside Tenants Union")

        let member = try openRepository()
        try member.repository.joinSpace(space)
        let signed = try member.repository.signAlert(
            in: space,
            draft: restartDraft(headline: "Member's alert, written after joining")
        )

        let reopened = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: member.url),
            keyStore: member.keys
        )

        XCTAssertEqual(reopened.currentSpace, space)
        XCTAssertEqual(
            try reopened.currentEntries().map(\.entryID), [signed.entryID],
            "the member's own alert is gone from the board after a relaunch"
        )
    }

    /// An endorsement is this person vouching for an app to everyone they sync
    /// with. Rust keeps the marker in the same in-memory store as trust, so
    /// without a persisted claim to re-assert, a relaunch silently withdraws it.
    func testEndorsementSurvivesReopen() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let url = directory.appendingPathComponent("profile.json")
        let keys = TestWrappingKeyStore()
        let packs = try starterPacks()

        let first = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url),
            keyStore: keys,
            starterPacks: packs
        )
        _ = try first.createPublicSpace(title: "Riverside Tenants Union")
        let app = try XCTUnwrap(first.installedApps().first)
        let appID = try XCTUnwrap(RiotDirectoryRow.bytes(hex: app.appIDHex))
        let me = try first.me().id.lowercased()

        try first.endorseApp(appID: appID, note: "We use this every week", retract: false)
        XCTAssertTrue(
            try endorsers(of: app.appIDHex, in: first).contains(me),
            "precondition: the endorsement is there before the relaunch"
        )

        let reopened = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url),
            keyStore: keys,
            starterPacks: packs
        )

        XCTAssertTrue(
            try endorsers(of: app.appIDHex, in: reopened).contains(me),
            "this person's endorsement is gone from the directory after a relaunch"
        )
    }

    /// Retracting is a decision too: it must not come back from the dead on the
    /// next launch because the claim was still sitting in the snapshot.
    func testRetractedEndorsementStaysRetractedAcrossReopen() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let url = directory.appendingPathComponent("profile.json")
        let keys = TestWrappingKeyStore()
        let packs = try starterPacks()

        let first = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url),
            keyStore: keys,
            starterPacks: packs
        )
        _ = try first.createPublicSpace(title: "Riverside Tenants Union")
        let app = try XCTUnwrap(first.installedApps().first)
        let appID = try XCTUnwrap(RiotDirectoryRow.bytes(hex: app.appIDHex))
        let me = try first.me().id.lowercased()

        try first.endorseApp(appID: appID, note: "Recommended", retract: false)
        try first.endorseApp(appID: appID, note: "", retract: true)

        let reopened = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url),
            keyStore: keys,
            starterPacks: packs
        )

        XCTAssertFalse(
            try endorsers(of: app.appIDHex, in: reopened).contains(me),
            "a withdrawn endorsement was re-asserted on the next launch"
        )
    }

    func testLegacySnapshotWithoutSealedIdentityMigratesWithoutLosingSignedContent() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let snapshotURL = directory.appendingPathComponent("profile.json")
        let storage = try ProtectedProfileStorage(fileURL: snapshotURL)
        let keys = TestWrappingKeyStore()
        let original = try RiotProfileRepository.open(storage: storage, keyStore: keys)
        let space = try original.createPublicSpace(title: "Legacy public space")
        let signed = try original.signAlert(
            in: space,
            draft: restartDraft(headline: "Legacy signed content")
        )
        try removeSealedIdentity(from: snapshotURL)

        let migrated = try RiotProfileRepository.open(storage: storage, keyStore: keys)

        XCTAssertEqual(try migrated.currentEntries().map(\.entryID), [signed.entryID])
        XCTAssertEqual(try sealedIdentityBytes(in: snapshotURL).count, 112)
    }

    // MARK: - Starter-catalog generation (WU-001N)

    /// A fresh first save records the current starter-catalog generation (2)
    /// alongside the sealed identity. The exact retained internal `Option<u8>`
    /// is proven by Rust white-box tests; here we pin the durable JSON marker.
    func testFreshFirstSaveRecordsStarterCatalogGenerationTwo() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let snapshotURL = directory.appendingPathComponent("profile.json")
        let storage = try ProtectedProfileStorage(fileURL: snapshotURL)

        _ = try RiotProfileRepository.open(storage: storage, keyStore: TestWrappingKeyStore())

        XCTAssertNotNil(try sealedIdentityBytes(in: snapshotURL))
        XCTAssertEqual(try starterCatalogGeneration(in: snapshotURL), 2)
    }

    /// A legacy sealed snapshot with the generation key deleted is generation 1
    /// (absence). Reopening and performing a permitted save must LEAVE the key
    /// absent — never materialize `1` — because absence itself is generation 1.
    func testLegacySealedSnapshotKeepsGenerationKeyAbsentAfterPermittedSave() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let snapshotURL = directory.appendingPathComponent("profile.json")
        let storage = try ProtectedProfileStorage(fileURL: snapshotURL)
        let keys = TestWrappingKeyStore()
        _ = try RiotProfileRepository.open(storage: storage, keyStore: keys)
        try removeStarterCatalogGeneration(from: snapshotURL)
        XCTAssertFalse(try snapshotContainsGenerationKey(in: snapshotURL))

        let reopened = try RiotProfileRepository.open(storage: storage, keyStore: keys)
        _ = try reopened.createPublicSpace(title: "Legacy sealed space")

        XCTAssertFalse(try snapshotContainsGenerationKey(in: snapshotURL))
    }

    /// An explicit generation-1 snapshot reopens successfully and remains
    /// durably encoded as `1` across a permitted save (Task 1 proves the
    /// internal `Some(1)` retention).
    func testExplicitGenerationOneRemainsOneAcrossReopen() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let snapshotURL = directory.appendingPathComponent("profile.json")
        let storage = try ProtectedProfileStorage(fileURL: snapshotURL)
        let keys = TestWrappingKeyStore()
        _ = try RiotProfileRepository.open(storage: storage, keyStore: keys)
        try setStarterCatalogGeneration(1, in: snapshotURL)

        let reopened = try RiotProfileRepository.open(storage: storage, keyStore: keys)
        _ = try reopened.createPublicSpace(title: "Explicit generation one")

        XCTAssertEqual(try starterCatalogGeneration(in: snapshotURL), 1)
    }

    /// A sealed legacy snapshot reopens with the SAME signer, and an identityless
    /// legacy snapshot necessarily mints and seals a signer on its first reopen,
    /// which a second reopen then preserves. Both paths keep the generation key
    /// absent in subsequent durable JSON rather than materializing generation 2.
    func testLegacyRestoreFamiliesPreserveSignerAndKeepGenerationAbsent() throws {
        // Sealed legacy: same signer, generation stays absent.
        let sealedDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let sealedURL = sealedDir.appendingPathComponent("profile.json")
        let sealedStorage = try ProtectedProfileStorage(fileURL: sealedURL)
        let sealedKeys = TestWrappingKeyStore()
        let sealedOriginal = try RiotProfileRepository.open(storage: sealedStorage, keyStore: sealedKeys)
        let sealedSigner = try sealedOriginal.me().id
        try removeStarterCatalogGeneration(from: sealedURL)

        let sealedReopened = try RiotProfileRepository.open(storage: sealedStorage, keyStore: sealedKeys)
        _ = try sealedReopened.createPublicSpace(title: "Sealed legacy")
        XCTAssertEqual(try sealedReopened.me().id, sealedSigner)
        XCTAssertFalse(try snapshotContainsGenerationKey(in: sealedURL))

        // Identityless legacy: first reopen mints+seals; second reopen preserves.
        let idlessDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let idlessURL = idlessDir.appendingPathComponent("profile.json")
        let idlessStorage = try ProtectedProfileStorage(fileURL: idlessURL)
        let idlessKeys = TestWrappingKeyStore()
        _ = try RiotProfileRepository.open(storage: idlessStorage, keyStore: idlessKeys)
        try removeStarterCatalogGeneration(from: idlessURL)
        try removeSealedIdentity(from: idlessURL)

        let idlessFirst = try RiotProfileRepository.open(storage: idlessStorage, keyStore: idlessKeys)
        let mintedSigner = try idlessFirst.me().id
        // First reopen sealed a fresh identity durably, and it must not have
        // taken the fresh generation-2 marker.
        XCTAssertEqual(try sealedIdentityBytes(in: idlessURL).count, 112)
        XCTAssertFalse(try snapshotContainsGenerationKey(in: idlessURL))

        let idlessSecond = try RiotProfileRepository.open(storage: idlessStorage, keyStore: idlessKeys)
        _ = try idlessSecond.createPublicSpace(title: "Identityless legacy")
        XCTAssertEqual(try idlessSecond.me().id, mintedSigner)
        XCTAssertFalse(try snapshotContainsGenerationKey(in: idlessURL))
    }
}

private extension BindingSemanticsTests {
    func starterPacks() throws -> [(manifest: Data, bundle: Data)] {
        let apps = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // RiotTests
            .deletingLastPathComponent() // ios
            .deletingLastPathComponent() // apps
            .deletingLastPathComponent() // repo root
            .appendingPathComponent("fixtures/apps")
        return [(
            manifest: try Data(contentsOf: apps.appendingPathComponent("checklist.manifest.cbor")),
            bundle: try Data(contentsOf: apps.appendingPathComponent("checklist.bundle.cbor"))
        )]
    }

    /// The subspaces the directory currently shows as endorsing `appIDHex`,
    /// lowercased — core's own answer, not the snapshot's.
    func endorsers(
        of appIDHex: String,
        in repository: RiotProfileRepository
    ) throws -> [String] {
        let listing = try repository.directoryListings().first {
            RiotDirectoryRow.hex($0.appId).lowercased() == appIDHex.lowercased()
        }
        return try XCTUnwrap(listing).endorsingMetSubspaces
            .map { RiotDirectoryRow.hex($0).lowercased() }
    }
}

/// One profile's storage plus the wrapping key it was sealed under — everything
/// needed to open the SAME person again, which is what a relaunch does.
private struct OpenedRepository {
    let repository: RiotProfileRepository
    let url: URL
    let keys: WrappingKeyStore
}

private func openRepository() throws -> OpenedRepository {
    let directory = FileManager.default.temporaryDirectory
        .appendingPathComponent(UUID().uuidString, isDirectory: true)
    let url = directory.appendingPathComponent("profile.json")
    let keys = TestWrappingKeyStore()
    return OpenedRepository(
        repository: try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url),
            keyStore: keys
        ),
        url: url,
        keys: keys
    )
}

private final class TestWrappingKeyStore: WrappingKeyStore {
    private var key: Data?

    func loadOrCreateWrappingKey() throws -> Data {
        if let key { return key }
        let created = Data(repeating: 0x5a, count: 32)
        key = created
        return created
    }
}

private func restartDraft(headline: String) -> AlertDraft {
    AlertDraft(
        expiresAt: UInt64(Date().timeIntervalSince1970) + 3_600,
        headline: headline,
        description: "Signed before or after a simulated process restart.",
        sourceClaims: ["Local continuity test"],
        aiAssisted: false
    )
}

private func sealedIdentityBytes(in snapshotURL: URL) throws -> Data {
    let object = try XCTUnwrap(
        JSONSerialization.jsonObject(with: Data(contentsOf: snapshotURL)) as? [String: Any]
    )
    let encoded = try XCTUnwrap(object["sealedIdentity"] as? String)
    return try XCTUnwrap(Data(base64Encoded: encoded))
}

private func removeSealedIdentity(from snapshotURL: URL) throws {
    var object = try XCTUnwrap(
        JSONSerialization.jsonObject(with: Data(contentsOf: snapshotURL)) as? [String: Any]
    )
    object.removeValue(forKey: "sealedIdentity")
    try JSONSerialization.data(withJSONObject: object).write(to: snapshotURL, options: .atomic)
}

private func starterCatalogGeneration(in snapshotURL: URL) throws -> Int? {
    let object = try XCTUnwrap(
        JSONSerialization.jsonObject(with: Data(contentsOf: snapshotURL)) as? [String: Any]
    )
    return object["starterCatalogGeneration"] as? Int
}

private func snapshotContainsGenerationKey(in snapshotURL: URL) throws -> Bool {
    let object = try XCTUnwrap(
        JSONSerialization.jsonObject(with: Data(contentsOf: snapshotURL)) as? [String: Any]
    )
    return object.keys.contains("starterCatalogGeneration")
}

private func removeStarterCatalogGeneration(from snapshotURL: URL) throws {
    var object = try XCTUnwrap(
        JSONSerialization.jsonObject(with: Data(contentsOf: snapshotURL)) as? [String: Any]
    )
    object.removeValue(forKey: "starterCatalogGeneration")
    try JSONSerialization.data(withJSONObject: object).write(to: snapshotURL, options: .atomic)
}

private func setStarterCatalogGeneration(_ value: Int, in snapshotURL: URL) throws {
    var object = try XCTUnwrap(
        JSONSerialization.jsonObject(with: Data(contentsOf: snapshotURL)) as? [String: Any]
    )
    object["starterCatalogGeneration"] = value
    try JSONSerialization.data(withJSONObject: object).write(to: snapshotURL, options: .atomic)
}
