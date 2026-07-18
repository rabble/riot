import XCTest
@testable import RiotKit

final class TransportContractTests: XCTestCase {
    func testDiscoveryUsesFriendlyEphemeralNamesInsteadOfTechnicalIdentifiers() {
        let first = FriendlyNameGenerator.name(sessionNonce: 7)
        let second = FriendlyNameGenerator.name(sessionNonce: 8)

        // Assert the CONTRACT, not two magic strings. This test used to pin
        // "Quiet Harbor" and "Amber Kite" literally, which meant it broke the
        // moment the word lists grew — the names changing is not a regression,
        // it is the point. What must hold: a peer gets a stable, speakable,
        // two-word handle that is not a technical identifier, and different
        // phones get different handles. See `PeerNames` (and `PeerNamesTests`
        // for the name-space and independence properties).
        XCTAssertNotEqual(first, second, "different nonces must not collide this readily")
        XCTAssertEqual(first, FriendlyNameGenerator.name(sessionNonce: 7), "must be stable")
        for name in [first, second] {
            XCTAssertEqual(name.split(separator: " ").count, 2, "two words: \(name)")
            XCTAssertFalse(name.contains("-"), "not a technical identifier: \(name)")
            // A phone is not a person: a peer handle must never wear the
            // `Ana · a3f9` shape reserved for a name bound to a key.
            XCTAssertFalse(name.contains("·"), "a device handle must never carry a key tag: \(name)")
        }
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

    // MARK: - Telling an open app the store changed

    /// The refresh that redraws an already-open app fires on ACCEPT — after the
    /// import is in the store — and at no other moment. Content that has merely
    /// arrived is still sitting in the preview awaiting the person's yes, and
    /// redrawing then would show them data they never accepted.
    func testImportAcceptedFiresOnlyOnAccept() throws {
        let channels = LoopbackFrameChannel.pair()
        let connection = NearbyConnection(bluetooth: channels.first, localAttempt: { nil })
        connection.confirmPairing(); try connection.activate()
        let session = FakeSyncBoundary(
            outbound: [Data("accepted".utf8)],
            beginOutcome: .readyToPreview(count: 1),
            acceptOutcome: .done
        )
        let coordinator = SyncCoordinator(session: session, connection: connection, friendlyName: "Blue River")
        // Counted off the notification an open app actually listens for, not off
        // a hook this test supplied.
        var refreshes = 0
        let token = NotificationCenter.default.addObserver(
            forName: AppRuntimeView.dataChangedNotification, object: nil, queue: nil
        ) { _ in refreshes += 1 }
        defer { NotificationCenter.default.removeObserver(token) }

        coordinator.start()
        // Entries have ARRIVED and are being previewed. Nothing is in the store.
        XCTAssertEqual(coordinator.state, .preview(count: 1, name: "Blue River"))
        XCTAssertEqual(refreshes, 0, "a received-but-unaccepted import must not refresh an open app")

        coordinator.addPreviewedContent()
        XCTAssertEqual(refreshes, 1, "accepting the import must refresh the open app exactly once")
    }

    /// "Not now" must never redraw anything: a rejected import is not in the
    /// store, so there is nothing new for an open app to show.
    func testRejectingPreviewedContentNeverFiresTheRefresh() throws {
        let channels = LoopbackFrameChannel.pair()
        let connection = NearbyConnection(bluetooth: channels.first, localAttempt: { nil })
        connection.confirmPairing(); try connection.activate()
        let session = FakeSyncBoundary(
            outbound: [Data("reject".utf8)],
            beginOutcome: .readyToPreview(count: 1),
            rejectOutcome: .sendMore(terminal: true)
        )
        let coordinator = SyncCoordinator(session: session, connection: connection, friendlyName: "Blue River")
        var refreshes = 0
        let token = NotificationCenter.default.addObserver(
            forName: AppRuntimeView.dataChangedNotification, object: nil, queue: nil
        ) { _ in refreshes += 1 }
        defer { NotificationCenter.default.removeObserver(token) }

        coordinator.start()
        coordinator.rejectPreviewedContent()

        XCTAssertEqual(refreshes, 0, "declining an import must not refresh an open app")
    }

    // MARK: - Exactly one initiator

    /// The answering peer does NOT open the protocol. The core accepts a `Hello`
    /// only from an idle session, so if both peers began, each would hand the
    /// other a `Hello` in the wrong phase and both would fail. `answer()` leaves
    /// this side idle and ready to receive.
    func testAnswerDoesNotOpenTheProtocol() throws {
        let channels = LoopbackFrameChannel.pair()
        var wire: [Data] = []
        channels.second.onReceive = { wire.append($0) }
        let connection = NearbyConnection(bluetooth: channels.first, localAttempt: { nil })
        connection.confirmPairing(); try connection.activate()
        let session = FakeSyncBoundary(
            outbound: [Data("summary".utf8)],
            beginOutcome: .sendMore(terminal: false),
            receiveOutcome: .sendMore(terminal: false)
        )
        let coordinator = SyncCoordinator(session: session, connection: connection, friendlyName: "Blue River")

        coordinator.answer()

        XCTAssertFalse(session.didBegin, "the answering peer must not open the protocol")
        XCTAssertTrue(wire.isEmpty, "the answering peer must not send the first frame")
        XCTAssertEqual(
            coordinator.state, .gettingLatest(name: "Blue River"),
            "the person who accepted the prompt should see the exchange running"
        )

        // It is nonetheless live: the initiator's frame drives it, which is the
        // whole point of answering rather than sitting idle.
        try channels.second.send(Data("hello".utf8))
        XCTAssertEqual(wire, [Data("summary".utf8)], "the answering peer did not reply to the initiator")
    }
}

private final class FakeSyncBoundary: MobileSyncSessionBoundary {
    var didBegin = false
    var acceptCalls = 0
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
    func acceptImport() throws -> NearbySyncOutcome { acceptCalls += 1; return acceptOutcome }
    func rejectImport() throws -> NearbySyncOutcome { rejectOutcome }
    func close() throws { closeCalls += 1; if let closeError { throw closeError } }
}

/// Wraps a channel and records every frame sent through it, so a test can read
/// exactly what one device puts on the wire — the disclosure surface — while the
/// frame still travels to the peer.
private final class TappingChannel: FrameChannel {
    private let inner: FrameChannel
    private(set) var sent: [Data] = []

