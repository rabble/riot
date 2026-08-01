import Foundation
import OSLog

/// A reaction's complete internal identity. The post identifier is never used in
/// UI copy, accessibility announcements, or diagnostics.
public struct ReactionKey: Hashable, Sendable {
    public let postID: String
    public let kind: ReactionKind

    public init(postID: String, kind: ReactionKind) {
        self.postID = postID
        self.kind = kind
    }
}

/// The duplicate newswire rendering that initiated an operation. Presentation
/// state is shared by `ReactionKey`; this value scopes visible copy and the
/// single accessibility announcement.
public enum ReactionSurface: String, Hashable, Sendable {
    case frontPage = "front-page"
    case openWire = "open-wire"
}

public struct ReactionWriteSnapshot: Sendable {
    public let projection: NewswireProjectionView
    public let revision: UInt64

    public init(projection: NewswireProjectionView, revision: UInt64) {
        self.projection = projection
        self.revision = revision
    }
}

public enum ReactionFailureKind: String, Sendable {
    case retryablePersistence
    case authorityOrInput
    case capacity
    case clock
    /// The community itself has not arrived on this device yet — the sidebar
    /// calls it "Not synced yet". Recoverable by waiting for a sync, so it is
    /// NOT an authority failure and must not be reported as one.
    case communityNotSynced
}

/// The only error value allowed past the writer boundary. Every field is fixed
/// app copy; raw errors, identifiers, paths, signed bytes, and post content are
/// deliberately absent.
public struct ReactionFailure: Equatable, Sendable {
    public let kind: ReactionFailureKind
    public let publicCode: String
    public let message: String

    public init(kind: ReactionFailureKind, publicCode: String, message: String) {
        self.kind = kind
        self.publicCode = publicCode
        self.message = message
    }

    init(_ error: any Error) {
        switch error as? MobileError {
        case .StoreFull, .SessionLimit:
            self.init(
                kind: .capacity,
                publicCode: "reaction_capacity",
                message: "This community can’t hold another reaction right now.")
        case .ClockUnavailable:
            self.init(
                kind: .clock,
                publicCode: "reaction_clock",
                message: "Check this device’s Date & Time before reacting.")
        case .Database, .SessionFailed:
            self.init(
                kind: .retryablePersistence,
                publicCode: "reaction_persistence",
                message: "Couldn’t save your reaction. Try again.")
        case .CommunityUnavailable:
            self.init(
                kind: .communityNotSynced,
                publicCode: "reaction_community_not_synced",
                message: "This community hasn’t reached this device yet. Exchange with a member, then react.")
        case .InvalidInput, .ObjectClosed, .StalePreview, .AppRejected,
             .Internal, .DraftNotFound, .ImportRejected, .PreviewConsumed,
             .PlanConsumed, .EntropyUnavailable, .NotSpaceOrganizer,
             .LegacyProfileCannotOrganize, .none:
            self.init(
                kind: .authorityOrInput,
                publicCode: "reaction_authority_or_input",
                message: "Reactions aren’t available for this post.")
        }
    }
}

public enum ReactionWriteResult: Sendable {
    case accepted(ReactionWriteSnapshot)
    case committedNeedsRefresh(active: Bool, revision: UInt64)
    case rejected(ReactionFailure)
    case cancelled
}

public protocol NewswireReactionWriting: Sendable {
    func setReaction(
        descriptorID: String,
        postID: String,
        kind: ReactionKind,
        active: Bool
    ) async -> ReactionWriteResult
}

public enum ReactionOperationPhase: String, Sendable {
    case persistence
    case projectionRefresh = "projection-refresh"
}

public enum ReactionFeedbackKind: Sendable {
    case rejected
    case committedNeedsRefresh
    case stalled
}

public struct ReactionFailurePresentation: Equatable, Sendable {
    public let failure: ReactionFailure
    public let surface: ReactionSurface
    public let kind: ReactionFeedbackKind

    public var message: String { failure.message }
}

public struct ReactionAnnouncement: Equatable, Sendable {
    public let sequence: UInt64
    public let surface: ReactionSurface
    public let message: String
}

public protocol ReactionDiagnosticReporting: Sendable {
    func report(_ failure: ReactionFailure, phase: ReactionOperationPhase) async
}

/// Production diagnostics receive only the stable redacted code and phase.
public struct LocalReactionDiagnosticReporter: ReactionDiagnosticReporting {
    private let logger = Logger(subsystem: "net.protest.riot", category: "newswire-reaction")

    public init() {}

    public func report(_ failure: ReactionFailure, phase: ReactionOperationPhase) async {
        logger.error("Reaction \(phase.rawValue, privacy: .public): \(failure.publicCode, privacy: .public)")
    }
}

/// An injectable two-second gate. Tests use a continuation-backed clock, so
/// duration behavior has no wall-clock sleeps.
public protocol ReactionStallClock: Sendable {
    func waitUntilStalled() async
}

public struct ContinuousReactionStallClock: ReactionStallClock {
    public init() {}

    public func waitUntilStalled() async {
        try? await Task.sleep(for: .seconds(2))
    }
}

/// Serializes the generated thread-safe profile handle away from `MainActor`.
/// It owns no mutable repository state.
public actor MobileProfileReactionWriter: NewswireReactionWriting {
    private let profile: MobileProfile
    private var revision: UInt64 = 0
    #if DEBUG
    private let uiTestConfiguration: ReactionUITestConfiguration?
    #endif

    public init(profile: MobileProfile) {
        self.profile = profile
        #if DEBUG
        uiTestConfiguration = nil
        #endif
    }

    #if DEBUG
    public init(
        profile: MobileProfile,
        uiTestConfiguration: ReactionUITestConfiguration
    ) {
        self.profile = profile
        self.uiTestConfiguration = uiTestConfiguration
    }
    #endif

    public func setReaction(
        descriptorID: String,
        postID: String,
        kind: ReactionKind,
        active: Bool
    ) async -> ReactionWriteResult {
        guard !Task.isCancelled else { return .cancelled }

        #if DEBUG
        if let uiTestConfiguration {
            if uiTestConfiguration.reactionDelayMilliseconds > 0 {
                do {
                    try await Task.sleep(for: .milliseconds(
                        Int64(uiTestConfiguration.reactionDelayMilliseconds)
                    ))
                } catch {
                    return .cancelled
                }
            }
            guard !Task.isCancelled else { return .cancelled }
            if let failure = uiTestConfiguration.reactionMode.preCommitFailure {
                return .rejected(failure)
            }
        }
        #endif

        do {
            _ = try profile.toggleNewswireReaction(
                spaceDescriptorEntryId: descriptorID,
                parentEntryId: postID,
                kind: kind.rawValue,
                active: active)
        } catch {
            return .rejected(ReactionFailure(error))
        }

        revision &+= 1
        let completedRevision = revision
        do {
            let projection = try profile.projectNewswireSpace(
                spaceDescriptorEntryId: descriptorID)
            return .accepted(ReactionWriteSnapshot(
                projection: projection,
                revision: completedRevision))
        } catch {
            return .committedNeedsRefresh(
                active: active,
                revision: completedRevision)
        }
    }
}
