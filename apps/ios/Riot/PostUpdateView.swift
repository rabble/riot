import SwiftUI

// MARK: - Publishing identity

/// Who a post is signed as. The newswire scope calls for "persistent local
/// pseudonymous signing identities by default" and "clearly labeled one-off
/// ephemeral publishing identities" (newswire design §Scope). The default is the
/// person's persistent pseudonym; a one-off is chosen deliberately when
/// continuity is unsafe, and MUST be labeled as such so nobody posts under it
/// believing it is their usual name.
public enum PublishingIdentity: Equatable, Sendable {
    /// The default: the persistent local pseudonym, drawn as `Ana · a3f91122`.
    case persistent(RiotPerson)
    /// A deliberately chosen throwaway identity. Unrecoverable once lost; the
    /// review states this plainly so it is never mistaken for the pseudonym.
    case ephemeralOneOff(RiotPerson)

    public var person: RiotPerson {
        switch self {
        case let .persistent(person), let .ephemeralOneOff(person):
            return person
        }
    }

    public var isEphemeralOneOff: Bool {
        if case .ephemeralOneOff = self { return true }
        return false
    }

    /// The label the review screen shows for this identity. An ephemeral identity
    /// is explicitly marked so the "which identity am I posting as" answer can
    /// never silently read as the persistent pseudonym.
    public var reviewLabel: String {
        isEphemeralOneOff ? "\(person.rendered) · one-off identity" : person.rendered
    }
}

// MARK: - Posting target

/// The community a post is published to, named the way a person would recognize
/// it. Carries the space descriptor entry id that every newswire call threads
/// through, but that id is evidence, not reading material — the review shows
/// `name`, never the id.
public struct PostingCommunity: Equatable, Sendable {
    public let name: String
    public let spaceDescriptorEntryID: String

    public init(name: String, spaceDescriptorEntryID: String) {
        self.name = name
        self.spaceDescriptorEntryID = spaceDescriptorEntryID
    }
}

// MARK: - Composer mode

/// What kind of post is being written. The newswire route's freeform default
/// requires only headline + body; choosing an operational alert or request
/// switches the composer to that profile's stricter required fields (source
/// claims, expiry, coarse location). This rule supersedes the navigation
/// design's blanket "sources + expiry required" for the newswire route
/// (newswire design §"The default Newswire composer…").
public enum ComposerMode: Equatable, Sendable, CaseIterable {
    case freeform
    case operationalAlert
    case operationalRequest

    /// Only an operational profile pulls in the stricter required fields. A
    /// freeform update is exactly headline + body.
    public var requiresStricterFields: Bool { self != .freeform }

    /// The outcome-language label for the mode picker — never mechanism.
    public var label: String {
        switch self {
        case .freeform: return "Update"
        case .operationalAlert: return "Alert"
        case .operationalRequest: return "Request"
        }
    }
}

// MARK: - Validation

/// Whether the draft may be posted, and if not, why — in plain language, so the
/// composer can show what is still needed rather than silently disabling the
/// button with no explanation.
public enum PostUpdateValidation: Equatable, Sendable {
    case ready
    case needsHeadlineAndBody
    /// An operational profile is selected but its stricter fields are missing.
    case needsOperationalFields([String])

    public var isReady: Bool { self == .ready }
}

// MARK: - Publish request + result

/// The complete, reviewed set of fields for one signed post. Built by the view
/// model only after validation passes, so a publisher never has to re-validate.
public struct PostUpdateRequest: Equatable, Sendable {
    public let spaceDescriptorEntryID: String
    public let headline: String
    public let body: String
    public let aiAssisted: Bool
    public let sourceClaims: [String]
    public let expiresAtUnixSeconds: UInt64?
    public let coarseLocation: String?
    public let operationalProfile: NewswireOperationalProfile?

    public init(
        spaceDescriptorEntryID: String,
        headline: String,
        body: String,
        aiAssisted: Bool,
        sourceClaims: [String],
        expiresAtUnixSeconds: UInt64?,
        coarseLocation: String?,
        operationalProfile: NewswireOperationalProfile?
    ) {
        self.spaceDescriptorEntryID = spaceDescriptorEntryID
        self.headline = headline
        self.body = body
        self.aiAssisted = aiAssisted
        self.sourceClaims = sourceClaims
        self.expiresAtUnixSeconds = expiresAtUnixSeconds
        self.coarseLocation = coarseLocation
        self.operationalProfile = operationalProfile
    }
}