    init(inner: FrameChannel) { self.inner = inner }

    var onReceive: ((Data) -> Void)? {
        get { inner.onReceive }
        set { inner.onReceive = newValue }
    }
    var onFailure: (() -> Void)? {
        get { inner.onFailure }
        set { inner.onFailure = newValue }
    }
    func send(_ frame: Data) throws {
        sent.append(frame)
        try inner.send(frame)
    }
    func disconnect() { inner.disconnect() }
}

/// A pairing host with a fixed space and an inert sync boundary — enough to drive
/// `SpacePairing`'s confirmation and announce phases without a real repository.
private final class FakeNearbyHost: NearbySpaceHost {
    var currentSpace: RiotSpace?
    private let boundary: MobileSyncSessionBoundary

    init(space: RiotSpace?, boundary: MobileSyncSessionBoundary = FakeSyncBoundary(outbound: [])) {
        self.currentSpace = space
        self.boundary = boundary
    }
    func joinSpace(_ space: RiotSpace) throws { currentSpace = space }
    func openSyncBoundary() throws -> MobileSyncSessionBoundary { boundary }
}

extension TransportContractTests {
    func testNearbyPreparationRunsOnceBeforeSpaceJoinMutation() {
        var order: [String] = []
        var callbacksAreValid = true
        var transportWasStopped = false
        let gate = CommunityTransitionGate()
        let token = gate.registerPreparation { preparation in
            order.append("prepare")
            callbacksAreValid = false
            if !preparation.transportMustContinue {
                transportWasStopped = true
            }
        }
        let space = RiotSpace(namespaceID: "community-b", title: "Community B")

        let result = NearbySpaceJoinPreparation.run(
            joining: space,
            prepare: gate.prepareForNearbyAdoption,
            resume: {
                order.append("join")
                XCTAssertFalse(callbacksAreValid, "old shell callbacks are invalid before mutation")
                XCTAssertFalse(transportWasStopped, "the in-flight adoption wire must remain alive")
                return "resumed"
            }
        )

        XCTAssertEqual(order, ["prepare", "join"])
        XCTAssertEqual(result, "resumed")
        gate.unregister(token)
    }

