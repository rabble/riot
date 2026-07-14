import XCTest
@testable import RiotKit

final class ShellNavigationTests: XCTestCase {
    func testConferenceShellExposesOnlyWorkingSurfaces() {
        XCTAssertEqual(
            RiotDestination.phoneTabs.map(\.title),
            [
                "Spaces",
                "App directory",
                "Incident board",
                "Post an update",
                "Connection",
            ]
        )
        XCTAssertEqual(
            RiotDestination.phoneTabs.map(\.tabTitle),
            ["Spaces", "Apps", "Board", "Post", "Connect"]
        )
    }

    @MainActor
    func testEveryPhoneTabCanBecomeTheVisibleDestination() {
        let model = RiotAppModel()

        for destination in RiotDestination.phoneTabs {
            model.select(destination)
            XCTAssertEqual(model.destination, destination)
        }
    }

    @MainActor
    func testConnectionStartsExplicitlyOffline() {
        let model = RiotAppModel()
        XCTAssertEqual(model.connectionStatus, .offline)
        XCTAssertEqual(model.connectionDisclosure, "Offline · local device only")
    }

    @MainActor
    func testDismissingAnAlertClearsItsBackingError() {
        let model = RiotAppModel(testError: "InvalidInput")

        model.dismissError()

        XCTAssertNil(model.errorMessage)
    }

    // MARK: - Looking closer at a board entry

    private static func entry(
        headline: String = "Medic tent moved to the north gate",
        validFrom: UInt64? = 1_720_000_500,
        aiAssisted: Bool = false
    ) -> RiotEntry {
        RiotEntry(
            entryID: String(repeating: "a", count: 64),
            namespaceID: String(repeating: "b", count: 64),
            signerID: String(repeating: "c", count: 64),
            headline: headline,
            createdAt: 1_720_000_000,
            validFrom: validFrom,
            expiresAt: 1_720_003_600,
            aiAssisted: aiAssisted
        )
    }

    /// Tapping a board row opens the signed detail. What that sheet may show
    /// WITHOUT being asked is the product decision: the alert's words and the
    /// window it is good for. The 64-hex identifiers are evidence, not reading
    /// material — they stay behind **Technical details** (accessibility contract:
    /// full ids never lead a surface).
    func testTheAlertDetailKeepsFullIdentifiersBehindTechnicalDetails() {
        let entry = Self.entry()
        let detail = AlertDetail(entry: entry)

        XCTAssertEqual(detail.headline, "Medic tent moved to the north gate")

        // Nothing shown on open is a raw identifier.
        let onOpen = detail.summary.map(\.value).joined(separator: " ")
        for identifier in [entry.entryID, entry.namespaceID, entry.signerID] {
            XCTAssertFalse(
                onOpen.contains(identifier),
                "a full identifier must not be shown before Technical details is opened"
            )
        }
        XCTAssertEqual(detail.summary.map(\.label), ["Created", "Valid from", "Expires"])

        // And they are all reachable behind the disclosure, in full — hidden by
        // default is not the same as withheld.
        XCTAssertEqual(AlertDetail.technicalDisclosureTitle, "Technical details")
        XCTAssertEqual(detail.technical.map(\.label), ["Entry", "Namespace", "Signer"])
        XCTAssertEqual(
            detail.technical.map(\.value),
            [entry.entryID, entry.namespaceID, entry.signerID],
            "the ids are shown whole — a truncated id proves nothing"
        )
    }

    /// An alert with no start time has no "Valid from" row at all, rather than a
    /// row printing an epoch zero.
    func testAnAlertWithNoStartTimeShowsNoValidFromRow() {
        let detail = AlertDetail(entry: Self.entry(validFrom: nil))

        XCTAssertEqual(detail.summary.map(\.label), ["Created", "Expires"])
    }

    /// The AI-assistance flag reaches the detail, because it changes how a
    /// person reads the alert.
    func testTheAIAssistanceFlagReachesTheDetail() {
        XCTAssertFalse(AlertDetail(entry: Self.entry()).aiAssisted)
        XCTAssertTrue(AlertDetail(entry: Self.entry(aiAssisted: true)).aiAssisted)
    }

    // MARK: - Discovery must not start before there is anything to announce

