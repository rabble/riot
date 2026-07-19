import SwiftUI

// MARK: - Editorial action kind (app-side mirror)

/// The six editorial actions, mirrored on the app side so the closed field
/// table, the labels, and the pre-signing review can be reasoned about without
/// dragging the FFI enum through every value type. `toFFI` is the ONLY bridge to
/// `NewswireEditorialActionKind`; nothing else constructs the FFI kind, so the
/// mapping is checked in one place.
public enum EditorialActionKind: String, CaseIterable, Equatable, Sendable, Identifiable {
    case feature
    case verify
    case correct
    case hide
    case tombstone
    case retract

    public var id: String { rawValue }

    /// The person-facing verb shown on the action control and the review.
    public var label: String {
        switch self {
        case .feature: "Feature"
        case .verify: "Verify"
        case .correct: "Correct"
        case .hide: "Hide"
        case .tombstone: "Safety tombstone"
        case .retract: "Retract"
        }
    }

    public func toFFI() -> NewswireEditorialActionKind {
        switch self {
        case .feature: .feature
        case .verify: .verify
        case .correct: .correct
        case .hide: .hide
        case .tombstone: .tombstone
        case .retract: .retract
        }
    }
}

// MARK: - Closed editorial field table

/// Whether a field is forbidden or required for a given action — the closed
/// contract from the newswire design's field table. There is no "optional": each
/// of the two fields is exactly one of these for each of the six kinds.
public enum EditorialFieldRequirement: Equatable, Sendable {
    case forbidden
    case required
}

/// The two-field rule for one action. This is the app-side statement of the
/// newswire design's closed field table:
///
/// | Action                    | Reason            | Correction text   |
/// | feature, verify           | forbidden         | forbidden         |
/// | correct                   | required non-empty| required non-empty|
/// | hide, tombstone, retract  | required non-empty| forbidden         |
///
/// Core enforces the same table on the wire; this is the surface refusing to
/// build a malformed action in the first place, so a reader never signs one.
public struct EditorialFieldRules: Equatable, Sendable {
    public let reason: EditorialFieldRequirement
    public let correctionText: EditorialFieldRequirement
}

public extension EditorialActionKind {
    /// The closed field rule for this kind. Frozen by construction: any new kind
    /// must add a row here or the switch stops compiling.
    var fieldRules: EditorialFieldRules {
        switch self {
        case .feature, .verify:
            EditorialFieldRules(reason: .forbidden, correctionText: .forbidden)
        case .correct:
            EditorialFieldRules(reason: .required, correctionText: .required)
        case .hide, .tombstone, .retract:
            EditorialFieldRules(reason: .required, correctionText: .forbidden)
        }
    }

    /// `correct` alone carries the mandatory "Editorial correction" label so it can
    /// never be mistaken for an author's own revision (newswire design: editorial
    /// action flow). No other kind labels itself a correction.
    var isEditorialCorrection: Bool { self == .correct }
}

/// The mandatory label a correction renders with. A constant so the view and the
/// test pin the exact string once.
public enum EditorialCorrectionLabel {
    public static let text = "Editorial correction"
}

/// Why a drafted action is not yet signable. Each case names the ONE field that
/// violates the closed table, so the composer can point at it precisely.
public enum EditorialFieldViolation: String, Equatable, Sendable, Error {
    case reasonForbidden
    case reasonRequired
    case correctionForbidden
    case correctionRequired

    /// A plain-language explanation shown under the offending field — never a raw
    /// enum name.
    public var message: String {
        switch self {
        case .reasonForbidden: "This action does not take a reason."
        case .reasonRequired: "A reason is required for this action."
        case .correctionForbidden: "This action does not take replacement text."
        case .correctionRequired: "Replacement text is required for a correction."
        }
    }
}

/// A drafted-but-not-yet-validated editorial action. Free-text, exactly as a
/// person typed it, so a failed sign can preserve it verbatim.
public struct EditorialActionDraft: Equatable, Sendable {
    public var kind: EditorialActionKind
    public var reason: String
    public var correctionText: String

    public init(kind: EditorialActionKind, reason: String = "", correctionText: String = "") {
        self.kind = kind
        self.reason = reason
        self.correctionText = correctionText
    }

    /// The draft is empty when nothing has been typed for the current kind — used
    /// to decide whether leaving needs a Stay-or-Discard confirmation.
    public var isEmpty: Bool {
        reason.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && correctionText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
}

/// A draft that passed the closed field table: the normalized (trimmed) fields
/// ready to hand to the FFI, plus the `nil`s a forbidden field must become. This
/// is the ONLY value the model will sign — an invalid draft never reaches core.
public struct ValidatedEditorialAction: Equatable, Sendable {
    public let kind: EditorialActionKind
    /// The reason, `nil` when the action forbids one, trimmed when it requires one.
    public let reason: String?
    /// The replacement text, `nil` when the action forbids one.
    public let correctionText: String?
}

/// Validates a draft against the closed field table. Pure, deterministic, and
/// shared by both platforms' surfaces (the Kotlin twin is `EditorialFieldTable`).
/// Non-empty means non-empty AFTER trimming, so a reason of spaces is no reason.
public enum EditorialActionValidator {
    public static func validate(
        _ draft: EditorialActionDraft
    ) -> Result<ValidatedEditorialAction, EditorialFieldViolation> {
        let reason = draft.reason.trimmingCharacters(in: .whitespacesAndNewlines)
        let correction = draft.correctionText.trimmingCharacters(in: .whitespacesAndNewlines)
        let rules = draft.kind.fieldRules

        switch rules.reason {
        case .forbidden where !reason.isEmpty:
            return .failure(.reasonForbidden)
        case .required where reason.isEmpty:
            return .failure(.reasonRequired)
        default:
            break
        }
        switch rules.correctionText {
        case .forbidden where !correction.isEmpty:
            return .failure(.correctionForbidden)
        case .required where correction.isEmpty:
            return .failure(.correctionRequired)
        default:
            break
        }

        return .success(ValidatedEditorialAction(
            kind: draft.kind,
            reason: rules.reason == .forbidden ? nil : reason,
            correctionText: rules.correctionText == .forbidden ? nil : correction
        ))
    }
}

// MARK: - Pre-signing review (immutable)

/// The immutable review a person sees before signing an editorial action
/// (newswire design: "Its immutable review shows the complete target entry ID,
/// community, acting editor key, action, reason, and replacement text before
/// signing."). Every field is `let`: once built, it is exactly what will be
/// signed, so the review can never drift from the action. Full identifiers are
/// shown UNTRUNCATED — this is the signing surface, where truncation would be a
/// security defect.
public struct EditorialActionReview: Equatable, Sendable {
    /// The complete 32-byte target entry id, hex, never truncated.
    public let targetEntryID: String
    /// The community this acts within, by name (its descriptor id is the parent).
    public let communityName: String
    /// The complete acting editor key, hex, never truncated.
    public let actingEditorKeyHex: String
    public let kind: EditorialActionKind
    /// The reason, present exactly when the action carries one.
    public let reason: String?
    /// The replacement text, present exactly for a correction.
    public let replacementText: String?

    public init(
        action: ValidatedEditorialAction,
        targetEntryID: String,
        communityName: String,
        actingEditorKeyHex: String
    ) {
        self.targetEntryID = targetEntryID
        self.communityName = communityName
        self.actingEditorKeyHex = actingEditorKeyHex
        self.kind = action.kind
        self.reason = action.reason
        self.replacementText = action.correctionText
    }

