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

// MARK: - Editor authority (UI visibility only — NEVER the authorization check)

/// Whether this profile should be OFFERED an editorial control. This is a
/// best-effort UI hint, deliberately separate from the authorization decision:
/// the real gate is core refusing to sign an action from a key outside the
/// descriptor's roster (`create_newswire_editorial_action` throws). A hidden
/// control is a courtesy; a rejected action is the security boundary. The two are
/// independent by construction — this function never talks to core, and core
/// never consults this function.
///
/// The MVP FFI exposes the founding roster only as CREATE input, never as a
/// read-back, so this can be computed only for a community whose roster this
/// device knows — one it created this session. A joined or loaded community's
/// roster is unknown (Risk 11: no descriptor re-hydration), so it reports `false`
/// and the control stays hidden until a real attempt would fail closed anyway.
public enum EditorialAuthority {
    /// `roster` is the founding editorial roster as hex subspace ids, exactly as
    /// passed to `createNewswireSpace`, or `nil` when this device does not know the
    /// community's roster (a joined or loaded community — Risk 11). An UNKNOWN
    /// roster is never an editor here: the control stays hidden and a real attempt
    /// fails closed at signing. An EMPTY roster means core's default — the founder
    /// alone — so the founder is an editor. A non-empty roster makes a key an
    /// editor only if it is named in it.
    public static func isRecognizedEditor(myKeyHex: String, roster: [String]?) -> Bool {
        let me = myKeyHex.lowercased()
        if me.isEmpty { return false }
        guard let roster else { return false }
        if roster.isEmpty { return true }
        return roster.contains { $0.lowercased() == me }
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
    public static let noFeatureLink = "Open wire"
    public static let offlineTitle = "Updates unavailable"
    public static let offlineMessage =
        "This community's wire is offline or has not synced yet. What you already have is still here."
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

    private let projector: NewswireProjecting
    private let editor: NewswireEditorialActing
    private let authority: NewswireEditorAuthorityChecking
    private let spaceDescriptorEntryID: String
    private let myKeyHex: String
    public let communityName: String

    /// Whether core recognizes this profile as an editor of this descriptor — read
    /// from the FFI predicate in `load()`, never a locally-asserted roster. `false`
    /// until loaded and `false` for an unknown / not-yet-synced descriptor (no
    /// error), by construction.
    @Published public private(set) var isEditor: Bool = false

    public init(
        projector: NewswireProjecting,
        editor: NewswireEditorialActing,
        authority: NewswireEditorAuthorityChecking,
        spaceDescriptorEntryID: String,
        communityName: String,
        myKeyHex: String,
        initialDraftKind: EditorialActionKind = .feature
    ) {
        self.projector = projector
        self.editor = editor
        self.authority = authority
        self.spaceDescriptorEntryID = spaceDescriptorEntryID
        self.communityName = communityName
        self.myKeyHex = myKeyHex
        self.wire = .offlineStale
        self.history = []
        self.draft = EditorialActionDraft(kind: initialDraftKind)
    }

    /// Whether the surface may OFFER an editorial control — UI visibility only.
    /// A `false` here hides the control; it does NOT authorize anything, and a
    /// `true` here does not either (core still decides at sign time). Reads the
    /// cached predicate answer resolved in `load()`, the SAME roster authority core
    /// enforces at admission.
    public var canOfferEditorialControls: Bool {
        !spaceDescriptorEntryID.isEmpty && isEditor
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
        // Editor status is core's descriptor answer, resolved once per load. An
        // unknown / not-yet-synced descriptor (or a closed profile) answers false —
        // never a throw here.
        isEditor = (try? authority.newswireIsEditor(
            spaceDescriptorEntryID: spaceDescriptorEntryID, subjectID: myKeyHex)) ?? false

        guard !spaceDescriptorEntryID.isEmpty else {
            wire = .offlineStale
            history = []
            return
        }
        do {
            let projection = try projector.projectNewswire(
                spaceDescriptorEntryID: spaceDescriptorEntryID
            )
            wire = .from(projection)
            history = projection.editorialHistory.map(EditorialHistoryRow.init)
        } catch {
            // Fixed, honest state — never a raw internal error, never invented
            // content.
            wire = .offlineStale
            history = []
        }
    }

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
    @Environment(\.colorScheme) private var colorScheme
    @State private var actionTarget: NewswirePostRow?

    public init(model: NewswireSurfaceModel, onPostUpdate: @escaping () -> Void = {}) {
        self.model = model
        self.onPostUpdate = onPostUpdate
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            if let note = model.editorialControlsPendingNote {
                Text(note)
                    .font(.riot(.body, size: 13, relativeTo: .caption))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    .accessibilityIdentifier("editorial-controls-pending-note")
            }
            wireSection
            if !model.history.isEmpty {
                editorialHistorySection
            }
        }
        .onAppear { model.load() }
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
                title: NewswireWireCopy.offlineTitle,
                message: NewswireWireCopy.offlineMessage,
                primary: ("Try again", { model.load() })
            )
        case .emptyWire:
            wireEmpty(
                id: model.wire.accessibilityID,
                title: NewswireWireCopy.emptyTitle,
                message: NewswireWireCopy.emptyMessage,
                primary: ("Post the first update", onPostUpdate)
            )
        case let .postsButNoFeature(openWire):
            VStack(alignment: .leading, spacing: 12) {
                wireEmpty(
                    id: model.wire.accessibilityID,
                    title: NewswireWireCopy.noFeatureTitle,
                    message: NewswireWireCopy.noFeatureMessage,
                    primary: (NewswireWireCopy.noFeatureLink, {})
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

    private func wireEmpty(
        id: String,
        title: String,
        message: String,
        primary: (label: String, action: () -> Void)
    ) -> some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 10) {
                eyebrow(title)
                Text(message)
                    .font(.riot(.body, size: 15, relativeTo: .callout))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                Button(primary.label, action: primary.action)
                    .buttonStyle(.riotPrimary)
                    .frame(minHeight: 44)
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
                Text(post.headline ?? "")
                    .font(.riot(.body, size: 17, relativeTo: .headline))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
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
            if model.canOfferEditorialControls {
                Button("Editorial action") { actionTarget = post }
                    .buttonStyle(.riotSecondary)
                    .frame(minHeight: 44)
                    .accessibilityIdentifier("editorial-action-\(post.id)")
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityIdentifier("wire-post-\(post.id)")
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
