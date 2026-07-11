import Foundation

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
    public var onReceive: ((Data) -> Void)?
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
        other.onReceive?(frame)
    }

    public func disconnect() {
        connected = false
    }
}