    /// The ordered label/value rows the review renders — each identifier complete.
    public var rows: [(label: String, value: String)] {
        var out: [(String, String)] = [
            ("Action", kind.label),
            ("Community", communityName),
            ("Target entry", targetEntryID),
            ("Acting editor", actingEditorKeyHex),
        ]
        if let reason { out.append(("Reason", reason)) }
        if let replacementText { out.append(("Replacement text", replacementText)) }
        return out
    }
}

// MARK: - Signing seam

/// The one call the editorial surface makes to sign an action.
/// `RiotProfileRepository` conforms (its Newswire extension); tests inject a stub
/// that mimics core's roster rejection (throws) so the surface's fail-closed
/// behaviour is provable without the store, AND the real repository is exercised
/// end-to-end in the FFI tests so the rejection is genuinely core's.
public protocol NewswireEditorialActing {
    @discardableResult
    func createNewswireEditorialAction(
        spaceDescriptorEntryID: String,
        targetEntryID: String,
        kind: NewswireEditorialActionKind,
        reason: String?,
        correctionText: String?
    ) throws -> NewswireSignedRecord
}

/// The one call the surface makes to sign a communal reply. A reply is communal
/// — core admits it for any member of the space, not only editors — so unlike
/// `NewswireEditorialActing` there is no roster gate here; a member who is not
/// followed yet, or whose parent post is not held, simply has their reply
/// dropped from the projection. `RiotProfileRepository` conforms; tests inject a
/// stub, and the real repository is exercised end-to-end in the FFI tests.
public protocol NewswireCommenting {
    @discardableResult
    func createNewswireComment(
        spaceDescriptorEntryID: String,
        parentEntryID: String,
        body: String,
        language: String
    ) throws -> NewswireSignedRecord
}

/// The one read the editorial surface makes to decide whether to OFFER a control:
/// core's descriptor-authenticated roster answer, identical to the authority core
/// enforces at admission (Unit 4a's shared `is_editorial_authority`). An unknown /
/// not-yet-synced descriptor answers `false` (never throws) so the surface can
/// render a "controls appear after first sync" note off a defined false. UI
/// VISIBILITY only — core still rejects a non-editor's action at signing regardless
/// of what this returns.
public protocol NewswireEditorAuthorityChecking {
    func newswireIsEditor(spaceDescriptorEntryID: String, subjectID: String) throws -> Bool
}

// MARK: - Post treatment display

/// How one projected post renders under active editorial actions. Read straight
/// from core's `treatment` — the surface never re-derives it, so every platform
/// shows the same treatment for the same records.
public enum NewswirePostDisplay: Equatable, Sendable {
    /// Body visible.
    case ordinary
    /// An active hide: body removed from the list; a warning interstitial stands in
    /// its place, through which a reader can still inspect the original and the
    /// signed actions (newswire design: ordinary hide).
    case hiddenInterstitial
    /// An active safety tombstone: payload suppressed; only id, author, timestamps,
    /// signer, reason, and history remain.
    case tombstoned

    public static func from(_ treatment: NewswirePostTreatment) -> NewswirePostDisplay {
        switch treatment {
        case .ordinary: .ordinary
        case .hidden: .hiddenInterstitial
        case .tombstoned: .tombstoned
        }
    }
}

/// The fixed copy the two redaction treatments show in place of the payload.
public enum NewswireTreatmentCopy {
    public static let hiddenTitle = "Hidden by the editorial collective"
    public static let hiddenBody =
        "The collective hid this report. You can still inspect the original and the signed actions."
    public static let tombstoneTitle = "Removed for safety"
    public static let tombstoneBody =
        "The collective tombstoned this report. Its content is withheld; the signed record of the act remains."
}

/// One post, ready to draw on the front page or open wire. Every field comes from
/// core's projection; the surface only re-shapes, never re-decides. A hidden or
/// tombstoned post arrives with `headline == nil`, so the row draws the treatment
/// copy instead of the payload.
public struct NewswirePostRow: Equatable, Identifiable, Sendable {
    public let id: String
    public let author: String
    public let authorKeyHex: String
    public let headline: String?
    public let display: NewswirePostDisplay
    /// True when core reports one or more active corrections — the row shows the
    /// mandatory "Editorial correction" label and links to the full history.
    public let hasCorrection: Bool
    public let verificationCount: Int
    public let aiAssisted: Bool

    public init(_ post: NewswireProjectedPost) {
        self.id = post.entryId
        self.author = post.author.rendered
        self.authorKeyHex = post.author.id
        self.headline = post.headline
        self.display = .from(post.treatment)
        self.hasCorrection = !post.correctionIds.isEmpty
        self.verificationCount = post.verificationIds.count
        self.aiAssisted = post.aiAssisted
    }
}

/// One communal reply, ready to draw indented under its parent post. Every field
/// comes from core's projection; the surface only re-shapes, never re-decides. A
/// hidden or tombstoned reply arrives with `body == nil`, so the row draws the
/// treatment copy instead of the words — the same redaction contract as a post.
public struct NewswireCommentRow: Equatable, Identifiable, Sendable {
    public let id: String
    /// The post this reply hangs under. The surface groups the flat list core
    /// returns by this id; core already dropped any reply with no held parent.
    public let parentID: String
    public let author: String
    public let authorKeyHex: String
    public let body: String?
    public let display: NewswirePostDisplay

    public init(_ comment: NewswireProjectedComment) {
        self.id = comment.entryId
        self.parentID = comment.parentEntryId
        self.author = comment.author.rendered
        self.authorKeyHex = comment.author.id
        self.body = comment.body
        self.display = .from(comment.treatment)
    }
}

/// The result of attempting to post a communal reply from the surface.
public enum NewswireCommentOutcome: Equatable, Sendable {
    /// Signed and committed locally; pending exchange with peers.
    case posted(entryID: String)
    /// The reply was empty after trimming — nothing was sent to core.
    case empty
    /// No commenter is wired (a preview/test construction) — the affordance is
    /// hidden in that case, so this is a defensive answer, never a user path.
    case unavailable
    /// Core refused to sign (closed store or clock). The draft is preserved so
    /// the person loses nothing.
    case rejected
}

/// One editorial act, ready to draw in the always-public Editorial history. Both
/// an active feature and the retraction that undid it appear here; `isActive`
/// distinguishes them. A correction's replacement text is redacted by core when
/// its target is hidden/tombstoned, so `replacementText` may be `nil` even for a
/// correction.
public struct EditorialHistoryRow: Equatable, Identifiable, Sendable {
    public let id: String
    public let signer: String
    public let kind: EditorialActionKind
    public let targetEntryID: String
    public let reason: String?
    public let replacementText: String?
    public let isActive: Bool

    public init(_ action: NewswireProjectedEditorialAction) {
        self.id = action.entryId
        self.signer = action.signer.rendered
        self.kind = EditorialActionKind.from(action.kind)
        self.targetEntryID = action.targetEntryId
        self.reason = action.reason
        self.replacementText = action.correctionText
        self.isActive = action.active
    }

    /// The mandatory correction label, present only for a correction so it can
    /// never read as an author edit.
    public var correctionLabel: String? {
        kind.isEditorialCorrection ? EditorialCorrectionLabel.text : nil
    }
}

extension EditorialActionKind {
    static func from(_ kind: NewswireEditorialActionKind) -> EditorialActionKind {
        switch kind {
        case .feature: .feature
        case .verify: .verify
        case .correct: .correct
        case .hide: .hide
        case .tombstone: .tombstone
        case .retract: .retract
        }
    }
}

// MARK: - Wire state (three DISTINCT empty states)

/// What the newswire surface is showing. The three non-featured states are
/// deliberately DISTINCT (newswire design + plan §4.7): an empty wire, a wire with
/// posts but no collective feature, and an offline/stale projection are three
/// different truths and must never collapse into one generic empty view.
///
/// Front page and open wire are read straight from core's already-split
/// projection — the surface never re-orders or re-selects, so every platform
/// derives the identical views. The only decision made here is WHICH of the four
/// states holds, a pure function of whether the wire and the front page are empty.
public enum NewswireWireState: Equatable, Sendable {
    /// The projection is unavailable or this device has no descriptor id for the
    /// community — offline or not yet re-hydrated (Risk 11), never fabricated
    /// content.
    case offlineStale
    /// No posts have arrived at all.
    case emptyWire
    /// The open wire has posts, but the collective has featured none — link to the
    /// open wire (newswire design).
    case postsButNoFeature(openWire: [NewswirePostRow])
    /// The collective has featured at least one post — the front page and the full
    /// open wire.
    case featured(frontPage: [NewswirePostRow], openWire: [NewswirePostRow])