/// What the composer shows after a successful signed write: the post is
/// committed locally, but has not yet reached any peer. Home surfaces this as a
/// "Pending nearby exchange" status (nav Posting step 5 / newswire Publishing
/// step 6) — committed, not yet exchanged.
public struct PostedUpdate: Equatable, Sendable {
    public static let pendingExchangeStatus = "Pending nearby exchange"

    public let entryID: String
    public var exchangeStatus: String { Self.pendingExchangeStatus }

    public init(entryID: String) {
        self.entryID = entryID
    }
}

/// The composer's lifecycle. A failure returns to `.editing` with the draft
/// intact and a fixed message set — it never blanks the draft and never shows a
/// raw internal error.
public enum PostUpdateStatus: Equatable, Sendable {
    case editing
    case posting
    case posted(PostedUpdate)
}

// MARK: - Publisher seam

/// The one signed-write seam. `RiotProfileRepository` conforms to it (below);
/// tests inject a stub so the composer flow is provable in isolation without a
/// real store or the FFI.
public protocol NewswirePostPublishing {
    func publishNewswirePost(_ request: PostUpdateRequest) throws -> NewswireSignedRecord
}

extension RiotProfileRepository: NewswirePostPublishing {
    public func publishNewswirePost(_ request: PostUpdateRequest) throws -> NewswireSignedRecord {
        try createNewswirePost(
            spaceDescriptorEntryID: request.spaceDescriptorEntryID,
            headline: request.headline,
            body: request.body,
            eventTimeUnixSeconds: nil,
            expiresAtUnixSeconds: request.expiresAtUnixSeconds,
            coarseLocation: request.coarseLocation,
            sourceClaims: request.sourceClaims,
            operationalProfile: request.operationalProfile,
            aiAssisted: request.aiAssisted
        )
    }
}

// MARK: - Draft persistence seam

/// A half-written post must survive the view being backgrounded or dismissed
/// (newswire design §recovery: "do not discard a locally successful pending
/// post"; the same principle protects an in-progress draft). The store is a
/// seam so the default can persist to `UserDefaults` while tests use memory.
public protocol PostDraftStore {
    func save(_ draft: PostDraft)
    func load() -> PostDraft?
    func clear()
}

/// The persisted shape of an in-progress draft — only the words and the toggles,
/// never the identity or a signature.
public struct PostDraft: Equatable, Codable, Sendable {
    public var headline: String
    public var body: String
    public var aiAssisted: Bool
    public var sourceClaims: [String]
    public var coarseLocation: String

    public init(
        headline: String,
        body: String,
        aiAssisted: Bool,
        sourceClaims: [String],
        coarseLocation: String
    ) {
        self.headline = headline
        self.body = body
        self.aiAssisted = aiAssisted
        self.sourceClaims = sourceClaims
        self.coarseLocation = coarseLocation
    }

    public var isEmpty: Bool {
        headline.isEmpty && body.isEmpty && !aiAssisted
            && sourceClaims.isEmpty && coarseLocation.isEmpty
    }
}

/// The shipping draft store: one `UserDefaults` key per community, so switching
/// communities never bleeds one draft into another.
public struct UserDefaultsPostDraftStore: PostDraftStore {
    private let defaults: UserDefaults
    private let key: String

    public init(communityID: String, defaults: UserDefaults = .standard) {
        self.defaults = defaults
        self.key = "riot.post-draft.\(communityID)"
    }

    public func save(_ draft: PostDraft) {
        guard let data = try? JSONEncoder().encode(draft) else { return }
        defaults.set(data, forKey: key)
    }

    public func load() -> PostDraft? {
        guard let data = defaults.data(forKey: key) else { return nil }
        return try? JSONDecoder().decode(PostDraft.self, from: data)
    }

    public func clear() {
        defaults.removeObject(forKey: key)
    }
}

// MARK: - View model

/// The composer's whole behaviour, testable without SwiftUI: validation, the
/// exact pre-write review, the single signed write, the pending-exchange result,
/// draft persistence across backgrounding, and a draft-preserving fixed-copy
/// failure.
@MainActor
public final class PostUpdateViewModel: ObservableObject {
    /// The fixed failure copy. A raw internal error string never reaches a
    /// person — "InvalidInput" leaking to a user is exactly the mistake this
    /// avoids (mirrors `AppModel.approvalFailureMessage`).
    public static let writeFailureMessage =
        "Couldn't post your update just now. Your draft is safe — try posting again."

