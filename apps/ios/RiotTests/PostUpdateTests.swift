import XCTest
@testable import RiotKit

/// Unit 1B — the Post-an-update composer, tested in isolation (no store, no FFI).
/// The signed-write seam and the draft store are injected so every contract is
/// deterministic: the outcome-language label, model-assistance-off default, the
/// exact pre-write review, one signed write, the pending-exchange result, a
/// draft that survives backgrounding, and a failure that preserves the draft
/// behind a fixed message.
final class PostUpdateTests: XCTestCase {
    // MARK: - Test doubles

    /// A publisher that records every call and returns a fixed record, or throws
    /// on demand. `callCount` is how "exactly one signed write" is proven.
    private final class StubPublisher: NewswirePostPublishing {
        private(set) var callCount = 0
        private(set) var lastRequest: PostUpdateRequest?
        var error: Error?
        let record = NewswireSignedRecord(
            entryId: String(repeating: "e", count: 64),
            signedBytes: Data([0x01, 0x02, 0x03])
        )

        func publishNewswirePost(_ request: PostUpdateRequest) throws -> NewswireSignedRecord {
            callCount += 1
            lastRequest = request
            if let error { throw error }
            return record
        }
    }

    private struct RawInternalError: Error {
        let description = "InvalidInput { field: \"headline\" }"
    }

    /// A draft store that lives only in memory, so backgrounding can be simulated
    /// by handing the same store to a second view model.
    private final class MemoryDraftStore: PostDraftStore {
        private(set) var stored: PostDraft?
        private(set) var clearCount = 0

        func save(_ draft: PostDraft) { stored = draft }
        func load() -> PostDraft? { stored }
        func clear() { stored = nil; clearCount += 1 }
    }

    // MARK: - Fixtures

    private static func person(_ name: String = "Ana", tag: String = "a3f91122") -> RiotPerson {
        RiotPerson(id: String(repeating: "1", count: 64), displayName: name, tag: tag)
    }

    private static func community() -> PostingCommunity {
        PostingCommunity(name: "Riverside Mutual Aid", spaceDescriptorEntryID: String(repeating: "d", count: 64))
    }

    @MainActor
    private func makeModel(
        identity: PublishingIdentity? = nil,
        publisher: StubPublisher = StubPublisher(),
        store: MemoryDraftStore = MemoryDraftStore()
    ) -> PostUpdateViewModel {
        PostUpdateViewModel(
            identity: identity ?? .persistent(Self.person()),
            community: Self.community(),
            publisher: publisher,
            draftStore: store
        )
    }

    // MARK: - Outcome language, not mechanism

    /// The primary action is a human outcome. If this ever regresses to
    /// "Compose & sign" (the framing `ComposeReviewSignView` used) the whole
    /// point of the unit is lost, so it is pinned.
    @MainActor
    func testPrimaryActionUsesOutcomeLanguageNotSigningMechanism() {
        XCTAssertEqual(PostUpdateViewModel.primaryActionTitle, "Post an update")
        let lowered = PostUpdateViewModel.primaryActionTitle.lowercased()
        XCTAssertFalse(lowered.contains("sign"), "the action must not name the signing mechanism")
        XCTAssertFalse(lowered.contains("compose"), "the action must not read as 'Compose & sign'")
    }

    // MARK: - Model assistance off by default

    @MainActor
    func testModelAssistanceIsOffByDefault() {
        let model = makeModel()
        XCTAssertFalse(model.aiAssisted, "any AI-assist toggle must start off")
    }

    // MARK: - Exact review before one signed write

    /// Reading the review does not sign anything, and it names exactly which
    /// identity and which community the post would go out as / to.
    @MainActor
    func testReviewNamesIdentityAndCommunityWithoutWriting() {
        let publisher = StubPublisher()
        let model = makeModel(publisher: publisher)

        XCTAssertEqual(model.review.identityLabel, "Ana · a3f91122")
        XCTAssertEqual(model.review.communityName, "Riverside Mutual Aid")
        XCTAssertFalse(model.review.isEphemeralIdentity)
        XCTAssertEqual(publisher.callCount, 0, "showing the review must not sign anything")
    }