    /// Builds the wire state from a core projection. Reads `frontPage`/`openWire`
    /// verbatim; the only logic is the empty-state selection.
    public static func from(_ projection: NewswireProjectionView) -> NewswireWireState {
        let openWire = projection.openWire.map(NewswirePostRow.init)
        let frontPage = projection.frontPage.map(NewswirePostRow.init)
        if openWire.isEmpty {
            return .emptyWire
        }
        if frontPage.isEmpty {
            return .postsButNoFeature(openWire: openWire)
        }
        return .featured(frontPage: frontPage, openWire: openWire)
    }

    /// The stable accessibility identifier for the state's primary view, so the
    /// three empty states are individually addressable and never conflated.
    public var accessibilityID: String {
        switch self {
        case .offlineStale: "newswire-offline-stale"
        case .emptyWire: "newswire-empty-wire"
        case .postsButNoFeature: "newswire-no-feature"
        case .featured: "newswire-featured"
        }
    }
}

/// The fixed copy for the three non-featured states — pinned once so the design's
/// distinct-messages requirement is testable directly.
public enum NewswireWireCopy {
    public static let emptyTitle = "No reports yet"
    public static let emptyMessage = "No reports have arrived on this wire yet."
    public static let noFeatureTitle = "Nothing featured yet"
    public static let noFeatureMessage =
        "The collective has not selected a feature. See the open wire for every report."
    public static let offlineTitle = "Updates unavailable"
    public static let offlineMessage =
        "This community's wire is offline or has not synced yet. What you already have is still here."
    public static let pendingSyncTitle = "Waiting for the first sync"
    public static let pendingSyncMessage =
        "You've joined this community, but no posts have arrived yet. They appear once a peer or seed connects. Rejoin with a link, or sync with a peer nearby."
}

/// A forward action a wire state offers, as pure data. The view maps each kind
/// to a handler; the model decides WHICH are offered and IN WHAT ORDER. Modeled
/// as data (never an inline `(label, {})` closure) so a test can assert every
/// terminal state has at least one reachable next action, none is a dead no-op,
/// and the red-on-main Nearby path is never a state's headline action.
public enum NewswireWireForwardAction: String, Equatable, Sendable, CaseIterable {
    /// Re-derive the descriptor id and reproject — a known-descriptor community
    /// that is transiently offline.
    case retry
    /// The empty wire's call to the composer (routed through `onPostUpdate`).
    case postFirstUpdate
    /// The verified-working join path (Unit 1's `JoinByReferenceSheet`) — the
    /// pending-first-sync headline.
    case rejoinWithLink
    /// Nearby — offered but SECONDARY, never a headline: two-peer nearby sync is
    /// red on main, so it must never be a state's first action.
    case syncWithPeer

    public var label: String {
        switch self {
        case .retry: "Try again"
        case .postFirstUpdate: "Post the first update"
        case .rejoinWithLink: "Rejoin with a link"
        case .syncWithPeer: "Sync with a peer"
        }
    }

    /// A stable per-action id so each forward button is individually addressable.
    public var accessibilityID: String { "newswire-action-\(rawValue)" }

    /// True for the red-on-main Nearby path. A state must NEVER place this first.
    public var isNearbyPath: Bool { self == .syncWithPeer }
}

// MARK: - Sign outcome

/// The result of attempting to sign an editorial action from the surface.
public enum EditorialSignOutcome: Equatable, Sendable {
    /// Signed and committed locally; pending exchange with peers.
    case signed(entryID: String)
    /// The draft violated the closed field table — nothing was sent to core.
    case invalid(EditorialFieldViolation)
    /// Core refused to sign (roster rejection, closed store, or clock). The draft
    /// is preserved so the person loses nothing.
    case rejected
}

// MARK: - Model

/// Drives the newswire surface for one community: loads the collective projection
/// into the three distinct states, exposes the editorial history to every reader,
/// and — only when this profile is a recognized editor — validates and signs
/// editorial actions.
///
/// Two authorization facts hold here at once and are kept independent:
///   1. `canOfferEditorialControls` decides whether to SHOW an action control.
///   2. `sign` hands the action to core, which REJECTS a non-editor regardless of
///      what the UI showed. A failed sign preserves the draft.
@MainActor
public final class NewswireSurfaceModel: ObservableObject {
    @Published public private(set) var wire: NewswireWireState
    @Published public private(set) var history: [EditorialHistoryRow]
    /// The last sign outcome, so the view can show Pending / an error inline.
    @Published public private(set) var lastOutcome: EditorialSignOutcome?
    /// The current editorial-action draft. Preserved verbatim across a rejected
    /// sign so a person never loses their words.
    @Published public var draft: EditorialActionDraft

    /// The unread / what's-new state for this community, recomputed on every
    /// `load()` against the per-device seen cursor. Drives the Home tab badge, the
    /// "N new since you last looked" delta, and the per-row new dot. `.none` (zero)
    /// whenever there is no cursor store, no descriptor, or an offline projection.
    @Published public private(set) var unread: NewswireUnread = .none

    /// Communal replies for the space, grouped under each parent post's entry id
    /// and kept in the flat, time-sorted order core returns them in. Repopulated
    /// on every `load()`; empty whenever the projection is unavailable.
    @Published public private(set) var commentsByParent: [String: [NewswireCommentRow]] = [:]

    private let projector: NewswireProjecting
    private let editor: NewswireEditorialActing
    private let authority: NewswireEditorAuthorityChecking
    /// The communal-reply signer. `nil` in preview/test constructions that never
    /// exercise replying — the reply affordance is then hidden by `canComment`.
    private let commenter: NewswireCommenting?
    private var spaceDescriptorEntryID: String
    private let myKeyHex: String
    public let communityName: String

    /// Re-derives the community's descriptor id on demand (the shell wires this to
    /// `RiotAppModel.rederivedNewswireDescriptorID()` — the same `listCommunities()`
    /// derivation `reload()` uses). `nil` in tests/constructions that never need it.
    private let descriptorResolver: (() -> String?)?

    /// The per-device seen-cursor store. `nil` in tests/constructions that never
    /// exercise unread — the unread state then stays `.none`, never a crash.
    private let seenCursor: SeenCursorStore?

    /// Whether core recognizes this profile as an editor of this descriptor — read
    /// from the FFI predicate in `load()`, never a locally-asserted roster. `false`
    /// until loaded and `false` for an unknown / not-yet-synced descriptor (no
    /// error), by construction.
    @Published public private(set) var isEditor: Bool = false

    /// Set by `load()`: true when a descriptor id is in hand (retry can reproject),
    /// false when none is derivable (a nearby-joined community — the wire must offer
    /// a forward path, not the silent .retry re-loop). Drives `forwardActions`.
    private var descriptorRecoverable = false

    public init(
        projector: NewswireProjecting,
        editor: NewswireEditorialActing,
        authority: NewswireEditorAuthorityChecking,
        spaceDescriptorEntryID: String,
        communityName: String,
        myKeyHex: String,
        initialDraftKind: EditorialActionKind = .feature,
        descriptorResolver: (() -> String?)? = nil,
        seenCursor: SeenCursorStore? = nil,
        commenter: NewswireCommenting? = nil
    ) {
        self.projector = projector
        self.editor = editor
        self.authority = authority
        self.commenter = commenter
        self.spaceDescriptorEntryID = spaceDescriptorEntryID
        self.communityName = communityName
        self.myKeyHex = myKeyHex
        self.descriptorResolver = descriptorResolver
        self.seenCursor = seenCursor
        self.wire = .offlineStale
        self.history = []
        self.draft = EditorialActionDraft(kind: initialDraftKind)
    }

    /// Whether the surface may OFFER a reply affordance. A reply is communal, so
    /// this is independent of `isEditor` — it needs only a wired commenter and a
    /// descriptor to reply within. Core still decides admission at signing time.
    public var canComment: Bool {
        commenter != nil && !spaceDescriptorEntryID.isEmpty
    }