    /// The primary action's label — outcome language, never mechanism. The view
    /// draws exactly this; a test pins it so it can never regress to
    /// "Compose & sign".
    public static let primaryActionTitle = "Post an update"

    // Draft fields.
    @Published public var headline: String = ""
    @Published public var body: String = ""
    /// Model assistance is OFF by default (nav + newswire contract).
    @Published public var aiAssisted: Bool = false
    @Published public var mode: ComposerMode = .freeform
    @Published public var sourceClaims: [String] = []
    @Published public var coarseLocation: String = ""
    @Published public var expiresAt: Date?

    @Published public private(set) var status: PostUpdateStatus = .editing
    /// Fixed, human failure copy — nil unless the last write failed.
    @Published public private(set) var errorMessage: String?

    public let identity: PublishingIdentity
    public let community: PostingCommunity

    private let publisher: NewswirePostPublishing
    private let draftStore: PostDraftStore
    private let now: () -> Date

    public init(
        identity: PublishingIdentity,
        community: PostingCommunity,
        publisher: NewswirePostPublishing,
        draftStore: PostDraftStore,
        now: @escaping () -> Date = Date.init
    ) {
        self.identity = identity
        self.community = community
        self.publisher = publisher
        self.draftStore = draftStore
        self.now = now
        restoreDraft()
    }

    // MARK: Review

    /// Exactly what the person is shown before a single signed write happens:
    /// which identity, which community, and — when it applies — that the identity
    /// is a labeled one-off. Nothing is signed until this has been presented.
    public var review: PostUpdateReview {
        PostUpdateReview(
            identityLabel: identity.reviewLabel,
            communityName: community.name,
            isEphemeralIdentity: identity.isEphemeralOneOff
        )
    }

    // MARK: Validation

    public var validation: PostUpdateValidation {
        let hasHeadline = !headline.trimmed.isEmpty
        let hasBody = !body.trimmed.isEmpty
        guard hasHeadline, hasBody else { return .needsHeadlineAndBody }

        if mode.requiresStricterFields {
            var missing: [String] = []
            if trimmedSourceClaims.isEmpty { missing.append("a source claim") }
            if expiresAt == nil { missing.append("an expiry") }
            if coarseLocation.trimmed.isEmpty { missing.append("a coarse location") }
            if !missing.isEmpty { return .needsOperationalFields(missing) }
        }
        return .ready
    }

    /// Whether the primary "Post an update" action is enabled.
    public var canPost: Bool {
        guard validation.isReady else { return false }
        switch status {
        case .editing:
            return true
        case .posting, .posted:
            return false
        }
    }

    /// Plain-language guidance for why Post is disabled, or nil when ready. So the
    /// composer explains what's still needed instead of a silent dead-disable —
    /// the exact stranding an operational mode would otherwise cause. This is
    /// presentation of the already-computed `validation`, not new business logic.
    public var validationGuidance: String? {
        switch validation {
        case .ready:
            return nil
        case .needsHeadlineAndBody:
            return "Add a headline and body to post."
        case let .needsOperationalFields(missing):
            // missing is already human: "a source claim", "an expiry", "a coarse location".
            return "To post \(mode.label.lowercased()), add \(missing.joined(separator: ", "))."
        }
    }

    // MARK: Draft persistence

    /// The current draft, as it would be persisted.
    public var currentDraft: PostDraft {
        PostDraft(
            headline: headline,
            body: body,
            aiAssisted: aiAssisted,
            sourceClaims: sourceClaims,
            coarseLocation: coarseLocation
        )
    }

    /// Persist the in-progress draft. Called when the view is backgrounded or
    /// dismissed so a half-written post is not lost. A successfully posted or
    /// empty draft is not persisted — there is nothing to restore.
    public func persistDraft() {
        if case .posted = status { return }
        let draft = currentDraft
        if draft.isEmpty {
            draftStore.clear()
        } else {
            draftStore.save(draft)
        }
    }

    private func restoreDraft() {
        guard let draft = draftStore.load() else { return }
        headline = draft.headline
        body = draft.body
        aiAssisted = draft.aiAssisted
        sourceClaims = draft.sourceClaims
        coarseLocation = draft.coarseLocation
    }

