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
