import Network
import XCTest
@testable import RiotKit

/// The claim this file exists to test: a phone with NOTHING on it — no space, no
/// board, a fresh install — can stand next to a phone that is in a space, join
/// that space, and end up holding everything it holds.
///
/// That is the finale of the demo script, and before this it could not happen at
/// all: `open_sync_session` refuses a profile with no space, so the fresh phone
/// could not even open a session to find out what it was missing, and nothing in
/// the app ever made it join anyone.
///
/// Nothing here stands in for the thing being proven. Both peers are real
/// `RiotProfileRepository` profiles over the real FFI, each with its own storage
/// and its own identity. They are wired to each other over a real TCP socket on
/// loopback carrying the app's real length-prefixed frames, and every step —
/// announcing a space, deciding what to do about it, joining, and syncing — runs
/// through the shipping `SpacePairing` and `SyncCoordinator`, in the order and on
/// the threads `NearbyTransportController` runs them in.
///
/// The one substitution, inherited from `AppSyncReplicationTests`: the sockets
/// are built here rather than through `LocalNetworkListener` /
/// `LocalTCPFrameChannel.attempt`, because those pin `requiredInterfaceType =
/// .wifi` and loopback is not Wi-Fi. Everything from the frame codec up is the
/// shipping code.
@MainActor
final class SpaceAdoptionTests: XCTestCase {

    // MARK: - The claim

    /// The demo's last beat. Alice is in a space, has approved the checklist, and
    /// has an item on it. Bob is a fresh install: no space, nothing on his board.
    /// He walks over, they pair, and he ends up in her space with her board, her
    /// approved app, and her item — having approved nothing himself.
    func testFreshPhoneWithNoSpaceJoinsItsPeersSpaceAndReceivesEverything() async throws {
        let alice = try openPeer(name: "Alice", inSpace: "Riverside Tenants Union")
        let bob = try openPeer(name: "Bob", inSpace: nil)
        let space = try XCTUnwrap(alice.repository.currentSpace)

        try alice.repository.trustApp(appID: alice.appID)
        _ = try alice.repository.signAlert(
            in: space,
            draft: AlertDraft(
                expiresAt: UInt64(Date().timeIntervalSince1970) + 3_600,
                headline: "Ladder truck on Mission at 3pm",
                description: "Bring water.",
                sourceClaims: ["Seen from the corner"],
                aiAssisted: false
            )
        )
        let key = "items/\(UUID().uuidString.lowercased())"
        let item = #"{"done":false,"text":"Bring water to the corner"}"#
        try XCTUnwrap(alice.repository.appDataBridge(appID: alice.appID)).put(key: key, valueJSON: item)

        XCTAssertNil(bob.repository.currentSpace, "Bob must start with no space, or this proves nothing")
        XCTAssertEqual(try bob.repository.currentEntries().count, 0, "Bob's board must start empty")

        // Bob taps Alice's name, so Bob is the one who opens the protocol — the
        // demo's own order, and the harder one: he cannot open a sync session
        // until he has joined, so the join has to come first or nothing happens.
        try await pair(initiator: bob, responder: alice)

        XCTAssertEqual(
            bob.repository.currentSpace, space,
            "Bob did not join Alice's space — the whole point of standing next to her"
        )
        XCTAssertEqual(
            try bob.repository.currentEntries().map(\.headline),
            ["Ladder truck on Mission at 3pm"],
            "Bob joined the space but Alice's board did not arrive"
        )

        let app = try XCTUnwrap(try bob.repository.spaceApps().first)
        XCTAssertTrue(
            app.trusted,
            "Bob must inherit the organizer's approval, not be asked to approve the app himself"
        )
        XCTAssertEqual(
            try XCTUnwrap(bob.repository.appDataBridge(appID: bob.appID)).get(key: key), item,
            "Bob cannot read the checklist item Alice added"
        )
    }