    // MARK: Posting

    /// The one signed write. Guarded so a double-tap or a post after success
    /// cannot sign twice. On success the draft is cleared and the status carries
    /// the pending-exchange result; on failure the draft is preserved untouched
    /// and a fixed message is shown.
    public func post() {
        guard canPost else { return }
        status = .posting
        errorMessage = nil

        let request = PostUpdateRequest(
            spaceDescriptorEntryID: community.spaceDescriptorEntryID,
            headline: headline.trimmed,
            body: body.trimmed,
            aiAssisted: aiAssisted,
            sourceClaims: mode.requiresStricterFields ? trimmedSourceClaims : [],
            expiresAtUnixSeconds: expiresAt.map { UInt64(max(0, $0.timeIntervalSince1970)) },
            coarseLocation: mode.requiresStricterFields ? coarseLocation.trimmedOrNil : nil,
            operationalProfile: operationalProfile
        )

        do {
            let record = try publisher.publishNewswirePost(request)
            draftStore.clear()
            status = .posted(PostedUpdate(entryID: record.entryId))
        } catch {
            // Fixed copy, never the raw error; draft is left exactly as it was so
            // a retry starts from the same words.
            status = .editing
            errorMessage = Self.writeFailureMessage
        }
    }

    private var trimmedSourceClaims: [String] {
        sourceClaims.map { $0.trimmed }.filter { !$0.isEmpty }
    }

    /// Builds the operational profile for the signed write when a stricter mode
    /// is selected. Full alert/request authoring (urgency, severity, request
    /// kind) is a later refinement; 1B carries a defaulted profile so the
    /// `operational_profile` field 1A added is exercised end to end.
    private var operationalProfile: NewswireOperationalProfile? {
        switch mode {
        case .freeform:
            return nil
        case .operationalAlert:
            return .alert(profile: NewswireAlertProfile(
                urgency: .immediate,
                severity: .severe,
                certainty: .observed,
                validFromUnixSeconds: nil
            ))
        case .operationalRequest:
            return .request(profile: NewswireRequestProfile(
                kind: .need,
                neededByUnixSeconds: expiresAt.map { UInt64(max(0, $0.timeIntervalSince1970)) },
                contactInstructions: coarseLocation.trimmed
            ))
        }
    }
}

/// The immutable pre-write review a person reads before anything is signed.
public struct PostUpdateReview: Equatable, Sendable {
    public let identityLabel: String
    public let communityName: String
    public let isEphemeralIdentity: Bool

    public init(identityLabel: String, communityName: String, isEphemeralIdentity: Bool) {
        self.identityLabel = identityLabel
        self.communityName = communityName
        self.isEphemeralIdentity = isEphemeralIdentity
    }
}

private extension String {
    var trimmed: String { trimmingCharacters(in: .whitespacesAndNewlines) }
    var trimmedOrNil: String? {
        let value = trimmed
        return value.isEmpty ? nil : value
    }
}

// MARK: - View

