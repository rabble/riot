import Foundation

/// What THIS phone does about its space when it meets another one.
public enum SpaceDecision: Equatable, Sendable {
    /// Open the sync session. Either both phones are in the same space, or this
    /// phone is in one and the other is not — in which case the other is being
    /// asked, right now, whether to join THIS one, and the session is what they
    /// join into.
    case proceed
    /// This phone is in no space and the peer is in one: joining theirs is what
    /// makes a sync possible at all. The person is asked first.
    case adopt(RiotSpace)
    /// Both are in a space, and they are not the same one. A phone can only be
    /// in one space, so this is refused — never silently switched.
    case differentSpace
    /// Neither phone is in a space. There is nothing to send either way.
    case nothingToShare
}

public enum SpaceAdoption {
    /// The whole rule, in one place, so it can be read and tested without a
    /// radio. Namespaces are compared case-insensitively because the id is hex
    /// text on this side of the FFI; the wire carries the raw bytes.
    public static func decide(local: RiotSpace?, remote: RiotSpace?) -> SpaceDecision {
        switch (local, remote) {
        case (nil, nil):
            // Two empty phones. Neither can even open a sync session.
            return .nothingToShare
        case let (nil, .some(theirs)):
            return .adopt(theirs)
        case (.some, nil):
            // The mirror image of `.adopt`, seen from the phone that HAS the
            // space: the other one is deciding whether to join it. So open the
            // session and let them arrive. This must not end the connection —
            // ending it here is exactly what would make the fresh phone's join
            // land on a peer that has already hung up.
            return .proceed
        case let (.some(mine), .some(theirs)):
            let same = mine.namespaceID.lowercased() == theirs.namespaceID.lowercased()
            return same ? .proceed : .differentSpace
        }
    }
}

/// Whether a device may disclose its community metadata yet.
///
/// The whole rule in one place, so the security boundary can be read and tested
/// without a radio: a device announces its community — name, namespace, the
/// announce frame itself — ONLY once both it and its peer have confirmed. One
/// side's confirmation is never enough. Before that, the only thing on the wire
/// is the content-free `PairConfirmCodec` token.
public enum MutualConfirmationGate {
    public static func mayDiscloseMetadata(localConfirmed: Bool, remoteConfirmed: Bool) -> Bool {
        localConfirmed && remoteConfirmed
    }
}

/// The profile a pairing acts on. `RiotProfileRepository` is the real one; the
/// protocol exists so the transport does not reach into storage, and so a test
/// can pair two real repositories over a real socket.
public protocol NearbySpaceHost: AnyObject {
    var currentSpace: RiotSpace? { get }
    /// Joins `space` and RE-SEALS the identity — see `RiotProfileRepository`.
    func joinSpace(_ space: RiotSpace) throws
    func openSyncBoundary() throws -> MobileSyncSessionBoundary
}

/// The step before the sync: each phone says which space it is in, and only then
/// is a sync session opened.
///
/// This has to come first. `open_sync_session` refuses a profile with no space,
/// so a fresh phone cannot open a session to find out what it is missing — the
/// space has to arrive before the session is asked for, and the only thing that
/// knows it is the phone standing next to it.
///
/// It owns the connection's receive path from the moment it sends its announce
/// until it hands over. That is not incidental: the peer sends its `Hello` the
/// instant its own handshake finishes, which can be while the person here is
/// still reading "Join Riverside Tenants Union?". Those frames are buffered and
/// replayed IN ORDER into the coordinator once it exists. If the coordinator
/// grabbed `onReceive` for itself, a frame arriving between that swap and the
/// replay would jump the queue and the session would fail on an out-of-phase
/// frame.
public final class SpacePairing {
    private let connection: NearbyConnection
    private let host: NearbySpaceHost
    private let friendlyName: String
    /// Frames that arrive after the peer's announce, held until a coordinator is
    /// ready for them. Same type the channels use, so ordering and the
    /// attach-while-delivering race are already solved.
    private let inbox = BoundedFrameInbox()
    private let lock = NSLock()
    /// This device's human has consented to pair (by initiating or accepting the
    /// connection). `begin` is reached only on that human action — discovery never
    /// auto-connects or auto-accepts — so reaching `begin` IS the local consent.
    private var localConfirmed = false
    /// A `PairConfirmCodec` token has arrived from the peer: their human consented.
    private var remoteConfirmed = false
    /// This device has put its space announce on the wire. Guards against a second
    /// disclosure and marks the transition out of the confirmation phase.
    private var disclosed = false
    private var announced = false
    private var finished = false
    private var onDecision: ((SpaceDecision) -> Void)?
    private var onFailure: (() -> Void)?
    /// The peer's announced space, captured during the handshake so `adopt`
    /// can compute a deterministic sync role without depending on discovery
    /// timing (`isInboundRequest`).
    public private(set) var remoteSpace: RiotSpace?