    /// The replies to draw under `postID`, flat and time-sorted as core returned
    /// them. Empty when the post has none — never `nil`, so the view never branches.
    public func comments(under postID: String) -> [NewswireCommentRow] {
        commentsByParent[postID] ?? []
    }

    /// Signs a communal reply to `parentEntryID` and, on success, reloads so the
    /// reply appears under its post. An empty draft never reaches core. A core
    /// refusal preserves nothing to lose (the caller keeps the text). The
    /// language is fixed to the post model's minimum-valid tag for v1 — the
    /// compose sheet does not yet ask for one.
    @discardableResult
    public func submitComment(
        parentEntryID: String,
        body: String,
        language: String = "en"
    ) -> NewswireCommentOutcome {
        guard !body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return .empty
        }
        guard let commenter else { return .unavailable }
        do {
            let record = try commenter.createNewswireComment(
                spaceDescriptorEntryID: spaceDescriptorEntryID,
                parentEntryID: parentEntryID,
                body: body,
                language: language
            )
            load()
            return .posted(entryID: record.entryId)
        } catch {
            return .rejected
        }
    }

    /// Groups core's flat, already-sorted comment list under each parent post's
    /// entry id, preserving order — the surface never re-sorts.
    static func groupComments(_ comments: [NewswireProjectedComment]) -> [String: [NewswireCommentRow]] {
        var byParent: [String: [NewswireCommentRow]] = [:]
        for comment in comments {
            byParent[comment.parentEntryId, default: []].append(NewswireCommentRow(comment))
        }
        return byParent
    }

    /// Whether the surface may OFFER an editorial control — UI visibility only.
    /// A `false` here hides the control; it does NOT authorize anything, and a
    /// `true` here does not either (core still decides at sign time). Reads the
    /// cached predicate answer resolved in `load()`, the SAME roster authority core
    /// enforces at admission.
    public var canOfferEditorialControls: Bool {
        !spaceDescriptorEntryID.isEmpty && isEditor
    }

    /// The ordered forward actions the current wire state offers — pure over the
    /// model's loaded state. `postsButNoFeature`/`featured` return `[]` because the
    /// next action is the visible content itself (the open wire renders directly
    /// below, already labeled "Open wire" — the redundant no-op button is gone).
    public var forwardActions: [NewswireWireForwardAction] {
        switch wire {
        case .offlineStale:
            // A known descriptor that is merely offline → Try again; a community
            // with no derivable descriptor (nearby-joined / pending first sync) →
            // a real forward path, never a silent re-loop, verified path first.
            return descriptorRecoverable ? [.retry] : [.rejoinWithLink, .syncWithPeer]
        case .emptyWire:
            return [.postFirstUpdate]
        case .postsButNoFeature, .featured:
            return []
        }
    }

    /// The offlineStale title: the honest pending-first-sync headline when no
    /// descriptor is derivable, the transient-offline headline when one is in hand.
    public var offlineTitle: String {
        descriptorRecoverable ? NewswireWireCopy.offlineTitle : NewswireWireCopy.pendingSyncTitle
    }
    /// The offlineStale message, matched to the title.
    public var offlineMessage: String {
        descriptorRecoverable ? NewswireWireCopy.offlineMessage : NewswireWireCopy.pendingSyncMessage
    }

    /// The one honest line shown where a control would be, when this profile is not
    /// (yet) an editor AND the descriptor has not projected (a joined community
    /// before its first sync — the predicate can't tell "not synced" from "not a
    /// member", so the note is scoped to the offline/stale state to avoid telling a
    /// *synced* reader they will gain controls). `nil` for an editor, for a synced
    /// non-editor, and when there is no descriptor id at all — never a bare empty
    /// view for the pre-sync editor.
    public var editorialControlsPendingNote: String? {
        guard !spaceDescriptorEntryID.isEmpty, !isEditor, wire == .offlineStale else { return nil }
        return "Editorial controls appear after this community's first sync."
    }

    /// Loads the collective projection. A missing descriptor id or a projection
    /// failure is the offline/stale state — never a fabricated empty wire.
    public func load() {
        // A descriptor may have landed since this model was built (a switched or
        // joined community whose registry row now carries one; the shell built this
        // model with "" before the sync). Picking it up here is what turns a silent
        // offlineStale re-loop into a real projection — and it must precede the
        // editor-status read below so the predicate answers off the re-derived id.
        if spaceDescriptorEntryID.isEmpty, let resolved = descriptorResolver?(), !resolved.isEmpty {
            spaceDescriptorEntryID = resolved
        }
        // Editor status is core's descriptor answer, resolved once per load. An
        // unknown / not-yet-synced descriptor (or a closed profile) answers false —
        // never a throw here.
        isEditor = (try? authority.newswireIsEditor(
            spaceDescriptorEntryID: spaceDescriptorEntryID, subjectID: myKeyHex)) ?? false

        guard !spaceDescriptorEntryID.isEmpty else {
            // No descriptor to project — the forward-path state (rejoin / sync),
            // never invented content, never a silent retry.
            wire = .offlineStale
            history = []
            commentsByParent = [:]
            unread = .none
            descriptorRecoverable = false
            return
        }
        do {
            let projection = try projector.projectNewswire(
                spaceDescriptorEntryID: spaceDescriptorEntryID
            )
            wire = .from(projection)
            history = projection.editorialHistory.map(EditorialHistoryRow.init)
            commentsByParent = Self.groupComments(projection.comments)
            // Unread is a per-device read against the seen cursor for THIS
            // descriptor — a pure function of what the projection now shows and
            // how far the reader had caught up. Recomputed here (never mutated by
            // marking seen, so the delta survives the current visit).
            unread = NewswireUnread(
                posts: seenRefs(from: projection),
                cursor: seenCursor?.cursor(forCommunity: spaceDescriptorEntryID))
            descriptorRecoverable = true
        } catch {
            // We hold a descriptor id but it is transiently offline — retry can
            // reproject the id we already have. Fixed, honest state — never a raw
            // internal error, never invented content.
            wire = .offlineStale
            history = []
            commentsByParent = [:]
            unread = .none
            descriptorRecoverable = true
        }
    }

    /// The de-duplicated set of posts the projection is showing, as the minimal
    /// `SeenPostRef` the unread math needs. Front-page and open-wire may overlap
    /// (a featured post is still on the wire); keying by entry id counts each post
    /// once so the unread total matches what the reader can actually see.
    private func seenRefs(from projection: NewswireProjectionView) -> [SeenPostRef] {
        var byID: [String: SeenPostRef] = [:]
        for post in projection.openWire + projection.frontPage {
            byID[post.entryId] = SeenPostRef(
                entryID: post.entryId, taiJ2000Micros: post.taiJ2000Micros)
        }
        return Array(byID.values)
    }

    /// Advance the seen cursor to the newest post currently shown — the reader has
    /// looked. Deliberately does NOT recompute `unread`, so the "N new" delta stays
    /// visible for the rest of this visit; the NEXT `load()` reads the advanced
    /// cursor and reports zero. A no-op when nothing is shown or no cursor store is
    /// wired, and monotonic in the store, so it can never mark seen unseen content.
    public func markAllSeen() {
        guard let latest = unread.latestTimestamp else { return }
        seenCursor?.advance(community: spaceDescriptorEntryID, to: latest)
    }

    /// The offlineStale "Try again" action: re-derive + reproject. A no-op if the
    /// community still has no derivable descriptor (the view then shows the forward
    /// paths, not this button).
    public func retry() { load() }

    /// Builds the immutable pre-signing review for the current draft against a
    /// target, or the field violation that stops it. Pure: no signing happens.
    public func review(targetEntryID: String) -> Result<EditorialActionReview, EditorialFieldViolation> {
        EditorialActionValidator.validate(draft).map { validated in
            EditorialActionReview(
                action: validated,
                targetEntryID: targetEntryID,
                communityName: communityName,
                actingEditorKeyHex: myKeyHex
            )
        }
    }

