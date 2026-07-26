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

    func testCompactToolVocabularyUsesToolsAndCommunities() {
        XCTAssertEqual(ToolStrings.emptyTitle, "No tools yet")
        XCTAssertEqual(ToolStrings.headerEyebrow(communityTitle: "River City Wire"), "River City Wire")
        XCTAssertEqual(ToolStrings.inCommunity(communityTitle: "River City Wire"), "In River City Wire")
        XCTAssertEqual(ToolStrings.availableToAdd, "Available to add")
        XCTAssertEqual(ToolStrings.moreTools, "More tools")
        XCTAssertEqual(
            ToolStrings.recommend(communityTitle: "River City Wire"),
            "Recommend to River City Wire"
        )
        XCTAssertEqual(
            ToolStrings.retractRecommendation(communityTitle: "River City Wire"),
            "Take back recommendation from River City Wire"
        )
        XCTAssertEqual(
            ToolStrings.makeAvailable(communityTitle: "River City Wire"),
            "Make available in River City Wire"
        )
        XCTAssertEqual(ToolStrings.retry, "Try again")
        XCTAssertEqual(
            ToolStrings.retryAccessibilityLabel(communityTitle: "River City Wire"),
            "Try tools for River City Wire again"
        )
        XCTAssertEqual(
            ToolStrings.emptyMessage(communityTitle: "River City Wire"),
            "Tools added to River City Wire will appear here."
        )
        XCTAssertEqual(
            ToolStrings.inlineEmpty(communityTitle: "River City Wire"),
            "No tools in River City Wire yet"
        )
        XCTAssertEqual(ToolStrings.chooseCommunity, "Choose a community to see its tools")
        XCTAssertEqual(
            ToolStrings.loading(communityTitle: "River City Wire"),
            "Loading tools for River City Wire…"
        )
        for value in ToolStrings.userFacingVocabulary {
            XCTAssertFalse(value.localizedCaseInsensitiveContains("space app"))
            XCTAssertFalse(value.localizedCaseInsensitiveContains("renderer:"))
            XCTAssertFalse(value.localizedCaseInsensitiveContains("from your communities"))
            XCTAssertFalse(value.localizedCaseInsensitiveContains("this community"))
            XCTAssertFalse(value.localizedCaseInsensitiveContains("built in"))
            XCTAssertFalse(value.localizedCaseInsensitiveContains("review"))
            XCTAssertFalse(value.localizedCaseInsensitiveContains("share"))
        }
    }

    func testDirectoryCanReleaseAClosedRepositoryAndAttachItsReopenedReplacement() throws {
        let first = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Chat")],
            space: RiotSpace(namespaceID: "aa", title: "Community A")
        )
        let reopened = FakeDirectoryPort(
            listings: [
                listing(appID: appID, name: "Chat", trustedInSpaces: [Data([0xbb])])
            ],
            installed: [heldApp(appID: appID, name: "Chat", trusted: true)],
            space: RiotSpace(namespaceID: "bb", title: "River City Wire")
        )
        let model = RiotDirectoryModel(port: first)
        model.refresh(approval: .organizer)
        XCTAssertEqual(model.snapshot?.communityTitle, "Community A")

        model.detach()
        model.refresh(approval: .organizer)
        XCTAssertNil(model.snapshot)
        XCTAssertTrue(model.rows.isEmpty)

        model.attach(port: reopened)
        model.refresh(approval: .organizer)

        XCTAssertEqual(model.snapshot?.communityTitle, "River City Wire")
        XCTAssertEqual(
            try XCTUnwrap(model.snapshot?.inCommunity.first).primaryAction,
            .open(title: "Open Chat")
        )
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
        XCTAssertFalse(model.rows[0].badges.contains("On in this space"))
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

    func testBuiltInIsNotAProminentBadge() {
        let model = RiotDirectoryModel(
            port: FakeDirectoryPort(listings: [listing(appID: appID, name: "Checklist", builtIn: true)])
        )

        model.refresh()

        XCTAssertFalse(model.rows[0].badges.contains("Built in"))
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
        XCTAssertEqual(model.confirmation, "Recommended Checklist to Berlin Mutual Aid")
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
        XCTAssertEqual(
            model.confirmation,
            "Took back recommendation of Checklist from Berlin Mutual Aid"
        )
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
            installed: [heldApp(appID: appID, name: "Checklist", trusted: false)],
            space: space
        )
        let withSpace = RiotDirectoryModel(port: port)
        withSpace.refresh()
        XCTAssertTrue(withSpace.rows[0].canShare)

        withSpace.share(withSpace.rows[0])

        XCTAssertEqual(port.shared, [appID])
        XCTAssertEqual(
            withSpace.confirmation,
            "Made Checklist available in Berlin Mutual Aid"
        )
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
        XCTAssertEqual(model.selectedCommunityTitle, nil)
    }

    // MARK: - Selected-community presentation

    func testSnapshotNamesAndGroupsTheSelectedCommunityWithEnabledToolsFirst() throws {
        let chatID = Data([0x03])
        let checklistID = Data([0x02])
        let supplyID = Data([0x01])
        let riverCity = RiotSpace(namespaceID: RiotDirectoryRow.hex(spaceID), title: "River City Wire")
        let port = FakeDirectoryPort(
            listings: [
                listing(appID: supplyID, name: "Supply Board", builtIn: true),
                listing(appID: checklistID, name: "Checklist", version: "99.0", trustedInSpaces: [spaceID]),
                listing(
                    appID: chatID,
                    name: "Chat",
                    version: "0.1",
                    trustedInSpaces: [spaceID],
                    endorsingMet: [Data([0x77])]
                ),
            ],
            installed: [
                heldApp(appID: checklistID, name: "Checklist", trusted: true),
                heldApp(appID: supplyID, name: "Supply Board", trusted: false),
            ],
            space: riverCity
        )
        let model = RiotDirectoryModel(port: port)

        model.refresh(approval: .organizer)

        let snapshot = try XCTUnwrap(model.snapshot)
        XCTAssertEqual(snapshot.namespaceID, riverCity.namespaceID)
        XCTAssertEqual(snapshot.communityTitle, "River City Wire")
        XCTAssertEqual(snapshot.inCommunity.map(\.name), ["Chat", "Checklist"])
        XCTAssertEqual(snapshot.availableToAdd.map(\.name), ["Supply Board"])
        XCTAssertEqual(snapshot.inCommunity.map(\.primaryAction.title), ["Open Chat", "Open Checklist"])
        XCTAssertEqual(
            snapshot.availableToAdd[0].primaryAction.title,
            "Add Supply Board to River City Wire"
        )
        XCTAssertEqual(
            snapshot.moreTools,
            [.importVerifiedPair(title: "Add a tool from a file")]
        )
        XCTAssertEqual(
            Set(snapshot.inCommunity.map(\.id) + snapshot.availableToAdd.map(\.id)).count,
            3,
            "profile-wide listings appear once and are never duplicated under More tools"
        )
    }

    func testEnabledCompleteButUnadmittedToolStillHasImmediateOpenAction() throws {
        let port = FakeDirectoryPort(
            listings: [
                listing(
                    appID: appID,
                    name: "Chat",
                    bundlePresent: true,
                    trustedInSpaces: [spaceID]
                )
            ],
            installed: [],
            space: space
        )
        let model = RiotDirectoryModel(port: port)

        model.refresh(approval: .member)

        let row = try XCTUnwrap(model.snapshot?.inCommunity.first)
        XCTAssertEqual(row.availability, .get, "the flat compatibility row keeps lazy-admission semantics")
        XCTAssertEqual(row.primaryAction, .open(title: "Open Chat"))
        XCTAssertTrue(row.enabledInCurrentCommunity)
    }

    func testDisabledToolUsesOrganizerMemberAndLegacyNamedCopy() throws {
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Chat")],
            installed: [heldApp(appID: appID, name: "Chat", trusted: false)],
            space: RiotSpace(namespaceID: RiotDirectoryRow.hex(spaceID), title: "River City Wire")
        )
        let model = RiotDirectoryModel(port: port)

        model.refresh(approval: .organizer)
        XCTAssertEqual(
            try XCTUnwrap(model.snapshot?.availableToAdd.first).primaryAction.title,
            "Add Chat to River City Wire"
        )

        model.refresh(approval: .member)
        XCTAssertEqual(
            try XCTUnwrap(model.snapshot?.availableToAdd.first).primaryAction,
            .ask(title: "Ask an organizer to add Chat")
        )
        XCTAssertTrue(try XCTUnwrap(model.snapshot?.moreTools).isEmpty)

        model.refresh(approval: .unavailable)
        XCTAssertEqual(
            try XCTUnwrap(model.snapshot?.availableToAdd.first).primaryAction,
            .unavailable(
                message: "This profile can’t add tools to River City Wire. Start a new profile to organize a community."
            )
        )
        XCTAssertTrue(try XCTUnwrap(model.snapshot?.moreTools).isEmpty)
    }

    func testBadgesExposeOfflineBenefitOnlyForVerifiedInstalledPair() throws {
        let installedID = Data([0x01])
        let unadmittedID = Data([0x02])
        let unlistedID = Data([0x03])
        let port = FakeDirectoryPort(
            listings: [
                listing(appID: installedID, name: "Installed", builtIn: true),
                listing(appID: unadmittedID, name: "Unadmitted", builtIn: true),
            ],
            installed: [
                heldApp(appID: installedID, name: "Installed", trusted: false),
                heldApp(appID: unlistedID, name: "Unlisted", trusted: false),
            ],
            space: space
        )
        let model = RiotDirectoryModel(port: port)

        model.refresh(approval: .organizer)

        let rows = Dictionary(uniqueKeysWithValues: model.rows.map { ($0.name, $0) })
        XCTAssertEqual(rows["Installed"]?.badges, ["Works offline"])
        XCTAssertFalse(rows["Unadmitted"]?.badges.contains("Works offline") == true)
        XCTAssertEqual(rows["Unlisted"]?.badges, ["Works offline"])
        XCTAssertFalse(model.rows.flatMap(\.badges).contains("Built in"))
        XCTAssertFalse(model.rows.flatMap(\.badges).contains("On in this space"))
    }

    func testSnapshotDeduplicatesAndSortsByNameThenFullID() throws {
        let first = Data([0x01])
        let second = Data([0x02])
        let port = FakeDirectoryPort(
            listings: [
                listing(appID: second, name: "chat"),
                listing(appID: first, name: "Chat"),
                listing(appID: first, name: "Duplicate must not appear"),
            ],
            space: space
        )
        let model = RiotDirectoryModel(port: port)

        model.refresh(approval: .organizer)

        let rows = try XCTUnwrap(model.snapshot).availableToAdd
        XCTAssertEqual(rows.map(\.appIDHex), ["01", "02"])
        XCTAssertEqual(rows.map(\.name), ["Chat", "chat"])
        XCTAssertEqual(rows[0].accessibilityIdentifier, "directory-tool-01")
        XCTAssertEqual(rows[1].accessibilityIdentifier, "directory-tool-02")
    }

    func testNoSelectedCommunityPublishesNoSnapshotOrActionContext() {
        let model = RiotDirectoryModel(
            port: FakeDirectoryPort(
                listings: [listing(appID: appID, name: "Chat")],
                installed: [heldApp(appID: appID, name: "Chat", trusted: false)]
            )
        )

        model.refresh(approval: .organizer)

        XCTAssertNil(model.snapshot)
        XCTAssertNil(model.rows.first?.actionContext)
        XCTAssertFalse(model.isLoading)
    }

    func testRefreshFailureRetainsOnlyASameNamespaceSnapshot() throws {
        let a = RiotSpace(namespaceID: "aa", title: "Community A")
        let b = RiotSpace(namespaceID: "bb", title: "River City Wire")
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Chat")],
            space: a
        )
        let model = RiotDirectoryModel(port: port)
        model.refresh(approval: .organizer)
        XCTAssertEqual(model.snapshot?.communityTitle, "Community A")
        XCTAssertEqual(model.selectedCommunityTitle, "Community A")

        port.failure = FakeDirectoryError.unavailable
        model.refresh(approval: .organizer)
        XCTAssertEqual(model.snapshot?.communityTitle, "Community A")
        XCTAssertEqual(model.selectedCommunityTitle, "Community A")
        XCTAssertEqual(model.failedNamespace, "aa")
        XCTAssertEqual(model.errorMessage, "Couldn’t load tools for Community A.")

        port.currentSpace = b
        model.refresh(approval: .organizer)
        XCTAssertNil(model.snapshot, "a new namespace never keeps the previous community’s rows")
        XCTAssertEqual(
            model.selectedCommunityTitle,
            "River City Wire",
            "the header remains named while the new community is loading or failed"
        )
        XCTAssertTrue(model.rows.isEmpty)
        XCTAssertEqual(model.failedNamespace, "bb")
        XCTAssertEqual(model.errorMessage, "Couldn’t load tools for River City Wire.")
        XCTAssertFalse(model.isLoading)

        port.failure = nil
        model.retry()
        XCTAssertEqual(model.snapshot?.communityTitle, "River City Wire")
        XCTAssertNil(model.failedNamespace)
        XCTAssertNil(model.errorMessage)
    }

    func testSelectionGenerationInvalidatesAContextAcrossABackToASelection() throws {
        let a = RiotSpace(namespaceID: "aa", title: "Community A")
        let b = RiotSpace(namespaceID: "bb", title: "Community B")
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Chat")],
            space: a
        )
        let model = RiotDirectoryModel(port: port)

        model.refresh(approval: .organizer)
        let firstA = try XCTUnwrap(model.snapshot?.availableToAdd.first?.actionContext)

        port.currentSpace = b
        model.refresh(approval: .organizer)
        port.currentSpace = a
        model.refresh(approval: .organizer)
        let secondA = try XCTUnwrap(model.snapshot?.availableToAdd.first?.actionContext)

        XCTAssertEqual(firstA.namespaceID, secondA.namespaceID)
        XCTAssertNotEqual(firstA.selectionGeneration, secondA.selectionGeneration)
    }

    func testImportContextIsBoundAcrossBothFilePickersAndRejectsABackToA() throws {
        let a = RiotSpace(namespaceID: "aa", title: "Community A")
        let b = RiotSpace(namespaceID: "bb", title: "Community B")
        let port = FakeDirectoryPort(space: a)
        let model = RiotDirectoryModel(port: port)

        model.refresh(approval: .organizer)
        let firstA = try XCTUnwrap(model.captureImportContext())
        XCTAssertNoThrow(try model.validate(firstA))

        port.currentSpace = b
        model.refresh(approval: .organizer)
        port.currentSpace = a
        model.refresh(approval: .organizer)

        XCTAssertThrowsError(try model.validate(firstA)) { error in
            XCTAssertEqual(error as? RiotDirectoryActionError, .staleSelection)
        }
        let secondA = try XCTUnwrap(model.captureImportContext())
        XCTAssertNotEqual(firstA.selectionGeneration, secondA.selectionGeneration)
        XCTAssertNoThrow(try model.validate(secondA))
    }

    func testImportedUntrustedToolImmediatelyPreparesItsNamedAddGate() throws {
        let installed = heldApp(appID: appID, name: "Chat", trusted: false)
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Chat")],
            installed: [installed],
            space: RiotSpace(
                namespaceID: RiotDirectoryRow.hex(spaceID),
                title: "River City Wire"
            )
        )
        let model = RiotDirectoryModel(port: port)
        model.refresh(approval: .organizer)

        let prepared = try model.prepareImportedTool(installed)

        XCTAssertEqual(
            prepared.row.primaryAction,
            .add(title: "Add Chat to River City Wire")
        )
        XCTAssertFalse(prepared.app.trusted)
        XCTAssertEqual(prepared.context.communityTitle, "River City Wire")
        XCTAssertTrue(model.snapshot?.inCommunity.isEmpty == true)
        XCTAssertEqual(model.snapshot?.availableToAdd.map(\.name), ["Chat"])
    }

    func testModelRejectsStaleGenerationAndWrongNamespaceContexts() throws {
        let a = RiotSpace(namespaceID: "aa", title: "Community A")
        let b = RiotSpace(namespaceID: "bb", title: "Community B")
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Chat")],
            space: a
        )
        let model = RiotDirectoryModel(port: port)

        model.refresh(approval: .organizer)
        let firstA = try XCTUnwrap(model.snapshot?.availableToAdd.first?.actionContext)
        XCTAssertNoThrow(try model.validate(firstA))

        port.currentSpace = b
        model.refresh(approval: .organizer)
        port.currentSpace = a
        model.refresh(approval: .organizer)

        XCTAssertThrowsError(try model.validate(firstA)) { error in
            XCTAssertEqual(error as? RiotDirectoryActionError, .staleSelection)
        }

        let currentA = try XCTUnwrap(model.snapshot?.availableToAdd.first?.actionContext)
        let wrongNamespace = RiotDirectoryActionContext(
            appID: currentA.appID,
            appIDHex: currentA.appIDHex,
            appName: currentA.appName,
            namespaceID: b.namespaceID,
            communityTitle: b.title,
            selectionGeneration: currentA.selectionGeneration
        )
        XCTAssertThrowsError(try model.validate(wrongNamespace)) { error in
            XCTAssertEqual(error as? RiotDirectoryActionError, .staleSelection)
        }
        XCTAssertNoThrow(try model.validate(currentA))
    }

    // MARK: - Namespace-bound actions

    func testPrepareOpenLazilyAdmitsAnEnabledToolAndKeepsItOpen() throws {
        let port = FakeDirectoryPort(
            listings: [
                listing(appID: appID, name: "Chat", trustedInSpaces: [spaceID])
            ],
            space: space
        )
        let model = RiotDirectoryModel(port: port)
        model.refresh(approval: .organizer)
        let row = try XCTUnwrap(model.snapshot?.inCommunity.first)
        let context = try XCTUnwrap(row.actionContext)

        let admitted = try model.prepareOpen(row, context: context)

        XCTAssertEqual(admitted.appIDHex, row.appIDHex)
        XCTAssertTrue(admitted.trusted)
        XCTAssertEqual(port.gotten, [appID])
        XCTAssertEqual(port.getNamespaces, [space.namespaceID])
        XCTAssertEqual(model.snapshot?.inCommunity.first?.primaryAction, .open(title: "Open Chat"))
    }

    func testRuntimeAuthorizedInstalledToolStaysInCommunityWhenListingProjectionIsStale() throws {
        let trustedApp = heldApp(appID: appID, name: "Chat", trusted: true)
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Chat", trustedInSpaces: [])],
            installed: [trustedApp],
            space: space
        )
        let model = RiotDirectoryModel(port: port)

        model.refresh(approval: .organizer)

        let row = try XCTUnwrap(model.snapshot?.inCommunity.first)
        XCTAssertEqual(row.primaryAction, .open(title: "Open Chat"))
        guard case let .open(app) = row.availability else {
            return XCTFail("the runtime security gate authorized Chat, so it must remain open")
        }
        XCTAssertTrue(app.trusted)
        XCTAssertFalse(
            model.snapshot?.availableToAdd.contains { $0.appIDHex == row.appIDHex } == true
        )
    }

    func testPrepareAddLazilyAdmitsADisabledToolWithoutTrustingIt() throws {
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Chat")],
            space: space
        )
        let model = RiotDirectoryModel(port: port)
        model.refresh(approval: .organizer)
        let row = try XCTUnwrap(model.snapshot?.availableToAdd.first)
        let context = try XCTUnwrap(row.actionContext)

        let admitted = try model.prepareAdd(row, context: context)

        XCTAssertFalse(admitted.trusted)
        XCTAssertEqual(port.getNamespaces, [space.namespaceID])
        XCTAssertEqual(
            model.snapshot?.availableToAdd.first?.primaryAction,
            .add(title: "Add Chat to Berlin Mutual Aid")
        )
        guard case let .review(refreshedApp) =
            try XCTUnwrap(model.snapshot?.availableToAdd.first).availability
        else {
            return XCTFail("Add preparation must keep the untrusted tool available to add")
        }
        XCTAssertFalse(refreshedApp.trusted)
    }

    func testPreparationCannotCrossOpenAndAddIntent() throws {
        let enabledPort = FakeDirectoryPort(
            listings: [
                listing(appID: appID, name: "Chat", trustedInSpaces: [spaceID])
            ],
            space: space
        )
        let enabledModel = RiotDirectoryModel(port: enabledPort)
        enabledModel.refresh(approval: .organizer)
        let enabled = try XCTUnwrap(enabledModel.snapshot?.inCommunity.first)

        XCTAssertThrowsError(
            try enabledModel.prepareAdd(
                enabled,
                context: XCTUnwrap(enabled.actionContext)
            )
        )
        XCTAssertTrue(enabledPort.gotten.isEmpty)

        let disabledPort = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Chat")],
            space: space
        )
        let disabledModel = RiotDirectoryModel(port: disabledPort)
        disabledModel.refresh(approval: .organizer)
        let disabled = try XCTUnwrap(disabledModel.snapshot?.availableToAdd.first)

        XCTAssertThrowsError(
            try disabledModel.prepareOpen(
                disabled,
                context: XCTUnwrap(disabled.actionContext)
            )
        )
        XCTAssertTrue(disabledPort.gotten.isEmpty)
    }

    func testPrepareAddRejectsAToolThatBecomesEnabledDuringAdmission() throws {
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Chat")],
            space: space
        )
        let model = RiotDirectoryModel(port: port)
        model.refresh(approval: .organizer)
        let before = try XCTUnwrap(model.snapshot)
        let row = try XCTUnwrap(before.availableToAdd.first)
        let context = try XCTUnwrap(row.actionContext)
        port.onGet = { [weak port] in
            port?.listings = [
                self.listing(
                    appID: self.appID,
                    name: "Chat",
                    trustedInSpaces: [self.spaceID]
                )
            ]
        }

        let prepared = try? model.prepareAdd(row, context: context)

        XCTAssertNil(prepared, "an enabled tool must never reach the Add confirmation sheet")
        XCTAssertEqual(
            model.errorMessage,
            "Couldn’t add Chat to Berlin Mutual Aid. Nothing changed. Try again."
        )
        XCTAssertEqual(model.snapshot, before)
        XCTAssertEqual(
            model.snapshot?.availableToAdd.first?.primaryAction,
            .add(title: "Add Chat to Berlin Mutual Aid")
        )
    }

    func testPrepareAddRejectsATrustedAdmissionResultForADisabledListing() throws {
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Chat")],
            space: space
        )
        port.getTrustedOverride = true
        let model = RiotDirectoryModel(port: port)
        model.refresh(approval: .organizer)
        let before = try XCTUnwrap(model.snapshot)
        let row = try XCTUnwrap(before.availableToAdd.first)
        let context = try XCTUnwrap(row.actionContext)

        let prepared = try? model.prepareAdd(row, context: context)

        XCTAssertNil(prepared, "a trusted result must never reach the Add confirmation sheet")
        XCTAssertEqual(
            model.errorMessage,
            "Couldn’t add Chat to Berlin Mutual Aid. Nothing changed. Try again."
        )
        XCTAssertEqual(model.snapshot, before)
        XCTAssertEqual(
            model.snapshot?.availableToAdd.first?.primaryAction,
            .add(title: "Add Chat to Berlin Mutual Aid")
        )
    }

    func testPreparationFailuresKeepTheSnapshotAndUseNamedRetryCopy() throws {
        for operation in ["open", "add"] {
            let trustedSpaces = operation == "open" ? [spaceID] : []
            let port = FakeDirectoryPort(
                listings: [
                    listing(appID: appID, name: "Chat", trustedInSpaces: trustedSpaces)
                ],
                space: space
            )
            port.getFailure = FakeDirectoryError.unavailable
            let model = RiotDirectoryModel(port: port)
            model.refresh(approval: .organizer)
            let before = try XCTUnwrap(model.snapshot)
            let row = operation == "open"
                ? try XCTUnwrap(before.inCommunity.first)
                : try XCTUnwrap(before.availableToAdd.first)
            let context = try XCTUnwrap(row.actionContext)

            XCTAssertThrowsError(
                try operation == "open"
                    ? model.prepareOpen(row, context: context)
                    : model.prepareAdd(row, context: context)
            )

            XCTAssertEqual(model.snapshot, before)
            XCTAssertNil(model.confirmation)
            XCTAssertEqual(
                model.errorMessage,
                operation == "open"
                    ? "Couldn’t open Chat in Berlin Mutual Aid. Nothing changed. Try again."
                    : "Couldn’t add Chat to Berlin Mutual Aid. Nothing changed. Try again."
            )
            XCTAssertEqual(
                (operation == "open"
                    ? model.snapshot?.inCommunity.first
                    : model.snapshot?.availableToAdd.first)?.primaryAction,
                row.primaryAction
            )
        }
    }

    func testStaleActionsNeverReachAnyPortMutationIncludingABackToA() throws {
        let a = space
        let b = RiotSpace(namespaceID: "bb", title: "Community B")
        let port = FakeDirectoryPort(
            listings: [
                listing(appID: appID, name: "Chat", trustedInSpaces: [spaceID])
            ],
            installed: [heldApp(appID: appID, name: "Chat", trusted: true)],
            space: a
        )
        let model = RiotDirectoryModel(port: port)
        model.refresh(approval: .organizer)
        let row = try XCTUnwrap(model.snapshot?.inCommunity.first)
        let context = try XCTUnwrap(row.actionContext)

        port.currentSpace = b
        model.refresh(approval: .organizer)
        port.currentSpace = a
        model.refresh(approval: .organizer)

        XCTAssertThrowsError(try model.prepareOpen(row, context: context))
        model.recommend(row, note: "Useful", context: context)
        model.retract(row, context: context)
        model.makeAvailable(row, context: context)

        XCTAssertTrue(port.gotten.isEmpty)
        XCTAssertTrue(port.endorsed.isEmpty)
        XCTAssertTrue(port.shared.isEmpty)
        XCTAssertEqual(model.errorMessage, String(describing: RiotDirectoryActionError.staleSelection))
    }

    func testManagementActionsUseNamedReceiptsAndExpectedNamespace() throws {
        let port = FakeDirectoryPort(
            listings: [
                listing(appID: appID, name: "Chat", trustedInSpaces: [spaceID])
            ],
            installed: [heldApp(appID: appID, name: "Chat", trusted: true)],
            space: space
        )
        let model = RiotDirectoryModel(port: port)
        model.refresh(approval: .organizer)
        var row = try XCTUnwrap(model.snapshot?.inCommunity.first)
        var context = try XCTUnwrap(row.actionContext)

        model.recommend(row, note: "Useful", context: context)
        XCTAssertEqual(model.confirmation, "Recommended Chat to Berlin Mutual Aid")
        XCTAssertEqual(port.endorseNamespaces, [space.namespaceID])

        row = try XCTUnwrap(model.snapshot?.inCommunity.first)
        context = try XCTUnwrap(row.actionContext)
        model.retract(row, context: context)
        XCTAssertEqual(model.confirmation, "Took back recommendation of Chat from Berlin Mutual Aid")

        row = try XCTUnwrap(model.snapshot?.inCommunity.first)
        context = try XCTUnwrap(row.actionContext)
        XCTAssertTrue(row.canMakeAvailable)
        model.makeAvailable(row, context: context)
        XCTAssertEqual(model.confirmation, "Made Chat available in Berlin Mutual Aid")
        XCTAssertEqual(port.shareNamespaces, [space.namespaceID])
    }

    func testMakeAvailableRequiresALocallyResolvableVerifiedPair() throws {
        let completeUnadmitted = RiotDirectoryRow.make(
            listing: listing(appID: appID, name: "Chat"),
            installed: nil,
            space: space
        )
        let arriving = RiotDirectoryRow.make(
            listing: listing(appID: otherID, name: "Poll", bundlePresent: false),
            installed: nil,
            space: space
        )
        let admitted = RiotDirectoryRow.make(
            listing: listing(appID: appID, name: "Chat"),
            installed: heldApp(appID: appID, name: "Chat", trusted: false),
            space: space
        )

        XCTAssertTrue(
            completeUnadmitted.canMakeAvailable,
            "the verified pair is locally present even before runtime admission"
        )
        XCTAssertFalse(arriving.canMakeAvailable)
        XCTAssertTrue(admitted.canMakeAvailable)
    }

    func testMemberCannotPrepareAddOrAdmitTheTool() throws {
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Chat")],
            space: space
        )
        let model = RiotDirectoryModel(port: port)
        model.refresh(approval: .member)
        let row = try XCTUnwrap(model.snapshot?.availableToAdd.first)
        let context = try XCTUnwrap(row.actionContext)

        XCTAssertThrowsError(try model.prepareAdd(row, context: context))
        XCTAssertTrue(port.gotten.isEmpty)
        XCTAssertTrue(port.installed.isEmpty)
    }

    func testDuplicateUnlistedInstalledRowsKeepFirstSeenOrderWithLastValue() {
        let thirdID = Data([0x03])
        let firstID = Data([0x01])
        let secondID = Data([0x02])
        let model = RiotDirectoryModel(
            port: FakeDirectoryPort(
                installed: [
                    heldApp(appID: thirdID, name: "Third original", trusted: false),
                    heldApp(appID: firstID, name: "First", trusted: false),
                    heldApp(appID: thirdID, name: "Third restored", trusted: false),
                    heldApp(appID: secondID, name: "Second", trusted: false),
                ],
                space: space
            )
        )

        model.refresh(approval: .organizer)

        XCTAssertEqual(model.rows.map(\.appIDHex), ["03", "01", "02"])
        XCTAssertEqual(model.rows.map(\.name), ["Third restored", "First", "Second"])
    }

    func testSuccessfulInternalRefreshesPreserveOrganizerApproval() throws {
        let disabledPort = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Chat")],
            space: space
        )
        let disabledModel = RiotDirectoryModel(port: disabledPort)
        disabledModel.refresh(approval: .organizer)

        disabledModel.get(try XCTUnwrap(disabledModel.snapshot?.availableToAdd.first))

        XCTAssertEqual(
            disabledModel.snapshot?.availableToAdd.first?.primaryAction,
            .add(title: "Add Chat to Berlin Mutual Aid")
        )
        XCTAssertEqual(
            disabledModel.snapshot?.moreTools,
            [.importVerifiedPair(title: "Add a tool from a file")]
        )

        let enabledPort = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Chat", trustedInSpaces: [spaceID])],
            installed: [heldApp(appID: appID, name: "Chat", trusted: true)],
            space: space
        )
        let enabledModel = RiotDirectoryModel(port: enabledPort)
        enabledModel.refresh(approval: .organizer)

        enabledModel.recommend(try XCTUnwrap(enabledModel.snapshot?.inCommunity.first), note: "Useful")
        XCTAssertEqual(
            enabledModel.snapshot?.moreTools,
            [.importVerifiedPair(title: "Add a tool from a file")]
        )

        enabledModel.share(try XCTUnwrap(enabledModel.snapshot?.inCommunity.first))
        XCTAssertEqual(
            enabledModel.snapshot?.moreTools,
            [.importVerifiedPair(title: "Add a tool from a file")]
        )

        enabledModel.retract(try XCTUnwrap(enabledModel.snapshot?.inCommunity.first))
        XCTAssertEqual(
            enabledModel.snapshot?.moreTools,
            [.importVerifiedPair(title: "Add a tool from a file")]
        )
    }

    func testSuccessfulApprovalPublishesExactNamedReceipt() throws {
        let port = FakeDirectoryPort(
            listings: [listing(appID: appID, name: "Chat", trustedInSpaces: [spaceID])],
            installed: [heldApp(appID: appID, name: "Chat", trusted: true)],
            space: RiotSpace(
                namespaceID: RiotDirectoryRow.hex(spaceID),
                title: "River City Wire"
            )
        )
        let model = RiotDirectoryModel(port: port)
        model.refresh(approval: .organizer)
        let row = try XCTUnwrap(model.snapshot?.inCommunity.first)
        let context = try XCTUnwrap(row.actionContext)

        model.confirmAdded(context)

        XCTAssertEqual(model.confirmation, "Added Chat to River City Wire")
    }

    func testUserFacingVocabularyNeverUsesGenericThisCommunity() {
        for value in ToolStrings.userFacingVocabulary {
            XCTAssertFalse(value.localizedCaseInsensitiveContains("this community"))
        }
    }

    // MARK: - Named add confirmation

    func testAddSheetCopyNamesTheToolAndSelectedCommunity() {
        let context = RiotDirectoryActionContext(
            appID: appID,
            appIDHex: RiotDirectoryRow.hex(appID),
            appName: "Chat",
            namespaceID: RiotDirectoryRow.hex(spaceID),
            communityTitle: "River City Wire",
            selectionGeneration: 7
        )
        let copy = AppReviewSheetCopy(context: context)

        XCTAssertEqual(copy.title, "Add Chat to River City Wire?")
        XCTAssertEqual(copy.confirmation, "Add to River City Wire")
        XCTAssertEqual(
            copy.memberReason,
            "Only an organizer of River City Wire can add this tool."
        )
        XCTAssertEqual(
            copy.legacyReason,
            "This profile can’t add tools to River City Wire. Start a new profile to organize a community."
        )
        XCTAssertEqual(
            copy.failure,
            "Couldn’t add Chat to River City Wire. Nothing changed. Try again."
        )
    }

    func testAddSheetSubmissionStatePreventsDuplicatesAndDismissesOnlyOnSuccess() {
        var state = AppReviewSheetSubmissionState.idle

        XCTAssertTrue(state.begin())
        XCTAssertEqual(state, .adding)
        XCTAssertFalse(state.begin(), "a second tap while approval is running must do nothing")
        XCTAssertFalse(state.shouldDismiss)

        state.finish(.notAdded(message: "Couldn’t add Chat to River City Wire. Nothing changed. Try again."))
        XCTAssertEqual(
            state,
            .failed(message: "Couldn’t add Chat to River City Wire. Nothing changed. Try again.")
        )
        XCTAssertTrue(state.canSubmit, "failure keeps the sheet open with a retry")
        XCTAssertFalse(state.shouldDismiss)

        XCTAssertTrue(state.begin())
        state.finish(.added)
        XCTAssertEqual(state, .succeeded)
        XCTAssertFalse(state.canSubmit)
        XCTAssertTrue(state.shouldDismiss)
    }

    func testDurableSavedApprovalNeverClaimsNothingChangedOrOffersDuplicateSubmit() {
        let message =
            "The tool approval was saved, but Riot couldn’t verify it after reopening "
            + "your profile. Restart Riot and check the tool before trying again."
        var state = AppReviewSheetSubmissionState.adding

        state.finish(.savedNeedsRestart(message: message))

        XCTAssertEqual(state, .savedNeedsRestart(message: message))
        XCTAssertFalse(state.canSubmit)
        XCTAssertFalse(state.shouldDismiss)
        XCTAssertFalse(state.message?.contains("Nothing changed") == true)
    }

    func testDurableSavedApprovalRemainsVisibleAfterRecoveryClearsOrganizerCapability() {
        let context = RiotDirectoryActionContext(
            appID: appID,
            appIDHex: RiotDirectoryRow.hex(appID),
            appName: "Chat",
            namespaceID: RiotDirectoryRow.hex(spaceID),
            communityTitle: "River City Wire",
            selectionGeneration: 7
        )
        let message =
            "The tool approval was saved, but Riot couldn’t verify it after reopening "
            + "your profile. Restart Riot and check the tool before trying again."

        let presentation = AppReviewSheetPresentation(
            permissions: ["Read messages"],
            context: context,
            capability: .member,
            submissionState: .savedNeedsRestart(message: message)
        )

        XCTAssertEqual(
            presentation.elements,
            [
                .permissions(["Read messages"]),
                .approval(
                    title: "Add to River City Wire",
                    isEnabled: false,
                    accessibilityHint: message
                ),
            ]
        )
    }

    func testRetainedEmptySnapshotKeepsInlineEmptyMessageAlongsideScopedError() {
        let snapshot = RiotDirectorySnapshot(
            namespaceID: RiotDirectoryRow.hex(spaceID),
            communityTitle: "River City Wire",
            inCommunity: [],
            availableToAdd: [],
            moreTools: []
        )
        let state = DirectoryScreenState(
            snapshot: snapshot,
            scopedError: "Couldn’t load tools for River City Wire."
        )

        XCTAssertEqual(state.scopedError, "Couldn’t load tools for River City Wire.")
        XCTAssertEqual(state.inlineEmptyMessage, "No tools in River City Wire yet")
    }

    func testApprovalViewStatePreservesDurableRecoverySheetAndFocusesOnlyOnDismiss() {
        let fullID = String(repeating: "ab", count: 32)
        var state = DirectoryApprovalFlowState()

        state.record(.added, appIDHex: fullID)
        XCTAssertEqual(state.pendingFocusAppID, fullID)
        XCTAssertEqual(
            state.consumeFocusOnDismiss(),
            fullID,
            "successful approval stores full-ID focus until sheet dismissal"
        )
        XCTAssertNil(state.pendingFocusAppID)

        let recovery =
            "The tool approval was saved, but Riot couldn’t verify it after reopening "
            + "your profile. Restart Riot and check the tool before trying again."
        state.record(.savedNeedsRestart(message: recovery), appIDHex: fullID)
        XCTAssertFalse(
            state.shouldCancelSheet(currentNamespaceID: nil),
            "coherent recovery clears space, but its restart guidance must remain visible"
        )
        XCTAssertTrue(
            state.shouldCancelSheet(currentNamespaceID: "another-community"),
            "an actual community switch still cancels the stale sheet"
        )
        XCTAssertNil(state.consumeFocusOnDismiss())
    }

    func testAddSheetPresentationPutsPermissionsBeforeOrganizerApproval() throws {
        let context = RiotDirectoryActionContext(
            appID: appID,
            appIDHex: RiotDirectoryRow.hex(appID),
            appName: "Chat",
            namespaceID: RiotDirectoryRow.hex(spaceID),
            communityTitle: "River City Wire",
            selectionGeneration: 7
        )
        let presentation = AppReviewSheetPresentation(
            permissions: ["Read messages", "Keep its own notes"],
            context: context,
            capability: .organizer,
            submissionState: .idle
        )

        XCTAssertEqual(
            presentation.elements,
            [
                .permissions(["Read messages", "Keep its own notes"]),
                .approval(
                    title: "Add to River City Wire",
                    isEnabled: true,
                    accessibilityHint: nil
                ),
            ]
        )

        let failed = AppReviewSheetPresentation(
            permissions: ["Read messages"],
            context: context,
            capability: .organizer,
            submissionState: .failed(
                message: "Couldn’t add Chat to River City Wire. Nothing changed. Try again."
            )
        )
        guard case let .approval(_, isEnabled, accessibilityHint) =
            try XCTUnwrap(failed.elements.last)
        else {
            return XCTFail("organizer failure must keep the named approval action")
        }
        XCTAssertTrue(isEnabled)
        XCTAssertEqual(
            accessibilityHint,
            "Couldn’t add Chat to River City Wire. Nothing changed. Try again."
        )
    }

    func testAddSheetMemberAndLegacyPresentReasonsWithoutApprovalActions() {
        let context = RiotDirectoryActionContext(
            appID: appID,
            appIDHex: RiotDirectoryRow.hex(appID),
            appName: "Chat",
            namespaceID: RiotDirectoryRow.hex(spaceID),
            communityTitle: "River City Wire",
            selectionGeneration: 7
        )

        let member = AppReviewSheetPresentation(
            permissions: ["Read messages"],
            context: context,
            capability: .member,
            submissionState: .idle
        )
        XCTAssertEqual(
            member.elements,
            [
                .permissions(["Read messages"]),
                .unavailable("Only an organizer of River City Wire can add this tool."),
            ]
        )
        XCTAssertFalse(member.elements.contains { $0.isApproval })

        let legacy = AppReviewSheetPresentation(
            permissions: ["Read messages"],
            context: context,
            capability: .unavailable,
            submissionState: .idle
        )
        XCTAssertEqual(
            legacy.elements,
            [
                .permissions(["Read messages"]),
                .unavailable(
                    "This profile can’t add tools to River City Wire. Start a new profile to organize a community."
                ),
            ]
        )
        XCTAssertFalse(legacy.elements.contains { $0.isApproval })
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
    var listings: [DirectoryListing]
    private(set) var installed: [RiotSpaceApp]
    var currentSpace: RiotSpace?
    var failure: Error?
    /// What the core refuses `getCarriedApp` with — in production, an app whose
    /// bytes have not all arrived.
    var getFailure: Error?
    /// Adversarial admission transition: lets a test change the listing after
    /// the action starts but before the model refreshes it.
    var onGet: (() -> Void)?
    /// A deliberately inconsistent runtime result for proving Add rejects an
    /// app reported trusted while its listing remains disabled.
    var getTrustedOverride: Bool?

    private(set) var endorsed: [(appID: Data, note: String, retract: Bool)] = []
    private(set) var shared: [Data] = []
    private(set) var gotten: [Data] = []
    private(set) var endorseNamespaces: [String] = []
    private(set) var shareNamespaces: [String] = []
    private(set) var getNamespaces: [String] = []

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
        try getCarriedApp(appID: appID, expectedNamespaceID: currentSpace?.namespaceID ?? "")
    }

    func getCarriedApp(appID: Data, expectedNamespaceID: String) throws -> RiotSpaceApp {
        if let getFailure { throw getFailure }
        if let currentSpace {
            guard currentSpace.namespaceID.caseInsensitiveCompare(expectedNamespaceID) == .orderedSame else {
                throw RepositoryError.spaceMismatch
            }
        } else if !expectedNamespaceID.isEmpty {
            throw RepositoryError.spaceMismatch
        }
        guard let listing = listings.first(where: { $0.appId == appID }) else {
            throw MobileError.AppRejected
        }
        gotten.append(appID)
        getNamespaces.append(expectedNamespaceID)
        onGet?()
        let app = RiotSpaceApp(
            appIDHex: RiotDirectoryRow.hex(appID),
            name: listing.name,
            description: listing.description,
            version: listing.version,
            permissions: listing.permissions,
            trusted: getTrustedOverride ?? RiotDirectoryRow.trustedInCurrentSpace(
                listing: listing,
                space: currentSpace
            )
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
        try endorseApp(
            appID: appID,
            note: note,
            retract: retract,
            expectedNamespaceID: currentSpace?.namespaceID ?? ""
        )
    }

    func endorseApp(
        appID: Data,
        note: String,
        retract: Bool,
        expectedNamespaceID: String
    ) throws {
        if let failure { throw failure }
        if let currentSpace {
            guard currentSpace.namespaceID.caseInsensitiveCompare(expectedNamespaceID) == .orderedSame else {
                throw RepositoryError.spaceMismatch
            }
        } else if !expectedNamespaceID.isEmpty {
            throw RepositoryError.spaceMismatch
        }
        endorsed.append((appID: appID, note: note, retract: retract))
        endorseNamespaces.append(expectedNamespaceID)
        let hex = RiotDirectoryRow.hex(appID).lowercased()
        if retract {
            endorsedAppIDs.remove(hex)
        } else {
            endorsedAppIDs.insert(hex)
        }
    }

    func shareApp(appID: Data) throws {
        try shareApp(appID: appID, expectedNamespaceID: currentSpace?.namespaceID ?? "")
    }

    func shareApp(appID: Data, expectedNamespaceID: String) throws {
        if let failure { throw failure }
        if let currentSpace {
            guard currentSpace.namespaceID.caseInsensitiveCompare(expectedNamespaceID) == .orderedSame else {
                throw RepositoryError.spaceMismatch
            }
        } else if !expectedNamespaceID.isEmpty {
            throw RepositoryError.spaceMismatch
        }
        shared.append(appID)
        shareNamespaces.append(expectedNamespaceID)
    }
}
