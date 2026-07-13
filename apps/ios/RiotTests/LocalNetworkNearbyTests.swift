import XCTest
@testable import RiotKit

/// Covers the transport changes that let two instances on ONE machine discover
/// each other and sync — the path Bluetooth cannot serve, because a single radio
/// never hears its own advertisement.
final class LocalNetworkNearbyTests: XCTestCase {

    // MARK: - LocalEndpoint: loopback is local

    func testLoopbackIsAcceptedAsALocalAddress() {
        // Same machine. Definitionally not "the internet", so the local-only
        // guarantee is intact — and it means two instances pair with Wi-Fi off.
        XCTAssertNotNil(LocalEndpoint(host: "127.0.0.1", port: 9000))
        XCTAssertNotNil(LocalEndpoint(host: "::1", port: 9000))
    }

    func testPrivateAndLinkLocalAddressesStillAccepted() {
        XCTAssertNotNil(LocalEndpoint(host: "192.168.1.5", port: 9000))
        XCTAssertNotNil(LocalEndpoint(host: "10.0.0.2", port: 9000))
        XCTAssertNotNil(LocalEndpoint(host: "172.16.0.9", port: 9000))
        XCTAssertNotNil(LocalEndpoint(host: "169.254.1.1", port: 9000))
    }

    /// The load-bearing half of the promise: a routable internet address is
    /// still refused, so widening this to loopback did not open a door.
    func testRoutableInternetAddressesAreStillRefused() {
        XCTAssertNil(LocalEndpoint(host: "8.8.8.8", port: 9000))
        XCTAssertNil(LocalEndpoint(host: "93.184.216.34", port: 9000))
        XCTAssertNil(LocalEndpoint(host: "172.32.0.1", port: 9000)) // just outside 172.16/12
        XCTAssertNil(LocalEndpoint(host: "2606:4700:4700::1111", port: 9000))
    }

    // MARK: - NearbyConnection: the local network can BE the base channel

    func testLocalNetworkCanBeTheBaseChannelWithoutAnyRadio() throws {
        // The whole point: on one Mac there is no usable radio, so the TCP
        // channel must be able to carry the session on its own.
        let (mine, theirs) = LoopbackFrameChannel.pair()
        let connection = NearbyConnection(base: mine, baseRoute: .localNetwork, localAttempt: { nil })
        connection.confirmPairing()

        try connection.activate()

        XCTAssertEqual(connection.route, .localNetwork)

        var received: Data?
        theirs.onReceive = { received = $0 }
        try connection.send(Data([0xAB]))
        XCTAssertEqual(received, Data([0xAB]))
    }

    /// A local-network base must not be swapped out by the "upgrade" path — there
    /// is nothing better to upgrade to, and swapping would drop the very channel
    /// the pairing arrived on.
    func testLocalNetworkBaseIsNotDiscardedByTheUpgradeAttempt() throws {
        let (base, basePeer) = LoopbackFrameChannel.pair()
        let (upgrade, upgradePeer) = LoopbackFrameChannel.pair()
        let connection = NearbyConnection(base: base, baseRoute: .localNetwork, localAttempt: { upgrade })
        connection.confirmPairing()

        try connection.activate()

        XCTAssertEqual(connection.route, .localNetwork)

        var onBase: Data?
        var onUpgrade: Data?
        basePeer.onReceive = { onBase = $0 }
        upgradePeer.onReceive = { onUpgrade = $0 }

        try connection.send(Data([0x01]))

        // The traffic went out on the channel that carried the pairing.
        XCTAssertEqual(onBase, Data([0x01]))
        XCTAssertNil(onUpgrade)
    }

    /// The existing Bluetooth path is unchanged: it still upgrades to TCP.
    func testBluetoothBaseStillUpgradesToLocalNetwork() throws {
        let (bluetooth, _) = LoopbackFrameChannel.pair()
        let (local, _) = LoopbackFrameChannel.pair()
        let connection = NearbyConnection(bluetooth: bluetooth, localAttempt: { local })
        connection.confirmPairing()

        try connection.activate()

        XCTAssertEqual(connection.route, .localNetwork)
    }