    /// The same join, with the phones in the other order: ALICE taps Bob, so she
    /// opens the protocol and her `Hello` is on the wire while Bob is still being
    /// asked whether he wants to join her space.
    ///
    /// This is the ordering the space handshake exists to survive. That `Hello`
    /// arrives before Bob has a sync session to give it to; it is held, and
    /// replayed into the session the moment he has one. Take the buffer out of
    /// `SpacePairing` — let the coordinator take the wire for itself — and the
    /// frame is either dropped or arrives out of phase, and this test fails while
    /// the one above still passes.
    func testPeerSHelloArrivesWhileTheFreshPhoneIsStillDecidingAndIsNotLost() async throws {
        let alice = try openPeer(name: "Alice", inSpace: "Riverside Tenants Union")
        let bob = try openPeer(name: "Bob", inSpace: nil)
        let space = try XCTUnwrap(alice.repository.currentSpace)
        _ = try alice.repository.signAlert(
            in: space,
            draft: AlertDraft(
                expiresAt: UInt64(Date().timeIntervalSince1970) + 3_600,
                headline: "Water at the north gate",
                description: "All afternoon.",
                sourceClaims: ["Organizer"],
                aiAssisted: false
            )
        )

        // Alice initiates; Bob sits on the join prompt for a beat, which is what
        // puts her Hello on the wire ahead of his session.
        try await pair(initiator: alice, responder: bob, joinPromptDelay: 0.25)

        XCTAssertEqual(bob.repository.currentSpace, space, "Bob did not join Alice's space")
        XCTAssertEqual(
            try bob.repository.currentEntries().map(\.headline),
            ["Water at the north gate"],
            "the frame that arrived while Bob was deciding was lost"
        )
    }

    /// Two phones in DIFFERENT spaces refuse each other. A phone is in one space;
    /// it is not silently switched, and nothing crosses.
    func testPhonesInDifferentSpacesRefuseAndNeitherSpaceChanges() async throws {
        let alice = try openPeer(name: "Alice", inSpace: "Riverside Tenants Union")
        let bob = try openPeer(name: "Bob", inSpace: "Eastside Mutual Aid")
        let aliceSpace = try XCTUnwrap(alice.repository.currentSpace)
        let bobSpace = try XCTUnwrap(bob.repository.currentSpace)
        _ = try alice.repository.signAlert(
            in: aliceSpace,
            draft: AlertDraft(
                expiresAt: UInt64(Date().timeIntervalSince1970) + 3_600,
                headline: "Not for Bob",
                description: "Different space.",
                sourceClaims: ["Alice"],
                aiAssisted: false
            )
        )

        let (initiator, responder) = try await pair(initiator: bob, responder: alice)

        XCTAssertEqual(initiator.decision, .differentSpace)
        XCTAssertEqual(responder.decision, .differentSpace)
        XCTAssertEqual(
            bob.repository.currentSpace, bobSpace,
            "Bob's space must be untouched — a peer must never switch the space you are in"
        )
        XCTAssertEqual(alice.repository.currentSpace, aliceSpace, "Alice's space must be untouched")
        XCTAssertEqual(
            try bob.repository.currentEntries().count, 0,
            "nothing may cross between two different spaces"
        )
        XCTAssertTrue(
            initiator.states.contains(.differentSpace(name: "Alice")),
            "Bob is told plainly that Alice is in a different space"
        )
    }

    /// Two fresh phones have nothing to say to each other, and say so.
    func testTwoPhonesWithNoSpaceHaveNothingToShare() async throws {
        let alice = try openPeer(name: "Alice", inSpace: nil)
        let bob = try openPeer(name: "Bob", inSpace: nil)

        let (initiator, responder) = try await pair(initiator: bob, responder: alice)

        XCTAssertEqual(initiator.decision, .nothingToShare)
        XCTAssertEqual(responder.decision, .nothingToShare)
        XCTAssertNil(bob.repository.currentSpace)
        XCTAssertNil(alice.repository.currentSpace)
        XCTAssertTrue(initiator.states.contains(.nothingToShare))
    }

    /// Saying no means no: the person is offered the space, declines, and is not
    /// in it.
    func testDecliningTheJoinLeavesThePhoneWithNoSpace() async throws {
        let alice = try openPeer(name: "Alice", inSpace: "Riverside Tenants Union")
        let bob = try openPeer(name: "Bob", inSpace: nil)

        _ = try await pair(initiator: bob, responder: alice, acceptJoin: false)

        XCTAssertNil(
            bob.repository.currentSpace,
            "Bob said no and must not be in Alice's space"
        )
        XCTAssertEqual(try bob.repository.currentEntries().count, 0)
    }

