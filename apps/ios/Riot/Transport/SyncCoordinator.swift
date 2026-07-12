import Foundation

public enum NearbySyncOutcome: Equatable, Sendable {
    case sendMore(terminal: Bool = false)
    case readyToPreview(count: UInt32)
    case done
    case failed
}

public protocol MobileSyncSessionBoundary: AnyObject {
    func begin() throws -> NearbySyncOutcome
    func nextOutbound() throws -> Data?
    func receive(_ frame: Data) throws -> NearbySyncOutcome
    func acceptImport() throws -> NearbySyncOutcome
    func rejectImport() throws -> NearbySyncOutcome
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
        case .idle: "Find nearby devices"
        case .looking: "Looking for nearby devices..."
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
    private var sessionClosed = false
    private var acceptedImport = false

    public init(session: MobileSyncSessionBoundary, connection: NearbyConnection, friendlyName: String) {
        self.session = session
        self.connection = connection
        self.friendlyName = friendlyName
        connection.onReceive = { [weak self] frame in self?.receive(frame) }
        connection.onFailure = { [weak self] in self?.fail() }
    }

    public func start() {
        do {
            state = .gettingLatest(name: friendlyName)
            try process(try session.begin(), terminalState: .alreadyCurrent)
        } catch {
            fail()
        }
    }

    public func addPreviewedContent() {
        do {
            acceptedImport = true
            try process(try session.acceptImport(), terminalState: .caughtUp)
            if !sessionClosed { state = .gettingLatest(name: friendlyName) }
        } catch { fail() }
    }

    public func rejectPreviewedContent() {
        do {
            try process(try session.rejectImport(), terminalState: .idle)
        } catch { fail() }
    }

    public func stop() {
        closeSession()
        connection.disconnect()
    }

    private func receive(_ frame: Data) {
        guard !sessionClosed else { return }
        do {
            try process(try session.receive(frame), terminalState: acceptedImport ? .caughtUp : .alreadyCurrent)
        } catch {
            fail()
        }
    }

    private func sendNextOutbound() throws {
        guard let frame = try session.nextOutbound() else { throw NearbyTransportError.notConnected }
        try connection.send(frame)
    }

    private func process(_ outcome: NearbySyncOutcome, terminalState: NearbyConnectionState) throws {
        switch outcome {
        case let .sendMore(terminal):
            try sendNextOutbound()
            if terminal { state = terminalState; closeSession() }
        case let .readyToPreview(count): state = .preview(count: count, name: friendlyName)
        case .done: state = terminalState; closeSession()
        case .failed: fail()
        }
    }

    private func fail() {
        state = .failed
        closeSession()
        connection.disconnect()
    }

    private func closeSession() {
        guard !sessionClosed else { return }
        sessionClosed = true
        try? session.close()
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

    public func acceptImport() throws -> NearbySyncOutcome {
        guard let pendingBundle else { throw NearbyTransportError.notConnected }
        try persistBundle(pendingBundle)
        let outcome = map(try backend.acceptImport())
        self.pendingBundle = nil
        return outcome
    }

    public func rejectImport() throws -> NearbySyncOutcome {
        let generated = try backend.rejectImport(code: 1)
        pendingBundle = nil
        return generated.kind == .rejected ? .done : map(generated)
    }

    public func close() throws { try backend.cancel() }

    private func map(_ outcome: SyncOutcome) -> NearbySyncOutcome {
        if let bundle = outcome.importBundleBytes { pendingBundle = bundle }
        switch outcome.kind {
        case .frameReady: return .sendMore(terminal: outcome.terminal)
        case .reviewImport: return .readyToPreview(count: UInt32(outcome.entries.count))
        case .complete: return .done
        case .rejected: return .failed
        }
    }
}