    // MARK: - Happy path: one signed write → pending exchange

    @MainActor
    func testHappyPathSignsOnceAndReportsPendingExchange() {
        let publisher = StubPublisher()
        let store = MemoryDraftStore()
        let model = makeModel(publisher: publisher, store: store)

        model.headline = "Water at the east entrance"
        model.body = "Bring a bottle; volunteers are refilling the tank."

        XCTAssertTrue(model.canPost)
        model.post()

        // Exactly one signed write.
        XCTAssertEqual(publisher.callCount, 1)

        // It carried what the review named: the reviewed community's descriptor,
        // and the composed words.
        let request = try? XCTUnwrap(publisher.lastRequest)
        XCTAssertEqual(request?.spaceDescriptorEntryID, Self.community().spaceDescriptorEntryID)
        XCTAssertEqual(request?.headline, "Water at the east entrance")
        XCTAssertEqual(request?.body, "Bring a bottle; volunteers are refilling the tank.")
        XCTAssertEqual(request?.aiAssisted, false)
        XCTAssertNil(request?.operationalProfile, "a freeform post has no operational profile")

        // The committed-but-not-exchanged result.
        guard case let .posted(update) = model.status else {
            return XCTFail("a successful post lands in .posted")
        }
        XCTAssertEqual(update.entryID, publisher.record.entryId)
        XCTAssertEqual(update.exchangeStatus, "Pending nearby exchange")
        XCTAssertNil(model.errorMessage)

        // A posted draft leaves nothing to restore.
        XCTAssertEqual(store.clearCount, 1)
        XCTAssertNil(store.load())
    }

    /// A second tap after a successful post must not sign again.
    @MainActor
    func testPostingAgainAfterSuccessDoesNotSignTwice() {
        let publisher = StubPublisher()
        let model = makeModel(publisher: publisher)
        model.headline = "Headline"
        model.body = "Body"

        model.post()
        model.post()

        XCTAssertEqual(publisher.callCount, 1)
        XCTAssertFalse(model.canPost, "the composer is spent once the post is committed")
    }

    // MARK: - Failure preserves the draft behind a fixed message

    @MainActor
    func testWriteFailurePreservesDraftAndShowsFixedError() {
        let publisher = StubPublisher()
        publisher.error = RawInternalError()
        let store = MemoryDraftStore()
        let model = makeModel(publisher: publisher, store: store)

        model.headline = "Medic tent moved"
        model.body = "It is now at the north gate."
        model.post()

        // The draft is untouched — not blanked.
        XCTAssertEqual(model.headline, "Medic tent moved")
        XCTAssertEqual(model.body, "It is now at the north gate.")
        // Back to editing so a retry is possible.
        XCTAssertEqual(model.status, .editing)
        XCTAssertTrue(model.canPost)

        // A fixed message, never the raw internal error.
        XCTAssertEqual(model.errorMessage, PostUpdateViewModel.writeFailureMessage)
        XCTAssertFalse(
            model.errorMessage?.contains("InvalidInput") ?? false,
            "a raw internal error string must never reach a person"
        )
        // The failed write did not wipe the store.
        XCTAssertEqual(store.clearCount, 0)
    }

    // MARK: - Ephemeral one-off identity clearly labeled

    @MainActor
    func testEphemeralIdentityIsClearlyLabeled() {
        let ephemeral = makeModel(identity: .ephemeralOneOff(Self.person("Guest", tag: "9c0011aa")))
        XCTAssertTrue(ephemeral.review.isEphemeralIdentity)
        XCTAssertTrue(
            ephemeral.review.identityLabel.contains("one-off"),
            "an ephemeral identity's review label must say so"
        )

        let persistent = makeModel(identity: .persistent(Self.person()))
        XCTAssertFalse(persistent.review.isEphemeralIdentity)
        XCTAssertFalse(persistent.review.identityLabel.contains("one-off"))
    }