    /// Validates the draft, and if it passes, asks core to sign it. Core is the
    /// authorization boundary: a non-editor's attempt throws here and the draft is
    /// preserved. On success the draft's free-text fields are cleared and the
    /// projection reloads so the effect (or its absence) is visible.
    @discardableResult
    public func sign(targetEntryID: String) -> EditorialSignOutcome {
        let validated: ValidatedEditorialAction
        switch EditorialActionValidator.validate(draft) {
        case let .success(value): validated = value
        case let .failure(violation):
            let outcome = EditorialSignOutcome.invalid(violation)
            lastOutcome = outcome
            return outcome
        }
        do {
            let record = try editor.createNewswireEditorialAction(
                spaceDescriptorEntryID: spaceDescriptorEntryID,
                targetEntryID: targetEntryID,
                kind: validated.kind.toFFI(),
                reason: validated.reason,
                correctionText: validated.correctionText
            )
            // Clear only the free text; keep the selected kind for a next action.
            draft.reason = ""
            draft.correctionText = ""
            let outcome = EditorialSignOutcome.signed(entryID: record.entryId)
            lastOutcome = outcome
            load()
            return outcome
        } catch {
            // Core refused (roster / store / clock). Preserve the draft verbatim —
            // the person loses nothing — and do not pretend anything changed.
            let outcome = EditorialSignOutcome.rejected
            lastOutcome = outcome
            return outcome
        }
    }
}

// MARK: - View

/// The newswire surface on Home: the collective Front page, the Open wire, and
/// the always-public Editorial history, plus — for a recognized editor only — the
/// editorial-action composer. The three distinct wire states each draw their own
/// copy; hidden and tombstoned posts draw the treatment interstitial in place of
/// the payload.
public struct NewswireSurfaceView: View {
    @ObservedObject private var model: NewswireSurfaceModel
    private let onPostUpdate: () -> Void
    private let onSyncWithPeer: () -> Void
    private let onRejoinWithLink: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var actionTarget: NewswirePostRow?
    @State private var replyTarget: NewswirePostRow?

    public init(
        model: NewswireSurfaceModel,
        onPostUpdate: @escaping () -> Void = {},
        onSyncWithPeer: @escaping () -> Void = {},
        onRejoinWithLink: @escaping () -> Void = {}
    ) {
        self.model = model
        self.onPostUpdate = onPostUpdate
        self.onSyncWithPeer = onSyncWithPeer
        self.onRejoinWithLink = onRejoinWithLink
    }

    /// Maps a pure forward action to its handler. Every case leads somewhere real —
    /// no branch is a dead `{}`; `.retry` re-derives + reprojects, the two
    /// navigation cases go through the shell's callbacks.
    private func perform(_ action: NewswireWireForwardAction) {
        switch action {
        case .retry: model.retry()
        case .postFirstUpdate: onPostUpdate()
        case .rejoinWithLink: onRejoinWithLink()
        case .syncWithPeer: onSyncWithPeer()
        }
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            if let note = model.editorialControlsPendingNote {
                Text(note)
                    .font(.riot(.body, size: 13, relativeTo: .caption))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    .accessibilityIdentifier("editorial-controls-pending-note")
            }
            if model.unread.hasUnread {
                unreadDelta(model.unread.count)
            }
            wireSection
            if !model.history.isEmpty {
                editorialHistorySection
            }
        }
        // Load computes the unread delta against the OLD cursor (so it is visible
        // this visit); marking seen then advances the cursor so the NEXT visit is
        // clean. markAllSeen does not recompute, so the delta survives the visit.
        .onAppear {
            model.load()
            model.markAllSeen()
        }
        .sheet(item: $replyTarget) { target in
            NewswireCommentComposeSheet(
                model: model, target: target, onClose: { replyTarget = nil })
        }
        .sheet(item: $actionTarget) { target in
            EditorialActionSheet(model: model, target: target, onClose: { actionTarget = nil })
        }
    }

    @ViewBuilder
    private var wireSection: some View {
        switch model.wire {
        case .offlineStale:
            wireEmpty(
                id: model.wire.accessibilityID,
                title: model.offlineTitle,           // pending-first-sync vs transient-offline
                message: model.offlineMessage,
                actions: model.forwardActions
            )
        case .emptyWire:
            wireEmpty(
                id: model.wire.accessibilityID,
                title: NewswireWireCopy.emptyTitle,
                message: NewswireWireCopy.emptyMessage,
                actions: model.forwardActions
            )
        case let .postsButNoFeature(openWire):
            VStack(alignment: .leading, spacing: 12) {
                wireEmpty(
                    id: model.wire.accessibilityID,
                    title: NewswireWireCopy.noFeatureTitle,
                    message: NewswireWireCopy.noFeatureMessage,
                    actions: []                       // the open wire below IS the next action
                )
                openWireCard(openWire)
            }
        case let .featured(frontPage, openWire):
            VStack(alignment: .leading, spacing: 16) {
                frontPageCard(frontPage)
                openWireCard(openWire)
            }
            .accessibilityIdentifier(model.wire.accessibilityID)
        }
    }

    /// The "N new since you last looked" delta at the top of the wire — the
    /// retention cue. Hidden when zero (the caller gates on `hasUnread`); subtle by
    /// design (a small pink dot + a mono count line), never a modal or a takeover.
    private func unreadDelta(_ count: Int) -> some View {
        HStack(spacing: 8) {
            Circle()
                .fill(RiotTheme.pink(for: colorScheme))
                .frame(width: 8, height: 8)
            Text("\(count) new since you last looked")
                .font(.riot(.mono, size: 12, relativeTo: .caption))
                .textCase(.uppercase)
                .tracking(0.5)
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityIdentifier("newswire-unread-delta")
        .accessibilityLabel("\(count) new report\(count == 1 ? "" : "s") since you last looked")
    }

    private func wireEmpty(
        id: String,
        title: String,
        message: String,
        actions: [NewswireWireForwardAction]
    ) -> some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 10) {
                eyebrow(title)
                Text(message)
                    .font(.riot(.body, size: 15, relativeTo: .callout))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                ForEach(actions, id: \.self) { action in
                    Button(action.label) { perform(action) }
                        .buttonStyle(action.isNearbyPath ? .riotSecondary : .riotPrimary)
                        .frame(minHeight: 44)
                        .accessibilityIdentifier(action.accessibilityID)
                }
            }
        }
        .accessibilityIdentifier(id)
    }

    private func frontPageCard(_ posts: [NewswirePostRow]) -> some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 12) {
                eyebrow("Front page")
                ForEach(posts) { post in postRow(post) }
            }
        }
        .accessibilityIdentifier("newswire-front-page")
    }

    private func openWireCard(_ posts: [NewswirePostRow]) -> some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 12) {
                eyebrow("Open wire")
                ForEach(posts) { post in postRow(post) }
            }
        }
        .accessibilityIdentifier("newswire-open-wire")
    }

    @ViewBuilder
    private func postRow(_ post: NewswirePostRow) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            switch post.display {
            case .ordinary:
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    if model.unread.isNew(post.id) {
                        newDot(for: post.id)
                    }
                    Text(post.headline ?? "")
                        .font(.riot(.body, size: 17, relativeTo: .headline))
                        .foregroundStyle(RiotTheme.ink(for: colorScheme))
                }
                if post.hasCorrection {
                    RiotBadge(EditorialCorrectionLabel.text)
                        .accessibilityIdentifier("correction-label-\(post.id)")
                }
                if post.verificationCount > 0 {
                    Text("\(post.verificationCount) verification\(post.verificationCount == 1 ? "" : "s")")
                        .font(.riot(.mono, size: 11, relativeTo: .caption2))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                }
                if post.aiAssisted {
                    RiotBadge("AI-assisted · human reviewed and signed")
                }
            case .hiddenInterstitial:
                treatmentInterstitial(
                    id: "hidden-interstitial-\(post.id)",
                    title: NewswireTreatmentCopy.hiddenTitle,
                    body: NewswireTreatmentCopy.hiddenBody
                )
            case .tombstoned:
                treatmentInterstitial(
                    id: "tombstone-\(post.id)",
                    title: NewswireTreatmentCopy.tombstoneTitle,
                    body: NewswireTreatmentCopy.tombstoneBody
                )
            }
            Text(post.author)
                .font(.riot(.mono, size: 11, relativeTo: .caption2))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            commentsSection(for: post)
            HStack(spacing: 8) {
                if model.canComment {
                    Button("Reply") { replyTarget = post }
                        .buttonStyle(.riotSecondary)
                        .frame(minHeight: 44)
                        .accessibilityIdentifier("reply-\(post.id)")
                }
                if model.canOfferEditorialControls {
                    Button("Editorial action") { actionTarget = post }
                        .buttonStyle(.riotSecondary)
                        .frame(minHeight: 44)
                        .accessibilityIdentifier("editorial-action-\(post.id)")
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityIdentifier("wire-post-\(post.id)")
    }

    /// The communal replies under one post, indented and time-ordered as core
    /// returned them. Drawn only when the post has replies, so an ordinary post
    /// gains no empty chrome.
    @ViewBuilder
    private func commentsSection(for post: NewswirePostRow) -> some View {
        let comments = model.comments(under: post.id)
        if !comments.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(comments) { comment in commentRow(comment) }
            }
            .padding(.leading, 14)
            .accessibilityIdentifier("wire-comments-\(post.id)")
        }
    }

    @ViewBuilder
    private func commentRow(_ comment: NewswireCommentRow) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            switch comment.display {
            case .ordinary:
                Text(comment.body ?? "")
                    .font(.riot(.body, size: 15, relativeTo: .body))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
            case .hiddenInterstitial:
                Text(NewswireTreatmentCopy.hiddenTitle)
                    .font(.riot(.body, size: 13, relativeTo: .caption))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            case .tombstoned:
                Text(NewswireTreatmentCopy.tombstoneTitle)
                    .font(.riot(.body, size: 13, relativeTo: .caption))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            }
            Text(comment.author)
                .font(.riot(.mono, size: 11, relativeTo: .caption2))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityIdentifier("wire-comment-\(comment.id)")
    }

    /// The per-row "new" marker — a small pink dot on a post newer than the seen
    /// cursor. Addressable per post so a UI test can assert exactly which rows are
    /// marked, and labeled for VoiceOver.
    private func newDot(for id: String) -> some View {
        Circle()
            .fill(RiotTheme.pink(for: colorScheme))
            .frame(width: 8, height: 8)
            .accessibilityIdentifier("wire-post-new-\(id)")
            .accessibilityLabel("New")
    }

    private func treatmentInterstitial(id: String, title: String, body: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.riot(.body, size: 15, relativeTo: .headline))
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
            Text(body)
                .font(.riot(.body, size: 13, relativeTo: .caption))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RiotTheme.paper2(for: colorScheme))
        .accessibilityIdentifier(id)
    }

    private var editorialHistorySection: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 12) {
                eyebrow("Editorial history")
                ForEach(model.history) { row in
                    VStack(alignment: .leading, spacing: 3) {
                        HStack {
                            Text(row.kind.label)
                                .font(.riot(.body, size: 15, relativeTo: .headline))
                                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                            if let correction = row.correctionLabel {
                                RiotBadge(correction)
                            }
                            if !row.isActive {
                                Text("retracted")
                                    .font(.riot(.mono, size: 11, relativeTo: .caption2))
                                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            }
                            Spacer()
                        }
                        if let reason = row.reason {
                            Text(reason)
                                .font(.riot(.body, size: 13, relativeTo: .caption))
                                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        }
                        Text(row.signer)
                            .font(.riot(.mono, size: 11, relativeTo: .caption2))
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .accessibilityIdentifier("history-\(row.id)")
                }
            }
        }
        .accessibilityIdentifier("newswire-editorial-history")
    }

    private func eyebrow(_ text: String) -> some View {
        Text(text)
            .font(.riot(.mono, size: 12, relativeTo: .caption))
            .textCase(.uppercase)
            .tracking(1)
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
    }
}