/// The Post-an-update composer. Built as a standalone view + model; Unit 2A
/// hosts it as a primary Home action. It never says "Compose & sign" — a person
/// posts an update; the signing is the app's job, not the label's.
public struct PostUpdateView: View {
    @ObservedObject var model: PostUpdateViewModel
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.scenePhase) private var scenePhase
    var onPosted: (PostedUpdate) -> Void = { _ in }

    public init(model: PostUpdateViewModel, onPosted: @escaping (PostedUpdate) -> Void = { _ in }) {
        self.model = model
        self.onPosted = onPosted
    }

    public var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                modeCard
                draftCard
                if model.mode.requiresStricterFields { operationalCard }
                reviewCard
                if let error = model.errorMessage {
                    failureCard(error)
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Share with your community", "Post an update")
        // A half-written post survives the view leaving the foreground.
        .onChange(of: scenePhase) { _, phase in
            if phase != .active { model.persistDraft() }
        }
        .onDisappear { model.persistDraft() }
        .onChange(of: model.status) { _, status in
            if case let .posted(update) = status { onPosted(update) }
        }
    }

    private var modeCard: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 10) {
                eyebrow("What kind of post")
                Picker("Post kind", selection: $model.mode) {
                    ForEach(ComposerMode.allCases, id: \.self) { mode in
                        Text(mode.label).tag(mode)
                    }
                }
                .pickerStyle(.segmented)
                .accessibilityIdentifier("post-mode-picker")
            }
        }
    }

    // A single-source-claim binding onto the model's [String] (finer multi-source
    // authoring is a later refinement; validation needs one non-empty claim).
    private var sourceClaimBinding: Binding<String> {
        Binding(
            get: { model.sourceClaims.first ?? "" },
            set: { model.sourceClaims = $0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? [] : [$0] }
        )
    }

    // The expiry starts unset (model.expiresAt == nil) so Alert/Request are honestly
    // incomplete until the person sets one. A toggle reveals the picker; turning it
    // off clears the expiry back to nil (validation fails again — no silent default).
    private var hasExpiryBinding: Binding<Bool> {
        Binding(
            get: { model.expiresAt != nil },
            set: { model.expiresAt = $0 ? (model.expiresAt ?? Date()) : nil }
        )
    }

    private var operationalCard: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 14) {
                eyebrow(model.mode == .operationalAlert ? "Alert details" : "Request details")
                TextField("Source (how you know)", text: sourceClaimBinding, axis: .vertical)
                    .font(.riot(.body, size: 15, relativeTo: .body))
                    .accessibilityIdentifier("post-source-claim")
                TextField("Coarse location (area, not a precise point)", text: $model.coarseLocation)
                    .font(.riot(.body, size: 15, relativeTo: .body))
                    .accessibilityIdentifier("post-coarse-location")
                Toggle("Set an expiry", isOn: hasExpiryBinding)
                    .tint(RiotTheme.pink(for: colorScheme))
                    .accessibilityIdentifier("post-expiry-toggle")
                if model.expiresAt != nil {
                    DatePicker(
                        "Expires",
                        selection: Binding(get: { model.expiresAt ?? Date() }, set: { model.expiresAt = $0 }),
                        displayedComponents: [.date, .hourAndMinute]
                    )
                    .accessibilityIdentifier("post-expiry-picker")
                }
            }
        }
    }

    private var draftCard: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 14) {
                eyebrow("Draft")
                TextField("Headline", text: $model.headline, axis: .vertical)
                    .font(.riot(.body, size: 17, relativeTo: .body))
                    .accessibilityIdentifier("post-headline")
                TextField("What people need to know", text: $model.body, axis: .vertical)
                    .font(.riot(.body, size: 15, relativeTo: .body))
                    .lineLimit(4...8)
                    .accessibilityIdentifier("post-body")
                Toggle("Started with model assistance", isOn: $model.aiAssisted)
                    .tint(RiotTheme.pink(for: colorScheme))
                    .accessibilityIdentifier("post-ai-assist")
            }
        }
    }

    private var reviewCard: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 14) {
                eyebrow("Review before posting")
                    .accessibilityIdentifier("post-review")
                Text("Posting as \(model.review.identityLabel).")
                    .font(.riot(.body, size: 15, relativeTo: .callout))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    .accessibilityIdentifier("post-review-identity")
                Text("To \(model.review.communityName). Only you can post it.")
                    .font(.riot(.body, size: 15, relativeTo: .callout))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    .accessibilityIdentifier("post-review-community")
                if model.review.isEphemeralIdentity {
                    RiotBadge("One-off identity · not recoverable if lost")
                        .accessibilityIdentifier("post-review-ephemeral")
                }
                if case let .posted(update) = model.status {
                    Text(update.exchangeStatus)
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        .accessibilityIdentifier("post-pending-exchange")
                } else {
                    if let guidance = model.validationGuidance {
                        Text(guidance)
                            .font(.riot(.body, size: 13, relativeTo: .footnote))
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            .accessibilityIdentifier("post-validation-guidance")
                    }
                    Button(PostUpdateViewModel.primaryActionTitle, action: model.post)
                        .buttonStyle(.riotPrimary)
                        .accessibilityIdentifier("post-update")
                        .disabled(!model.canPost)
                }
            }
        }
    }

    private func failureCard(_ error: String) -> some View {
        RiotCard {
            Text(error)
                .font(.riot(.body, size: 15, relativeTo: .callout))
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                .accessibilityIdentifier("post-error")
        }
    }

    private func eyebrow(_ text: String) -> some View {
        Text(text)
            .font(.riot(.mono, size: 12, relativeTo: .caption))
            .textCase(.uppercase)
            .tracking(1)
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
    }
}
