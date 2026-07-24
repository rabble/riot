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

        XCTAssertTrue(model.trustApp(appID: model.apps[0].appIDHex))
        XCTAssertTrue(model.apps[0].trusted)
        XCTAssertNil(model.errorMessage)
    }

    func testTrustAppUsesTheExplicitCommunityAndRejectsAStaleCommunity() throws {
        let model = RiotAppModel()
        model.bootstrap(
            storageDirectory: isolatedDirectory(),
            keyStore: TestWrappingKeyStore(),
            starterPacks: try Self.starterPacks()
        )
        model.createSpace(title: "Berlin Mutual Aid")

        let app = try XCTUnwrap(model.apps.first)
        let namespaceID = try XCTUnwrap(model.space?.namespaceID)

        XCTAssertFalse(
            model.trustApp(appID: app.appIDHex, expectedNamespaceID: "stale-\(namespaceID)")
        )
        XCTAssertFalse(try XCTUnwrap(model.apps.first).trusted)
        XCTAssertNotNil(model.errorMessage)

        XCTAssertTrue(
            model.trustApp(appID: app.appIDHex, expectedNamespaceID: namespaceID)
        )
        XCTAssertTrue(try XCTUnwrap(model.apps.first).trusted)
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

    /// The "Add a tool" model path: installing from a chosen pair shows the tool
    /// UNTRUSTED (install turns nothing on), and the same trust step AppReviewSheet
    /// drives flips it to trusted — the empty state stops being a dead-end.
    func testInstallToolAddsUntrustedToolThenAppReviewTrustPathFlipsIt() throws {
        let model = RiotAppModel()
        model.bootstrap(
            storageDirectory: isolatedDirectory(),
            keyStore: TestWrappingKeyStore(),
            starterPacks: []                       // start with NO tools -> the empty state
        )
        model.createSpace(title: "Berlin Mutual Aid")
        XCTAssertTrue(model.apps.isEmpty)          // the dead-end this unit fixes

        let (manifest, bundle) = try XCTUnwrap(try Self.starterPacks().first)
        let installed = try XCTUnwrap(model.installTool(manifest: manifest, bundle: bundle))

        XCTAssertEqual(model.apps.count, 1)
        XCTAssertEqual(model.apps[0].name, "Checklist")
        XCTAssertEqual(installed.appIDHex, model.apps[0].appIDHex)
        XCTAssertFalse(installed.trusted)
        XCTAssertFalse(model.apps[0].trusted, "installed via Add-a-tool must be untrusted until AppReviewSheet trusts it")
        XCTAssertNil(model.errorMessage)

        // The AppReviewSheet trust decision — the same path DirectoryView wires to onApprove.
        XCTAssertTrue(model.trustApp(appID: model.apps[0].appIDHex))
        XCTAssertTrue(model.apps[0].trusted)
        XCTAssertNil(model.errorMessage)
    }

    func testDurableTrustRecoveryVerificationRequiresExactCommunityAppAndState() {
        let app = RiotSpaceApp(
            appIDHex: "aabb",
            name: "Checklist",
            description: "Coordinate work",
            version: "1",
            permissions: [],
            trusted: true
        )
        let space = RiotSpace(namespaceID: "community-a", title: "Community A")

        XCTAssertTrue(RiotAppModel.durableTrustDecisionIsPresent(
            namespaceID: "COMMUNITY-A",
            appID: "AABB",
            trusted: true,
            currentSpace: space,
            apps: [app]
        ))
        XCTAssertFalse(RiotAppModel.durableTrustDecisionIsPresent(
            namespaceID: "community-b",
            appID: "aabb",
            trusted: true,
            currentSpace: space,
            apps: [app]
        ))
        XCTAssertFalse(RiotAppModel.durableTrustDecisionIsPresent(
            namespaceID: "community-a",
            appID: "ccdd",
            trusted: true,
            currentSpace: space,
            apps: [app]
        ))
        XCTAssertFalse(RiotAppModel.durableTrustDecisionIsPresent(
            namespaceID: "community-a",
            appID: "aabb",
            trusted: false,
            currentSpace: space,
            apps: [app]
        ))
    }

    func testDurableTrustRecoveryReopensPersistedProfileAndVerifiesGrant() throws {
        let model = RiotAppModel()
        model.bootstrap(
            storageDirectory: isolatedDirectory(),
            keyStore: TestWrappingKeyStore(),
            starterPacks: try Self.starterPacks()
        )
        model.createSpace(title: "Berlin Mutual Aid")
        let appID = try XCTUnwrap(model.apps.first?.appIDHex)
        let namespaceID = try XCTUnwrap(model.space?.namespaceID)
        XCTAssertTrue(model.trustApp(appID: appID, expectedNamespaceID: namespaceID))

        XCTAssertTrue(model.recoverDurableTrustDecision(
            namespaceID: namespaceID,
            appID: appID,
            trusted: true,
            requestedNamespaceID: namespaceID,
            requestedAppID: appID
        ))

        XCTAssertTrue(model.isProfileOpen)
        XCTAssertNotNil(model.profileRepository)
        XCTAssertEqual(model.space?.namespaceID, namespaceID)
        XCTAssertTrue(try XCTUnwrap(model.apps.first { $0.appIDHex == appID }).trusted)
        XCTAssertNil(model.errorMessage)
    }

    func testDurableTrustPayloadMustMatchTheOriginalApprovalRequest() {
        XCTAssertTrue(RiotAppModel.durableTrustPayloadMatchesRequest(
            namespaceID: "COMMUNITY-A",
            appID: "AABB",
            trusted: true,
            requestedNamespaceID: "community-a",
            requestedAppID: "aabb"
        ))
        XCTAssertFalse(RiotAppModel.durableTrustPayloadMatchesRequest(
            namespaceID: "community-b",
            appID: "aabb",
            trusted: true,
            requestedNamespaceID: "community-a",
            requestedAppID: "aabb"
        ))
        XCTAssertFalse(RiotAppModel.durableTrustPayloadMatchesRequest(
            namespaceID: "community-a",
            appID: "ccdd",
            trusted: true,
            requestedNamespaceID: "community-a",
            requestedAppID: "aabb"
        ))
        XCTAssertFalse(RiotAppModel.durableTrustPayloadMatchesRequest(
            namespaceID: "community-a",
            appID: "aabb",
            trusted: false,
            requestedNamespaceID: "community-a",
            requestedAppID: "aabb"
        ))
    }

    func testFailedDurableTrustReopenClearsTheRepositoryProjection() throws {
        let root = isolatedDirectory()
        let storage = root.appendingPathComponent("durable-profile")
        let target = root.appendingPathComponent("profile-target")
        try FileManager.default.createDirectory(at: target, withIntermediateDirectories: true)
        try Data("{corrupt-profile".utf8).write(
            to: target.appendingPathComponent("riot-profile.json")
        )
        try FileManager.default.createSymbolicLink(at: storage, withDestinationURL: target)
        defer { try? FileManager.default.removeItem(at: root) }

        let model = RiotAppModel()
        model.bootstrap(
            storageDirectory: storage,
            keyStore: TestWrappingKeyStore(),
            starterPacks: try Self.starterPacks()
        )
        XCTAssertNotNil(model.recoveryNotice)
        model.createSpace(title: "Berlin Mutual Aid")
        XCTAssertTrue(model.setDisplayName("Ana"))
        model.markCommunityUnavailable(CommunityUnavailable(name: "Berlin Mutual Aid"))
        let appID = try XCTUnwrap(model.apps.first?.appIDHex)
        let namespaceID = try XCTUnwrap(model.space?.namespaceID)
        model.recordSynced(namespaceID: namespaceID)
        XCTAssertTrue(model.trustApp(appID: appID, expectedNamespaceID: namespaceID))
        XCTAssertNotNil(model.me)
        XCTAssertFalse(model.communities.isEmpty)
        XCTAssertTrue(model.canApproveApps)
        XCTAssertNotNil(model.lastSyncedText(for: namespaceID))

        try FileManager.default.removeItem(at: storage)
        XCTAssertTrue(FileManager.default.createFile(atPath: storage.path, contents: Data()))

        XCTAssertFalse(model.recoverDurableTrustDecision(
            namespaceID: namespaceID,
            appID: appID,
            trusted: true,
            requestedNamespaceID: namespaceID,
            requestedAppID: appID
        ))

        XCTAssertEqual(
            model.errorMessage,
            "The tool approval was saved, but Riot couldn’t verify it after reopening "
                + "your profile. Restart Riot and check the tool before trying again."
        )
        XCTAssertNil(model.profileRepository)
        XCTAssertFalse(model.isProfileOpen)
        XCTAssertNil(model.space)
        XCTAssertNil(model.newswireDescriptorEntryID)
        XCTAssertNil(model.communityUnavailable)
        XCTAssertTrue(model.entries.isEmpty)
        XCTAssertTrue(model.apps.isEmpty)
        XCTAssertTrue(model.followedSites.isEmpty)
        XCTAssertTrue(model.displayNames.isEmpty)
        XCTAssertFalse(model.isDemoMode)
        XCTAssertNil(model.me)
        XCTAssertNil(model.claimedName)
        XCTAssertTrue(model.arrivals.isEmpty)
        XCTAssertTrue(model.communities.isEmpty)
        XCTAssertFalse(model.canApproveApps)
        XCTAssertFalse(model.isLegacyProfile)
        XCTAssertNil(model.demoLoader)
        XCTAssertNil(model.recoveryNotice)
        XCTAssertNil(model.openOutcome)
        XCTAssertNil(model.relaySyncResult)
        XCTAssertNil(model.relaySyncError)
        XCTAssertFalse(model.isRelaySyncing)
        XCTAssertTrue(model.lastSyncedByNamespace.isEmpty)
        XCTAssertEqual(model.connectionStatus, .offline)
    }

    /// A rejected file must say why, never vanish silently (the InvalidInput bug).
    func testInstallToolWithBrokenBytesSurfacesAMessageNotASilentNoOp() throws {
        let model = RiotAppModel()
        model.bootstrap(storageDirectory: isolatedDirectory(),
                        keyStore: TestWrappingKeyStore(), starterPacks: [])
        model.createSpace(title: "Berlin Mutual Aid")

        var (manifest, bundle) = try XCTUnwrap(try Self.starterPacks().first)
        bundle.removeLast(bundle.count / 2)
        model.installTool(manifest: manifest, bundle: bundle)

        XCTAssertTrue(model.apps.isEmpty)
        XCTAssertNotNil(model.errorMessage, "a rejected file must say why, never vanish silently (the InvalidInput bug)")
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