/// The editorial-action composer + immutable pre-signing review, presented for a
/// single target post. The kind picker drives which fields the closed table
/// requires; the review shows every complete identifier before a signature; a
/// failed sign keeps the sheet open with the draft intact.
private struct EditorialActionSheet: View {
    @ObservedObject var model: NewswireSurfaceModel
    let target: NewswirePostRow
    let onClose: () -> Void
    @Environment(\.colorScheme) private var colorScheme

    private var rules: EditorialFieldRules { model.draft.kind.fieldRules }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Picker("Action", selection: $model.draft.kind) {
                    ForEach(EditorialActionKind.allCases) { kind in
                        Text(kind.label).tag(kind)
                    }
                }
                .accessibilityIdentifier("editorial-kind-picker")

                if rules.reason == .required {
                    field("Reason", text: $model.draft.reason, id: "editorial-reason")
                }
                if rules.correctionText == .required {
                    if model.draft.kind.isEditorialCorrection {
                        Text(EditorialCorrectionLabel.text)
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    }
                    field("Replacement text", text: $model.draft.correctionText, id: "editorial-correction")
                }

                reviewCard
                signButton
                if case let .invalid(violation) = model.lastOutcome {
                    Text(violation.message)
                        .font(.riot(.body, size: 13, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.pink(for: colorScheme))
                        .accessibilityIdentifier("editorial-violation")
                }
                if case .rejected = model.lastOutcome {
                    Text("That action was not accepted. Your draft is kept.")
                        .font(.riot(.body, size: 13, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.pink(for: colorScheme))
                        .accessibilityIdentifier("editorial-rejected")
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Editorial action", model.draft.kind.label)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) { Button("Close", action: onClose) }
        }
    }

    @ViewBuilder
    private var reviewCard: some View {
        if case let .success(review) = model.review(targetEntryID: target.id) {
            RiotCard {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Review before signing")
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .textCase(.uppercase)
                        .tracking(1)
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    ForEach(review.rows, id: \.label) { row in
                        VStack(alignment: .leading, spacing: 3) {
                            Text(row.label)
                                .font(.riot(.mono, size: 11, relativeTo: .caption2))
                                .textCase(.uppercase)
                                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            Text(row.value)
                                .font(.riot(.mono, size: 13, relativeTo: .footnote))
                                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                                .textSelection(.enabled)
                        }
                    }
                }
            }
            .accessibilityIdentifier("editorial-review")
        }
    }

    @ViewBuilder
    private var signButton: some View {
        let isReady: Bool = {
            if case .success = model.review(targetEntryID: target.id) { return true }
            return false
        }()
        Button("Sign and post") {
            if case .signed = model.sign(targetEntryID: target.id) { onClose() }
        }
        .buttonStyle(.riotPrimary)
        .frame(minHeight: 44)
        .disabled(!isReady)
        .accessibilityIdentifier("editorial-sign")
    }

    private func field(_ label: String, text: Binding<String>, id: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label)
                .font(.riot(.mono, size: 12, relativeTo: .caption))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            TextField(label, text: text, axis: .vertical)
                .font(.riot(.body, size: 15, relativeTo: .body))
                .accessibilityIdentifier(id)
        }
    }
}

/// The communal-reply composer, presented for a single parent post. A reply is
/// communal — there is no roster gate and no kind picker — so the sheet is just a
/// body field and a post button. An empty reply is disabled; a core refusal keeps
/// the sheet open with the words intact.
private struct NewswireCommentComposeSheet: View {
    @ObservedObject var model: NewswireSurfaceModel
    let target: NewswirePostRow
    let onClose: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var text: String = ""
    @State private var rejected = false

