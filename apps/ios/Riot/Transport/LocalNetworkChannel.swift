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
    public var onFailure: (() -> Void)? {
        get { failureLatch.callback }
        set { failureLatch.callback = newValue }
    }
    public var onReceive: ((Data) -> Void)? {
        get { inbox.onReceive }
        set { inbox.onReceive = newValue }
    }
    private let inbox = BoundedFrameInbox()
    private let connection: NWConnection
    private let queue = DispatchQueue(label: "net.protest.riot.local-tcp")
    private var decoder = FrameDecoder()
    private let failureLatch = FailureLatch()

    init(connection: NWConnection) {
        self.connection = connection
        connection.stateUpdateHandler = { [weak self] state in
            if case .failed = state { self?.fail() }
            if case .cancelled = state { self?.fail() }
        }
        receiveNext()
    }

    public func send(_ frame: Data) throws {
        connection.send(content: try FrameDecoder.encode(frame), completion: .contentProcessed { [weak self] error in
            if error != nil { self?.fail() }
        })
    }

    public func disconnect() {
        failureLatch.cancel()
        connection.cancel()
    }

    private func receiveNext() {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 64 * 1024) { [weak self] data, _, complete, error in
            guard let self else { return }
            if let data {
                do {
                    let frames = try self.decoder.append(data)
                    for frame in frames where !self.inbox.receive(frame) {
                        self.fail()
                        return
                    }
                } catch {
                    self.fail()
                    return
                }
            }
            if error == nil && !complete { self.receiveNext() }
            else { self.fail() }
        }
    }

    private func fail() {
        failureLatch.fail()
        connection.cancel()
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
            case .ready:
                let channel = LocalTCPFrameChannel(connection: connection)
                if !finish.call(channel) { channel.disconnect() }
            case .failed, .cancelled: finish.call(nil)
            default: break
            }
        }
        connection.start(queue: queue)
        queue.asyncAfter(deadline: .now() + timeout) {
            if finish.call(nil) { connection.cancel() }
        }
    }
}


public final class LocalNetworkListener: @unchecked Sendable {
    public var onAccepted: ((LocalTCPFrameChannel) -> Void)?
    private let queue = DispatchQueue(label: "net.protest.riot.local-listener")
    private var listener: NWListener?
    private let admissionLock = NSLock()
    private var accepted = false

    public init() {}

    public func start(completion: @escaping @Sendable (LocalEndpoint?) -> Void) {
        do {
            admissionLock.lock()
            accepted = false
            admissionLock.unlock()
            let parameters = NWParameters.tcp
            parameters.requiredInterfaceType = .wifi
            parameters.includePeerToPeer = true
            let listener = try NWListener(using: parameters, on: .any)
            let finishEndpoint = OneShotEndpointCompletion(completion)
            self.listener = listener
            listener.newConnectionHandler = { [weak self] connection in
                guard let self else { return }
                self.admissionLock.lock()
                let shouldAccept = !self.accepted
                self.accepted = true
                self.admissionLock.unlock()
                guard shouldAccept else { connection.cancel(); return }
                connection.start(queue: self.queue)
                self.onAccepted?(LocalTCPFrameChannel(connection: connection))
                listener.cancel()
            }
            listener.stateUpdateHandler = { state in
                if case .ready = state, let port = listener.port, let host = Self.localIPv4Address() {
                    finishEndpoint.call(LocalEndpoint(host: host, port: port.rawValue))
                } else if case .failed = state {
                    finishEndpoint.call(nil)
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

    @discardableResult
    func call(_ channel: LocalTCPFrameChannel?) -> Bool {
        lock.lock()
        let callback = completion
        completion = nil
        lock.unlock()
        callback?(channel)
        return callback != nil
    }
}

final class OneShotEndpointCompletion: @unchecked Sendable {
    private let lock = NSLock()
    private var completion: (@Sendable (LocalEndpoint?) -> Void)?

    init(_ completion: @escaping @Sendable (LocalEndpoint?) -> Void) {
        self.completion = completion
    }

    func call(_ endpoint: LocalEndpoint?) {
        lock.lock()
        let callback = completion
        completion = nil
        lock.unlock()
        callback?(endpoint)
    }
}
