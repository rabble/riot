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
    /// This phone is in no space, and the phone it just connected to is in one.
    /// Joining is how a fresh phone gets a space, so it is offered by name — and
    /// it is the person's to accept, not ours.
    case joinSpace(title: String, name: String)
    case gettingLatest(name: String)
    case preview(count: UInt32, name: String)
    case caughtUp
    case alreadyCurrent
    /// Both phones are in a space, and it is not the same space. A phone can only
    /// be in one, so nothing is synced and nothing is switched.
    case differentSpace(name: String)
    /// Neither phone is in a space yet, so there is nothing to send either way.
    case nothingToShare
    case outOfRange(name: String)
    case failed

    public var message: String {
        switch self {
        case .idle: "Find nearby devices"
        case .looking: "Looking for nearby devices..."
        case let .confirm(name): "Connect with \(name)?"
        case .connecting: "Connecting..."
        case let .joinSpace(title, name): "Join \(title) from \(name)?"
        case let .gettingLatest(name): "Getting the latest from \(name)..."
        case let .preview(count, name): "\(count) new update\(count == 1 ? "" : "s") from \(name)"
        case .caughtUp: "All caught up"
        case .alreadyCurrent: "You’re already up to date"
        case let .differentSpace(name): "\(name) is in a different space"
        case .nothingToShare: "Nothing to share yet"
        case let .outOfRange(name): "\(name) went out of range"
        case .failed: "Couldn’t connect — try again"
        }
    }
}

public final class SyncCoordinator {
    public var onStateChanged: ((NearbyConnectionState) -> Void)?
    /// An extra hook for anyone who wants to know an import landed. The redraw
    /// of open apps does NOT depend on it — `addPreviewedContent` announces that
    /// itself — so leaving this nil loses nothing.
    public var onImportAccepted: (() -> Void)?
    public private(set) var state: NearbyConnectionState = .idle {
        didSet { onStateChanged?(state) }
    }

    private let session: MobileSyncSessionBoundary
    private let connection: NearbyConnection
    private let friendlyName: String
    private var sessionClosed = false
    private var acceptedImport = false

    /// - Parameter framesFromConnection: when false, frames arrive through
    ///   `deliver(_:)` instead. `SpacePairing` needs that: it has owned the
    ///   connection's receive path since before this session existed, and it
    ///   replays what it buffered while the two phones settled their spaces. A
    ///   coordinator that took `onReceive` for itself would let a frame arriving
    ///   during the handover jump ahead of the buffered ones.
    public init(
        session: MobileSyncSessionBoundary,
        connection: NearbyConnection,
        friendlyName: String,
        framesFromConnection: Bool = true
    ) {
        self.session = session
        self.connection = connection
        self.friendlyName = friendlyName
        if framesFromConnection {
            connection.onReceive = { [weak self] frame in self?.receive(frame) }
        }
        connection.onFailure = { [weak self] in self?.fail() }
    }

    /// Feeds one frame in, for a caller that owns the wire (see the initializer).
    public func deliver(_ frame: Data) {
        receive(frame)
    }

    /// Opens the protocol. EXACTLY ONE of the two peers may call this.
    ///
    /// The core's `ReconcileSession` accepts a `Hello` only while it is idle,
    /// and `begin()` is what moves it out of idle. If both peers called this,
    /// each would receive the other's `Hello` in the wrong phase, the core
    /// would answer `UnexpectedFrame`, and both sessions would fail with
    /// nothing replicated. The other peer calls `answer()`.
    public func start() {
        do {
            state = .gettingLatest(name: friendlyName)
            try process(try session.begin(), terminalState: .alreadyCurrent)
        } catch {
            fail()
        }
    }

    /// The answering half: ready to receive, but does NOT open the protocol.
    ///
    /// This peer stays idle so the initiator's `Hello` lands in the one phase
    /// that accepts it. `receive` — already wired to the connection in `init` —
    /// carries the rest of the exchange, including this side's own preview and
    /// accept, so the answering peer imports too. See `start()`.
    public func answer() {
        state = .gettingLatest(name: friendlyName)
    }

    /// The person tapped "Add them". The import is applied to the store — and
    /// every app they have OPEN right now is told to re-read it.
    ///
    /// The announcement lives here, not in whichever host happens to own this
    /// coordinator, because this is the one place that knows an import was both
    /// accepted and committed. A host that forgot to wire it up would silently
    /// leave a synced change invisible on an open screen, which is precisely the
    /// bug this exists to prevent — so it is not a host's job to remember.
    ///
    /// It fires on ACCEPT and never on receipt: entries that have merely arrived
    /// are still sitting in the preview awaiting the person's yes, and are not in
    /// the store, so redrawing then would show them content they never accepted.
    /// By the time `acceptImport()` returns, the core has committed the plan.
    public func addPreviewedContent() {
        do {
            acceptedImport = true
            try process(try session.acceptImport(), terminalState: .caughtUp)
            // Announced before the state juggling below, so an open app refreshes
            // whether this accept ends the session or leaves it offering our own
            // entries back.
            AppRuntimeView.postDataChanged()
            onImportAccepted?()
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
