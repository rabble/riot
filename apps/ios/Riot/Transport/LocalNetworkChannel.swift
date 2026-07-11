import Darwin
import Foundation
@preconcurrency import Network

public struct LocalEndpoint: Equatable, Sendable {
    public let host: String
    public let port: UInt16

    public init?(host: String, port: UInt16) {
        guard port > 0, Self.isLocalAddress(host) else { return nil }
        self.host = host
        self.port = port
    }

    private static func isLocalAddress(_ host: String) -> Bool {
        var ipv6 = in6_addr()
        if host.withCString({ inet_pton(AF_INET6, $0, &ipv6) }) == 1 {
            return withUnsafeBytes(of: &ipv6) { bytes in
                let first = bytes[0]
                let second = bytes[1]
                return (first == 0xfe && (second & 0xc0) == 0x80)
                    || (first & 0xfe) == 0xfc
            }
        }
        let parts = host.split(separator: ".").compactMap { UInt8($0) }
        guard parts.count == 4 else { return false }
        return parts[0] == 10
            || (parts[0] == 172 && (16...31).contains(parts[1]))
            || (parts[0] == 192 && parts[1] == 168)
            || (parts[0] == 169 && parts[1] == 254)
    }
}

public final class LocalTCPFrameChannel: FrameChannel, @unchecked Sendable {
    public var onReceive: ((Data) -> Void)?
    private let connection: NWConnection
    private let queue = DispatchQueue(label: "net.protest.riot.local-tcp")
    private var decoder = FrameDecoder()

    init(connection: NWConnection) {
        self.connection = connection
        receiveNext()
    }

    public func send(_ frame: Data) throws {
        connection.send(content: FrameDecoder.encode(frame), completion: .contentProcessed { _ in })
    }

    public func disconnect() {
        connection.cancel()
    }

    private func receiveNext() {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 64 * 1024) { [weak self] data, _, complete, error in
            guard let self else { return }
            if let data {
                if let frames = try? self.decoder.append(data) {
                    frames.forEach { self.onReceive?($0) }
                }
            }
            if error == nil && !complete { self.receiveNext() }
        }
    }

    public static func attempt(
        endpoint: LocalEndpoint,
        timeout: TimeInterval = 2,
        completion: @escaping @Sendable (LocalTCPFrameChannel?) -> Void
    ) {
        let parameters = NWParameters.tcp
        parameters.requiredInterfaceType = .wifi
        parameters.includePeerToPeer = true
        let connection = NWConnection(
            host: NWEndpoint.Host(endpoint.host),
            port: NWEndpoint.Port(rawValue: endpoint.port)!,
            using: parameters
        )
        let queue = DispatchQueue(label: "net.protest.riot.local-attempt")
        let finish = OneShotCompletion(completion)
        connection.stateUpdateHandler = { state in
            switch state {
            case .ready: finish.call(LocalTCPFrameChannel(connection: connection))
            case .failed, .cancelled: finish.call(nil)
            default: break
            }
        }
        connection.start(queue: queue)
        queue.asyncAfter(deadline: .now() + timeout) {
            finish.call(nil)
            connection.cancel()
        }
    }
}

private final class OneShotCompletion: @unchecked Sendable {
    private let lock = NSLock()
    private var completion: (@Sendable (LocalTCPFrameChannel?) -> Void)?

    init(_ completion: @escaping @Sendable (LocalTCPFrameChannel?) -> Void) {
        self.completion = completion
    }

    func call(_ channel: LocalTCPFrameChannel?) {
        lock.lock()
        let callback = completion
        completion = nil
        lock.unlock()
        callback?(channel)
    }
}
