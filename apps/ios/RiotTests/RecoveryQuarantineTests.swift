import XCTest
@testable import RiotKit

/// The reusable recovery CORE, tested independently of any boundary. Every
/// restore/import boundary (profile open now; per-community, sync, app-drop,
/// storage-blob load later) shares these pieces, so their contract is pinned
/// here: quarantine MOVES/writes aside + writes a manifest + never deletes the
/// source's bytes; `recovering` degrades instead of throwing; `RecoveryReport`
/// names what happened.
final class RecoveryQuarantineTests: XCTestCase {
    private enum PoisonError: Error { case rejected }

    private func makeStorageDir() throws -> URL {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("quarantine-core-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    // MARK: - quarantine()

    /// A poisoned file is MOVED aside, a manifest records what/why/when/version,
    /// and the bytes are preserved — nothing is deleted.
    func testQuarantineFileWritesManifestAndPreservesBytes() throws {
        let storage = try makeStorageDir()
        let source = storage.appendingPathComponent("poison.bin")
        let payload = Data("corrupt-persisted-state".utf8)
        try payload.write(to: source)

        let quarantine = RecoveryQuarantine(storageDirectory: storage, appVersion: "test-1.0")
        let ref = try quarantine.quarantine(
            [.file(source)], reason: .profileOpen, error: PoisonError.rejected
        )

        // Manifest: what / why / when / version.
        XCTAssertEqual(ref.manifest.reason, "profile-open")
        XCTAssertEqual(ref.manifest.appVersion, "test-1.0")
        XCTAssertFalse(ref.manifest.timestamp.isEmpty)
        XCTAssertNotNil(ref.manifest.error)
        XCTAssertTrue(ref.manifest.artifacts.contains("poison.bin"))
        XCTAssertTrue(FileManager.default.fileExists(atPath: ref.manifestURL.path),
            "manifest.json written beside the data")

        // Preserved, not deleted: the source path is emptied (moved) but the bytes
        // survive verbatim in the quarantine directory.
        XCTAssertFalse(FileManager.default.fileExists(atPath: source.path),
            "the source was relocated off the primary path")
        let moved = ref.directory.appendingPathComponent("poison.bin")
        XCTAssertEqual(try Data(contentsOf: moved), payload,
            "the quarantined bytes are identical — no data loss")

        // The append-only recovery.log recorded the auto-fix.
        let log = storage.appendingPathComponent("quarantine").appendingPathComponent("recovery.log")
        let logText = try String(contentsOf: log, encoding: .utf8)
        XCTAssertTrue(logText.contains("profile-open"), "recovery.log names the reason")
    }

    /// A poisoned blob (bytes in hand, no source file) is written aside verbatim.
    func testQuarantineBlobWritesBytes() throws {
        let storage = try makeStorageDir()
        let quarantine = RecoveryQuarantine(storageDirectory: storage, appVersion: "test-1.0")
        let poison = Data([0xDE, 0xAD, 0xBE, 0xEF])

        let ref = try quarantine.quarantine(
            [.blob(name: "bundle.cbor", poison)], reason: .space, error: nil
        )

        XCTAssertEqual(ref.manifest.reason, "space")
        XCTAssertNil(ref.manifest.error, "a nil error is recorded as nil, not fabricated")
        XCTAssertTrue(ref.manifest.artifacts.contains("bundle.cbor"))
        XCTAssertEqual(
            try Data(contentsOf: ref.directory.appendingPathComponent("bundle.cbor")), poison)
    }

    /// A missing source is skipped, but the manifest is still written so the fault
    /// is logged even when there was no file to relocate (an in-memory-only
    /// failure).
    func testMissingFileSkippedButManifestStillWritten() throws {
        let storage = try makeStorageDir()
        let quarantine = RecoveryQuarantine(storageDirectory: storage, appVersion: "test-1.0")

        let ref = try quarantine.quarantine(
            [.file(storage.appendingPathComponent("does-not-exist"))],
            reason: .alertReplay, error: PoisonError.rejected
        )

        XCTAssertTrue(ref.manifest.artifacts.isEmpty, "nothing relocated")
        XCTAssertTrue(FileManager.default.fileExists(atPath: ref.manifestURL.path),
            "the fault is still recorded")
        XCTAssertEqual(ref.manifest.reason, "alert-replay")
    }

    /// `list()` enumerates every quarantined item for a recovery UI.
    func testListReturnsEveryQuarantinedItem() throws {
        let storage = try makeStorageDir()
        let quarantine = RecoveryQuarantine(storageDirectory: storage, appVersion: "test-1.0")

        _ = try quarantine.quarantine([.blob(name: "a", Data("a".utf8))], reason: .space, error: nil)
        _ = try quarantine.quarantine([.blob(name: "b", Data("b".utf8))], reason: .alertReplay, error: nil)

        let all = try quarantine.list()
        XCTAssertEqual(all.count, 2)
        XCTAssertEqual(Set(all.map(\.manifest.reason)), ["space", "alert-replay"])
    }

    // MARK: - recovering()

    func testRecoveringReturnsBodyOnSuccess() {
        let value = recovering(step: .space) { 42 } onFailure: { _ in -1 }
        XCTAssertEqual(value, 42)
    }

    func testRecoveringDegradesOnFailure() {
        var seen: Error?
        let value: Int = recovering(step: .space) {
            throw PoisonError.rejected
        } onFailure: { error in
            seen = error
            return 7
        }
        XCTAssertEqual(value, 7, "the degraded fallback is returned, not thrown")
        XCTAssertNotNil(seen, "the failure handler saw the underlying error")
    }

    // MARK: - RecoveryReport

    func testReportRecordsHealedDroppedAndQuarantined() throws {
        let storage = try makeStorageDir()
        let quarantine = RecoveryQuarantine(storageDirectory: storage, appVersion: "test-1.0")
        let ref = try quarantine.quarantine([.blob(name: "x", Data())], reason: .profileOpen, error: nil)

        let report = RecoveryReport()
        XCTAssertTrue(report.isClean, "a fresh report is clean")

        report.recordHealed(.profileOpen, quarantine: ref)
        report.recordDropped(.space, quarantine: nil)
        report.recordDropped(.alertReplay, quarantine: nil)
        report.recordDropped(.alertReplay, quarantine: nil)

        XCTAssertFalse(report.isClean)
        XCTAssertTrue(report.quarantinedProfile)
        XCTAssertTrue(report.spaceDropped)
        XCTAssertEqual(report.alertsSkipped, 2)
        XCTAssertEqual(report.quarantined.count, 1, "only the healed step carried a ref")
        XCTAssertEqual(report.quarantineLocation, ref.directory)
    }
}
