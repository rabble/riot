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

    func testSelectingLocalNetworkClosesUnusedBluetoothChannel() throws {
        let bluetooth = RecordingFrameChannel()
        let local = RecordingFrameChannel()
        let connection = NearbyConnection(bluetooth: bluetooth, localAttempt: { local })
        connection.confirmPairing()

        try connection.activate()

        XCTAssertEqual(connection.route, .localNetwork)
        XCTAssertTrue(bluetooth.didDisconnect)
        XCTAssertFalse(local.didDisconnect)
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

    func testCompletedResponderStaysAlreadyCurrentWhenTerminalCleanupIsAlreadyClosed() throws {
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

        XCTAssertEqual(coordinator.state, .alreadyCurrent)
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

    func testInboxDoesNotLetLiveFrameOvertakeRegistrationBacklog() throws {
        let inbox = BoundedFrameInbox()
        XCTAssertTrue(inbox.receive(Data([1])))
        let enteredFirst = DispatchSemaphore(value: 0)
        let releaseFirst = DispatchSemaphore(value: 0)
        let finished = DispatchSemaphore(value: 0)
        let received = LockedBytes()

        DispatchQueue.global().async {
            inbox.onReceive = { frame in
                if frame == Data([1]) {
                    enteredFirst.signal()
                    releaseFirst.wait()
                }
                received.append(frame[0])
                if frame == Data([2]) { finished.signal() }
            }
        }
        XCTAssertEqual(enteredFirst.wait(timeout: .now() + 1), .success)
        XCTAssertTrue(inbox.receive(Data([2])))
        releaseFirst.signal()
        XCTAssertEqual(finished.wait(timeout: .now() + 1), .success)
        XCTAssertEqual(received.value, [1, 2])
    }

    func testMalformedLengthFailsDecoderClosed() {
        var decoder = FrameDecoder()
        let oversized = UInt32(NearbyLimits.maxFrameBytes + 1).bigEndian
        let header = withUnsafeBytes(of: oversized) { Data($0) }

        XCTAssertThrowsError(try decoder.append(header))
    }

    func testBLEEnvelopeAllowsExactMaximumCoreFrameButRejectsLarger() throws {
        XCTAssertNoThrow(try BLEEnvelope.content(Data(count: NearbyLimits.maxFrameBytes)))
        XCTAssertThrowsError(try BLEEnvelope.content(Data(count: NearbyLimits.maxFrameBytes + 1)))
        let envelope = try BLEEnvelope.content(Data(count: NearbyLimits.maxFrameBytes))
        XCTAssertNoThrow(try FrameDecoder.encode(envelope, maxFrameBytes: NearbyLimits.maxBLEEnvelopeBytes))
    }

    func testPeripheralPeerRegistryBindsConfirmationAndDecodersToOnePeer() throws {
        var registry = BLEPeripheralPeerRegistry()
        let alice = UUID()
        let mallory = UUID()
        XCTAssertTrue(registry.beginPairing(with: alice))
        XCTAssertFalse(registry.beginPairing(with: mallory))
        XCTAssertEqual(registry.confirmPending(), alice)
        XCTAssertTrue(registry.acceptsContent(from: alice))
        XCTAssertFalse(registry.acceptsContent(from: mallory))

        let aliceWire = try FrameDecoder.encode(Data([3, 1]))
        let malloryWire = try FrameDecoder.encode(Data([3, 2]))
        XCTAssertEqual(try registry.decode(aliceWire.prefix(2), from: alice), [])
        XCTAssertEqual(try registry.decode(malloryWire, from: mallory), [Data([3, 2])])
        XCTAssertEqual(try registry.decode(aliceWire.dropFirst(2), from: alice), [Data([3, 1])])
    }

    func testMalformedPeerCannotReserveInboundPairingSlot() {
        var registry = BLEPeripheralPeerRegistry()
        let attacker = UUID()
        let phone = UUID()

        XCTAssertNil(registry.validatedPairingRequest(Data("malformed".utf8), from: attacker))
        XCTAssertNil(registry.pendingPeer)
        let valid = PairingHandoff.encode(name: "Blue River", endpoint: nil, tieBreaker: "phone-token")
        XCTAssertNotNil(registry.validatedPairingRequest(valid, from: phone))
        XCTAssertEqual(registry.pendingPeer, phone)
    }

    func testRevokingPeerAfterFailedAcknowledgementRemovesContentAuthority() {
        var registry = BLEPeripheralPeerRegistry()
        let phone = UUID()
        XCTAssertTrue(registry.beginPairing(with: phone))
        XCTAssertEqual(registry.confirmPending(), phone)
        XCTAssertTrue(registry.acceptsContent(from: phone))

        registry.remove(phone)

        XCTAssertFalse(registry.acceptsContent(from: phone))
        XCTAssertNil(registry.confirmedPeer)
    }

    func testPartiallySentBLECursorStillChargesItsFullRetainedAllocation() {
        var cursor = BLEWriteCursor(data: Data(count: NearbyLimits.maxFrameBytes))
        _ = cursor.nextChunk(limit: 20)

        XCTAssertEqual(cursor.retainedCount, NearbyLimits.maxFrameBytes)
        XCTAssertEqual(cursor.remainingCount, NearbyLimits.maxFrameBytes - 20)
    }

    func testHiddenTieBreakerSelectsExactlyOneLocalNetworkDialerWhenNamesCollide() {
        XCTAssertEqual(LocalNetworkRole.select(localName: "Blue River", localToken: "a", remoteName: "Blue River", remoteToken: "b"), .attempt)
        XCTAssertEqual(LocalNetworkRole.select(localName: "Blue River", localToken: "b", remoteName: "Blue River", remoteToken: "a"), .wait)
    }

    func testListenerEndpointCompletionIsOneShot() {
        let values = LockedEndpoints()
        let completion = OneShotEndpointCompletion { values.append($0) }
        let endpoint = LocalEndpoint(host: "192.168.1.2", port: 9_001)!

        completion.call(endpoint)
        completion.call(nil)

        XCTAssertEqual(values.value, [endpoint])
    }

    func testLateAcceptedLocalChannelIsRejectedAfterWinnerOrNewSearchGeneration() {
        let first = UUID()
        let second = UUID()

        XCTAssertTrue(LocalChannelAdmission.accepts(callbackGeneration: first, currentGeneration: first, routeChosen: false))
        XCTAssertFalse(LocalChannelAdmission.accepts(callbackGeneration: first, currentGeneration: first, routeChosen: true))
        XCTAssertFalse(LocalChannelAdmission.accepts(callbackGeneration: first, currentGeneration: second, routeChosen: false))
        XCTAssertFalse(LocalChannelAdmission.accepts(callbackGeneration: first, currentGeneration: nil, routeChosen: false))
    }

    func testActiveChannelFailureClosesSessionAndSurfacesPlainFailure() throws {
        let channel = RecordingFrameChannel()
        let connection = NearbyConnection(bluetooth: channel, localAttempt: { nil })
        connection.confirmPairing(); try connection.activate()
        let session = FakeSyncBoundary(outbound: [Data("hello".utf8)])
        let coordinator = SyncCoordinator(session: session, connection: connection, friendlyName: "Blue River")
        coordinator.start()

        channel.triggerFailure()

        XCTAssertEqual(coordinator.state, .failed)
        XCTAssertEqual(session.closeCalls, 1)
        XCTAssertThrowsError(try connection.send(Data([1])))
    }

    func testFailureBeforeChannelActivationIsDeliveredWhenCoordinatorRegisters() {
        let latch = FailureLatch()
        let calls = LockedCounter()
        latch.fail()

        latch.callback = { calls.increment() }

        XCTAssertEqual(calls.value, 1)
        latch.fail()
        XCTAssertEqual(calls.value, 1)
    }

    func testCoordinatorClosesTerminalSessionsAndDisconnectsOnlyOnFailure() throws {
        for action in ["accept", "reject", "failure"] {
            let channels = LoopbackFrameChannel.pair()
            let connection = NearbyConnection(bluetooth: channels.first, localAttempt: { nil })
            connection.confirmPairing(); try connection.activate()
            let session = FakeSyncBoundary(
                outbound: [],
                beginOutcome: action == "failure" ? .failed : .readyToPreview(count: 1)
            )
            let coordinator = SyncCoordinator(session: session, connection: connection, friendlyName: "Blue River")
            coordinator.start()
            if action == "accept" { coordinator.addPreviewedContent() }
            if action == "reject" { coordinator.rejectPreviewedContent() }

            XCTAssertEqual(session.closeCalls, 1, action)
            if action == "failure" {
                XCTAssertThrowsError(try connection.send(Data([1])), action)
            } else {
                XCTAssertNoThrow(try connection.send(Data([1])), action)
            }
        }
    }

    func testAcceptPumpsNonterminalFrameThenWaitsForPeerCompleteBeforeClosing() throws {
        let channels = LoopbackFrameChannel.pair()
        var wire: [Data] = []
        channels.second.onReceive = { wire.append($0) }
        let connection = NearbyConnection(bluetooth: channels.first, localAttempt: { nil })
        connection.confirmPairing(); try connection.activate()
        let session = FakeSyncBoundary(
            outbound: [Data("accepted".utf8)],
            beginOutcome: .readyToPreview(count: 1),
            receiveOutcome: .done,
            acceptOutcome: .sendMore(terminal: false)
        )
        let coordinator = SyncCoordinator(session: session, connection: connection, friendlyName: "Blue River")
        coordinator.start()

        coordinator.addPreviewedContent()
        XCTAssertEqual(wire, [Data("accepted".utf8)])
        XCTAssertEqual(session.outboundCalls, 1)
        XCTAssertEqual(session.closeCalls, 0)
        XCTAssertEqual(coordinator.state, .gettingLatest(name: "Blue River"))

        try channels.second.send(Data("complete".utf8))
        XCTAssertEqual(coordinator.state, .caughtUp)
        XCTAssertEqual(session.closeCalls, 1)
    }

    func testTerminalRejectPumpsExactlyOneFrameThenClosesWithoutDisconnectingEarly() throws {
        let channels = LoopbackFrameChannel.pair()
        var wire: [Data] = []
        channels.second.onReceive = { wire.append($0) }
        let connection = NearbyConnection(bluetooth: channels.first, localAttempt: { nil })
        connection.confirmPairing(); try connection.activate()
        let session = FakeSyncBoundary(
            outbound: [Data("reject".utf8)],
            beginOutcome: .readyToPreview(count: 1),
            rejectOutcome: .sendMore(terminal: true)
        )
        let coordinator = SyncCoordinator(session: session, connection: connection, friendlyName: "Blue River")
        coordinator.start()

        coordinator.rejectPreviewedContent()

        XCTAssertEqual(wire, [Data("reject".utf8)])
        XCTAssertEqual(session.outboundCalls, 1)
        XCTAssertEqual(session.closeCalls, 1)
        XCTAssertEqual(coordinator.state, .idle)
        XCTAssertNoThrow(try connection.send(Data([9])))
    }

    func testResponderTerminalFramePumpsOnceThenShowsAlreadyCurrent() throws {
        let channels = LoopbackFrameChannel.pair()
        var wire: [Data] = []
        channels.second.onReceive = { wire.append($0) }
        let connection = NearbyConnection(bluetooth: channels.first, localAttempt: { nil })
        connection.confirmPairing(); try connection.activate()
        let session = FakeSyncBoundary(outbound: [Data("complete".utf8)], beginOutcome: .sendMore(terminal: true))
        let coordinator = SyncCoordinator(session: session, connection: connection, friendlyName: "Blue River")

        coordinator.start()

        XCTAssertEqual(wire, [Data("complete".utf8)])
        XCTAssertEqual(session.outboundCalls, 1)
        XCTAssertEqual(session.closeCalls, 1)
        XCTAssertEqual(coordinator.state, .alreadyCurrent)
    }
}

private final class FakeSyncBoundary: MobileSyncSessionBoundary {
    var didBegin = false
    var outboundCalls = 0
    var closeCalls = 0
    private var outbound: [Data]
    private let beginOutcome: NearbySyncOutcome
    private let receiveOutcome: NearbySyncOutcome
    private let acceptOutcome: NearbySyncOutcome
    private let rejectOutcome: NearbySyncOutcome
    private let closeError: Error?
    private let outboundErrorAfterTerminal: Bool

    init(
        outbound: [Data],
        beginOutcome: NearbySyncOutcome = .sendMore(),
        receiveOutcome: NearbySyncOutcome = .sendMore(),
        acceptOutcome: NearbySyncOutcome = .done,
        rejectOutcome: NearbySyncOutcome = .done,
        closeError: Error? = nil,
        outboundErrorAfterTerminal: Bool = false
    ) {
        self.outbound = outbound
        self.beginOutcome = beginOutcome
        self.receiveOutcome = receiveOutcome
        self.acceptOutcome = acceptOutcome
        self.rejectOutcome = rejectOutcome
        self.closeError = closeError
        self.outboundErrorAfterTerminal = outboundErrorAfterTerminal
    }
    func begin() throws -> NearbySyncOutcome { didBegin = true; return beginOutcome }
    func nextOutbound() throws -> Data? {
        outboundCalls += 1
        if outboundErrorAfterTerminal && beginOutcome == .done { throw NearbyTransportError.disconnected }
        return outbound.isEmpty ? nil : outbound.removeFirst()
    }
    func receive(_ frame: Data) throws -> NearbySyncOutcome { receiveOutcome }
    func acceptImport() throws -> NearbySyncOutcome { acceptOutcome }
    func rejectImport() throws -> NearbySyncOutcome { rejectOutcome }
    func close() throws { closeCalls += 1; if let closeError { throw closeError } }
}

private final class LockedBytes: @unchecked Sendable {
    private let lock = NSLock()
    private var bytes: [UInt8] = []
    func append(_ byte: UInt8) { lock.lock(); bytes.append(byte); lock.unlock() }
    var value: [UInt8] { lock.lock(); defer { lock.unlock() }; return bytes }
}

private final class LockedEndpoints: @unchecked Sendable {
    private let lock = NSLock()
    private var endpoints: [LocalEndpoint] = []
    func append(_ endpoint: LocalEndpoint?) {
        guard let endpoint else { return }
        lock.lock(); endpoints.append(endpoint); lock.unlock()
    }
    var value: [LocalEndpoint] { lock.lock(); defer { lock.unlock() }; return endpoints }
}

private final class LockedCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0
    func increment() { lock.lock(); count += 1; lock.unlock() }
    var value: Int { lock.lock(); defer { lock.unlock() }; return count }
}

private final class RecordingFrameChannel: FrameChannel {
    var onReceive: ((Data) -> Void)?
    var onFailure: (() -> Void)?
    private(set) var didDisconnect = false
    func send(_ frame: Data) throws {}
    func disconnect() { didDisconnect = true }
    func triggerFailure() { onFailure?() }
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
