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

public final class LocalNetworkListener: @unchecked Sendable {
    public var onAccepted: ((LocalTCPFrameChannel) -> Void)?
    private let queue = DispatchQueue(label: "net.protest.riot.local-listener")
    private var listener: NWListener?

    public init() {}

    public func start(completion: @escaping @Sendable (LocalEndpoint?) -> Void) {
        do {
            let parameters = NWParameters.tcp
            parameters.requiredInterfaceType = .wifi
            parameters.includePeerToPeer = true
            let listener = try NWListener(using: parameters, on: .any)
            self.listener = listener
            listener.newConnectionHandler = { [weak self] connection in
                guard let self else { return }
                connection.start(queue: self.queue)
                self.onAccepted?(LocalTCPFrameChannel(connection: connection))
            }
            listener.stateUpdateHandler = { state in
                if case .ready = state, let port = listener.port, let host = Self.localIPv4Address() {
                    completion(LocalEndpoint(host: host, port: port.rawValue))
                } else if case .failed = state {
                    completion(nil)
                }
            }
            listener.start(queue: queue)
        } catch {
            completion(nil)
        }
    }

    public func stop() {
        listener?.cancel()
        listener = nil
    }

    private static func localIPv4Address() -> String? {
        var interfaces: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&interfaces) == 0, let first = interfaces else { return nil }
        defer { freeifaddrs(interfaces) }
        for pointer in sequence(first: first, next: { $0.pointee.ifa_next }) {
            let interface = pointer.pointee
            guard interface.ifa_addr.pointee.sa_family == UInt8(AF_INET),
                  (interface.ifa_flags & UInt32(IFF_LOOPBACK)) == 0 else { continue }
            var address = [CChar](repeating: 0, count: Int(INET_ADDRSTRLEN))
            var socketAddress = interface.ifa_addr.pointee
            guard getnameinfo(&socketAddress, socklen_t(interface.ifa_addr.pointee.sa_len), &address, socklen_t(address.count), nil, 0, NI_NUMERICHOST) == 0 else { continue }
            let host = String(decoding: address.prefix { $0 != 0 }.map(UInt8.init(bitPattern:)), as: UTF8.self)
            if LocalEndpoint(host: host, port: 1) != nil { return host }
        }
        return nil
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
