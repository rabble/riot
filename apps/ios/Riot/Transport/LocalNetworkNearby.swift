import Foundation
@preconcurrency import Network

/// Lets exactly one of several racing completion paths win. `NSLock`-backed so it
/// is safe to share across the `@Sendable` closures Network.framework calls.
/// Named distinctly to avoid colliding with any similar helper elsewhere.
private final class DialLatch: @unchecked Sendable {
    private let lock = NSLock()
    private var fired = false
    func claim() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if fired { return false }
        fired = true
        return true
    }
}

/// Who a peer is, as carried in the Bonjour TXT record.
///
/// `instanceID` — not the friendly name — is identity. Bonjour returns services
/// advertised by *other processes on the same host*, including our own, so
/// without a stable per-process id an instance discovers itself and offers to
/// sync with itself. Two instances of the same build can also draw the same
/// friendly name by chance, so the name cannot be the discriminator either.
public struct NearbyPeerIdentity: Equatable, Sendable {
    public let instanceID: String
    public let friendlyName: String
    public let tieBreaker: String

    public init(instanceID: String, friendlyName: String, tieBreaker: String) {
        self.instanceID = instanceID
        self.friendlyName = friendlyName
        self.tieBreaker = tieBreaker
    }

    public init?(txt: [String: String]) {
        guard let instanceID = txt[Key.instance],
              let friendlyName = txt[Key.name],
              let tieBreaker = txt[Key.tie]
        else { return nil }
        self.init(instanceID: instanceID, friendlyName: friendlyName, tieBreaker: tieBreaker)
    }

    public var txtRecord: [String: String] {
        [Key.instance: instanceID, Key.name: friendlyName, Key.tie: tieBreaker]
    }

    public func isSelf(_ other: NearbyPeerIdentity) -> Bool {
        instanceID == other.instanceID
    }

    private enum Key {
        static let instance = "instance"
        static let name = "name"
        static let tie = "tie"
    }
}

/// The handshake that runs on a freshly opened peer connection, before the
/// channel is handed to the sync session. Nothing is accepted automatically:
/// a request only becomes a session when the person on the other side confirms.
public enum LocalPairingMessage: Equatable, Sendable {
    case request(NearbyPeerIdentity)
    case accept(NearbyPeerIdentity)
    case decline

    public func encoded() -> Data {
        let payload: [String: String]
        switch self {
        case let .request(identity):
            payload = identity.txtRecord.merging([Kind.key: Kind.request]) { current, _ in current }
        case let .accept(identity):
            payload = identity.txtRecord.merging([Kind.key: Kind.accept]) { current, _ in current }
        case .decline:
            payload = [Kind.key: Kind.decline]
        }
        return (try? JSONEncoder().encode(payload)) ?? Data()
    }

    public init?(frame: Data) {
        guard !frame.isEmpty,
              let payload = try? JSONDecoder().decode([String: String].self, from: frame),
              let kind = payload[Kind.key]
        else { return nil }
        switch kind {
        case Kind.request:
            guard let identity = NearbyPeerIdentity(txt: payload) else { return nil }
            self = .request(identity)
        case Kind.accept:
            guard let identity = NearbyPeerIdentity(txt: payload) else { return nil }
            self = .accept(identity)
        case Kind.decline:
            self = .decline
        default:
            return nil
        }
    }

    private enum Kind {
        static let key = "kind"
        static let request = "request"
        static let accept = "accept"
        static let decline = "decline"
    }
}

/// Discovery and pairing over the local link, for peers a radio cannot reach.
///
/// WHY THIS EXISTS: Bluetooth cannot find a peer on the same machine — one BLE
/// controller never hears its own advertisement — so two instances on one Mac
/// can never see each other over the radio. Bonjour can: mDNS resolves services
/// advertised by other processes on the same host. That makes multi-instance
/// testing possible, and makes desktop-to-desktop sync work at all.
///
/// WHAT IT DOES NOT CHANGE: the channel stays link-local (see
/// `LocalEndpoint.isLocalAddress`, which still refuses any routable address),
/// and pairing still requires an explicit confirmation from the person on each
/// side. Discovery is not consent.
public final class LocalNetworkNearbyService: @unchecked Sendable {
    /// Bonjour type. Kept distinct from any Bluetooth service id so the two
    /// discovery paths cannot be confused for one another.
    public static let serviceType = "_riot-sync._tcp"