    func testBluetoothBaseFallsBackToBluetoothWhenNoLocalRoute() throws {
        let (bluetooth, _) = LoopbackFrameChannel.pair()
        let connection = NearbyConnection(bluetooth: bluetooth, localAttempt: { nil })
        connection.confirmPairing()

        try connection.activate()

        XCTAssertEqual(connection.route, .bluetooth)
    }

    // MARK: - Self-filtering

    /// Bonjour DOES return services advertised by other processes on the same
    /// host — including our own. Without this filter an instance discovers
    /// itself and offers to sync with itself.
    func testAnInstanceDoesNotDiscoverItself() {
        let me = NearbyPeerIdentity(instanceID: "A", friendlyName: "Blue River", tieBreaker: "t1")
        let alsoMe = NearbyPeerIdentity(instanceID: "A", friendlyName: "Blue River", tieBreaker: "t1")
        let other = NearbyPeerIdentity(instanceID: "B", friendlyName: "Amber Kite", tieBreaker: "t2")

        XCTAssertTrue(me.isSelf(alsoMe))
        XCTAssertFalse(me.isSelf(other))
    }

    /// Two instances of the same build on one Mac share a friendly name only by
    /// coincidence; identity is the instance id, so same-name peers are distinct.
    func testSameFriendlyNameDifferentInstanceIsNotSelf() {
        let me = NearbyPeerIdentity(instanceID: "A", friendlyName: "Blue River", tieBreaker: "t1")
        let twin = NearbyPeerIdentity(instanceID: "B", friendlyName: "Blue River", tieBreaker: "t2")

        XCTAssertFalse(me.isSelf(twin))
    }

    // MARK: - TXT record codec

    func testPeerIdentityRoundTripsThroughATXTRecord() throws {
        let identity = NearbyPeerIdentity(instanceID: "abc-123", friendlyName: "Quiet Harbor", tieBreaker: "zz")

        let decoded = NearbyPeerIdentity(txt: identity.txtRecord)

        XCTAssertEqual(decoded, identity)
    }

    func testMalformedTXTRecordDecodesToNil() {
        XCTAssertNil(NearbyPeerIdentity(txt: [:]))
        XCTAssertNil(NearbyPeerIdentity(txt: ["instance": "a"])) // missing name/tieBreaker
    }

    // MARK: - Pairing handshake codec

    func testPairingRequestRoundTrips() throws {
        let identity = NearbyPeerIdentity(instanceID: "abc", friendlyName: "Blue River", tieBreaker: "t1")
        let frame = LocalPairingMessage.request(identity).encoded()

        guard case let .request(decoded)? = LocalPairingMessage(frame: frame) else {
            return XCTFail("expected a request")
        }
        XCTAssertEqual(decoded, identity)
    }

    func testPairingAcceptRoundTrips() throws {
        let identity = NearbyPeerIdentity(instanceID: "xyz", friendlyName: "Amber Kite", tieBreaker: "t2")
        let frame = LocalPairingMessage.accept(identity).encoded()

        guard case let .accept(decoded)? = LocalPairingMessage(frame: frame) else {
            return XCTFail("expected an accept")
        }
        XCTAssertEqual(decoded, identity)
    }

    func testPairingDeclineRoundTrips() throws {
        guard case .decline? = LocalPairingMessage(frame: LocalPairingMessage.decline.encoded()) else {
            return XCTFail("expected a decline")
        }
    }

    /// A sync frame must never be mistaken for a pairing frame once the channel
    /// has been handed to the session.
    func testGarbageFrameDecodesToNil() {
        XCTAssertNil(LocalPairingMessage(frame: Data()))
        XCTAssertNil(LocalPairingMessage(frame: Data([0xFF, 0x00, 0x01])))
    }
}

// MARK: - The demo, in a test

