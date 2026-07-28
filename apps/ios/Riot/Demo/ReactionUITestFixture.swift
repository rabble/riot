import Foundation

#if DEBUG
public enum ReactionUITestFixtureError: Error, Equatable {
    case syncFailed
    case missingOutboundFrame
    case incompleteExchange
    case transferLimitExceeded(limit: Int)
    case projectionMissingPost
}

struct ReactionUITestPair {
    let reader: RiotProfileRepository
    let descriptorEntryID: String
    let postEntryID: String
}

/// A bounded in-memory wire between two real generated sync sessions.
public struct ReactionFixtureSyncPump {
    public let maxTransfers: Int

    public init(maxTransfers: Int = 64) {
        precondition(maxTransfers > 0)
        self.maxTransfers = maxTransfers
    }

    @discardableResult
    public func run(
        author: MobileSyncSessionBoundary,
        reader: MobileSyncSessionBoundary
    ) throws -> Int {
        enum Side: Hashable {
            case author
            case reader

            var opposite: Side { self == .author ? .reader : .author }
        }

        func boundary(_ side: Side) -> MobileSyncSessionBoundary {
            side == .author ? author : reader
        }

        defer {
            try? author.close()
            try? reader.close()
        }

        var transfers = 0
        var terminal: Set<Side> = []
        var pending: [(Side, NearbySyncOutcome)] = [(.author, try author.begin())]

        while let (side, outcome) = pending.first {
            pending.removeFirst()
            switch outcome {
            case let .sendMore(isTerminal):
                guard transfers < maxTransfers else {
                    throw ReactionUITestFixtureError.transferLimitExceeded(limit: maxTransfers)
                }
                guard let frame = try boundary(side).nextOutbound() else {
                    throw ReactionUITestFixtureError.missingOutboundFrame
                }
                transfers += 1
                let receiver = side.opposite
                let received = try boundary(receiver).receive(frame)
                if isTerminal {
                    terminal.insert(side)
                }
                pending.append((receiver, received))

            case .readyToPreview:
                pending.append((side, try boundary(side).acceptImport()))

            case .done:
                terminal.insert(side)

            case .failed:
                throw ReactionUITestFixtureError.syncFailed
            }

            if terminal.count == 2 {
                return transfers
            }
        }

        throw ReactionUITestFixtureError.incompleteExchange
    }
}

public enum ReactionUITestReactionMode: String, Equatable, Sendable {
    case success
    case retryable
    case authority
    case capacity
    case clock
    case stall

    public var preCommitFailure: ReactionFailure? {
        switch self {
        case .success, .stall:
            nil
        case .retryable:
            ReactionFailure(
                kind: .retryablePersistence,
                publicCode: "reaction_persistence",
                message: "Couldn’t save your reaction. Try again."
            )
        case .authority:
            ReactionFailure(
                kind: .authorityOrInput,
                publicCode: "reaction_authority_or_input",
                message: "Reactions aren’t available for this post."
            )
        case .capacity:
            ReactionFailure(
                kind: .capacity,
                publicCode: "reaction_capacity",
                message: "This community can’t hold another reaction right now."
            )
        case .clock:
            ReactionFailure(
                kind: .clock,
                publicCode: "reaction_clock",
                message: "Check this device’s Date & Time before reacting."
            )
        }
    }
}

public struct ReactionUITestConfiguration: Equatable, Sendable {
    public let runID: UUID
    public let reactionMode: ReactionUITestReactionMode
    public let reactionDelayMilliseconds: UInt64
    public let windowWidth: Double?

    public init(
        runID: UUID,
        reactionMode: ReactionUITestReactionMode,
        reactionDelayMilliseconds: UInt64,
        windowWidth: Double?
    ) {
        self.runID = runID
        self.reactionMode = reactionMode
        self.reactionDelayMilliseconds = reactionDelayMilliseconds
        self.windowWidth = windowWidth
    }
}

public enum ReactionUITestLaunchState: Equatable, Sendable {
    case inactive
    case invalid
    case valid(ReactionUITestConfiguration)
}

/// Closed, fail-closed parser for the reactions fixture.
public enum ReactionUITestEnvironment {
    public static func resolve(
        _ environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> ReactionUITestLaunchState {
        guard let fixture = environment["RIOT_UI_TEST_FIXTURE"] else {
            return .inactive
        }
        guard fixture == "reactions-joined",
              let rawRunID = environment["RIOT_UI_TEST_RUN_ID"],
              let runID = UUID(uuidString: rawRunID)
        else {
            return .invalid
        }

        let mode: ReactionUITestReactionMode
        if let rawMode = environment["RIOT_UI_TEST_REACTION_MODE"] {
            guard let parsed = ReactionUITestReactionMode(rawValue: rawMode) else {
                return .invalid
            }
            mode = parsed
        } else {
            mode = .success
        }

        let delay: UInt64
        if let rawDelay = environment["RIOT_UI_TEST_REACTION_DELAY_MS"] {
            guard let parsed = UInt64(rawDelay), parsed <= 5_000 else {
                return .invalid
            }
            delay = parsed
        } else {
            delay = 0
        }

        let width: Double?
        if let rawWidth = environment["RIOT_UI_TEST_WINDOW_WIDTH"] {
            guard rawWidth == "900" || rawWidth == "1200" else {
                return .invalid
            }
            width = Double(rawWidth)
        } else {
            width = nil
        }

        return .valid(ReactionUITestConfiguration(
            runID: runID,
            reactionMode: mode,
            reactionDelayMilliseconds: delay,
            windowWidth: width
        ))
    }
}

/// Stable in-memory wrapping key for UUID-gated automation only.
public struct UIAutomationWrappingKeyStore: WrappingKeyStore {
    public let runID: UUID

    public init(runID: UUID) {
        self.runID = runID
    }

    public func loadOrCreateWrappingKey() throws -> Data {
        Data((runID.uuidString + runID.uuidString).utf8.prefix(32))
    }
}
#endif
