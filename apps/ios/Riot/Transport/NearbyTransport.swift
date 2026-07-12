import Foundation

enum NearbyLimits {
    static let maxFrameBytes = (8 * 1024 * 1024) + 128
    static let maxBLEEnvelopeBytes = maxFrameBytes + 1
    static let maxPendingFrames = 64
}

public enum NearbyRoute: CaseIterable, Hashable, Sendable {
    case localNetwork
    case bluetooth
}

public enum NearbyTransportError: Error, Equatable {
    case pairingNotConfirmed
    case notConnected
    case disconnected
}

final class FailureLatch: @unchecked Sendable {
    private let lock = NSLock()
    private var storedCallback: (() -> Void)?
    private var failed = false
    private var cancelled = false

    var callback: (() -> Void)? {
        get { lock.lock(); defer { lock.unlock() }; return storedCallback }
        set {
            lock.lock()
            storedCallback = newValue
            let deliver = failed && !cancelled ? newValue : nil
            lock.unlock()
            deliver?()
        }
    }

    func fail() {
        lock.lock()
        guard !failed, !cancelled else { lock.unlock(); return }
        failed = true
        let deliver = storedCallback
        lock.unlock()
        deliver?()
    }

    func cancel() {
        lock.lock()
        cancelled = true
        storedCallback = nil
        lock.unlock()
    }
}

public enum FriendlyNameGenerator {
    private static let adjectives = ["Amber", "Blue", "Silver", "Quiet"]
    private static let nouns = ["Kite", "River", "Harbor", "Pine"]

    public static func name(sessionNonce: UInt64) -> String {
        let adjective = adjectives[Int(sessionNonce % UInt64(adjectives.count))]
        let noun = nouns[Int((sessionNonce &* 2) % UInt64(nouns.count))]
        return "\(adjective) \(noun)"
    }
}

public protocol FrameChannel: AnyObject {
    var onReceive: ((Data) -> Void)? { get set }
    var onFailure: (() -> Void)? { get set }
    func send(_ frame: Data) throws
    func disconnect()
}

final class BoundedFrameInbox {
    private let lock = NSLock()
    private var receiver: ((Data) -> Void)?
    private var pending: [Data] = []
    private var pendingBytes = 0
    private var failed = false
    private var delivering = false

    var onReceive: ((Data) -> Void)? {
        get {
            lock.lock()
            let value = receiver
            lock.unlock()
            return value
        }
        set {
            lock.lock()
            receiver = newValue
            let shouldDrain = newValue != nil && !failed && !pending.isEmpty && !delivering
            if shouldDrain { delivering = true }
            lock.unlock()
            if shouldDrain { drain() }
        }
    }

    @discardableResult
    func receive(_ frame: Data) -> Bool {
        lock.lock()
        guard !failed else { lock.unlock(); return false }
        guard pending.count < NearbyLimits.maxPendingFrames,
              pendingBytes + frame.count <= NearbyLimits.maxFrameBytes else {
            failed = true
            pending.removeAll()
            pendingBytes = 0
            lock.unlock()
            return false
        }
        pending.append(frame)
        pendingBytes += frame.count
        let shouldDrain = receiver != nil && !delivering
        if shouldDrain { delivering = true }
        lock.unlock()
        if shouldDrain { drain() }
        return true
    }

    private func drain() {
        while true {
            lock.lock()
            guard !failed, let receiver, !pending.isEmpty else {
                delivering = false
                lock.unlock()
                return
            }
            let next = pending.removeFirst()
            pendingBytes -= next.count
            lock.unlock()
            receiver(next)
        }
    }
}

public final class NearbyConnection {
    public var onReceive: ((Data) -> Void)? {
        didSet { activeChannel?.onReceive = onReceive }
    }
    public private(set) var route: NearbyRoute?
    public var onFailure: (() -> Void)? {
        get { failureLatch.callback }
        set { failureLatch.callback = newValue }
    }

    private let base: FrameChannel
    private let baseRoute: NearbyRoute
    private let localAttempt: () -> FrameChannel?
    private var activeChannel: FrameChannel?
    private var pairingConfirmed = false
    private var activated = false
    private var isDisconnected = false
    private let failureLatch = FailureLatch()

    /// The general form: whichever transport carried the pairing carries the
    /// session, with an optional upgrade to something better.
    ///
    /// Bluetooth is not always available — two instances on one machine have no
    /// usable radio (a single BLE controller never hears its own advertisement),
    /// so a peer found over the local network arrives here with `.localNetwork`
    /// as its base and nothing to upgrade to.
    public init(
        base: FrameChannel,
        baseRoute: NearbyRoute,
        localAttempt: @escaping () -> FrameChannel?
    ) {
        self.base = base
        self.baseRoute = baseRoute
        self.localAttempt = localAttempt
    }

    /// The Bluetooth-first path: pair over the radio, then upgrade to the local
    /// network if one is reachable.
    public convenience init(
        bluetooth: FrameChannel,
        localAttempt: @escaping () -> FrameChannel?
    ) {
        self.init(base: bluetooth, baseRoute: .bluetooth, localAttempt: localAttempt)
    }

    public func confirmPairing() {
        pairingConfirmed = true
    }

    public func activate() throws {
        guard pairingConfirmed else { throw NearbyTransportError.pairingNotConfirmed }
        guard !isDisconnected else { throw NearbyTransportError.disconnected }
        guard !activated else { return }
        activated = true

        // Only worth upgrading if the base is not already the better route —
        // a local-network base has nothing to trade up to, and swapping it out
        // would drop the very channel the pairing arrived on.
        if baseRoute != .localNetwork, let local = localAttempt() {
            activeChannel = local
            route = .localNetwork
            base.disconnect()
        } else {
            activeChannel = base
            route = baseRoute
        }
        activeChannel?.onReceive = onReceive
        activeChannel?.onFailure = { [weak self] in self?.fail() }
    }

    public func send(_ frame: Data) throws {
        guard pairingConfirmed else { throw NearbyTransportError.pairingNotConfirmed }
        guard !isDisconnected else { throw NearbyTransportError.disconnected }
        guard let activeChannel else { throw NearbyTransportError.notConnected }
        try activeChannel.send(frame)
    }

    public func disconnect() {
        isDisconnected = true
        failureLatch.cancel()
        activeChannel?.disconnect()
        activeChannel = nil
    }

    private func fail() {
        guard !isDisconnected else { return }
        isDisconnected = true
        activeChannel = nil
        failureLatch.fail()
    }
}

public final class LoopbackFrameChannel: FrameChannel {
    public var onReceive: ((Data) -> Void)? {
        get { inbox.onReceive }
        set { inbox.onReceive = newValue }
    }
    private let inbox = BoundedFrameInbox()
    private weak var other: LoopbackFrameChannel?
    private var connected = true
    public var onFailure: (() -> Void)?

    public static func pair() -> (first: LoopbackFrameChannel, second: LoopbackFrameChannel) {
        let first = LoopbackFrameChannel()
        let second = LoopbackFrameChannel()
        first.other = second
        second.other = first
        return (first, second)
    }

    public func send(_ frame: Data) throws {
        guard connected, let other, other.connected else { throw NearbyTransportError.disconnected }
        guard other.inbox.receive(frame) else {
            other.connected = false
            throw NearbyTransportError.disconnected
        }
    }

    public func disconnect() {
        connected = false
    }
}