    func testNearbyPreparationDoesNotRunForSameCommunitySync() {
        var prepareCount = 0
        _ = NearbySpaceJoinPreparation.run(
            joining: nil,
            prepare: { prepareCount += 1 },
            resume: { "resumed" }
        )
        XCTAssertEqual(prepareCount, 0)
    }

    fileprivate func makePairing(
        over channel: FrameChannel,
        host: NearbySpaceHost,
        peerName: String
    ) throws -> SpacePairing {
        let connection = NearbyConnection(bluetooth: channel, localAttempt: { nil })
        connection.confirmPairing()
        try connection.activate()
        return SpacePairing(connection: connection, host: host, friendlyName: peerName)
    }
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

// MARK: - Unit 2B: metadata opacity, bilateral confirmation, fail-closed

extension TransportContractTests {

    /// The confirmation token that crosses the wire BEFORE any metadata is a pure
    /// consent marker: it carries no community title, no namespace, no identity —
    /// nothing a listener could learn a community from — and it is not, and cannot
    /// be mistaken for, a space announce.
    func testConfirmTokenCarriesNoCommunityMetadataAndIsNotAnAnnounce() throws {
        let token = PairConfirmCodec.encode()
        XCTAssertTrue(PairConfirmCodec.isConfirmation(token))

        // A space announce and a confirm token are never confusable in either
        // direction: the strict announce decoder rejects the token, and the token
        // check rejects a real announce.
        XCTAssertThrowsError(try SpaceAnnounceCodec.decode(token))
        let announce = try SpaceAnnounceCodec.encode(
            RiotSpace(namespaceID: String(repeating: "3f", count: 32), title: "Riverside Tenants Union")
        )
        XCTAssertFalse(PairConfirmCodec.isConfirmation(announce))

        // The token is fixed-shape and content-free: it embeds none of a community's
        // bytes, whatever community this device is in — neither the title (UTF-8 on
        // the wire) nor the namespace (32 RAW bytes on the wire; `3f` × 32 hex is
        // `0x3f` × 32, which is what a leak would actually look like).
        XCTAssertNil(token.range(of: Data("Riverside Tenants Union".utf8)))
        XCTAssertNil(token.range(of: Data(repeating: 0x3f, count: SpaceAnnounceCodec.namespaceBytes)))
    }

    /// The rule, on its own: metadata may be disclosed only once BOTH sides have
    /// confirmed. One side's confirmation is never enough.
    func testMutualConfirmationGateWithholdsUntilBothConfirm() {
        XCTAssertFalse(MutualConfirmationGate.mayDiscloseMetadata(localConfirmed: false, remoteConfirmed: false))
        XCTAssertFalse(MutualConfirmationGate.mayDiscloseMetadata(localConfirmed: true, remoteConfirmed: false))
        XCTAssertFalse(MutualConfirmationGate.mayDiscloseMetadata(localConfirmed: false, remoteConfirmed: true))
        XCTAssertTrue(MutualConfirmationGate.mayDiscloseMetadata(localConfirmed: true, remoteConfirmed: true))
    }

