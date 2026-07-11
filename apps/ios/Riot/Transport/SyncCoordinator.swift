import Foundation

public enum NearbySyncOutcome: Equatable, Sendable {
    case sendMore
    case readyToPreview(count: UInt32)
    case done
    case failed
}

public protocol MobileSyncSessionBoundary: AnyObject {
    func nextOutbound() throws -> Data?
    func receive(_ frame: Data) throws -> NearbySyncOutcome
    func acceptImport() throws
    func rejectImport() throws
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
            switch try session.receive(frame) {
            case .sendMore: try pumpOutbound()
            case let .readyToPreview(count): state = .preview(count: count, name: friendlyName)
            case .done: state = .caughtUp
            case .failed: state = .failed; connection.disconnect()
            }
        } catch {
            state = .failed
            connection.disconnect()
        }
    }

    private func pumpOutbound() throws {
        while let frame = try session.nextOutbound() { try connection.send(frame) }
    }
}