    // MARK: - The identity the join mints

    /// A JOINER'S SIGNING IDENTITY MUST SURVIVE A RESTART.
    ///
    /// `join_public_space` regenerates the author for the namespace it joins
    /// (`generate_communal_author_for_namespace` — a fresh random subspace), and
    /// iOS seals the identity at FIRST open, before any space exists. So a join
    /// that does not RE-SEAL leaves the old, pre-join identity on disk: the next
    /// launch restores it, re-joins, and mints ANOTHER random subspace. The
    /// person's signer would churn on every launch, and everything they wrote
    /// last time would be orphaned from everything they write next.
    ///
    /// `signerID` on an entry IS the author's subspace id (Rust sets
    /// `signing_key_id` to the subspace), so pinning it across a reopen pins the
    /// subspace. Remove the re-seal from `RiotProfileRepository.joinSpace` and
    /// this test fails.
    func testJoinersSubspaceIdIsIdenticalAfterReopening() async throws {
        let alice = try openPeer(name: "Alice", inSpace: "Riverside Tenants Union")
        let bob = try openPeer(name: "Bob", inSpace: nil)

        try await pair(initiator: bob, responder: alice)
        XCTAssertNotNil(bob.repository.currentSpace)

        let signedBeforeReopen = try sign(as: bob, headline: "Before the restart")

        // The same profile, opened again from its own storage and wrapping key —
        // what happens every time the person launches the app.
        let reopened = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: bob.storageURL),
            keyStore: bob.keyStore,
            starterPacks: try starterPacks()
        )
        XCTAssertEqual(
            reopened.currentSpace, alice.repository.currentSpace,
            "the joined space must still be there after a restart"
        )
        let space = try XCTUnwrap(reopened.currentSpace)
        _ = try reopened.signAlert(
            in: space,
            draft: draft(headline: "After the restart")
        )

        let signers = Set(try reopened.currentEntries().map(\.signerID))
        XCTAssertEqual(
            signers.count, 1,
            """
            Bob's signing identity changed across the restart: he wrote \
            \(signers.sorted()). The join regenerated his author and it was never \
            re-sealed, so the next open restored the pre-join identity and minted \
            a different subspace — everything he wrote before the restart is now \
            orphaned from everything he writes after it.
            """
        )
        XCTAssertTrue(
            signers.contains(signedBeforeReopen),
            "the entry Bob signed before the restart must still be his"
        )
    }

    // MARK: - The rule, on its own

    func testAdoptionRules() {
        let mine = RiotSpace(namespaceID: String(repeating: "a", count: 64), title: "Mine")
        let theirs = RiotSpace(namespaceID: String(repeating: "b", count: 64), title: "Theirs")
        let sameIDDifferentCase = RiotSpace(
            namespaceID: String(repeating: "A", count: 64),
            title: "Mine, as they spell it"
        )

        XCTAssertEqual(SpaceAdoption.decide(local: nil, remote: nil), .nothingToShare)
        XCTAssertEqual(SpaceAdoption.decide(local: nil, remote: theirs), .adopt(theirs))
        XCTAssertEqual(SpaceAdoption.decide(local: mine, remote: mine), .proceed)
        XCTAssertEqual(SpaceAdoption.decide(local: mine, remote: theirs), .differentSpace)
        XCTAssertEqual(
            SpaceAdoption.decide(local: mine, remote: sameIDDifferentCase), .proceed,
            "the same namespace spelled in a different case is the same space"
        )
        XCTAssertEqual(
            SpaceAdoption.decide(local: mine, remote: nil), .proceed,
            """
            the phone that HAS the space must open the session and wait: the other \
            one is deciding whether to join it, and hanging up here is exactly what \
            would make their join land on a peer that has gone.
            """
        )
    }

    /// A phone already in a space cannot be talked into a different one by a
    /// repository-level call either — the refusal is in the store, not only in
    /// the transport.
    func testJoiningADifferentSpaceIsRefusedByTheRepository() throws {
        let alice = try openPeer(name: "Alice", inSpace: "Riverside Tenants Union")
        let mine = try XCTUnwrap(alice.repository.currentSpace)
        let theirs = RiotSpace(namespaceID: String(repeating: "b", count: 64), title: "Elsewhere")

        XCTAssertThrowsError(try alice.repository.joinSpace(theirs))
        XCTAssertEqual(alice.repository.currentSpace, mine)
        // Joining the space you are already in is a no-op, not a failure.
        XCTAssertNoThrow(try alice.repository.joinSpace(mine))
        XCTAssertEqual(alice.repository.currentSpace, mine)
    }

    // MARK: - The announce on the wire

    func testAnnounceRoundTrips() throws {
        let space = RiotSpace(namespaceID: String(repeating: "3f", count: 32), title: "Riverside Tenants Union")
        XCTAssertEqual(try SpaceAnnounceCodec.decode(try SpaceAnnounceCodec.encode(space)), space)
        XCTAssertNil(try SpaceAnnounceCodec.decode(try SpaceAnnounceCodec.encode(nil)))
    }

    /// The namespace in this frame can become the space this person joins, so
    /// anything that is not exactly one well-formed announce is refused rather
    /// than interpreted.
    func testMalformedAnnouncesAreRefused() throws {
        let space = RiotSpace(namespaceID: String(repeating: "3f", count: 32), title: "Riverside")
        let good = try SpaceAnnounceCodec.encode(space)

        XCTAssertThrowsError(try SpaceAnnounceCodec.decode(Data()), "empty")
        XCTAssertThrowsError(try SpaceAnnounceCodec.decode(good.dropLast()), "truncated title")
        XCTAssertThrowsError(try SpaceAnnounceCodec.decode(good + Data([0])), "trailing byte")
        XCTAssertThrowsError(
            try SpaceAnnounceCodec.decode(Data("XXXXXXXX".utf8) + good.dropFirst(8)),
            "wrong magic"
        )
        var unknownFlag = Data(good)
        unknownFlag[8] = 7
        XCTAssertThrowsError(try SpaceAnnounceCodec.decode(unknownFlag), "unknown flag")

        var emptyTitle = Data(good.prefix(8 + 1 + 32))
        emptyTitle.append(contentsOf: [0, 0])
        XCTAssertThrowsError(try SpaceAnnounceCodec.decode(emptyTitle), "empty title")

        XCTAssertThrowsError(
            try SpaceAnnounceCodec.encode(RiotSpace(namespaceID: "not-hex", title: "x")),
            "a namespace that is not 32 bytes of hex never reaches the wire"
        )
    }

    // MARK: - Peers

    private struct Peer {
        let name: String
        let repository: RiotProfileRepository
        let appID: String
        let storageURL: URL
        let keyStore: WrappingKeyStore
    }

    /// One phone. `inSpace: nil` is a fresh install — the state phone B is in at
    /// the start of the demo's last beat.
    private func openPeer(name: String, inSpace title: String?) throws -> Peer {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("space-adoption-\(name)-\(UUID().uuidString).json")
        let keyStore = TestWrappingKeyStore()
        let repository = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url),
            keyStore: keyStore,
            starterPacks: try starterPacks()
        )
        if let title { _ = try repository.createPublicSpace(title: title) }
        let appID = try XCTUnwrap(repository.installedApps().first).appIDHex
        return Peer(name: name, repository: repository, appID: appID, storageURL: url, keyStore: keyStore)
    }

    private func draft(headline: String) -> AlertDraft {
        AlertDraft(
            expiresAt: UInt64(Date().timeIntervalSince1970) + 3_600,
            headline: headline,
            description: "…",
            sourceClaims: ["Local conference participant"],
            aiAssisted: false
        )
    }

    @discardableResult
    private func sign(as peer: Peer, headline: String) throws -> String {
        let space = try XCTUnwrap(peer.repository.currentSpace)
        return try peer.repository.signAlert(in: space, draft: draft(headline: headline)).signerID
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

    // MARK: - Pairing two real phones

    /// Puts two real profiles through the real pairing, over a real socket.
    ///
    /// `initiator` is the person who tapped the other phone's name — the one
    /// `NearbyTransportController` starts the protocol on. `joinPromptDelay` is
    /// the person reading the join question before tapping; `acceptJoin: false` is
    /// them saying no.
    @discardableResult
    private func pair(
        initiator: Peer,
        responder: Peer,
        acceptJoin: Bool = true,
        joinPromptDelay: TimeInterval = 0,
        timeout: TimeInterval = 30
    ) async throws -> (initiator: PairDriver, responder: PairDriver) {
        let wire = DispatchQueue(label: "net.protest.riot.tests.adopt-wire")
        let (dialled, accepted) = try await connectedChannels(on: wire)

        let initiatorSide = try PairDriver(
            host: initiator.repository,
            channel: dialled,
            peerName: responder.name,
            wire: wire,
            isInitiator: true,
            acceptJoin: acceptJoin,
            joinPromptDelay: joinPromptDelay
        )
        let responderSide = try PairDriver(
            host: responder.repository,
            channel: accepted,
            peerName: initiator.name,
            wire: wire,
            isInitiator: false,
            acceptJoin: acceptJoin,
            joinPromptDelay: joinPromptDelay
        )
        defer {
            initiatorSide.stop()
            responderSide.stop()
        }

        wire.async {
            // Both phones announce; neither has opened a sync session yet,
            // because the fresh one cannot until it has joined.
            responderSide.begin()
            initiatorSide.begin()
        }
        await fulfillment(of: [initiatorSide.done, responderSide.done], timeout: timeout)
        return (initiatorSide, responderSide)
    }

    /// A connected pair of the app's real `LocalTCPFrameChannel`s over a real TCP
    /// socket on loopback, both delivered on `wire` so each side is only ever
    /// entered from one queue. (Same construction as `AppSyncReplicationTests`,
    /// and for the same reason: `LocalTCPFrameChannel.attempt` pins Wi-Fi.)
    private func connectedChannels(
        on wire: DispatchQueue
    ) async throws -> (dialled: LocalTCPFrameChannel, accepted: LocalTCPFrameChannel) {
        let listener = try NWListener(using: .tcp, on: .any)
        let acceptedChannel = AdoptOneShot<LocalTCPFrameChannel>()
        let listening = AdoptOneShot<AdoptListenerStart>()
        listener.newConnectionHandler = { connection in
            connection.start(queue: wire)
            acceptedChannel.resume(with: LocalTCPFrameChannel(connection: connection))
        }
        listener.stateUpdateHandler = { state in
            switch state {
            case .ready: listener.port.map { listening.resume(with: .ready($0)) }
            case .failed: listening.resume(with: .failed)
            default: break
            }
        }
        listener.start(queue: wire)
        guard case let .ready(port) = await listening.value() else {
            throw NearbyTransportError.notConnected
        }

        let connection = NWConnection(host: "127.0.0.1", port: port, using: .tcp)
        let dialledChannel = AdoptOneShot<LocalTCPFrameChannel>()
        connection.stateUpdateHandler = { state in
            if case .ready = state {
                dialledChannel.resume(with: LocalTCPFrameChannel(connection: connection))
            }
        }
        connection.start(queue: wire)

        let dialled = await dialledChannel.value()
        let accepted = await acceptedChannel.value()
        listener.cancel()
        return (dialled, accepted)
    }
}

