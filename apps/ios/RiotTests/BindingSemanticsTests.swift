import XCTest
@testable import RiotKit

final class BindingSemanticsTests: XCTestCase {
    func testEmptyProtectedProfileOpensWithAnEmptyBoard() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let storage = try ProtectedProfileStorage(fileURL: directory.appendingPathComponent("profile.json"))

        let repository = try RiotProfileRepository.open(storage: storage)

        XCTAssertNil(repository.currentSpace)
        XCTAssertEqual(try repository.currentEntries(), [])
    }

    func testSignedAlertSurvivesProtectedOfflineReloadWithFullIdentityMetadata() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let snapshotURL = directory.appendingPathComponent("profile.json")
        let storage = try ProtectedProfileStorage(fileURL: snapshotURL)

        let first = try RiotProfileRepository.open(storage: storage)
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
        let reloaded = try RiotProfileRepository.open(storage: storage)
        let entries = try reloaded.currentEntries()

        XCTAssertEqual(entries.count, 1)
        XCTAssertEqual(entries[0].entryID, signed.entryID)
        XCTAssertEqual(entries[0].namespaceID, signed.namespaceID)
        XCTAssertEqual(entries[0].signerID, signed.signerID)
        XCTAssertEqual(entries[0].createdAt, signed.createdAt)
        XCTAssertEqual(entries[0].expiresAt, expiresAt)
        XCTAssertTrue(entries[0].aiAssisted)
    }
}