    private var composer: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if let headline = target.headline {
                    VStack(alignment: .leading, spacing: 3) {
                        Text("Replying to")
                            .font(.riot(.mono, size: 11, relativeTo: .caption2))
                            .textCase(.uppercase)
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        Text(headline)
                            .font(.riot(.body, size: 15, relativeTo: .headline))
                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    }
                    .accessibilityIdentifier("comment-parent-\(target.id)")
                }
                VStack(alignment: .leading, spacing: 4) {
                    Text("Your reply")
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    TextField("Your reply", text: $text, axis: .vertical)
                        .font(.riot(.body, size: 15, relativeTo: .body))
                        .accessibilityIdentifier("comment-body")
                }
                Button("Reply") {
                    switch model.submitComment(parentEntryID: target.id, body: text) {
                    case .posted:
                        onClose()
                    case .rejected, .unavailable:
                        rejected = true
                    case .empty:
                        break
                    }
                }
                .buttonStyle(.riotPrimary)
                .frame(minHeight: 44)
                .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .accessibilityIdentifier("comment-submit")
                if rejected {
                    Text("That reply was not accepted. Your words are kept.")
                        .font(.riot(.body, size: 13, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.pink(for: colorScheme))
                        .accessibilityIdentifier("comment-rejected")
                }
            }
            .padding(20)
        }
    }

    var body: some View {
        composer
            .riotHeader(eyebrow: "Reply", model.communityName)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Close", action: onClose) }
            }
    }
}

// MARK: - Composite-site moderation read surface (fix #4, read path)

/// How a composite site's content surface is presented, resolved from core's
/// single `SiteDegradation` verdict. `held` is a SECURITY control, not decoration:
/// when core reports `.moderationLoading` the moderation list is not yet current,
/// so the whole content surface is HELD — items are NEVER rendered as
/// trustworthy-clean, because showing not-yet-moderated content as if it were
/// moderated is the exact failure the freshness gate exists to prevent. A
/// more-severe honest degradation (invalid manifest, rollback/equivocation alarm,
/// blocked transport) holds the surface too; the mild states (editorial-only,
/// member-unverified) show content with a notice.
public enum CompositeContentHold: Equatable, Sendable {
    /// Content renders normally (possibly with a mild informational banner).
    case shown
    /// Content is gated behind the banner — not presented as trustworthy.
    case held(reason: SiteDegradation)
}

/// One accountable row on the composite read surface. The resolved item carries
/// no body (this is the moderation/trust view, not the article reader), so a row
/// is identity + trust tier + moderation treatment. A Hidden/Tombstoned item is an
/// accountable placeholder (its `display`), never a silent disappearance.
public struct CompositeItemRow: Equatable, Identifiable, Sendable {
    public let id: String
    public let authorTag: String
    public let tier: SiteTrustTier
    public let display: NewswirePostDisplay

    public init(_ item: ResolvedSiteItem) {
        self.id = item.entryId
        self.authorTag = String(item.authorSubspace.prefix(8))
        self.tier = item.trustTier
        self.display = NewswirePostDisplay.fromSite(item.treatment)
    }
}

public extension NewswirePostDisplay {
    /// Map a composite-site item treatment to the shared post-display treatment —
    /// the same accountable-placeholder rendering a moderated newswire post gets.
    static func fromSite(_ treatment: SiteItemTreatment) -> NewswirePostDisplay {
        switch treatment {
        case .ordinary: .ordinary
        case .hidden: .hiddenInterstitial
        case .tombstoned: .tombstoned
        }
    }
}

/// The read-model for a resolved composite site: the hold decision, the honest
/// banner copy, and the accountable rows. Pure over core's `ResolvedCompositeSite`
/// — the view renders exactly this and makes no trust decision of its own.
public struct CompositeSiteReadModel: Equatable, Sendable {
    public let root: String
    public let hold: CompositeContentHold
    public let bannerMessage: String?
    public let items: [CompositeItemRow]

    /// Whether the content surface is HELD (gated, not trustworthy). The view MUST
    /// honour this — under a hold it never renders the rows as clean content.
    public var isContentHeld: Bool {
        if case .held = hold { return true }
        return false
    }

    public static func from(_ resolved: ResolvedCompositeSite) -> CompositeSiteReadModel {
        CompositeSiteReadModel(
            root: resolved.root,
            hold: holdFor(resolved.degradation),
            bannerMessage: banner(for: resolved.degradation),
            items: resolved.items.map(CompositeItemRow.init))
    }

    /// The hold decision. Every state where content must NOT be trusted holds the
    /// surface; the two mild states still show content with a notice.
    static func holdFor(_ degradation: SiteDegradation) -> CompositeContentHold {
        switch degradation {
        case .none, .memberUnverified, .editorialOnly:
            .shown
        case .moderationLoading, .manifestInvalid, .transportBlocked,
            .manifestRollbackAlarm, .equivocationAlarm:
            .held(reason: degradation)
        }
    }

    /// Honest, plain-language banner copy per degradation. `nil` only when fully
    /// current — every degraded state names its own "why".
    static func banner(for degradation: SiteDegradation) -> String? {
        switch degradation {
        case .none:
            nil
        case .moderationLoading:
            "Moderation loading — posts stay held until this site's moderation list catches up."
        case .manifestInvalid:
            "This site couldn't be verified. Its content is held until a valid signature syncs."
        case .memberUnverified:
            "A section of this site couldn't be verified."
        case .editorialOnly:
            "Comments and the open wire are still syncing."
        case .transportBlocked:
            "This site requires a connection that isn't available right now."
        case .manifestRollbackAlarm:
            "This site's configuration looks rolled back — content is held."
        case .equivocationAlarm:
            "This site has conflicting owner signatures — content is held."
        }
    }
}

public extension SiteTrustTier {
    /// A short, stable label for the item's owner-resolved trust tier.
    var label: String {
        switch self {
        case .editorial: "Editorial"
        case .openWire: "Open wire"
        case .comment: "Comment"
        }
    }
}

/// The badge identity for one resolved item's trust tier — a SECURITY-relevant
/// type, not decoration: an open-wire or comment item must never be able to
/// wear editorial's badge or tint, so `for(_:)` is required to produce a
/// distinct `badgeSymbol` AND a distinct `tintToken` per tier (an open-wire
/// item can never be confused for an editorial one at a glance). `tintToken`
/// names a `RiotTheme` color function rather than holding a `Color` directly,
/// so this type stays plain `Equatable`/`Hashable` and testable without a
/// `ColorScheme`; `tint(for:)` resolves it at render time.
public struct CompositeSiteTierStyle: Equatable, Hashable, Sendable {
    /// Stable per-tier token: doubles as the accessibility-identifier suffix.
    public let token: String
    public let badgeSymbol: String
    public let tintToken: String

    public static func `for`(_ tier: SiteTrustTier) -> CompositeSiteTierStyle {
        switch tier {
        case .editorial:
            CompositeSiteTierStyle(
                token: "editorial", badgeSymbol: "checkmark.seal.fill", tintToken: "pink")
        case .openWire:
            CompositeSiteTierStyle(
                token: "open-wire", badgeSymbol: "antenna.radiowaves.left.and.right",
                tintToken: "blue")
        case .comment:
            CompositeSiteTierStyle(
                token: "comment", badgeSymbol: "bubble.left.fill", tintToken: "inkSoft")
        }
    }

    /// Resolves `tintToken` to a themed color for the given color scheme.
    public func tint(for scheme: ColorScheme) -> Color {
        switch tintToken {
        case "pink": RiotTheme.pink(for: scheme)
        case "blue": RiotTheme.blue(for: scheme)
        default: RiotTheme.inkSoft(for: scheme)
        }
    }
}

// MARK: - Composite-site owner moderation authoring (fix #4, write path)

/// Which moderation action the site owner is authoring. Mirrors the newswire
/// editorial-action shape, but the moderation overlay is per-content / per-author,
/// not the editorial vocabulary.
public enum SiteModerationTargetKind: String, Equatable, Sendable, CaseIterable, Identifiable {
    /// Ban an author-key — every item that author signed is Hidden read-side.
    case revoke
    /// Hide one specific entry by its `(namespace, entry-id)` identity.
    case tombstone

    public var id: String { rawValue }
    public var label: String {
        switch self {
        case .revoke: "Revoke author"
        case .tombstone: "Tombstone entry"
        }
    }
}

/// The owner's in-progress moderation action. Only the fields the chosen kind
/// needs are consulted; the rest are ignored (and cleared when the kind switches
/// is a UI concern). Mirrors `EditorialActionDraft`.
public struct SiteModerationDraft: Equatable, Sendable {
    public var kind: SiteModerationTargetKind
    /// For `.revoke`: the author subspace id to ban (64 hex).
    public var authorKey: String
    /// For `.tombstone`: the namespace of the entry to hide (64 hex).
    public var targetNamespace: String
    /// For `.tombstone`: the entry id to hide (64 hex).
    public var targetEntry: String

