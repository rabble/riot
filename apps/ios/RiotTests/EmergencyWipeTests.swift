import XCTest

@testable import RiotKit

/// Stands in for the Keychain, which is unavailable to an unsigned test host.
private final class FakeSecretStore: DestructibleSecretStore {
    var key: Data?
    private(set) var destroyCount = 0
    private(set) var restoreCount = 0

    init(key: Data? = Data(repeating: 7, count: 32)) { self.key = key }

    func loadOrCreateWrappingKey() throws -> Data {
        if let key { return key }
        let fresh = Data(repeating: 1, count: 32)
        key = fresh
        return fresh
    }

    @discardableResult
    func destroyWrappingKey() throws -> Data? {
        destroyCount += 1
        defer { key = nil }
        return key
    }

    func restoreWrappingKey(_ key: Data) throws {
        restoreCount += 1
        self.key = key
    }
}

private final class FakeFiles: FileRemoving {
    var existing: Set<String>
    private(set) var removed: [String] = []
    var failOn: Set<String> = []

    init(existing: Set<String>) { self.existing = existing }

    func fileExists(atPath path: String) -> Bool { existing.contains(path) }

    func removeItem(atPath path: String) throws {
        if failOn.contains(path) {
            throw NSError(domain: NSCocoaErrorDomain, code: NSFileWriteNoPermissionError)
        }
        removed.append(path)
        existing.remove(path)
    }
}

final class EmergencyWipeTests: XCTestCase {
    private let paths = WipePaths(
        databasePath: "/store/riot.db",
        profilePath: "/store/riot-profile.json",
        quarantineDirectory: "/store/quarantine")

    private func makeFiles() -> FakeFiles {
        FakeFiles(existing: [
            "/store/riot.db", "/store/riot.db-wal", "/store/riot.db-shm",
            "/store/riot-profile.json", "/store/quarantine",
        ])
    }

    /// The security boundary: arming destroys the key IMMEDIATELY, before any
    /// file work. From that instant the sealed identity is unrecoverable
    /// ciphertext, however big or slow the database is.
    func testArmingDestroysTheKeyBeforeAnyFileIsTouched() throws {
        let keys = FakeSecretStore()
        let files = makeFiles()
        let wipe = EmergencyWipe(keyStore: keys, paths: paths, files: files)

        try wipe.arm()

        XCTAssertNil(keys.key, "the key is gone the moment the wipe is armed")
        XCTAssertEqual(keys.destroyCount, 1)
        XCTAssertTrue(files.removed.isEmpty, "files wait for the undo window to elapse")
        XCTAssertTrue(wipe.isArmed)
    }

    /// The undo window exists for the accidental triple-tap.
    func testUndoRestoresTheKeyAndLeavesEveryFileInPlace() throws {
        let original = Data(repeating: 7, count: 32)
        let keys = FakeSecretStore(key: original)
        let files = makeFiles()
        let wipe = EmergencyWipe(keyStore: keys, paths: paths, files: files)

        try wipe.arm()
        try wipe.undo()

        XCTAssertEqual(keys.key, original, "the same key comes back, so the identity survives")
        XCTAssertEqual(keys.restoreCount, 1)
        XCTAssertTrue(files.removed.isEmpty)
        XCTAssertFalse(wipe.isArmed)
    }

    /// After the window, everything goes — including the `-wal`, where the most
    /// recent entries live, and the quarantine directory, where the recovery
    /// system deliberately KEEPS copies of profile state.
    func testCommitRemovesSidecarsAndTheQuarantineDirectory() throws {
        let keys = FakeSecretStore()
        let files = makeFiles()
        let wipe = EmergencyWipe(keyStore: keys, paths: paths, files: files)

        try wipe.arm()
        let removed = try wipe.commit()

        XCTAssertEqual(
            Set(removed),
            [
                "/store/riot.db", "/store/riot.db-wal", "/store/riot.db-shm",
                "/store/riot-profile.json", "/store/quarantine",
            ])
        XCTAssertTrue(
            removed.contains("/store/quarantine"),
            "a wipe that spared quarantine would preserve copies of what it destroyed")
        XCTAssertTrue(removed.contains("/store/riot.db-wal"), "the newest entries live here")
    }

    /// Undo must be impossible once the files are gone — otherwise the UI could
    /// restore a key for data that no longer exists and present a half-alive
    /// profile.
    func testUndoAfterCommitDoesNotResurrectTheKey() throws {
        let keys = FakeSecretStore()
        let wipe = EmergencyWipe(keyStore: keys, paths: paths, files: makeFiles())

        try wipe.arm()
        _ = try wipe.commit()
        try wipe.undo()

        XCTAssertNil(keys.key, "the key stays destroyed")
        XCTAssertEqual(keys.restoreCount, 0)
    }

    /// A locked or undeletable file must not strand the rest on disk.
    func testCommitKeepsDeletingAfterAFailureThenReportsIt() throws {
        let keys = FakeSecretStore()
        let files = makeFiles()
        files.failOn = ["/store/riot.db"]
        let wipe = EmergencyWipe(keyStore: keys, paths: paths, files: files)

        try wipe.arm()
        XCTAssertThrowsError(try wipe.commit())

        XCTAssertTrue(files.removed.contains("/store/riot-profile.json"))
        XCTAssertTrue(files.removed.contains("/store/quarantine"))
        XCTAssertTrue(files.removed.contains("/store/riot.db-wal"))
    }

    /// Double-arming must not overwrite the retained key with nothing — that
    /// would silently strand the undo and turn a recoverable mis-tap into a
    /// permanent wipe.
    func testArmingTwiceKeepsTheUndoWorking() throws {
        let original = Data(repeating: 7, count: 32)
        let keys = FakeSecretStore(key: original)
        let wipe = EmergencyWipe(keyStore: keys, paths: paths, files: makeFiles())

        try wipe.arm()
        try wipe.arm()
        try wipe.undo()

        XCTAssertEqual(keys.key, original)
        XCTAssertEqual(keys.destroyCount, 1, "the second arm is a no-op, not a second destroy")
    }

    /// Arming with no identity yet (fresh install) is not an error, and undo has
    /// nothing to put back.
    func testArmingWithNoStoredIdentityIsHarmless() throws {
        let keys = FakeSecretStore(key: nil)
        let wipe = EmergencyWipe(keyStore: keys, paths: paths, files: makeFiles())

        try wipe.arm()
        try wipe.undo()

        XCTAssertNil(keys.key)
        XCTAssertEqual(keys.restoreCount, 0)
    }

    /// Missing paths are skipped rather than throwing: a fresh profile has no
    /// `-wal`, and a never-recovered install has no quarantine directory.
    func testAbsentPathsAreSkipped() throws {
        let keys = FakeSecretStore()
        let files = FakeFiles(existing: ["/store/riot.db"])
        let wipe = EmergencyWipe(keyStore: keys, paths: paths, files: files)

        try wipe.arm()
        XCTAssertEqual(try wipe.commit(), ["/store/riot.db"])
    }
}
