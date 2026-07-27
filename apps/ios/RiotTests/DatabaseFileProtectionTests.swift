import XCTest

@testable import RiotKit

/// Records what was asked of the filesystem so the protection contract can be
/// asserted on any platform — the real data-protection class is an iOS device
/// behaviour, so testing through `FileManager` would pass vacuously on macOS
/// and in the simulator.
private final class SpyAttributeWriter: FileAttributeWriter {
    var existing: Set<String>
    private(set) var applied: [(path: String, attributes: [FileAttributeKey: Any])] = []
    var failOn: String?

    init(existing: Set<String>) { self.existing = existing }

    func fileExists(atPath path: String) -> Bool { existing.contains(path) }

    func setAttributes(_ attributes: [FileAttributeKey: Any], ofItemAtPath path: String) throws {
        if path == failOn {
            throw NSError(domain: NSCocoaErrorDomain, code: NSFileWriteNoPermissionError)
        }
        applied.append((path, attributes))
    }
}

final class DatabaseFileProtectionTests: XCTestCase {
    private let db = "/tmp/riot-test/riot.db"

    /// The whole point: the `-wal` sidecar holds the most RECENT entries, so
    /// protecting only the main database file would leave the newest content at
    /// the platform default. Every sidecar that exists is covered.
    func testProtectsTheDatabaseAndEveryExistingSidecar() throws {
        let writer = SpyAttributeWriter(existing: [db, db + "-wal", db + "-shm"])
        let updated = try DatabaseFileProtection.apply(databasePath: db, using: writer)

        XCTAssertEqual(updated, [db, db + "-wal", db + "-shm"])
        XCTAssertEqual(writer.applied.count, 3)
        for entry in writer.applied {
            XCTAssertEqual(
                entry.attributes[.protectionKey] as? FileProtectionType,
                .completeUntilFirstUserAuthentication,
                "\(entry.path) must be pinned to the declared class")
        }
    }

    /// `-wal`/`-shm` exist only while the database is open in WAL mode and
    /// `-journal` only in rollback mode, so an absent sidecar is normal and must
    /// not throw — but the applier must not claim to have protected it either.
    func testSkipsSidecarsThatDoNotExistWithoutFailing() throws {
        let writer = SpyAttributeWriter(existing: [db])
        let updated = try DatabaseFileProtection.apply(databasePath: db, using: writer)

        XCTAssertEqual(updated, [db])
        XCTAssertEqual(writer.applied.map(\.path), [db])
    }

    /// A database that was never created (in-memory fallback) protects nothing
    /// and is not an error.
    func testAbsentDatabaseIsANoOp() throws {
        let writer = SpyAttributeWriter(existing: [])
        XCTAssertEqual(try DatabaseFileProtection.apply(databasePath: db, using: writer), [])
        XCTAssertTrue(writer.applied.isEmpty)
    }

    /// The class must be `completeUntilFirstUserAuthentication`, NOT `complete`:
    /// background sync and notification work read the store while the screen is
    /// locked, and `complete` would make it unreadable exactly there. Pinned so
    /// an "upgrade" to the stricter class is a deliberate, test-breaking choice.
    func testPinsUntilFirstUserAuthenticationSoLockedScreenSyncKeepsWorking() {
        XCTAssertEqual(
            DatabaseFileProtection.protectionClass, .completeUntilFirstUserAuthentication)
    }

    /// A filesystem refusal propagates rather than being swallowed — a database
    /// silently left at the default class is the bug this whole type exists to
    /// prevent.
    func testPropagatesAFilesystemFailure() {
        let writer = SpyAttributeWriter(existing: [db, db + "-wal"])
        writer.failOn = db + "-wal"
        XCTAssertThrowsError(try DatabaseFileProtection.apply(databasePath: db, using: writer))
    }
}