    public var onPhonesChanged: (([DiscoveredPhone]) -> Void)?
    public var onInboundPairingRequested: ((String) -> Void)?
    /// The pairing succeeded: here is the live channel and the peer's tie-breaker.
    public var onPaired: ((FrameChannel, NearbyPeerIdentity) -> Void)?
    public var onDisconnected: (() -> Void)?

    public let identity: NearbyPeerIdentity
    public var friendlyName: String { identity.friendlyName }
    public var tieBreaker: String { identity.tieBreaker }

    private let queue = DispatchQueue(label: "net.protest.riot.local-nearby")
    private let lock = NSLock()

    private var listener: NWListener?
    private var browser: NWBrowser?

    /// Peers we can see, keyed by the id we hand to the UI.
    private var discovered: [UUID: (endpoint: NWEndpoint, identity: NearbyPeerIdentity)] = [:]
    /// Dialled-in connections that have not yet said what they want.
    ///
    /// They must be held STRONGLY from the moment they are accepted: the channel
    /// owns its `NWConnection`, so if nothing retains it between `newConnection`
    /// and the first frame arriving, it deallocates, the connection dies, and the
    /// pairing request is never seen.
    private var inboundChannels: [LocalTCPFrameChannel] = []
    /// A connection that has sent us a request and is waiting on a human.
    private var pendingInbound: (channel: LocalTCPFrameChannel, identity: NearbyPeerIdentity)?
    /// A connection we opened that is waiting on the far side's human.
    private var pendingOutbound: LocalTCPFrameChannel?

    /// The Bonjour type this instance advertises and browses on. Always
    /// ``serviceType`` in the app; a test passes a unique one so the two peers it
    /// starts meet each other and NOT the Riot instances that happen to be running
    /// on the same machine or the same LAN. Without that, a test's peers discover
    /// every Riot in the room and auto-connect to a stranger.
    private let serviceType: String

    public init(
        nonce: UInt64 = UInt64.random(in: 0...UInt64.max),
        serviceType: String = LocalNetworkNearbyService.serviceType
    ) {
        self.serviceType = serviceType
        identity = NearbyPeerIdentity(
            instanceID: UUID().uuidString,
            friendlyName: FriendlyNameGenerator.name(sessionNonce: nonce),
            tieBreaker: UUID().uuidString
        )
    }

    // MARK: - Discovery

    public func startLooking() {
        startAdvertising()
        startBrowsing()
    }

    /// Note the absence of `requiredInterfaceType = .wifi`, which the
    /// Bluetooth-upgrade path sets: same-machine traffic goes over loopback, not
    /// Wi-Fi, so pinning the interface would make the very case this exists for
    /// impossible — and would also break with Wi-Fi switched off.
    private func parameters() -> NWParameters {
        let parameters = NWParameters.tcp
        parameters.includePeerToPeer = true
        return parameters
    }

    private func startAdvertising() {
        do {
            let listener = try NWListener(using: parameters(), on: .any)
            // Name the service explicitly. Left unset, Network.framework defaults
            // the Bonjour instance name to the DEVICE name ("Jane's MacBook Pro")
            // — the owner's real name, broadcast in cleartext to the whole subnet
            // (the exact leak the AWDL/Bonjour research documents). The instance
            // id is a random per-session UUID: ephemeral and non-identifying, and
            // distinct per instance so two on one machine do not collide.
            listener.service = NWListener.Service(
                name: identity.instanceID,
                type: serviceType,
                txtRecord: NWTXTRecord(identity.txtRecord)
            )
            listener.newConnectionHandler = { [weak self] connection in
                guard let self else { return connection.cancel() }
                connection.start(queue: self.queue)
                self.acceptInbound(LocalTCPFrameChannel(connection: connection))
            }
            listener.start(queue: queue)
            self.listener = listener
        } catch {
            onDisconnected?()
        }
    }

    private func startBrowsing() {
        let browser = NWBrowser(
            for: .bonjourWithTXTRecord(type: serviceType, domain: nil),
            using: parameters()
        )
        browser.browseResultsChangedHandler = { [weak self] results, _ in
            self?.updateDiscovered(results)
        }
        browser.start(queue: queue)
        self.browser = browser
    }

