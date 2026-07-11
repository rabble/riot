import Foundation

enum NearbyLimits {
    static let maxFrameBytes = (8 * 1024 * 1024) + 128
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
    func send(_ frame: Data) throws
    func disconnect()
}

final class BoundedFrameInbox {
    private let lock = NSLock()
    private var receiver: ((Data) -> Void)?
    private var pending: [Data] = []
    private var pendingBytes = 0
    private var failed = false

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
            let queued = newValue != nil && !failed ? pending : []
            if !queued.isEmpty { pending.removeAll(); pendingBytes = 0 }
            lock.unlock()
            if let newValue { queued.forEach(newValue) }
        }
    }

    @discardableResult
    func receive(_ frame: Data) -> Bool {
        lock.lock()
        guard !failed else { lock.unlock(); return false }
        if let receiver {
            lock.unlock()
            receiver(frame)
            return true
        }
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
        lock.unlock()
        return true
    }
}

public final class NearbyConnection {
    public var onReceive: ((Data) -> Void)? {
        didSet { activeChannel?.onReceive = onReceive }
    }
    public private(set) var route: NearbyRoute?

    private let bluetooth: FrameChannel
    private let localAttempt: () -> FrameChannel?
    private var activeChannel: FrameChannel?
    private var pairingConfirmed = false
    private var activated = false
    private var isDisconnected = false

    public init(
        bluetooth: FrameChannel,
        localAttempt: @escaping () -> FrameChannel?
    ) {
        self.bluetooth = bluetooth
        self.localAttempt = localAttempt
    }

    public func confirmPairing() {
        pairingConfirmed = true
    }

    public func activate() throws {
        guard pairingConfirmed else { throw NearbyTransportError.pairingNotConfirmed }
        guard !isDisconnected else { throw NearbyTransportError.disconnected }
        guard !activated else { return }
        activated = true

        if let local = localAttempt() {
            activeChannel = local
            route = .localNetwork
        } else {
            activeChannel = bluetooth
            route = .bluetooth
        }
        activeChannel?.onReceive = onReceive
    }

    public func send(_ frame: Data) throws {
        guard pairingConfirmed else { throw NearbyTransportError.pairingNotConfirmed }
        guard !isDisconnected else { throw NearbyTransportError.disconnected }
        guard let activeChannel else { throw NearbyTransportError.notConnected }
        try activeChannel.send(frame)
    }

    public func disconnect() {
        isDisconnected = true
        activeChannel?.disconnect()
        activeChannel = nil
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