/// Two whole phones, in one process, over the real network.
///
/// Everything above this line tests a piece. This tests the thing: two real
/// `NearbyTransportController`s — the exact object the app puts on screen —
/// finding each other over real Bonjour (`_riot-sync._tcp`, a real `NWListener`
/// advertising and a real `NWBrowser` browsing), dialling a real TCP connection,
/// running the real space handshake and the real sync, with a person tapping
/// "Connect", "Add them" on each side. Then Bob's store has the item Alice added.
///
/// Nothing here hand-picks who opens the protocol. Both controllers are driven
/// through the SAME public API the UI calls, and the initiator election happens
/// inside `NearbyTransportController.adopt` where it does on a real phone. That
/// is the whole point: this test is wired the way the app is wired, so it is
/// capable of catching what the app actually does.
///
/// It would have caught the both-peers-`start()` bug. Before the one-initiator
/// fix, both sides called `begin()`, each handed the other a `Hello` the core
/// rejects outside `Phase::Idle`, both sessions failed, and nothing replicated —
/// while every other sync test in the repo stayed green, because they all start
/// exactly one side by hand.
@MainActor
final class TwoPeerNearbySyncTests: XCTestCase {

    private struct Peer {
        let repository: RiotProfileRepository
        let appID: String
    }

    /// THE test. Alice adds an item; Bob ends up with it, having crossed a real
    /// network to get it.
    func testTwoRealControllersFindEachOtherOverBonjourAndSyncAnItem() async throws {
        let (alice, bob) = try openPair()

        let key = "items/\(UUID().uuidString.lowercased())"
        let added = try item(text: "Bring water to the corner", by: "Alice")
        // The call the checklist page makes when someone types an item and taps Add.
        try XCTUnwrap(alice.repository.appDataBridge(appID: alice.appID)).put(key: key, valueJSON: added)
        XCTAssertNil(
            try bob.repository.appDataGet(appID: bob.appID, key: key),
            "Bob must not already have the item — otherwise this proves nothing"
        )

        let realm = Self.privateServiceType()
        let aliceNearby = NearbyTransportController(usesBluetooth: false, serviceType: realm)
        let bobNearby = NearbyTransportController(usesBluetooth: false, serviceType: realm)
        defer {
            aliceNearby.stop()
            bobNearby.stop()
        }

        // Both phones open "Find nearby devices". Real advertising, real browsing.
        aliceNearby.findNearby(host: alice.repository)
        bobNearby.findNearby(host: bob.repository)

        // Bob's phone SEES Alice's phone. This is Bonjour doing its job — no
        // endpoint is handed to anyone here.
        _ = try await firstDiscoveredPhone(on: bobNearby)

        // NOBODY TAPS "CONNECT". Both phones auto-connect on sight, which is what
        // they do on a table at a conference, and it is what broke this: both dialled,
        // each bound its session to a different socket, and the space handshake on
        // each side then talked into a socket the other had abandoned. Which phone
        // dials is the transport's business — the point is that exactly one does.
        //
        // This used to be hand-driven (`bobNearby.requestConnection` /
        // `confirmConnection`, then Alice confirming) and skipped when the dial did
        // not land, which is why it never caught any of this.

        // From here both people just tap "Add them" when their phone offers. The
        // sync — who begins, who answers — is the controller's business, not the
        // test's.
        try await settle(
            until: { (try? bob.repository.appDataGet(appID: bob.appID, key: key)) == added },
            failing: "Alice's item never reached Bob over the local network",
            aliceNearby, bobNearby
        )

        XCTAssertEqual(
            try bob.repository.appDataGet(appID: bob.appID, key: key), added,
            "Bob did not receive the item Alice added"
        )
        XCTAssertFalse(
            aliceNearby.state == .failed || bobNearby.state == .failed,
            "a peer's session failed: Alice \(aliceNearby.state), Bob \(bobNearby.state)"
        )
    }

