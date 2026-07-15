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

    /// Rust hands the directory raw bytes and the installed store hex text; an
    /// app we do not hold cannot be opened or reviewed — but if its bytes are
    /// here in full, the row offers to get it rather than dead-ending.
    func testListingWeDoNotHoldButCarryIsOfferedToGet() {
        let model = RiotDirectoryModel(
            port: FakeDirectoryPort(
                listings: [listing(appID: appID, name: "Checklist")],
                installed: [heldApp(appID: otherID, name: "Something else", trusted: true)]
            )
        )

        model.refresh()

        XCTAssertEqual(model.rows[0].availability, .get)
    }

    /// An app this device holds that the directory does not list — what a carried
    /// app becomes after a relaunch, once the in-memory index it arrived in is
    /// gone. It keeps its row and stays openable rather than disappearing.
    func testHeldAppWithNoListingKeepsItsRow() {
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Checklist")],
            installed: [heldApp(appID: otherID, name: "Legal support", trusted: true)],
            space: space
        )
        let model = RiotDirectoryModel(port: port)

        model.refresh()

        let carried = try? XCTUnwrap(model.rows.first { $0.name == "Legal support" })
        XCTAssertEqual(carried?.appID, otherID)
        XCTAssertEqual(carried?.availability, .open(port.installed[0]))
        XCTAssertTrue(carried?.canShare == true)
        // No index means no endorsements to show for it — the row says nothing
        // rather than inventing a number.
        XCTAssertNil(carried?.endorsement)
    }

    func testHexIDsRoundTripToTheRawBytesTheActionsTake() {
        XCTAssertEqual(RiotDirectoryRow.bytes(hex: "000fa0ff"), Data([0x00, 0x0f, 0xa0, 0xff]))
        XCTAssertNil(RiotDirectoryRow.bytes(hex: "abc"))
        XCTAssertNil(RiotDirectoryRow.bytes(hex: "zz"))
    }

    // MARK: - Getting an app someone carried to you

    /// The last hop of community discovery. Getting the app does not turn it on:
    /// it joins the held apps untrusted, so the row flips to Review — the sheet
    /// still stands between a neighbour's app and a running WebView.
    func testGettingACarriedAppMakesItReviewableAndConfirmsIt() {
        let port = FakeDirectoryPort(listings: [listing(appID: appID, name: "Checklist")])
        let model = RiotDirectoryModel(port: port)
        model.refresh()
        XCTAssertEqual(model.rows[0].availability, .get)

        model.get(model.rows[0])

        XCTAssertEqual(port.gotten, [appID])
        XCTAssertNil(model.errorMessage)
        XCTAssertEqual(model.confirmation, "Got Checklist — review it before it runs")
        // Held now, and untrusted: Review, not Open.
        guard case let .review(app) = model.rows[0].availability else {
            return XCTFail("expected a gotten app to be reviewable, got \(model.rows[0].availability)")
        }
        XCTAssertFalse(app.trusted)
        XCTAssertEqual(app.appIDHex, RiotDirectoryRow.hex(appID))
    }

    /// Once an organizer turns it on, the app this profile got from a neighbour
    /// opens like any other — the whole point of getting it.
    func testAGottenAppOpensOnceThisSpaceTurnsItOn() {
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Checklist", trustedInSpaces: [spaceID])],
            space: space
        )
        let model = RiotDirectoryModel(port: port)
        model.refresh()

        model.get(model.rows[0])

        guard case let .open(app) = model.rows[0].availability else {
            return XCTFail("expected a gotten app in a space that trusts it to open")
        }
        XCTAssertEqual(app.appIDHex, RiotDirectoryRow.hex(appID))
    }

    /// The refusal the core actually makes: the bytes are not all here. The
    /// person is told so in their own language, the row keeps offering to get
    /// the app, and nothing pretends to have happened.
    func testGettingAnAppWhoseBytesAreStillArrivingSaysSo() {
        let port = FakeDirectoryPort(listings: [listing(appID: appID, name: "Checklist")])
        port.getFailure = MobileError.AppRejected
        let model = RiotDirectoryModel(port: port)
        model.refresh()

        model.get(model.rows[0])

        XCTAssertEqual(
            model.errorMessage,
            "Checklist isn’t all here yet. Sync with the group carrying it, then try again."
        )
        XCTAssertNil(model.confirmation)
        XCTAssertEqual(model.rows[0].availability, .get)
    }

    /// An app whose bytes have not arrived is not something to get at all —
    /// there is nothing here to take up yet, trusted or not.
    func testAnAppWhoseBytesHaveNotArrivedOffersNothingToGet() {
        let model = RiotDirectoryModel(
            port: FakeDirectoryPort(
                listings: [listing(appID: appID, name: "Checklist", bundlePresent: false)]
            )
        )

        model.refresh()

        XCTAssertEqual(model.rows[0].availability, .arriving)
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

    // MARK: - Taking a recommendation back

    /// The affordance is exclusive: a row this person recommended offers the
    /// take-back, and only a row they have NOT recommended offers "Recommend".
    /// `endorsedByMe` is what the view switches on, so it is the contract.
    func testOnlyARowThisProfileRecommendedOffersTheTakeBack() {
        let on = listing(appID: appID, name: "Checklist", trustedInSpaces: [spaceID])
        let port = FakeDirectoryPort(
            listings: [on],
            installed: [heldApp(appID: appID, name: "Checklist", trusted: true)],
            space: space,
            endorsedByMe: [appID]
        )
        let model = RiotDirectoryModel(port: port)
        model.refresh()

        XCTAssertTrue(model.rows[0].endorsedByMe, "this profile recommended it, so the row offers the take-back")

        // The same row, unendorsed, offers the recommend path instead.
        let fresh = RiotDirectoryModel(
            port: FakeDirectoryPort(
                listings: [on],
                installed: [heldApp(appID: appID, name: "Checklist", trusted: true)],
                space: space
            )
        )
        fresh.refresh()
        XCTAssertFalse(fresh.rows[0].endorsedByMe)
        XCTAssertTrue(fresh.rows[0].canRecommend)
    }

    /// Taking it back writes a RETRACTION (an endorsement with `retract: true`
    /// and no note), confirms it in plain language, and — the part that matters —
    /// the row stops offering the take-back once the directory is re-read.
    func testRetractingWithdrawsTheRecommendationAndClearsTheRow() {
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Checklist", trustedInSpaces: [spaceID])],
            installed: [heldApp(appID: appID, name: "Checklist", trusted: true)],
            space: space,
            endorsedByMe: [appID]
        )
        let model = RiotDirectoryModel(port: port)
        model.refresh()
        XCTAssertTrue(model.rows[0].endorsedByMe)

        model.retract(model.rows[0])

        XCTAssertEqual(port.endorsed.count, 1)
        XCTAssertEqual(port.endorsed[0].appID, appID)
        XCTAssertTrue(port.endorsed[0].retract, "a take-back is an endorsement marked retracted")
        XCTAssertEqual(port.endorsed[0].note, "", "a retraction carries no note to explain")
        XCTAssertEqual(model.confirmation, "Took back recommendation of Checklist")
        XCTAssertNil(model.errorMessage)

        model.refresh()
        XCTAssertFalse(model.rows[0].endorsedByMe, "the take-back actually cleared it")
        XCTAssertTrue(model.rows[0].canRecommend, "and the row can be recommended again")
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
    private(set) var installed: [RiotSpaceApp]
    let currentSpace: RiotSpace?
    var failure: Error?
    /// What the core refuses `getCarriedApp` with — in production, an app whose
    /// bytes have not all arrived.
    var getFailure: Error?

    private(set) var endorsed: [(appID: Data, note: String, retract: Bool)] = []
    private(set) var shared: [Data] = []
    private(set) var gotten: [Data] = []

    /// The ids this profile has recommended. Held as real state rather than a
    /// call log, so a retract actually CLEARS the affordance the way the
    /// repository's persisted endorsements do — a fake that only recorded the
    /// call would let "take back" pass while leaving the row still endorsed.
    private(set) var endorsedAppIDs: Set<String> = []

    init(
        listings: [DirectoryListing] = [],
        installed: [RiotSpaceApp] = [],
        space: RiotSpace? = nil,
        endorsedByMe: [Data] = []
    ) {
        self.listings = listings
        self.installed = installed
        self.currentSpace = space
        self.endorsedAppIDs = Set(endorsedByMe.map { RiotDirectoryRow.hex($0).lowercased() })
    }

    /// Mirrors the repository: the app joins the held apps UNTRUSTED, built from
    /// the listing the directory already showed.
    func getCarriedApp(appID: Data) throws -> RiotSpaceApp {
        if let getFailure { throw getFailure }
        guard let listing = listings.first(where: { $0.appId == appID }) else {
            throw MobileError.AppRejected
        }
        gotten.append(appID)
        let app = RiotSpaceApp(
            appIDHex: RiotDirectoryRow.hex(appID),
            name: listing.name,
            description: listing.description,
            version: listing.version,
            permissions: listing.permissions,
            trusted: false
        )
        installed.append(app)
        return app
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
        let hex = RiotDirectoryRow.hex(appID).lowercased()
        if retract {
            endorsedAppIDs.remove(hex)
        } else {
            endorsedAppIDs.insert(hex)
        }
    }

    func shareApp(appID: Data) throws {
        if let failure { throw failure }
        shared.append(appID)
    }
}
