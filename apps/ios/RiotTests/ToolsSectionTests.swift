import XCTest
@testable import RiotKit

/// Model-layer tests for the Tools surface: the starter tools appear once a
/// space exists, trusting one flips its listing, and a starter set that fails to
/// load leaves the list empty without surfacing an error.
@MainActor
final class ToolsSectionTests: XCTestCase {
    /// `fixtures/apps` resolved four levels up from this file, matching
    /// `AppRepositoryTests`.
    private static func starterPacks(file: StaticString = #filePath) throws -> [(manifest: Data, bundle: Data)] {
        let apps = URL(fileURLWithPath: "\(file)")
            .deletingLastPathComponent() // RiotTests
            .deletingLastPathComponent() // ios
            .deletingLastPathComponent() // apps
            .deletingLastPathComponent() // repo root
            .appendingPathComponent("fixtures/apps")
        let manifest = try Data(contentsOf: apps.appendingPathComponent("checklist.manifest.cbor"))
        let bundle = try Data(contentsOf: apps.appendingPathComponent("checklist.bundle.cbor"))
        return [(manifest: manifest, bundle: bundle)]
    }

    private func isolatedDirectory() -> URL {
        FileManager.default.temporaryDirectory.appendingPathComponent("tools-\(UUID().uuidString)")
    }

    func testAppsRefreshAfterSpaceCreationAndTrustFlipsListing() throws {
        let model = RiotAppModel()
        model.bootstrap(
            storageDirectory: isolatedDirectory(),
            keyStore: TestWrappingKeyStore(),
            starterPacks: try Self.starterPacks()
        )

        // No space joined yet: the Tools list mirrors the empty entries list.
        XCTAssertTrue(model.apps.isEmpty)

        model.createSpace(title: "Berlin Mutual Aid")
        XCTAssertEqual(model.apps.count, 1)
        XCTAssertEqual(model.apps[0].name, "Checklist")
        XCTAssertFalse(model.apps[0].trusted)

        model.trustApp(appID: model.apps[0].appIDHex)
        XCTAssertTrue(model.apps[0].trusted)
        XCTAssertNil(model.errorMessage)
    }

    func testStarterPacksFailingToLoadLeavesAppsEmptyWithoutError() throws {
        let model = RiotAppModel()
        model.bootstrap(
            storageDirectory: isolatedDirectory(),
            keyStore: TestWrappingKeyStore(),
            starterPacks: []
        )
        model.createSpace(title: "Berlin Mutual Aid")

        XCTAssertTrue(model.apps.isEmpty)
        XCTAssertNil(model.errorMessage)
    }
}

/// Duplicated per the project convention (the copies in the other test files are
/// `private`); a fixed 32-byte key so sealed identities round-trip.
private final class TestWrappingKeyStore: WrappingKeyStore {
    private var key: Data?

    func loadOrCreateWrappingKey() throws -> Data {
        if let key { return key }
        let created = Data(repeating: 0x5a, count: 32)
        key = created
        return created
    }
}
