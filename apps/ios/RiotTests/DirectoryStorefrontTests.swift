import XCTest
@testable import RiotKit

/// The storefront's product decisions, tested with no FFI behind them: a fake
/// `DirectoryPorting` stands in for the Rust directory, so every rule below —
/// which row can be opened, who may recommend, what the badges say — is pinned
/// independently of a profile, a space, or a signed bundle.
@MainActor
final class DirectoryStorefrontTests: XCTestCase {
    private let appID = Data([0xAB, 0xCD, 0xEF, 0x01])
    private let otherID = Data([0x99, 0x88])
    private let spaceID = Data([0x0A, 0x1B, 0x2C])

    private var space: RiotSpace {
        RiotSpace(namespaceID: RiotDirectoryRow.hex(spaceID), title: "Berlin Mutual Aid")
    }

    // MARK: - Matching a listing to what this device actually holds

    func testAppIDBytesMatchTheHexKeyOfAHeldApp() {
        XCTAssertEqual(RiotDirectoryRow.hex(Data([0x00, 0x0f, 0xa0, 0xff])), "000fa0ff")

        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Checklist")],
            installed: [heldApp(appID: appID, name: "Checklist", trusted: false)]
        )
        let model = RiotDirectoryModel(port: port)

        model.refresh()

        XCTAssertEqual(model.rows.count, 1)
        XCTAssertEqual(model.rows[0].availability, .review(port.installed[0]))
    }

    /// Rust hands the directory raw bytes and the installed store hex text; a
    /// row whose bytes name an app we do not hold cannot be opened or reviewed.
    func testListingWeDoNotHoldIsListedOnly() {
        let model = RiotDirectoryModel(
            port: FakeDirectoryPort(
                listings: [listing(appID: appID, name: "Checklist")],
                installed: [heldApp(appID: otherID, name: "Something else", trusted: true)]
            )
        )

        model.refresh()

        XCTAssertEqual(model.rows[0].availability, .listedOnly)
    }

    // MARK: - Trust in the current space

    func testAppIsOnlyOnWhenThisSpaceTrustsIt() {
        let trusted = listing(appID: appID, name: "Checklist", trustedInSpaces: [spaceID])

        XCTAssertTrue(RiotDirectoryRow.trustedInCurrentSpace(listing: trusted, space: space))
        // No space yet, so nobody has turned anything on.
        XCTAssertFalse(RiotDirectoryRow.trustedInCurrentSpace(listing: trusted, space: nil))
        // Trusted somewhere else is not trusted here.
        XCTAssertFalse(
            RiotDirectoryRow.trustedInCurrentSpace(
                listing: trusted,
                space: RiotSpace(namespaceID: RiotDirectoryRow.hex(otherID), title: "Elsewhere")
            )
        )
        XCTAssertFalse(
            RiotDirectoryRow.trustedInCurrentSpace(
                listing: listing(appID: appID, name: "Checklist"),
                space: space
            )
        )
    }

    func testHeldAndTrustedAppOpensWhileUntrustedOneIsReviewed() {
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Checklist", trustedInSpaces: [spaceID])],
            installed: [heldApp(appID: appID, name: "Checklist", trusted: true)],
            space: space
        )
        let model = RiotDirectoryModel(port: port)

        model.refresh()

        XCTAssertEqual(model.rows[0].availability, .open(port.installed[0]))
        XCTAssertTrue(model.rows[0].badges.contains("On in this space"))
    }

    /// The last hop of community discovery: an app an organizer has turned on,
    /// whose bytes are still coming across. It cannot be opened yet, and the row
    /// says so instead of offering a dead button.
    func testTrustedAppWhoseBytesHaveNotArrivedIsStillArriving() {
        let model = RiotDirectoryModel(
            port: FakeDirectoryPort(
                listings: [
                    listing(
                        appID: appID,
                        name: "Checklist",
                        bundlePresent: false,
                        trustedInSpaces: [spaceID]
                    )
                ],
                space: space
            )
        )

        model.refresh()

        XCTAssertEqual(model.rows[0].availability, .arriving)
        XCTAssertTrue(model.rows[0].badges.contains("Still arriving from your group"))
    }

    func testBuiltInAppIsBadgedAsBuiltIn() {
        let model = RiotDirectoryModel(
            port: FakeDirectoryPort(listings: [listing(appID: appID, name: "Checklist", builtIn: true)])
        )

        model.refresh()

        XCTAssertEqual(model.rows[0].badges, ["Built in"])
    }

    // MARK: - Recommending is gated on the app being on in this space

    func testRecommendIsOfferedOnlyWhereTheAppIsAlreadyOn() {
        let on = listing(appID: appID, name: "Checklist", trustedInSpaces: [spaceID])
        let off = listing(appID: appID, name: "Checklist")

        XCTAssertTrue(RiotDirectoryRow.canRecommend(listing: on, space: space))
        // Recommending speaks for a space that already trusts the app, so an app
        // this space has not turned on cannot be recommended from here...
        XCTAssertFalse(RiotDirectoryRow.canRecommend(listing: off, space: space))
        // ...and neither can anything at all before a space exists.
        XCTAssertFalse(RiotDirectoryRow.canRecommend(listing: on, space: nil))
    }

    func testRecommendingWritesAnEndorsementAndConfirmsIt() {
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Checklist", trustedInSpaces: [spaceID])],
            installed: [heldApp(appID: appID, name: "Checklist", trusted: true)],
            space: space
        )
        let model = RiotDirectoryModel(port: port)
        model.refresh()

        XCTAssertTrue(model.rows[0].canRecommend)
        model.recommend(model.rows[0], note: "We used it all weekend")

        XCTAssertEqual(port.endorsed.count, 1)
        XCTAssertEqual(port.endorsed[0].appID, appID)
        XCTAssertEqual(port.endorsed[0].note, "We used it all weekend")
        XCTAssertFalse(port.endorsed[0].retract)
        XCTAssertEqual(model.confirmation, "Recommended Checklist")
        XCTAssertNil(model.errorMessage)
    }

    // MARK: - Who else recommends it

    func testEndorsementSummaryCountsGroupsMetAndUnmet() {
        // Nobody has recommended it: the surface stays silent rather than
        // printing a zero.
        XCTAssertNil(RiotDirectoryRow.endorsementSummary(met: 0, unmet: 0))
        XCTAssertEqual(
            RiotDirectoryRow.endorsementSummary(met: 1, unmet: 0),
            "Recommended by 1 group you’ve met"
        )
        XCTAssertEqual(
            RiotDirectoryRow.endorsementSummary(met: 3, unmet: 0),
            "Recommended by 3 groups you’ve met"
        )
        XCTAssertEqual(
            RiotDirectoryRow.endorsementSummary(met: 0, unmet: 4),
            "Recommended by 4 you haven’t met"
        )
        XCTAssertEqual(
            RiotDirectoryRow.endorsementSummary(met: 2, unmet: 5),
            "Recommended by 2 groups you’ve met, 5 you haven’t met"
        )
    }

    func testRowCarriesTheEndorsementSummaryFromTheListing() {
        let model = RiotDirectoryModel(
            port: FakeDirectoryPort(
                listings: [
                    listing(
                        appID: appID,
                        name: "Checklist",
                        endorsingMet: [Data([0x01]), Data([0x02])],
                        endorsingUnmet: 5
                    )
                ]
            )
        )

        model.refresh()

        XCTAssertEqual(
            model.rows[0].endorsement,
            "Recommended by 2 groups you’ve met, 5 you haven’t met"
        )
    }

    // MARK: - Passing an app on

    func testSharingIsOfferedOnlyOnceASpaceExists() {
        let withoutSpace = RiotDirectoryModel(
            port: FakeDirectoryPort(listings: [listing(appID: appID, name: "Checklist")])
        )
        withoutSpace.refresh()
        XCTAssertFalse(withoutSpace.rows[0].canShare)

        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Checklist")],
            space: space
        )
        let withSpace = RiotDirectoryModel(port: port)
        withSpace.refresh()
        XCTAssertTrue(withSpace.rows[0].canShare)

        withSpace.share(withSpace.rows[0])

        XCTAssertEqual(port.shared, [appID])
        XCTAssertEqual(withSpace.confirmation, "Shared Checklist to Berlin Mutual Aid")
    }

    // MARK: - Surface-level behaviour

    func testEveryListingBecomesARowCarryingItsPlainLanguageFields() {
        let model = RiotDirectoryModel(
            port: FakeDirectoryPort(
                listings: [
                    listing(
                        appID: appID,
                        name: "Checklist",
                        description: "Keep a shared list of what still needs doing.",
                        version: "1.2.0",
                        permissions: ["Keep its own notes on this device"]
                    ),
                    listing(appID: otherID, name: "Legal support"),
                ]
            )
        )

        model.refresh()

        XCTAssertEqual(model.rows.map(\.name), ["Checklist", "Legal support"])
        XCTAssertEqual(model.rows[0].version, "1.2.0")
        XCTAssertEqual(model.rows[0].description, "Keep a shared list of what still needs doing.")
        XCTAssertEqual(model.rows[0].permissions, ["Keep its own notes on this device"])
        // An app that asks for nothing shows no "This app can:" section at all.
        XCTAssertTrue(model.rows[1].permissions.isEmpty)
    }

    func testDirectoryIsEmptyUntilAProfileIsOpened() {
        let model = RiotDirectoryModel()

        model.refresh()

        XCTAssertTrue(model.rows.isEmpty)
        XCTAssertNil(model.errorMessage)
    }

    func testFailingRefreshRecordsAnErrorAndKeepsTheRowsItAlreadyHas() {
        let port = FakeDirectoryPort(listings: [listing(appID: appID, name: "Checklist")])
        let model = RiotDirectoryModel(port: port)
        model.refresh()
        XCTAssertEqual(model.rows.count, 1)

        port.failure = FakeDirectoryError.unavailable
        model.refresh()

        XCTAssertNotNil(model.errorMessage)
        // Deliberate: a failed refresh keeps the last good rows rather than
        // blanking the surface. The error is what tells the person the list
        // may be stale.
        XCTAssertEqual(model.rows.count, 1)
    }

    /// The failure that has no stale rows to fall back on: without an error
    /// to show, this surface would render "No apps yet" — telling the person
    /// there are no apps when in truth the directory never loaded.
    func testFirstLoadFailureSurfacesAnErrorRatherThanLookingEmpty() {
        let port = FakeDirectoryPort(listings: [listing(appID: appID, name: "Checklist")])
        port.failure = FakeDirectoryError.unavailable
        let model = RiotDirectoryModel(port: port)

        model.refresh()

        XCTAssertTrue(model.rows.isEmpty)
        XCTAssertNotNil(model.errorMessage)
    }

    // MARK: - Fixtures

    private func listing(
        appID: Data,
        name: String,
        description: String = "What this app is for.",
        version: String = "1.0.0",
        permissions: [String] = [],
        bundlePresent: Bool = true,
        builtIn: Bool = false,
        trustedInSpaces: [Data] = [],
        endorsingMet: [Data] = [],
        endorsingUnmet: UInt32 = 0
    ) -> DirectoryListing {
        DirectoryListing(
            appId: appID,
            name: name,
            description: description,
            version: version,
            authorSigningKeyId: Data([0x01]),
            permissions: permissions,
            bundlePresent: bundlePresent,
            builtIn: builtIn,
            installed: false,
            carrierSubspaceId: nil,
            trustedInSpaces: trustedInSpaces,
            endorsingMetSubspaces: endorsingMet,
            endorsingUnmetCount: endorsingUnmet,
            supersededBy: nil
        )
    }

    private func heldApp(appID: Data, name: String, trusted: Bool) -> RiotSpaceApp {
        RiotSpaceApp(
            appIDHex: RiotDirectoryRow.hex(appID),
            name: name,
            description: "What this app is for.",
            version: "1.0.0",
            permissions: [],
            trusted: trusted
        )
    }
}

private enum FakeDirectoryError: Error {
    case unavailable
}

/// The Rust directory, faked: it records what the storefront asked it to write
/// so the tests can assert on the calls rather than on a profile's state.
private final class FakeDirectoryPort: DirectoryPorting {
    let listings: [DirectoryListing]
    let installed: [RiotSpaceApp]
    let currentSpace: RiotSpace?
    var failure: Error?

    private(set) var endorsed: [(appID: Data, note: String, retract: Bool)] = []
    private(set) var shared: [Data] = []

    init(
        listings: [DirectoryListing] = [],
        installed: [RiotSpaceApp] = [],
        space: RiotSpace? = nil
    ) {
        self.listings = listings
        self.installed = installed
        self.currentSpace = space
    }

    func directoryListings() throws -> [DirectoryListing] {
        if let failure { throw failure }
        return listings
    }

    func installedApps() throws -> [RiotSpaceApp] {
        if let failure { throw failure }
        return installed
    }

    func endorseApp(appID: Data, note: String, retract: Bool) throws {
        if let failure { throw failure }
        endorsed.append((appID: appID, note: note, retract: retract))
    }

    func shareApp(appID: Data) throws {
        if let failure { throw failure }
        shared.append(appID)
    }
}