    /// The Connection screen is built at launch like every other tab, so its
    /// `onAppear` runs while `bootstrap` is still opening the profile. It may not
    /// advertise in that window: a phone that pairs with no repository behind it
    /// announces a nil space, and a peer cannot adopt a space it was never told
    /// about. This is the readiness the screen gates on.
    @MainActor
    func testAPhoneIsNotReadyToAdvertiseUntilItsProfileIsOpen() throws {
        let directory = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let model = RiotAppModel()

        // Before bootstrap there is no repository, so there is nothing a pairing
        // could even announce with.
        XCTAssertFalse(model.isProfileOpen)
        XCTAssertNil(model.nearbySpaceHost)

        model.bootstrap(
            storageDirectory: directory,
            keyStore: TestWrappingKeyStore(),
            starterPacks: []
        )

        XCTAssertTrue(model.isProfileOpen)
        XCTAssertNotNil(model.nearbySpaceHost)
    }

    /// The demo flow, and the bug: the organizer opens the app — which starts
    /// discovery immediately, with NO space — and only then taps "Create space".
    ///
    /// The handshake announces `host.currentSpace`, read live at pairing time, so
    /// the phone that started looking spaceless is holding the very host that will
    /// answer with the new space a moment later. Nothing needs to be re-captured;
    /// the announce just has to happen AGAIN, which is what `reannounceSpace` is
    /// for. If this test's second assertion ever fails, re-announcing cannot help
    /// and the space would have to travel some other way.
    @MainActor
    func testASpaceCreatedAfterLookingBeganIsVisibleToTheHostDiscoveryIsHolding() throws {
        let directory = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let model = RiotAppModel()
        model.bootstrap(
            storageDirectory: directory,
            keyStore: TestWrappingKeyStore(),
            starterPacks: []
        )

        // What `findNearby` captures at launch. The nil is the bug in the flesh:
        // pair now and this phone announces "I have no space".
        let host = try XCTUnwrap(model.nearbySpaceHost)
        XCTAssertNil(host.currentSpace)

        model.createSpace(title: "Fire Watch")

        XCTAssertEqual(host.currentSpace?.title, "Fire Watch")
        XCTAssertEqual(model.space?.title, "Fire Watch")
    }

    /// A space arriving later has to reach the peer — but a restart is only
    /// warranted where the handshake has ALREADY run and settled without sharing.
    /// Anywhere else it is either unnecessary (the handshake reads the space live)
    /// or destructive (it would tear down a running session).
    func testASpaceArrivingLaterIsReannouncedOnlyWhereAHandshakeIsAlreadyStuck() {
        // The bug, exactly: two spaceless phones settle here, the session ends, and
        // auto-connect can never re-dial. The organizer's new space reaches the peer
        // beside them ONLY if this restarts.
        XCTAssertTrue(NearbyReannounceGate.needsReannounce(state: .nothingToShare))
        XCTAssertTrue(NearbyReannounceGate.needsReannounce(state: .failed))

        // Still looking: no restart needed and none wanted. `SpacePairing` reads
        // `currentSpace` when it shakes hands, so the peer who turns up next will be
        // told about the space then. Restarting would only churn discovery.
        XCTAssertFalse(NearbyReannounceGate.needsReannounce(state: .looking))

        // Never mid-session: adopting a peer's space is itself a nil -> space change
        // and it lands in the middle of the sync that carries it over.
        XCTAssertFalse(
            NearbyReannounceGate.needsReannounce(state: .joinSpace(title: "Fire Watch", name: "PATIENT BROOM"))
        )
        XCTAssertFalse(NearbyReannounceGate.needsReannounce(state: .gettingLatest(name: "PATIENT BROOM")))
        XCTAssertFalse(NearbyReannounceGate.needsReannounce(state: .preview(count: 6, name: "PATIENT BROOM")))
        XCTAssertFalse(NearbyReannounceGate.needsReannounce(state: .connecting))
        XCTAssertFalse(NearbyReannounceGate.needsReannounce(state: .caughtUp))

        // And never onto a phone that is not looking: `.idle` is a phone that never
        // started, or one whose person tapped "Stop looking". A space appearing must
        // not put it on the air behind their back.
        XCTAssertFalse(NearbyReannounceGate.needsReannounce(state: .idle))
    }

    private static func temporaryProfileDirectory() throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("riot-shell-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
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
