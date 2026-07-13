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
