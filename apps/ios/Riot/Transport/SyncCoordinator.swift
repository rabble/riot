import Foundation

public enum NearbySyncOutcome: Equatable, Sendable {
    case sendMore
    case readyToPreview(count: UInt32)
    case done
    case failed
}

public protocol MobileSyncSessionBoundary: AnyObject {
    func begin() throws -> NearbySyncOutcome
    func nextOutbound() throws -> Data?
    func receive(_ frame: Data) throws -> NearbySyncOutcome
    func acceptImport() throws
    func rejectImport() throws
    func close() throws
}

public enum NearbyConnectionState: Equatable, Sendable {
    case idle
    case looking
    case confirm(name: String)
    case connecting
    case gettingLatest(name: String)
    case preview(count: UInt32, name: String)
    case caughtUp
    case alreadyCurrent
    case outOfRange(name: String)
    case failed

    public var message: String {
        switch self {
        case .idle: "Find nearby phones"
        case .looking: "Looking for nearby phones..."
        case let .confirm(name): "Connect with \(name)?"
        case .connecting: "Connecting..."
        case let .gettingLatest(name): "Getting the latest from \(name)..."
        case let .preview(count, name): "\(count) new update\(count == 1 ? "" : "s") from \(name)"
        case .caughtUp: "All caught up"
        case .alreadyCurrent: "You’re already up to date"
        case let .outOfRange(name): "\(name) went out of range"
        case .failed: "Couldn’t connect — try again"
        }
    }
}

public final class SyncCoordinator {
    public var onStateChanged: ((NearbyConnectionState) -> Void)?
    public private(set) var state: NearbyConnectionState = .idle {
        didSet { onStateChanged?(state) }
    }

    private let session: MobileSyncSessionBoundary
    private let connection: NearbyConnection
    private let friendlyName: String

    public init(session: MobileSyncSessionBoundary, connection: NearbyConnection, friendlyName: String) {
        self.session = session
        self.connection = connection
        self.friendlyName = friendlyName
        connection.onReceive = { [weak self] frame in self?.receive(frame) }
    }

    public func start() {
        do {
            state = .gettingLatest(name: friendlyName)
            try handle(try session.begin())
            try pumpOutbound()
        } catch {
            state = .failed
            connection.disconnect()
        }
    }

    public func addPreviewedContent() {
        do {
            try session.acceptImport()
            state = .caughtUp
        } catch { state = .failed }
    }

    public func rejectPreviewedContent() {
        try? session.rejectImport()
        state = .idle
    }

    private func receive(_ frame: Data) {
        do {
            try handle(try session.receive(frame))
            try pumpOutbound()
        } catch {
            state = .failed
            connection.disconnect()
        }
    }

    private func pumpOutbound() throws {
        while let frame = try session.nextOutbound() { try connection.send(frame) }
    }

    private func handle(_ outcome: NearbySyncOutcome) throws {
        switch outcome {
        case .sendMore: break
        case let .readyToPreview(count): state = .preview(count: count, name: friendlyName)
        case .done: state = .caughtUp; try? session.close()
        case .failed: state = .failed; connection.disconnect(); try? session.close()
        }
    }
}

public protocol GeneratedSyncSessionBackend: AnyObject {
    func begin() throws -> SyncOutcome
    func takeOutboundFrame() throws -> Data?
    func receiveFrame(frameBytes: Data) throws -> SyncOutcome
    func acceptImport() throws -> SyncOutcome
    func rejectImport(code: UInt8) throws -> SyncOutcome
    func cancel() throws
}

extension MobileSyncSession: GeneratedSyncSessionBackend {}

public final class GeneratedSyncSessionAdapter: MobileSyncSessionBoundary {
    private let backend: GeneratedSyncSessionBackend
    private let persistBundle: (Data) throws -> Void
    private var pendingBundle: Data?

    public init(backend: GeneratedSyncSessionBackend, persistBundle: @escaping (Data) throws -> Void) {
        self.backend = backend
        self.persistBundle = persistBundle
    }

    public func begin() throws -> NearbySyncOutcome { map(try backend.begin()) }
    public func nextOutbound() throws -> Data? { try backend.takeOutboundFrame() }

    public func receive(_ frame: Data) throws -> NearbySyncOutcome {
        map(try backend.receiveFrame(frameBytes: frame))
    }

    public func acceptImport() throws {
        guard let pendingBundle else { throw NearbyTransportError.notConnected }
        try persistBundle(pendingBundle)
        _ = try backend.acceptImport()
        self.pendingBundle = nil
    }

    public func rejectImport() throws {
        _ = try backend.rejectImport(code: 1)
        pendingBundle = nil
    }

    public func close() throws { try backend.cancel() }

    private func map(_ outcome: SyncOutcome) -> NearbySyncOutcome {
        if let bundle = outcome.importBundleBytes { pendingBundle = bundle }
        switch outcome.kind {
        case .frameReady: return .sendMore
        case .reviewImport: return .readyToPreview(count: UInt32(outcome.entries.count))
        case .complete: return .done
        case .rejected: return .failed
        }
    }
}