    private func updateDiscovered(_ results: Set<NWBrowser.Result>) {
        lock.lock()
        var next: [UUID: (endpoint: NWEndpoint, identity: NearbyPeerIdentity)] = [:]
        for result in results {
            guard case let .bonjour(txt) = result.metadata,
                  let peer = NearbyPeerIdentity(txt: txt.dictionary),
                  // Bonjour hands us our own advertisement on the same host.
                  !identity.isSelf(peer)
            else { continue }
            // Keep a stable id across refreshes so the row does not flicker.
            let existing = discovered.first { $0.value.identity.instanceID == peer.instanceID }?.key
            next[existing ?? UUID()] = (result.endpoint, peer)
        }
        discovered = next
        let phones = next
            .map { DiscoveredPhone(id: $0.key, friendlyName: $0.value.identity.friendlyName) }
            .sorted { $0.friendlyName < $1.friendlyName }
        lock.unlock()
        onPhonesChanged?(phones)
    }

    /// True when this service is the one that discovered the phone — the
    /// controller runs Bluetooth and local-network discovery side by side and
    /// must route a pairing request back to whichever found the peer.
    public func canPair(with phone: DiscoveredPhone) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return discovered[phone.id] != nil
    }

    /// Which of two phones dials the other. EXACTLY ONE MAY.
    ///
    /// Both phones auto-connect the instant they see each other, so without a rule
    /// both dial, and each ends up holding TWO sockets to the same peer: the one it
    /// opened and the one the peer opened. Each then binds its session to whichever
    /// pairing completed first — and nothing makes the two phones choose the SAME
    /// socket. When they choose opposite ones, each announces its space into a
    /// socket the other has abandoned, the announce sits unread in a dead channel's
    /// inbox, no decision is ever reached, and the space handshake dies. A fresh
    /// phone never adopts the organizer's space.
    ///
    /// This is the tie-break the Bluetooth-to-TCP upgrade already uses, and it is
    /// symmetric: of any two peers, exactly one sees `.attempt`. The other waits,
    /// and the dial that is already on its way is auto-accepted.
    public func shouldDial(_ phone: DiscoveredPhone) -> Bool {
        lock.lock()
        let peer = discovered[phone.id]?.identity
        lock.unlock()
        guard let peer else { return false }
        return LocalNetworkRole.select(
            localName: identity.friendlyName,
            localToken: identity.tieBreaker,
            remoteName: peer.friendlyName,
            remoteToken: peer.tieBreaker
        ) == .attempt
    }

    // MARK: - Pairing (outbound)

    public func requestPairing(with phone: DiscoveredPhone) {
        lock.lock()
        let target = discovered[phone.id]
        lock.unlock()
        guard let target else {
            onDisconnected?()
            return
        }
        dial(endpoint: target.endpoint, peer: target.identity, attemptsLeft: 3)
    }

    /// A connect to a just-advertised Bonjour endpoint is unreliable in two
    /// distinct ways, and BOTH must be retried: (1) it fails or hangs (the far
    /// side's listener/mDNS records are not ready yet); (2) it reaches `.ready`
    /// but the stream never reaches the far side's listener — a "ready to
    /// nowhere" that delivers no reply. So the attempt deadline covers the WHOLE
    /// handshake through receiving `.accept`, not just the connect; a stalled
    /// attempt is cancelled and a fresh connection dialled.
    private func dial(endpoint: NWEndpoint, peer: NearbyPeerIdentity, attemptsLeft: Int) {
        let connection = NWConnection(to: endpoint, using: parameters())
        // First of {paired, declined, failed, deadline} wins; rest are no-ops.
        // A Sendable latch + a method (not local funcs) because the @Sendable
        // state handler cannot capture a local `var`/closure under Swift 6.
        let settled = DialLatch()
        connection.stateUpdateHandler = { [weak self] state in
            guard let self else { return }
            switch state {
            case .ready:
                // Do NOT claim here — connecting is not pairing. The handshake
                // still has to land; if it does not, the deadline retries.
                let channel = LocalTCPFrameChannel(connection: connection)
                self.beginOutboundHandshake(
                    on: channel,
                    peer: peer,
                    onPaired: { remote in
                        guard settled.claim() else { channel.disconnect(); return }
                        self.onPaired?(channel, remote)
                    },
                    onDeclined: {
                        guard settled.claim() else { return }
                        self.onDisconnected?() // a "no" is final; do not retry it
                    }
                )
            case .failed, .cancelled:
                guard settled.claim() else { return }
                self.retryDial(connection, endpoint: endpoint, peer: peer, attemptsLeft: attemptsLeft)
            default:
                break
            }
        }
        connection.start(queue: queue)
        queue.asyncAfter(deadline: .now() + 3.0) { [weak self] in
            guard settled.claim(), let self else { return }
            self.retryDial(connection, endpoint: endpoint, peer: peer, attemptsLeft: attemptsLeft)
        }
    }

    /// Cancels the stalled/failed attempt and dials afresh, or gives up once the
    /// attempt budget is spent.
    private func retryDial(
        _ connection: NWConnection,
        endpoint: NWEndpoint,
        peer: NearbyPeerIdentity,
        attemptsLeft: Int
    ) {
        connection.cancel()
        if attemptsLeft > 1 {
            queue.asyncAfter(deadline: .now() + 0.4) { [weak self] in
                self?.dial(endpoint: endpoint, peer: peer, attemptsLeft: attemptsLeft - 1)
            }
        } else {
            onDisconnected?()
        }
    }

    /// Sends our request and resolves via exactly one callback. It does not touch
    /// `onPaired`/`onDisconnected` directly — the dial owns the retry decision,
    /// because a lost handshake should be retried but a decline should not.
    private func beginOutboundHandshake(
        on channel: LocalTCPFrameChannel,
        peer: NearbyPeerIdentity,
        onPaired: @escaping (NearbyPeerIdentity) -> Void,
        onDeclined: @escaping () -> Void
    ) {
        lock.lock()
        pendingOutbound = channel
        lock.unlock()

        channel.onReceive = { [weak self, weak channel] frame in
            guard let self, let channel else { return }
            guard let message = LocalPairingMessage(frame: frame) else { return }
            switch message {
            case let .accept(remote):
                // Stop intercepting: everything after this belongs to the sync
                // session. Frames that arrive before it installs its own
                // receiver are buffered by the channel's inbox, not dropped.
                channel.onReceive = nil
                self.lock.lock()
                self.pendingOutbound = nil
                self.lock.unlock()
                onPaired(remote)
            case .decline:
                channel.disconnect()
                self.lock.lock()
                self.pendingOutbound = nil
                self.lock.unlock()
                onDeclined()
            case .request:
                break // The side that dialled does not answer requests.
            }
        }
        try? channel.send(LocalPairingMessage.request(identity).encoded())
    }

    // MARK: - Pairing (inbound)

    private func acceptInbound(_ channel: LocalTCPFrameChannel) {
        lock.lock()
        inboundChannels.append(channel)
        lock.unlock()

        channel.onReceive = { [weak self, weak channel] frame in
            guard let self, let channel else { return }
            guard case let .request(remote)? = LocalPairingMessage(frame: frame) else { return }
            self.lock.lock()
            // One pairing at a time; a second dialler is refused rather than queued.
            let busy = self.pendingInbound != nil
            if !busy { self.pendingInbound = (channel, remote) }
            self.inboundChannels.removeAll { $0 === channel }
            self.lock.unlock()
            if busy {
                try? channel.send(LocalPairingMessage.decline.encoded())
                channel.disconnect()
                return
            }
            self.onInboundPairingRequested?(remote.friendlyName)
        }
    }

    /// The person on this side said yes.
    public func confirmInboundPairing() {
        lock.lock()
        let pending = pendingInbound
        pendingInbound = nil
        lock.unlock()
        guard let pending else { return }
        pending.channel.onReceive = nil
        try? pending.channel.send(LocalPairingMessage.accept(identity).encoded())
        onPaired?(pending.channel, pending.identity)
    }

    public func cancelInboundPairing() {
        lock.lock()
        let pending = pendingInbound
        pendingInbound = nil
        lock.unlock()
        guard let pending else { return }
        try? pending.channel.send(LocalPairingMessage.decline.encoded())
        pending.channel.disconnect()
    }

    public func cancelPairing() {
        lock.lock()
        let outbound = pendingOutbound
        pendingOutbound = nil
        lock.unlock()
        outbound?.disconnect()
    }

    // MARK: - Teardown

    public func stop() {
        lock.lock()
        let inbound = pendingInbound?.channel
        let outbound = pendingOutbound
        let unclaimed = inboundChannels
        pendingInbound = nil
        pendingOutbound = nil
        inboundChannels = []
        discovered = [:]
        lock.unlock()
        inbound?.disconnect()
        outbound?.disconnect()
        unclaimed.forEach { $0.disconnect() }
        listener?.cancel()
        browser?.cancel()
        listener = nil
        browser = nil
    }
}