    public init(
        kind: SiteModerationTargetKind,
        authorKey: String = "",
        targetNamespace: String = "",
        targetEntry: String = ""
    ) {
        self.kind = kind
        self.authorKey = authorKey
        self.targetNamespace = targetNamespace
        self.targetEntry = targetEntry
    }

    /// Empty when the fields the current kind needs are blank — drives the
    /// Stay-or-Discard confirmation on leave.
    public var isEmpty: Bool {
        switch kind {
        case .revoke:
            authorKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        case .tombstone:
            targetNamespace.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                && targetEntry.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
    }
}

/// Why a moderation draft could not be turned into a signable action. Each names
/// the exact field so the surface can point at it — an invalid draft never
/// reaches core.
public enum SiteModerationFieldViolation: Error, Equatable, Sendable {
    case authorKeyRequired
    case authorKeyMalformed
    case targetNamespaceRequired
    case targetNamespaceMalformed
    case targetEntryRequired
    case targetEntryMalformed

    public var message: String {
        switch self {
        case .authorKeyRequired: "Enter the author key to revoke."
        case .authorKeyMalformed: "The author key must be 64 hex characters."
        case .targetNamespaceRequired: "Enter the entry's namespace."
        case .targetNamespaceMalformed: "The namespace must be 64 hex characters."
        case .targetEntryRequired: "Enter the entry id to tombstone."
        case .targetEntryMalformed: "The entry id must be 64 hex characters."
        }
    }
}

/// Validates a moderation draft into the FFI `SiteModerationAction`. Pure and
/// deterministic; produces the exact value the model signs, never a partial one.
public enum SiteModerationValidator {
    /// A 64-char lowercase-or-uppercase hex string (a 32-byte id).
    static func isHex32(_ value: String) -> Bool {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.count == 64 else { return false }
        return trimmed.allSatisfy(\.isHexDigit)
    }

    public static func validate(
        _ draft: SiteModerationDraft
    ) -> Result<SiteModerationAction, SiteModerationFieldViolation> {
        switch draft.kind {
        case .revoke:
            let key = draft.authorKey.trimmingCharacters(in: .whitespacesAndNewlines)
            if key.isEmpty { return .failure(.authorKeyRequired) }
            if !isHex32(key) { return .failure(.authorKeyMalformed) }
            return .success(.revoke(authorKey: key.lowercased()))
        case .tombstone:
            let ns = draft.targetNamespace.trimmingCharacters(in: .whitespacesAndNewlines)
            let entry = draft.targetEntry.trimmingCharacters(in: .whitespacesAndNewlines)
            if ns.isEmpty { return .failure(.targetNamespaceRequired) }
            if !isHex32(ns) { return .failure(.targetNamespaceMalformed) }
            if entry.isEmpty { return .failure(.targetEntryRequired) }
            if !isHex32(entry) { return .failure(.targetEntryMalformed) }
            return .success(.tombstone(targetNamespace: ns.lowercased(), targetEntry: entry.lowercased()))
        }
    }
}

/// The immutable review the owner sees before signing — every identifier shown
/// UNTRUNCATED (this is the signing surface; truncation would be a defect).
/// Mirrors `EditorialActionReview`.
public struct SiteModerationReview: Equatable, Sendable {
    public let kind: SiteModerationTargetKind
    /// The site root (owned namespace id), untruncated hex.
    public let siteRoot: String
    /// The ordered label/value rows — each identifier complete.
    public let rows: [ReviewRow]

    public struct ReviewRow: Equatable, Sendable {
        public let label: String
        public let value: String
    }

    public init(action: SiteModerationAction, siteRoot: String) {
        self.siteRoot = siteRoot
        switch action {
        case let .revoke(authorKey):
            self.kind = .revoke
            self.rows = [
                ReviewRow(label: "Action", value: "Revoke author"),
                ReviewRow(label: "Site", value: siteRoot),
                ReviewRow(label: "Author key", value: authorKey),
            ]
        case let .tombstone(targetNamespace, targetEntry):
            self.kind = .tombstone
            self.rows = [
                ReviewRow(label: "Action", value: "Tombstone entry"),
                ReviewRow(label: "Site", value: siteRoot),
                ReviewRow(label: "Namespace", value: targetNamespace),
                ReviewRow(label: "Target entry", value: targetEntry),
            ]
        }
    }
}

/// The one call the owner moderation surface makes to author + sign an action.
/// `RiotProfileRepository` conforms (it supplies the wrapping key from the
/// keychain and forwards to core, which auto-publishes the heartbeat). The
/// `sealedRoot` is the owner's proof of ownership — possession of the sealed
/// masthead IS the authority. Tests inject a stub.
public protocol SiteModerationAuthoring {
    @discardableResult
    func authorSiteModeration(
        sealedRoot: Data,
        action: SiteModerationAction
    ) throws -> SiteModerationOutcome
}

/// The result of attempting to sign a moderation action from the surface.
public enum SiteModerationSignOutcome: Equatable, Sendable {
    /// Signed + committed locally (and the coupled heartbeat published). Carries the
    /// full outcome — INCLUDING the signed bytes, which the app must hand onward to
    /// propagate to followers (owned-namespace /mod/ has no automatic sync yet).
    case signed(SiteModerationOutcome)
    /// The draft violated the field rules — nothing reached core.
    case invalid(SiteModerationFieldViolation)
    /// Core refused (wrong wrapping key, closed store, clock). Draft preserved.
    case rejected
}

/// Drives the owner moderation sheet for one composite site. Gated on ownership:
/// it is constructed only with the site's sealed masthead, so a non-owner never
/// gets one. Validates the draft, signs through the seam (core auto-publishes the
/// heartbeat), and RETAINS the outcome — `lastOutcome` keeps the signed bytes
/// reachable so the caller can propagate them (this surface does not itself ship a
/// share affordance; propagation is a tracked follow-up).
@MainActor
public final class SiteModerationModel: ObservableObject {
    @Published public var draft: SiteModerationDraft
    @Published public private(set) var lastOutcome: SiteModerationOutcome?
    @Published public private(set) var lastSignOutcome: SiteModerationSignOutcome?

    public let siteRoot: String
    private let sealedRoot: Data
    private let authoring: SiteModerationAuthoring

    public init(
        siteRoot: String,
        sealedRoot: Data,
        authoring: SiteModerationAuthoring,
        initialKind: SiteModerationTargetKind = .tombstone
    ) {
        self.siteRoot = siteRoot
        self.sealedRoot = sealedRoot
        self.authoring = authoring
        self.draft = SiteModerationDraft(kind: initialKind)
    }

    /// The immutable pre-signing review for the current draft, or the violation
    /// that blocks it. Pure — nothing is signed.
    public func review() -> Result<SiteModerationReview, SiteModerationFieldViolation> {
        SiteModerationValidator.validate(draft).map { action in
            SiteModerationReview(action: action, siteRoot: siteRoot)
        }
    }

    /// Validate and, if it passes, sign through core (which auto-publishes the
    /// heartbeat). Retains the outcome so the signed bytes stay reachable. A core
    /// refusal preserves the draft.
    @discardableResult
    public func sign() -> SiteModerationSignOutcome {
        let action: SiteModerationAction
        switch SiteModerationValidator.validate(draft) {
        case let .success(value): action = value
        case let .failure(violation):
            let outcome = SiteModerationSignOutcome.invalid(violation)
            lastSignOutcome = outcome
            return outcome
        }
        do {
            let outcome = try authoring.authorSiteModeration(sealedRoot: sealedRoot, action: action)
            lastOutcome = outcome
            let result = SiteModerationSignOutcome.signed(outcome)
            lastSignOutcome = result
            return result
        } catch {
            let result = SiteModerationSignOutcome.rejected
            lastSignOutcome = result
            return result
        }
    }
}