    /// THE SECURITY GATE. Two devices in different communities pair. This asserts
    /// the load-bearing property: a device discloses NOTHING about its community —
    /// not the name, not the namespace, not the announce frame itself — until the
    /// OTHER device has also confirmed. Before that, the only thing on the wire is
    /// the content-free consent token. "UI visibility is never an authorization
    /// check": this reads the actual bytes each side emits, not the screen.
    func testSpacePairingWithholdsCommunityMetadataUntilBothDevicesConfirm() throws {
        let (aInner, bInner) = LoopbackFrameChannel.pair()
        let aTap = TappingChannel(inner: aInner)
        let bTap = TappingChannel(inner: bInner)

        let aSpace = RiotSpace(namespaceID: String(repeating: "aa", count: 32), title: "Riverside Tenants Union")
        let bSpace = RiotSpace(namespaceID: String(repeating: "bb", count: 32), title: "Eastside Mutual Aid")
        let aPairing = try makePairing(over: aTap, host: FakeNearbyHost(space: aSpace), peerName: "Copper Heron")
        let bPairing = try makePairing(over: bTap, host: FakeNearbyHost(space: bSpace), peerName: "Amber Kite")

        // Only device A's human has confirmed so far.
        aPairing.begin(onDecision: { _ in }, onFailure: {})

        // A has put a single frame on the wire and it is the opaque consent token.
        // No announce, and nothing carrying A's community name or namespace.
        XCTAssertEqual(aTap.sent.count, 1, "A disclosed more than a bare consent token before B confirmed")
        XCTAssertTrue(PairConfirmCodec.isConfirmation(aTap.sent[0]))
        // The namespace travels as 32 RAW bytes, not as its hex text
        // (`FrameCodec.hexBytes`), so the check must look for the raw bytes the wire
        // actually carries — `aa` × 32 hex is `0xaa` × 32. Asserting absence of the
        // hex STRING would be a phantom: that encoding never appears on the wire.
        let aNamespaceWireBytes = Data(repeating: 0xaa, count: SpaceAnnounceCodec.namespaceBytes)
        for frame in aTap.sent {
            XCTAssertThrowsError(try SpaceAnnounceCodec.decode(frame), "A leaked a decodable announce pre-confirmation")
            XCTAssertNil(frame.range(of: Data(aSpace.title.utf8)), "A leaked its community name pre-confirmation")
            XCTAssertNil(frame.range(of: aNamespaceWireBytes), "A leaked its RAW namespace bytes pre-confirmation")
        }
        // B has confirmed nothing, so B has disclosed nothing at all.
        XCTAssertTrue(bTap.sent.isEmpty, "B disclosed something before its own human confirmed")

        // Now B's human confirms. Only now may either announce cross.
        bPairing.begin(onDecision: { _ in }, onFailure: {})

        XCTAssertTrue(
            aTap.sent.contains { (try? SpaceAnnounceCodec.decode($0)) == aSpace },
            "after mutual confirmation A must disclose its announce"
        )
        XCTAssertTrue(
            bTap.sent.contains { (try? SpaceAnnounceCodec.decode($0)) == bSpace },
            "after mutual confirmation B must disclose its announce"
        )
    }

    /// A non-consent frame arriving in the confirmation phase is refused, never
    /// interpreted — the same fail-closed strictness the announce decoder has.
    func testSpacePairingFailsClosedOnANonConfirmationFrameBeforeDisclosure() throws {
        let (aInner, bInner) = LoopbackFrameChannel.pair()
        let aPairing = try makePairing(
            over: TappingChannel(inner: aInner),
            host: FakeNearbyHost(space: RiotSpace(namespaceID: String(repeating: "aa", count: 32), title: "A")),
            peerName: "Copper Heron"
        )
        var failed = false
        aPairing.begin(onDecision: { _ in XCTFail("must not decide before confirmation") }, onFailure: { failed = true })

        // A hostile peer sends a real announce where a consent token is required.
        let announce = try SpaceAnnounceCodec.encode(RiotSpace(namespaceID: String(repeating: "bb", count: 32), title: "B"))
        try bInner.send(announce)

        XCTAssertTrue(failed, "an announce in the confirmation phase must fail closed, not disclose or decide")
    }

    /// A sync session that has been stopped — the exact thing a community switch
    /// does to the old coordinator — must NEVER commit a pending import. Today's
    /// `addPreviewedContent` calls straight into `acceptImport`; this pins the
    /// guard that makes a stopped session inert.
    func testStoppedSyncSessionRefusesToCommitAPendingImport() throws {
        let channels = LoopbackFrameChannel.pair()
        let connection = NearbyConnection(bluetooth: channels.first, localAttempt: { nil })
        connection.confirmPairing()
        try connection.activate()
        let session = FakeSyncBoundary(outbound: [], beginOutcome: .readyToPreview(count: 1))
        let coordinator = SyncCoordinator(session: session, connection: connection, friendlyName: "Copper Heron")
        coordinator.start()
        XCTAssertEqual(coordinator.state, .preview(count: 1, name: "Copper Heron"))

        // The community switches away: the owner stops the coordinator.
        coordinator.stop()
        // A racing "Add them" tap lands after the stop.
        coordinator.addPreviewedContent()

        XCTAssertEqual(session.acceptCalls, 0, "a stopped session committed a pending import — the wrong-community race")
    }