/// One phone's whole side of a pairing: the shipping `SpacePairing`, the decision
/// it reached, and — if there is anything to sync — the shipping `SyncCoordinator`
/// that follows it.
///
/// The sequence is `NearbyTransportController`'s, including the part that matters
/// most: the decision is handled OFF the thread the peer's frame arrived on (the
/// controller hops to the main actor to ask the person), so frames the peer sends
/// meanwhile land while no session exists yet. That is the race the pairing's
/// buffer is for, and reproducing it here is the point.
private final class PairDriver: @unchecked Sendable {
    let done = XCTestExpectation(description: "pairing settled")

    private(set) var decision: SpaceDecision?
    var states: [NearbyConnectionState] {
        lock.lock(); defer { lock.unlock() }
        return observed
    }

    private let pairing: SpacePairing
    private let connection: NearbyConnection
    private let peerName: String
    private let wire: DispatchQueue
    private let isInitiator: Bool
    private let acceptJoin: Bool
    private let joinPromptDelay: TimeInterval
    private var coordinator: SyncCoordinator?
    private let lock = NSLock()
    private var observed: [NearbyConnectionState] = []

    init(
        host: NearbySpaceHost,
        channel: FrameChannel,
        peerName: String,
        wire: DispatchQueue,
        isInitiator: Bool,
        acceptJoin: Bool,
        joinPromptDelay: TimeInterval
    ) throws {
        // `bluetooth:` is the base-channel slot, not a claim about the radio: with
        // no local upgrade to attempt, `activate()` runs on the channel passed
        // here — the real loopback socket.
        let connection = NearbyConnection(bluetooth: channel, localAttempt: { nil })
        connection.confirmPairing()
        try connection.activate()
        self.connection = connection
        self.pairing = SpacePairing(connection: connection, host: host, friendlyName: peerName)
        self.peerName = peerName
        self.wire = wire
        self.isInitiator = isInitiator
        self.acceptJoin = acceptJoin
        self.joinPromptDelay = joinPromptDelay
        done.assertForOverFulfill = false
    }