    public init(connection: NearbyConnection, host: NearbySpaceHost, friendlyName: String) {
        self.connection = connection
        self.host = host
        self.friendlyName = friendlyName
    }

    /// Records this device's consent, sends the opaque confirmation token, and
    /// waits for the peer's. The space announce is withheld until BOTH sides have
    /// confirmed — reaching this method is the local human's consent (discovery
    /// never auto-connects or auto-accepts), and the announce goes out only once
    /// the peer's token has arrived. `onDecision` fires exactly once, after mutual
    /// confirmation and the announce exchange; `onFailure` fires if the connection
    /// breaks, a non-consent frame arrives before disclosure, or the peer's
    /// announce is malformed.
    public func begin(
        onDecision: @escaping (SpaceDecision) -> Void,
        onFailure: @escaping () -> Void
    ) {
        lock.lock()
        self.onDecision = onDecision
        self.onFailure = onFailure
        localConfirmed = true
        lock.unlock()
        connection.onFailure = { [weak self] in self?.fail() }
        do {
            // Consent first; the community stays opaque until the peer consents too.
            try connection.send(PairConfirmCodec.encode())
        } catch {
            fail()
            return
        }
        // Wire receive AFTER our token is out, so a peer token already buffered on
        // the channel drains into a fully-armed receiver and can trigger disclosure.
        connection.onReceive = { [weak self] frame in self?.receive(frame) }
    }

    /// Opens the sync session, joining `space` first when the person has agreed
    /// to adopt the peer's. Joining BEFORE the boundary is opened is required in
    /// both directions: `join_public_space` refuses while a sync session is
    /// active, and `open_sync_session` refuses without a space.
    public func resume(joining space: RiotSpace?) throws -> SyncCoordinator {
        if let space { try host.joinSpace(space) }
        return SyncCoordinator(
            session: try host.openSyncBoundary(),
            connection: connection,
            friendlyName: friendlyName,
            framesFromConnection: false
        )
    }

    /// Hands the wire to the coordinator, replaying whatever arrived while the
    /// handshake was still deciding. Call AFTER `start()`/`answer()`: a `Hello`
    /// replayed into a coordinator that has not answered yet is a frame in the
    /// wrong phase, which is the exact failure this class exists to avoid.
    public func handOff(to coordinator: SyncCoordinator) {
        inbox.onReceive = { [weak coordinator] frame in coordinator?.deliver(frame) }
    }

    /// Stops routing. The connection is the caller's to tear down.
    public func cancel() {
        lock.lock()
        finished = true
        onDecision = nil
        onFailure = nil
        lock.unlock()
        inbox.onReceive = nil
    }

    private func receive(_ frame: Data) {
        lock.lock()
        if finished {
            lock.unlock()
            return
        }
        // Confirmation phase: the only frame allowed before disclosure is the
        // peer's consent token. Anything else — including a real announce from a
        // peer trying to skip the gate — fails closed rather than being interpreted.
        guard remoteConfirmed else {
            guard PairConfirmCodec.isConfirmation(frame) else {
                lock.unlock()
                fail()
                return
            }
            remoteConfirmed = true
            let discloseNow = MutualConfirmationGate.mayDiscloseMetadata(
                localConfirmed: localConfirmed,
                remoteConfirmed: remoteConfirmed
            ) && !disclosed
            if discloseNow { disclosed = true }
            lock.unlock()
            if discloseNow {
                do {
                    try connection.send(SpaceAnnounceCodec.encode(host.currentSpace))
                } catch {
                    fail()
                }
            }
            return
        }
        guard !announced else {
            lock.unlock()
            // Anything after the announce belongs to the sync session: hold it
            // (or pass it straight through once the coordinator is attached).
            inbox.receive(frame)
            return
        }
        announced = true
        let decision = onDecision
        lock.unlock()

        let remote: RiotSpace?
        do {
            remote = try SpaceAnnounceCodec.decode(frame)
        } catch {
            fail()
            return
        }
        remoteSpace = remote
        guard let decision else { return }
        decision(SpaceAdoption.decide(local: host.currentSpace, remote: remote))
    }

    private func fail() {
        lock.lock()
        guard !finished else { lock.unlock(); return }
        finished = true
        let failure = onFailure
        onDecision = nil
        onFailure = nil
        lock.unlock()
        failure?()
    }
}