    // MARK: - Freeform vs operational field requirements

    @MainActor
    func testFreeformNeedsOnlyHeadlineAndBody() {
        let model = makeModel()
        XCTAssertEqual(model.validation, .needsHeadlineAndBody)

        model.headline = "Headline"
        XCTAssertEqual(model.validation, .needsHeadlineAndBody, "a headline alone is not enough")

        model.body = "Body"
        XCTAssertEqual(model.validation, .ready)
        XCTAssertTrue(model.canPost)
    }

    /// The stricter fields are required ONLY once an operational profile is
    /// selected — the newswire rule that supersedes the nav design's blanket
    /// "sources + expiry required".
    @MainActor
    func testOperationalProfileRequiresStricterFields() {
        let publisher = StubPublisher()
        let model = makeModel(publisher: publisher)
        model.headline = "Tear gas at the south barricade"
        model.body = "Move north; medics are staging by the fountain."

        // Freeform: ready with just headline + body.
        XCTAssertEqual(model.validation, .ready)

        // Switching to an operational alert now demands the stricter fields.
        model.mode = .operationalAlert
        guard case let .needsOperationalFields(missing) = model.validation else {
            return XCTFail("an operational profile requires stricter fields")
        }
        XCTAssertFalse(missing.isEmpty)
        XCTAssertFalse(model.canPost)

        // Supplying all three makes it postable again, and the write carries the
        // operational profile.
        model.sourceClaims = ["Saw it myself"]
        model.coarseLocation = "South barricade"
        model.expiresAt = Date(timeIntervalSince1970: 1_720_003_600)
        XCTAssertEqual(model.validation, .ready)

        model.post()
        XCTAssertEqual(publisher.callCount, 1)
        XCTAssertNotNil(publisher.lastRequest?.operationalProfile)
        XCTAssertEqual(publisher.lastRequest?.sourceClaims, ["Saw it myself"])
        XCTAssertEqual(publisher.lastRequest?.coarseLocation, "South barricade")
        XCTAssertEqual(publisher.lastRequest?.expiresAtUnixSeconds, 1_720_003_600)
    }

    // MARK: - Draft survives backgrounding

    /// A half-written post persisted on backgrounding is restored when the view
    /// is returned to — modelled here as a second view model over the same store.
    @MainActor
    func testDraftSurvivesBackgroundingAndDismissal() {
        let store = MemoryDraftStore()
        let first = makeModel(store: store)
        first.headline = "Half a headline"
        first.body = "Half a body"
        first.aiAssisted = true

        // Backgrounding / dismissal persists the draft.
        first.persistDraft()

        // Returning to the composer restores it.
        let restored = makeModel(store: store)
        XCTAssertEqual(restored.headline, "Half a headline")
        XCTAssertEqual(restored.body, "Half a body")
        XCTAssertTrue(restored.aiAssisted)
    }

    /// A successful post is not re-persisted as a draft — there is nothing to
    /// restore, and the next composer starts clean.
    @MainActor
    func testPersistingAfterASuccessfulPostRestoresNothing() {
        let store = MemoryDraftStore()
        let model = makeModel(store: store)
        model.headline = "Headline"
        model.body = "Body"
        model.post()

        model.persistDraft()

        let next = makeModel(store: store)
        XCTAssertEqual(next.headline, "")
        XCTAssertEqual(next.body, "")
    }

    /// An empty composer persists nothing — no stale empty draft is left behind.
    @MainActor
    func testEmptyDraftIsNotPersisted() {
        let store = MemoryDraftStore()
        let model = makeModel(store: store)

        model.persistDraft()

        XCTAssertNil(store.load())
    }

    // MARK: - Mode picker (Unit 6)