    func begin() {
        pairing.begin(
            onDecision: { [self] decision in
                let delay: TimeInterval
                if case .adopt = decision { delay = joinPromptDelay } else { delay = 0 }
                // The hop the controller makes to ask the person. Whatever the peer
                // sends in this window has nowhere to go yet.
                wire.asyncAfter(deadline: .now() + delay) { self.settle(decision) }
            },
            onFailure: { [self] in
                record(.failed)
                done.fulfill()
            }
        )
    }

    func stop() {
        coordinator?.stop()
        pairing.cancel()
        connection.disconnect()
    }

    private func settle(_ decision: SpaceDecision) {
        self.decision = decision
        switch decision {
        case .proceed:
            startSync(joining: nil)
        case let .adopt(space):
            guard acceptJoin else {
                // "Not now": nothing is joined, and the connection ends — which is
                // what `declineJoinSpace` does.
                record(.looking)
                connection.disconnect()
                done.fulfill()
                return
            }
            startSync(joining: space)
        case .differentSpace:
            record(.differentSpace(name: peerName))
            connection.disconnect()
            done.fulfill()
        case .nothingToShare:
            record(.nothingToShare)
            connection.disconnect()
            done.fulfill()
        }
    }

    private func startSync(joining space: RiotSpace?) {
        do {
            let coordinator = try pairing.resume(joining: space)
            self.coordinator = coordinator
            coordinator.onStateChanged = { [self] state in
                record(state)
                switch state {
                case .preview:
                    // The person tapping "Add them". Queued rather than run inside
                    // the state callback so it lands after the frame that produced
                    // the preview is fully handled.
                    wire.async { coordinator.addPreviewedContent() }
                case .caughtUp, .alreadyCurrent, .failed:
                    done.fulfill()
                default:
                    break
                }
            }
            if isInitiator { coordinator.start() } else { coordinator.answer() }
            pairing.handOff(to: coordinator)
        } catch {
            record(.failed)
            done.fulfill()
        }
    }