    /// THE DEMO FINALE, as a test: a fresh phone that is in NO space meets an
    /// organizer who is in one, and ends up in theirs.
    ///
    /// This is the case the app existed to serve and could not do. Both phones
    /// auto-connected on sight, so BOTH dialled; each ended up holding two sockets
    /// to the other and bound its session to whichever pairing completed first, with
    /// nothing making the two agree. Half the time they chose opposite sockets, each
    /// announced its space into a socket the other had abandoned, neither ever read
    /// an announce, and `SpacePairing` reached no decision at all — so
    /// `SpaceAdoption.decide` was never asked, `.adopt` never fired, and the fresh
    /// phone stayed spaceless. Every unit test above stayed green throughout,
    /// because none of them lets two whole controllers meet.
    func testAFreshPhoneWithNoSpaceAdoptsTheOrganizersSpace() async throws {
        let organizer = try openPeer(name: "Organizer", joining: nil, approvingTheApp: true)
        let theirSpace = try XCTUnwrap(organizer.repository.currentSpace)

        let fresh = try openSpacelessPeer(name: "Fresh")
        XCTAssertNil(
            fresh.currentSpace,
            "the fresh phone must start in NO space — otherwise this proves nothing"
        )

        let realm = Self.privateServiceType()
        let organizerNearby = NearbyTransportController(usesBluetooth: false, serviceType: realm)
        let freshNearby = NearbyTransportController(usesBluetooth: false, serviceType: realm)
        defer {
            organizerNearby.stop()
            freshNearby.stop()
        }

        // Two phones on a table. Nobody taps "connect" on either one.
        organizerNearby.findNearby(host: organizer.repository)
        freshNearby.findNearby(host: fresh)

        // The only tap in the whole flow: the fresh phone asks whether to join, and
        // the person says yes. Joining a space IS a real decision and keeps its
        // confirmation — `settle` taps it, below.
        try await settle(
            until: { fresh.currentSpace != nil },
            failing: "the fresh phone never joined the organizer's space",
            organizerNearby, freshNearby
        )

        let joined = try XCTUnwrap(fresh.currentSpace)
        XCTAssertEqual(
            joined.namespaceID.lowercased(), theirSpace.namespaceID.lowercased(),
            "the fresh phone joined SOME space, but not the organizer's"
        )
        XCTAssertEqual(joined.title, theirSpace.title)
    }

    // MARK: - Driving two phones

    /// A Bonjour type nobody else is on, so this test's two phones find each other
    /// and nothing else.
    ///
    /// On the real type they find every Riot advertising on the machine and the LAN
    /// — other test processes, a developer's two app instances, the phone in
    /// someone's pocket. `phones` is sorted by name, so the peers would auto-connect
    /// to whichever STRANGER sorts first and never reach each other. That is not a
    /// hypothetical: this test saw seven strangers and dialled "Autumn Creek".
    ///
    /// A Bonjour type is at most 15 characters, hence the short random suffix.
    private static func privateServiceType() -> String {
        let suffix = String(UUID().uuidString.filter(\.isHexDigit).prefix(8)).lowercased()
        return "_riot\(suffix)._tcp"
    }

