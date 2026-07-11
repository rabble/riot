import XCTest
@testable import RiotKit

/// The storefront against the real Rust directory: the built-in app is listed on
/// a fresh profile, its raw id matches the app this device actually holds, and
/// turning it on for the space flips the row from Review to Open. The rules
/// themselves are pinned without an FFI in `DirectoryStorefrontTests`; this is
/// the seam test that the port is wired to the core the way those rules assume.
@MainActor
final class DirectoryRepositoryTests: XCTestCase {
    /// `fixtures/apps` resolved four levels up from this file, matching
    /// `ToolsSectionTests`.
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

    private func openRepository() throws -> RiotProfileRepository {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("directory-\(UUID().uuidString)", isDirectory: true)
        let storage = try ProtectedProfileStorage(fileURL: directory.appendingPathComponent("profile.json"))
        return try RiotProfileRepository.open(
            storage: storage,
            keyStore: TestWrappingKeyStore(),
            starterPacks: try Self.starterPacks()
        )
    }

    /// The empty-handed case rabble asked about: a brand-new profile that has
    /// met nobody still sees the built-in app, knows what it does, and can
    /// review it. Nothing here depends on a space existing yet.
    func testBuiltInAppIsListedAndReviewableBeforeAnySpaceExists() throws {
        let repository = try openRepository()
        let model = RiotDirectoryModel(port: repository)

        model.refresh()

        XCTAssertNil(model.errorMessage)
        let row = try XCTUnwrap(model.rows.first { $0.name == "Checklist" })
        XCTAssertFalse(row.description.isEmpty)
        XCTAssertTrue(row.badges.contains("Built in"))
        // Its bytes ship with the app, so it is never "still arriving".
        XCTAssertFalse(row.badges.contains("Still arriving from your group"))
        // Held on this device, but no organizer has turned it on: Review.
        guard case .review = row.availability else {
            return XCTFail("expected the built-in app to be reviewable, got \(row.availability)")
        }
        // Nothing to recommend to and nowhere to pass it on to yet.
        XCTAssertFalse(row.canRecommend)
        XCTAssertFalse(row.canShare)
    }

    /// The id seam: Rust addresses directory rows by raw bytes and the installed
    /// store by hex text. If these ever disagree the row silently loses its Open
    /// button, so it is asserted directly.
    func testDirectoryRowIDMatchesTheHeldAppsHexID() throws {
        let repository = try openRepository()

        let listing = try XCTUnwrap(
            repository.directoryListings().first { $0.name == "Checklist" }
        )
        let held = try XCTUnwrap(repository.installedApps().first { $0.name == "Checklist" })

        XCTAssertEqual(RiotDirectoryRow.hex(listing.appId), held.appIDHex.lowercased())
    }

    func testTurningTheAppOnForTheSpaceFlipsTheRowToOpenAndUnlocksRecommending() throws {
        let repository = try openRepository()
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")
        let model = RiotDirectoryModel(port: repository)

        model.refresh()
        let before = try XCTUnwrap(model.rows.first { $0.name == "Checklist" })
        // A space exists, so it can be passed on — but it is not on yet, so it
        // cannot be recommended.
        XCTAssertTrue(before.canShare)
        XCTAssertFalse(before.canRecommend)
        XCTAssertFalse(before.badges.contains("On in this space"))

        try repository.trustApp(appID: before.appIDHex)
        model.refresh()

        let after = try XCTUnwrap(model.rows.first { $0.name == "Checklist" })
        XCTAssertTrue(after.badges.contains("On in this space"))
        XCTAssertTrue(after.canRecommend)
        guard case let .open(app) = after.availability else {
            return XCTFail("expected a trusted, held app to be openable, got \(after.availability)")
        }
        XCTAssertEqual(app.appIDHex, after.appIDHex)
    }

    /// Recommending and passing an app on both reach Rust and come back with a
    /// plain-language receipt rather than an error.
    func testRecommendingAndSharingReachTheCoreWithoutError() throws {
        let repository = try openRepository()
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")
        let model = RiotDirectoryModel(port: repository)
        model.refresh()
        let row = try XCTUnwrap(model.rows.first { $0.name == "Checklist" })
        try repository.trustApp(appID: row.appIDHex)
        model.refresh()

        let onNow = try XCTUnwrap(model.rows.first { $0.name == "Checklist" })
        model.recommend(onNow, note: "We used it all weekend")
        XCTAssertNil(model.errorMessage)
        XCTAssertEqual(model.confirmation, "Recommended Checklist")

        model.share(onNow)
        XCTAssertNil(model.errorMessage)
        XCTAssertEqual(model.confirmation, "Shared Checklist to Berlin Mutual Aid")
    }

    /// Passing an app on requires somewhere to pass it to.
    func testSharingWithoutASpaceIsRefusedByTheRepository() throws {
        let repository = try openRepository()
        let listing = try XCTUnwrap(
            repository.directoryListings().first { $0.name == "Checklist" }
        )

        XCTAssertThrowsError(try repository.shareApp(appID: listing.appId)) { error in
            XCTAssertEqual(error as? RepositoryError, .noCurrentSpace)
        }
    }
}

extension RepositoryError: @retroactive Equatable {
    public static func == (lhs: RepositoryError, rhs: RepositoryError) -> Bool {
        String(describing: lhs) == String(describing: rhs)
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