    @MainActor
    func testModeSelectionSwitchesTheModel() {
        let model = makeModel()
        XCTAssertEqual(model.mode, .freeform, "the composer opens in Update mode")

        model.mode = .operationalAlert
        XCTAssertTrue(model.mode.requiresStricterFields)

        model.mode = .operationalRequest
        XCTAssertTrue(model.mode.requiresStricterFields)

        model.mode = .freeform
        XCTAssertFalse(model.mode.requiresStricterFields, "Update pulls in no extra fields")
    }

    @MainActor
    func testModeLabelsAreOutcomeLanguageNotMechanism() {
        XCTAssertEqual(ComposerMode.freeform.label, "Update")
        XCTAssertEqual(ComposerMode.operationalAlert.label, "Alert")
        XCTAssertEqual(ComposerMode.operationalRequest.label, "Request")
        XCTAssertEqual(ComposerMode.allCases.count, 3, "the picker offers exactly Update/Alert/Request")
    }

    // MARK: - Operational fields visibility (Unit 6)

    @MainActor
    func testOperationalFieldsAreHiddenForUpdateAndShownForAlertAndRequest() {
        let model = makeModel()
        XCTAssertFalse(model.mode.requiresStricterFields, "Update: no operational fields")

        model.mode = .operationalAlert
        XCTAssertTrue(model.mode.requiresStricterFields, "Alert: operational fields shown")

        model.mode = .operationalRequest
        XCTAssertTrue(model.mode.requiresStricterFields, "Request: operational fields shown")
    }

    @MainActor
    func testOperationalFieldBindingsFeedValidationAndTheSignedWrite() {
        let publisher = StubPublisher()
        let model = makeModel(publisher: publisher)
        model.headline = "Tear gas at the south barricade"
        model.body = "Move north; medics are staging by the fountain."
        model.mode = .operationalAlert

        // Fields empty → not ready.
        guard case .needsOperationalFields = model.validation else {
            return XCTFail("empty operational fields must not validate")
        }

        // The three inputs the view binds.
        model.sourceClaims = ["Saw it myself"]     // source-claim field
        model.coarseLocation = "South barricade"   // coarse-location field
        model.expiresAt = Date(timeIntervalSince1970: 1_720_003_600)  // expiry picker

        XCTAssertEqual(model.validation, .ready)
        model.post()
        XCTAssertEqual(publisher.lastRequest?.sourceClaims, ["Saw it myself"])
        XCTAssertEqual(publisher.lastRequest?.coarseLocation, "South barricade")
        XCTAssertEqual(publisher.lastRequest?.expiresAtUnixSeconds, 1_720_003_600)
    }

    // MARK: - Post is never dead-disabled (gate-r1 blocker, Unit 6)

    @MainActor
    func testAlertWithEmptyFieldsDisablesPostButShowsActionableGuidance() {
        let model = makeModel()
        model.headline = "Headline"
        model.body = "Body"
        // Update mode: ready, no guidance.
        XCTAssertTrue(model.canPost)
        XCTAssertNil(model.validationGuidance)

        // Alert with nothing supplied: disabled, but NOT silently — guidance lists what's missing.
        model.mode = .operationalAlert
        XCTAssertFalse(model.canPost)
        let guidance = try? XCTUnwrap(model.validationGuidance)
        XCTAssertTrue(guidance?.contains("source") ?? false)
        XCTAssertTrue(guidance?.contains("expiry") ?? false)
        XCTAssertTrue(guidance?.contains("location") ?? false)
    }

    @MainActor
    func testSupplyingOperationalFieldsEnablesPostAndClearsGuidance() {
        let model = makeModel()
        model.headline = "Headline"
        model.body = "Body"
        model.mode = .operationalRequest
        XCTAssertFalse(model.canPost)

        model.sourceClaims = ["A neighbour told me"]
        model.coarseLocation = "North gate"
        model.expiresAt = Date(timeIntervalSince1970: 1_720_003_600)

        XCTAssertTrue(model.canPost, "Alert/Request must never strand Post once its fields are supplied")
        XCTAssertNil(model.validationGuidance, "no guidance once ready")
    }
}