    private func record(_ state: NearbyConnectionState) {
        lock.lock()
        observed.append(state)
        lock.unlock()
    }
}

private enum AdoptListenerStart: Sendable {
    case ready(NWEndpoint.Port)
    case failed
}

/// A value produced once by a Network.framework callback and awaited elsewhere.
private final class AdoptOneShot<Value: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private var stored: Value?
    private var waiter: ((Value) -> Void)?

    func resume(with value: Value) {
        lock.lock()
        guard stored == nil else { return lock.unlock() }
        stored = value
        let waiter = self.waiter
        self.waiter = nil
        lock.unlock()
        waiter?(value)
    }

    func value() async -> Value {
        await withCheckedContinuation { continuation in
            lock.lock()
            if let stored {
                lock.unlock()
                return continuation.resume(returning: stored)
            }
            waiter = { continuation.resume(returning: $0) }
            lock.unlock()
        }
    }
}

/// Duplicated per the project convention (each suite keeps its own): a fixed
/// 32-byte key so sealed identities round-trip.
private final class TestWrappingKeyStore: WrappingKeyStore {
    private var key: Data?

    func loadOrCreateWrappingKey() throws -> Data {
        if let key { return key }
        let created = Data(repeating: 0x5a, count: 32)
        key = created
        return created
    }
}
