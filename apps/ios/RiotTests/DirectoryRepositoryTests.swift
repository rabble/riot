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

    // MARK: - Getting an app this profile carries but has not taken up

    /// A profile that holds an app's bytes in its store without having taken the
    /// app up — exactly the state a neighbour's app arrives in over sync. It is
    /// reached here without a second device: the repository is opened with no
    /// starter packs (so nothing is held), and `shareApp` writes the pair into
    /// the store from Rust's own starter catalog.
    private func repositoryCarryingChecklist(
        storage: ProtectedProfileStorage? = nil,
        keyStore: WrappingKeyStore = TestWrappingKeyStore()
    ) throws -> (RiotProfileRepository, DirectoryListing) {
        let storage = try storage ?? {
            let directory = FileManager.default.temporaryDirectory
                .appendingPathComponent("carried-\(UUID().uuidString)", isDirectory: true)
            return try ProtectedProfileStorage(fileURL: directory.appendingPathComponent("profile.json"))
        }()
        let repository = try RiotProfileRepository.open(
            storage: storage,
            keyStore: keyStore,
            starterPacks: []
        )
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")
        let listing = try XCTUnwrap(
            repository.directoryListings().first { $0.name == "Checklist" }
        )
        XCTAssertTrue(try repository.installedApps().isEmpty)
        try repository.shareApp(appID: listing.appId)
        return (repository, listing)
    }

    /// The dead end this closes: an app present in the store but not held offers
    /// to be got, and getting it flips the row to Review — untrusted, so nothing
    /// runs until this person approves it.
    func testGettingACarriedAppMakesItHeldAndReviewable() throws {
        let (repository, listing) = try repositoryCarryingChecklist()
        let model = RiotDirectoryModel(port: repository)

        model.refresh()
        let before = try XCTUnwrap(model.rows.first { $0.name == "Checklist" })
        XCTAssertEqual(before.availability, .get)

        model.get(before)

        XCTAssertNil(model.errorMessage)
        XCTAssertEqual(model.confirmation, "Got Checklist — review it before it runs")
        let after = try XCTUnwrap(model.rows.first { $0.name == "Checklist" })
        guard case let .review(app) = after.availability else {
            return XCTFail("expected a carried app to be reviewable once got, got \(after.availability)")
        }
        XCTAssertEqual(app.appIDHex, RiotDirectoryRow.hex(listing.appId))
        XCTAssertFalse(app.trusted)
    }

    /// The "No tools yet" regression, and the reason a got app appeared NOWHERE.
    ///
    /// The directory performed the get and refreshed only ITSELF. The app model —
    /// whose `apps` is the Spaces → Tools card, and Tools is the only route to
    /// Open — never heard, so the card still said "No tools yet" about an app the
    /// profile was holding. Both models here read the SAME repository, exactly as
    /// the shell wires them.
    func testGettingACarriedAppFillsTheToolsCardNotJustTheDirectory() throws {
        let storageDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("tools-card-\(UUID().uuidString)", isDirectory: true)
        let model = RiotAppModel()
        model.bootstrap(
            storageDirectory: storageDirectory,
            keyStore: TestWrappingKeyStore(),
            starterPacks: []
        )
        let repository = try XCTUnwrap(model.profileRepository)
        model.createSpace(title: "Berlin Mutual Aid")

        // Put the checklist in the store WITHOUT holding it — a neighbour's app,
        // as it arrives over sync.
        let listing = try XCTUnwrap(
            repository.directoryListings().first { $0.name == "Checklist" }
        )
        try repository.shareApp(appID: listing.appId)
        XCTAssertTrue(model.apps.isEmpty, "nothing is held yet: Tools correctly says 'No tools yet'")

        let directory = RiotDirectoryModel(port: repository)
        directory.refresh()
        let row = try XCTUnwrap(directory.rows.first { $0.name == "Checklist" })
        XCTAssertEqual(row.availability, .get)

        directory.get(row) // the exact tap

        XCTAssertNil(directory.errorMessage)
        XCTAssertEqual(
            model.apps.count, 1,
            "the app is held — the Tools card must not still say 'No tools yet'"
        )
        XCTAssertEqual(model.apps.first?.name, "Checklist")
        XCTAssertFalse(
            try XCTUnwrap(model.apps.first).trusted,
            "getting an app must not turn it on — the review gate still stands"
        )
    }

    /// Sharing and recommending do NOT change what this profile holds, so the
    /// Tools card has no equivalent gap. Pinned so the notification is not
    /// "fixed" later by firing it from everything that touches an app.
    func testSharingAnAppDoesNotChangeTheHeldAppsCard() throws {
        let storageDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("tools-share-\(UUID().uuidString)", isDirectory: true)
        let model = RiotAppModel()
        model.bootstrap(
            storageDirectory: storageDirectory,
            keyStore: TestWrappingKeyStore(),
            starterPacks: try Self.starterPacks()
        )
        let repository = try XCTUnwrap(model.profileRepository)
        model.createSpace(title: "Berlin Mutual Aid")
        let held = model.apps
        XCTAssertEqual(held.count, 1, "the starter checklist is held on a fresh profile")

        let listing = try XCTUnwrap(
            repository.directoryListings().first { $0.name == "Checklist" }
        )
        let directory = RiotDirectoryModel(port: repository)
        directory.refresh()
        try repository.shareApp(appID: listing.appId)
        directory.recommend(
            try XCTUnwrap(directory.rows.first { $0.name == "Checklist" }),
            note: "we use this every week"
        )

        XCTAssertEqual(model.apps.map(\.appIDHex), held.map(\.appIDHex))
    }

    // MARK: - Who may approve, and what the refusal says

    /// A fresh profile that creates a space IS its organizer: the button is offered
    /// and the approval lands. This is the path rabble expected to work.
    func testCreatingASpaceMakesYouItsOrganizerAndApprovalWorks() throws {
        let storageDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent("organizer-\(UUID().uuidString)", isDirectory: true)
        let model = RiotAppModel()
        model.bootstrap(
            storageDirectory: storageDirectory,
            keyStore: TestWrappingKeyStore(),
            starterPacks: try Self.starterPacks()
        )
        model.createSpace(title: "Berlin Mutual Aid")

        XCTAssertTrue(model.canApproveApps, "the space's creator is its organizer")
        XCTAssertFalse(model.isLegacyProfile)
        XCTAssertNil(AppReviewSheet.unavailableReason(canApprove: true, isLegacyProfile: false))

        let app = try XCTUnwrap(model.apps.first)
        XCTAssertFalse(app.trusted)
        model.trustApp(appID: app.appIDHex)

        XCTAssertNil(model.errorMessage, "an organizer's approval must not fail")
        XCTAssertTrue(try XCTUnwrap(model.apps.first).trusted, "the app is now on for the space")
    }

    /// The sheet must not offer a button that cannot succeed, and the sentence it
    /// shows instead has to be true for the person reading it. A member is told to
    /// ask the organizer; only a pre-organizer profile is told to start a new one.
    func testTheApproveButtonIsReplacedByAnHonestSentenceWhenItCannotSucceed() {
        let member = AppReviewSheet.unavailableReason(canApprove: false, isLegacyProfile: false)
        XCTAssertEqual(member, "Only the organizer of this space can turn an app on here.")

        let legacy = AppReviewSheet.unavailableReason(canApprove: false, isLegacyProfile: true)
        XCTAssertEqual(
            legacy,
            "This profile was made before spaces had organizers, so it can’t "
                + "approve apps for this space. Start a new profile to organize one."
        )
        XCTAssertNotEqual(member, legacy, "the two cases need opposite advice")

        for message in [member, legacy].compactMap({ $0 }) {
            XCTAssertFalse(message.contains("InvalidInput"), "no error codes")
            XCTAssertFalse(message.lowercased().contains("namespace"), "no jargon")
            XCTAssertFalse(message.lowercased().contains("subspace"), "no jargon")
        }
    }

    /// If an approval ever does fail, the person must get words rather than the
    /// `InvalidInput` that left rabble with a closed sheet and no explanation.
    func testApprovalFailuresAreTranslatedOutOfErrorCodes() {
        XCTAssertEqual(
            RiotAppModel.approvalFailureMessage(MobileError.LegacyProfileCannotOrganize),
            "This profile was made before spaces had organizers, so it can’t "
                + "approve apps for this space. Start a new profile to organize one."
        )
        XCTAssertEqual(
            RiotAppModel.approvalFailureMessage(MobileError.NotSpaceOrganizer),
            "Only the organizer of this space can turn an app on here."
        )
    }

    /// The other half of the hop: once approved, the app a neighbour carried is
    /// served to the WebView from the store's own bytes — a carried app has no
    /// file on this device, so this is the only copy there is.
    func testACarriedAppIsServedFromTheStoresBytesOnceApproved() throws {
        let (repository, listing) = try repositoryCarryingChecklist()
        let app = try repository.getCarriedApp(appID: listing.appId)

        // Untrusted, the host will not even hand out a resolver.
        XCTAssertNil(repository.appResolver(appID: app.appIDHex))

        try repository.trustApp(appID: app.appIDHex)

        let resolver = try XCTUnwrap(repository.appResolver(appID: app.appIDHex))
        let entry = try repository.appResource(appID: app.appIDHex, path: resolver.entryPoint)
        XCTAssertFalse(entry.bytes.isEmpty)
        XCTAssertTrue(entry.contentType.contains("html"))
    }

    /// A carried app you got and turned on is still yours after a relaunch. The
    /// store it arrived in is in-memory, so the profile keeps its own copy of the
    /// verified bytes and re-installs the app on open — and it is still trusted,
    /// still served, and still on the surface that offered it.
    func testACarriedAppSurvivesARelaunch() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("relaunch-\(UUID().uuidString)", isDirectory: true)
        let file = directory.appendingPathComponent("profile.json")
        let keyStore = TestWrappingKeyStore()

        let appIDHex: String
        do {
            let (repository, listing) = try repositoryCarryingChecklist(
                storage: try ProtectedProfileStorage(fileURL: file),
                keyStore: keyStore
            )
            let app = try repository.getCarriedApp(appID: listing.appId)
            try repository.trustApp(appID: app.appIDHex)
            appIDHex = app.appIDHex
        }

        // A new process: a new profile over the same protected snapshot. Nothing
        // is in the store — no sync has happened — so the app can only come back
        // from the bytes the snapshot kept.
        let reopened = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: file),
            keyStore: keyStore,
            starterPacks: []
        )

        let held = try XCTUnwrap(reopened.installedApps().first { $0.appIDHex == appIDHex })
        XCTAssertEqual(held.name, "Checklist")
        XCTAssertTrue(held.trusted)

        // Still served: the WebView can still load its pages.
        let resolver = try XCTUnwrap(reopened.appResolver(appID: appIDHex))
        XCTAssertFalse(try reopened.appResource(appID: appIDHex, path: resolver.entryPoint).bytes.isEmpty)

        // Still on the Apps surface, ready to open.
        let model = RiotDirectoryModel(port: reopened)
        model.refresh()
        let row = try XCTUnwrap(model.rows.first { $0.appIDHex == appIDHex })
        guard case .open = row.availability else {
            return XCTFail("expected a kept, trusted app to still open after relaunch, got \(row.availability)")
        }
    }

    /// Honest failure: an app whose bytes never arrived cannot be got, and the
    /// storefront says so in plain language rather than silently doing nothing.
    func testGettingAnAppThatNeverArrivedIsRefused() throws {
        let (repository, _) = try repositoryCarryingChecklist()
        let absent = Data(repeating: 0x11, count: 32)

        XCTAssertThrowsError(try repository.getCarriedApp(appID: absent)) { error in
            XCTAssertEqual(error as? MobileError, .AppRejected)
        }
        XCTAssertEqual(
            RiotDirectoryModel.getFailureMessage(name: "Checklist", error: MobileError.AppRejected),
            "Checklist isn’t all here yet. Sync with the group carrying it, then try again."
        )
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