    /// Waits for Bob's phone to list a peer, which only happens once the real
    /// browser has resolved the real advertisement.
    private func firstDiscoveredPhone(
        on controller: NearbyTransportController,
        timeout: TimeInterval = 30
    ) async throws -> DiscoveredPhone {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if let phone = controller.phones.first { return phone }
            try await Task.sleep(for: .milliseconds(50))
        }
        throw XCTSkip(
            "no peer appeared over Bonjour in \(Int(timeout))s — the test host cannot browse "
            + "the local network, so this machine cannot prove the two-phone path"
        )
    }

    /// Runs both phones until `until` holds, tapping "Add them" for whoever is being
    /// shown a preview and "Join" for whoever is being offered a space.
    ///
    /// Those are the only two taps in the product, and they are the two decisions
    /// that are actually the person's: content only lands in the store because
    /// someone said yes, and a space is never joined silently. Everything else —
    /// who dials, who opens the sync — the controller decides, exactly as it does
    /// on a real phone. Each side is tapped at most once per offer it is shown.
    private func settle(
        until done: () -> Bool,
        failing message: String,
        timeout: TimeInterval = 60,
        _ controllers: NearbyTransportController...
    ) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        var accepted = Set<ObjectIdentifier>()
        var joined = Set<ObjectIdentifier>()
        while Date() < deadline {
            if done() { return }
            for controller in controllers {
                switch controller.state {
                case .preview:
                    guard accepted.insert(ObjectIdentifier(controller)).inserted else { continue }
                    controller.addPreviewedContent()
                case .joinSpace:
                    guard joined.insert(ObjectIdentifier(controller)).inserted else { continue }
                    controller.confirmJoinSpace()
                default:
                    continue
                }
            }
            if controllers.contains(where: { $0.state == .failed }) {
                return XCTFail(
                    "\(message) — a session failed: "
                    + controllers.map { "\($0.state)" }.joined(separator: ", ")
                )
            }
            try await Task.sleep(for: .milliseconds(50))
        }
        if !done() {
            XCTFail("\(message) (timed out after \(Int(timeout))s: "
                    + controllers.map { "\($0.state)" }.joined(separator: ", ") + ")")
        }
    }

    // MARK: - Two phones' profiles

    /// Alice creates the space, so she is its organizer and the only one who may
    /// approve the checklist. Bob joins and approves nothing — a member inherits
    /// the organizer's approval over sync, and the core refuses a self-approval.
    private func openPair() throws -> (alice: Peer, bob: Peer) {
        let alice = try openPeer(name: "Alice", joining: nil, approvingTheApp: true)
        let space = try XCTUnwrap(alice.repository.currentSpace)
        let bob = try openPeer(name: "Bob", joining: space, approvingTheApp: false)
        return (alice, bob)
    }

    /// A phone straight out of the box: an open profile, an identity, and NO space.
    /// It cannot even open a sync session (the core refuses one without a space), so
    /// the only way it ever gets one is by hearing a peer announce theirs.
    private func openSpacelessPeer(name: String) throws -> RiotProfileRepository {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("two-peer-\(name)-\(UUID().uuidString).json")
        return try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url),
            keyStore: TwoPeerKeyStore(),
            starterPacks: try starterPacks()
        )
    }

    private func openPeer(name: String, joining space: RiotSpace?, approvingTheApp: Bool) throws -> Peer {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("two-peer-\(name)-\(UUID().uuidString).json")
        if let space {
            let snapshot: [String: Any] = [
                "space": ["namespaceID": space.namespaceID, "title": space.title],
                "alerts": [],
                "trustedAppIDs": [],
                "appDataBundles": [],
            ]
            try JSONSerialization.data(withJSONObject: snapshot).write(to: url, options: .atomic)
        }
        let repository = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url),
            keyStore: TwoPeerKeyStore(),
            starterPacks: try starterPacks()
        )
        if space == nil { _ = try repository.createPublicSpace(title: "Berlin Mutual Aid") }
        let appID = try XCTUnwrap(repository.spaceApps().first).appIDHex
        if approvingTheApp { try repository.trustApp(appID: appID) }
        return Peer(repository: repository, appID: appID)
    }

    private func starterPacks() throws -> [(manifest: Data, bundle: Data)] {
        let apps = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // RiotTests
            .deletingLastPathComponent() // ios
            .deletingLastPathComponent() // apps
            .deletingLastPathComponent() // repo root
            .appendingPathComponent("fixtures/apps")
        return [(
            manifest: try Data(contentsOf: apps.appendingPathComponent("checklist.manifest.cbor")),
            bundle: try Data(contentsOf: apps.appendingPathComponent("checklist.bundle.cbor"))
        )]
    }

    /// One checklist row, shaped exactly as `fixtures/apps/checklist/app.js` writes it.
    private func item(text: String, by author: String) throws -> String {
        String(
            decoding: try JSONSerialization.data(
                withJSONObject: ["text": text, "done": false, "updated_by": author, "updated_at": 1],
                options: [.sortedKeys]
            ),
            as: UTF8.self
        )
    }
}

private final class TwoPeerKeyStore: WrappingKeyStore {
    private var key: Data?

    func loadOrCreateWrappingKey() throws -> Data {
        if let key { return key }
        let created = Data(repeating: 0x5a, count: 32)
        key = created
        return created
    }
}
