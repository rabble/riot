import XCTest
@testable import RiotKit

final class TransportContractTests: XCTestCase {
    func testDiscoveryUsesFriendlyEphemeralNamesInsteadOfTechnicalIdentifiers() {
        let first = FriendlyNameGenerator.name(sessionNonce: 7)
        let second = FriendlyNameGenerator.name(sessionNonce: 8)

        XCTAssertEqual(first, "Quiet Harbor")
        XCTAssertEqual(second, "Amber Kite")
        XCTAssertNotEqual(first, second)
        XCTAssertFalse(first.contains("-"))
    }

    func testPairingMustBeExplicitlyConfirmedBeforeFramesCanMove() throws {
        let channels = LoopbackFrameChannel.pair()
        let connection = NearbyConnection(bluetooth: channels.first, localAttempt: { nil })

        XCTAssertThrowsError(try connection.activate())
        XCTAssertThrowsError(try connection.send(Data([1])))

        connection.confirmPairing()
        try connection.activate()
        XCTAssertEqual(connection.route, .bluetooth)
    }

    func testFramesPreserveBytesAndOrder() throws {
        let channels = LoopbackFrameChannel.pair()
        var received: [Data] = []
        channels.second.onReceive = { received.append($0) }
        let connection = NearbyConnection(bluetooth: channels.first, localAttempt: { nil })
        connection.confirmPairing()
        try connection.activate()

        let frames = [Data([0, 1, 2]), Data([255, 0]), Data("riot".utf8)]
        for frame in frames { try connection.send(frame) }

        XCTAssertEqual(received, frames)
    }

    func testDisconnectIsRecoverableWithAFreshSession() throws {
        let firstChannels = LoopbackFrameChannel.pair()
        let first = NearbyConnection(bluetooth: firstChannels.first, localAttempt: { nil })
        first.confirmPairing()
        try first.activate()
        first.disconnect()
        XCTAssertThrowsError(try first.send(Data([1])))

        let retryChannels = LoopbackFrameChannel.pair()
        var retried: [Data] = []
        retryChannels.second.onReceive = { retried.append($0) }
        let retry = NearbyConnection(bluetooth: retryChannels.first, localAttempt: { nil })
        retry.confirmPairing()
        try retry.activate()
        try retry.send(Data([2]))

        XCTAssertEqual(retried, [Data([2])])
    }

    func testOneFailedLocalAttemptFixesSessionToBluetoothWithoutInternetFallback() throws {
        let channels = LoopbackFrameChannel.pair()
        var localAttempts = 0
        let connection = NearbyConnection(bluetooth: channels.first) {
            localAttempts += 1
            return nil
        }
        connection.confirmPairing()
        try connection.activate()
        try connection.send(Data([1]))
        try connection.send(Data([2]))

        XCTAssertEqual(localAttempts, 1)
        XCTAssertEqual(connection.route, .bluetooth)
        XCTAssertEqual(Set(NearbyRoute.allCases), [.localNetwork, .bluetooth])
    }

    func testLocalHandoffAcceptsOnlyNumericPrivateOrLinkLocalEndpoints() {
        XCTAssertNotNil(LocalEndpoint(host: "192.168.4.2", port: 9_001))
        XCTAssertNotNil(LocalEndpoint(host: "10.0.0.7", port: 9_001))
        XCTAssertNotNil(LocalEndpoint(host: "fe80::1", port: 9_001))
        XCTAssertNotNil(LocalEndpoint(host: "fd12:3456::1", port: 9_001))

        XCTAssertNil(LocalEndpoint(host: "example.com", port: 443))
        XCTAssertNil(LocalEndpoint(host: "localhost", port: 9_001))
        XCTAssertNil(LocalEndpoint(host: "8.8.8.8", port: 53))
        XCTAssertNil(LocalEndpoint(host: "192.168.4.2", port: 0))
    }
}
