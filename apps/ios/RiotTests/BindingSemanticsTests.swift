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