    /// The import-admission rule, on its own: an import may commit only into the
    /// community whose session produced it. If the selected community has changed
    /// (or gone) since the session opened, the import is refused, not repointed.
    func testNearbyImportAdmissionFailsClosedWhenCommunityChanged() {
        let owned = String(repeating: "aa", count: 32)
        XCTAssertTrue(NearbyImportAdmission.permits(owned: owned, current: owned))
        XCTAssertTrue(
            NearbyImportAdmission.permits(owned: owned, current: owned.uppercased()),
            "the same namespace in a different case is the same community"
        )
        XCTAssertFalse(
            NearbyImportAdmission.permits(owned: owned, current: String(repeating: "bb", count: 32)),
            "an import must never commit into a community it did not come from"
        )
        XCTAssertFalse(NearbyImportAdmission.permits(owned: owned, current: nil), "no selected community: refuse")
        XCTAssertFalse(NearbyImportAdmission.permits(owned: nil, current: owned), "no owning session: refuse")
    }

    /// Denied Bluetooth / local-network permission offers a Settings deep link
    /// (§4.7), and the link points at this app's own settings page.
    func testDeniedNearbyPermissionOffersASettingsDeepLink() {
        // A real, openable settings deep link — the §4.7 "Open Settings" action.
        XCTAssertNotNil(NearbyPermissionRecovery.settingsURL)
        XCTAssertFalse(NearbyPermissionRecovery.message.isEmpty)
        // The copy explains what still works offline, never a raw permission error.
        XCTAssertFalse(NearbyPermissionRecovery.message.lowercased().contains("error"))
    }

    /// The ephemeral discovery handle must be per-construction random and NOT
    /// derived from any community or identity. A handle seeded from the namespace
    /// or profile would be a stable, passive fingerprint that links a device
    /// across sessions before anyone confirms anything — a linkability leak the
    /// opacity gate would not catch, because the handle is advertised openly by
    /// design. This guards the CALL SITE (`LocalNetworkNearbyService.init`'s random
    /// nonce), not merely the pure name generator: reseeding the nonce from a
    /// space or profile would reintroduce the fingerprint and fail here.
    ///
    /// `LocalNetworkNearbyService` is the constructor reachable in a unit test —
    /// its init only builds the identity (advertising/browsing start in
    /// `startLooking`), so it touches no radio. `CoreBluetoothNearbyService` draws
    /// its handle from the same random-nonce `FriendlyNameGenerator`, but merely
    /// constructing it spins up `CBCentralManager` and aborts the xctest host on a
    /// TCC violation, so its call site is asserted by structural equivalence, not
    /// exercised here.
    func testDiscoveryHandleIsPerConstructionRandomAndNotDerivedFromACommunity() {
        let names = (0..<8).map { _ in LocalNetworkNearbyService().friendlyName }

        // Distinct handles across constructions ⇒ the nonce is per-session random,
        // not a constant or a function of a fixed input. (16,384-name space over 8
        // draws: an all-identical run is ~10⁻³⁰, not a real flake.)
        XCTAssertGreaterThan(
            Set(names).count, 1,
            "the discovery handle is identical across constructions — derived from a constant/fixed input, not a per-session nonce"
        )

        // The service takes no community, and its handle must carry none of one:
        // not the title (UTF-8 on the wire) nor the 32 RAW namespace bytes.
        let space = RiotSpace(namespaceID: String(repeating: "cc", count: 32), title: "Riverside Tenants Union")
        let namespaceWireBytes = Data(repeating: 0xcc, count: SpaceAnnounceCodec.namespaceBytes)
        for name in names {
            let handle = Data(name.utf8)
            XCTAssertNil(handle.range(of: Data(space.title.utf8)), "the discovery handle carried a community title")
            XCTAssertNil(handle.range(of: namespaceWireBytes), "the discovery handle carried raw namespace bytes")
        }
    }
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
