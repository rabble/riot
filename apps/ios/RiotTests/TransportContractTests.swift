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

    func testManyMTUChunksReassembleOneLargeFrameWithoutReordering() throws {
        let payload = Data((0..<8_192).map { UInt8($0 % 251) })
        let encoded = try FrameDecoder.encode(payload)
        var decoder = FrameDecoder()
        var decoded: [Data] = []

        for offset in stride(from: 0, to: encoded.count, by: 19) {
            decoded.append(contentsOf: try decoder.append(encoded.subdata(in: offset..<min(offset + 19, encoded.count))))
        }

        XCTAssertEqual(decoded, [payload])
    }

    func testCoordinatorBeginsProtocolAndSendsHelloImmediately() throws {
        let channels = LoopbackFrameChannel.pair()
        var wire: [Data] = []
        channels.second.onReceive = { wire.append($0) }
        let connection = NearbyConnection(bluetooth: channels.first, localAttempt: { nil })
        connection.confirmPairing()
        try connection.activate()
        let session = FakeSyncBoundary(outbound: [Data("hello".utf8)])
        let coordinator = SyncCoordinator(session: session, connection: connection, friendlyName: "Quiet Harbor")

        coordinator.start()

        XCTAssertTrue(session.didBegin)
        XCTAssertEqual(wire, [Data("hello".utf8)])
    }

    func testGeneratedAdapterPersistsPendingBundleWhenImportIsAccepted() throws {
        let bundle = Data([9, 8, 7])
        let backend = FakeGeneratedBackend(bundle: bundle)
        var persisted: [Data] = []
        let adapter = GeneratedSyncSessionAdapter(backend: backend) { persisted.append($0) }

        XCTAssertEqual(try adapter.receive(Data([1])), .readyToPreview(count: 0))
        try adapter.acceptImport()

        XCTAssertEqual(persisted, [bundle])
        XCTAssertTrue(backend.didAccept)
    }

    func testCompletedSyncStaysCaughtUpWhenTerminalCleanupIsAlreadyClosed() throws {
        let channels = LoopbackFrameChannel.pair()
        let connection = NearbyConnection(bluetooth: channels.first, localAttempt: { nil })
        connection.confirmPairing()
        try connection.activate()
        let session = FakeSyncBoundary(
            outbound: [],
            beginOutcome: .done,
            closeError: NearbyTransportError.disconnected,
            outboundErrorAfterTerminal: true
        )
        let coordinator = SyncCoordinator(session: session, connection: connection, friendlyName: "Quiet Harbor")

        coordinator.start()

        XCTAssertEqual(coordinator.state, .caughtUp)
        XCTAssertEqual(session.outboundCalls, 0)
    }

    func testPersistenceFailurePreventsImportAcceptance() throws {
        let backend = FakeGeneratedBackend(bundle: Data([4, 5, 6]))
        let adapter = GeneratedSyncSessionAdapter(backend: backend) { _ in
            throw NearbyTransportError.disconnected
        }
        _ = try adapter.receive(Data([1]))

        XCTAssertThrowsError(try adapter.acceptImport())
        XCTAssertFalse(backend.didAccept)
    }

    func testFramesArrivingBeforeReceiverRegistrationDeliverExactlyOnceInOrder() throws {
        let channels = LoopbackFrameChannel.pair()
        let expected = [Data([1]), Data([2]), Data([3])]
        for frame in expected { try channels.first.send(frame) }
        var received: [Data] = []

        channels.second.onReceive = { received.append($0) }

        XCTAssertEqual(received, expected)
    }

    func testPreRegistrationInboxOverflowFailsClosed() throws {
        let channels = LoopbackFrameChannel.pair()
        for value in 0..<64 { try channels.first.send(Data([UInt8(value)])) }

        XCTAssertThrowsError(try channels.first.send(Data([255])))
        var received: [Data] = []
        channels.second.onReceive = { received.append($0) }
        XCTAssertTrue(received.isEmpty)
    }

    func testLargeBLEWriteUsesOneStreamingCursorInsteadOfExpandedChunkQueue() {
        let encoded = Data(repeating: 7, count: NearbyLimits.maxFrameBytes)
        var cursor = BLEWriteCursor(data: encoded)

        XCTAssertEqual(cursor.remainingCount, encoded.count)
        XCTAssertEqual(cursor.nextChunk(limit: 20), Data(repeating: 7, count: 20))
        XCTAssertEqual(cursor.remainingCount, encoded.count - 20)
        XCTAssertFalse(cursor.isComplete)
    }

    func testFrameEncoderRejectsAnythingBeyondCoreProtocolLimit() throws {
        XCTAssertNoThrow(try FrameDecoder.encode(Data(count: NearbyLimits.maxFrameBytes)))
        XCTAssertThrowsError(try FrameDecoder.encode(Data(count: NearbyLimits.maxFrameBytes + 1)))
    }
}

private final class FakeSyncBoundary: MobileSyncSessionBoundary {
    var didBegin = false
    var outboundCalls = 0
    private var outbound: [Data]
    private let beginOutcome: NearbySyncOutcome
    private let closeError: Error?
    private let outboundErrorAfterTerminal: Bool

    init(
        outbound: [Data],
        beginOutcome: NearbySyncOutcome = .sendMore,
        closeError: Error? = nil,
        outboundErrorAfterTerminal: Bool = false
    ) {
        self.outbound = outbound
        self.beginOutcome = beginOutcome
        self.closeError = closeError
        self.outboundErrorAfterTerminal = outboundErrorAfterTerminal
    }
    func begin() throws -> NearbySyncOutcome { didBegin = true; return beginOutcome }
    func nextOutbound() throws -> Data? {
        outboundCalls += 1
        if outboundErrorAfterTerminal && beginOutcome == .done { throw NearbyTransportError.disconnected }
        return outbound.isEmpty ? nil : outbound.removeFirst()
    }
    func receive(_ frame: Data) throws -> NearbySyncOutcome { .sendMore }
    func acceptImport() throws {}
    func rejectImport() throws {}
    func close() throws { if let closeError { throw closeError } }
}

private final class FakeGeneratedBackend: GeneratedSyncSessionBackend {
    let bundle: Data
    var didAccept = false

    init(bundle: Data) { self.bundle = bundle }
    func begin() throws -> SyncOutcome { outcome(kind: .frameReady) }
    func takeOutboundFrame() throws -> Data? { nil }
    func receiveFrame(frameBytes: Data) throws -> SyncOutcome { outcome(kind: .reviewImport, bundle: bundle) }
    func acceptImport() throws -> SyncOutcome { didAccept = true; return outcome(kind: .complete) }
    func rejectImport(code: UInt8) throws -> SyncOutcome { outcome(kind: .rejected) }
    func cancel() throws {}

    private func outcome(kind: SyncOutcomeKind, bundle: Data? = nil) -> SyncOutcome {
        SyncOutcome(kind: kind, entries: [], rejectionCode: nil, terminal: kind == .complete, importBundleBytes: bundle)
    }
}
