import XCTest
@testable import RiotKit

/// Unit 2C — editorial actions, front page & open wire (iOS surface).
///
/// This suite proves two kinds of thing:
///
///  1. **Pure surface logic** — the closed editorial field table, the three
///     distinct wire states, treatment rendering, the immutable pre-signing
///     review, and the editor-visibility hint — asserted as values, no store.
///     These are the SAME rules the Android `RiotControllerNewswireTest` asserts,
///     so both platforms derive an identical surface from identical records.
///
///  2. **Real authorization end-to-end** — driven through the REAL `MobileProfile`
///     and `riot-core` linked into `RiotKit` (exactly as `NewswireShareTests`
///     runs the real encoder). The `LiveNewswire` adapter forwards to the same FFI
///     calls the shipping `RiotProfileRepository` makes, so a rejection here is
///     genuinely core's, not a stub's imitation. The two authorization tests the
///     coordinator scrutinizes — a NON-EDITOR's action has NO EFFECT, and UI
///     visibility is NEVER the authorization check — assert the post's treatment
///     is UNCHANGED (not merely that a control is hidden).
@MainActor
final class NewswireSurfaceTests: XCTestCase {

    // MARK: - Live FFI adapter (the real core path)

    /// Forwards the two editorial seams to a real `MobileProfile`. Identical to
    /// what `RiotProfileRepository`'s Newswire extension does, so the model under
    /// test exercises the genuine core rejection.
    private final class LiveNewswire: NewswireProjecting, NewswireEditorialActing,
        NewswireEditorAuthorityChecking {
        let profile: MobileProfile
        init(_ profile: MobileProfile) { self.profile = profile }

        func projectNewswire(spaceDescriptorEntryID: String) throws -> NewswireProjectionView {
            try profile.projectNewswireSpace(spaceDescriptorEntryId: spaceDescriptorEntryID)
        }

        /// The genuine core predicate (Unit 4a) — identical to the shipping
        /// `RiotProfileRepository` wrapper, so the model reads the real authority.
        func newswireIsEditor(spaceDescriptorEntryID: String, subjectID: String) throws -> Bool {
            try profile.newswireIsEditor(descriptorEntryId: spaceDescriptorEntryID, subjectId: subjectID)
        }

        func createNewswireEditorialAction(
            spaceDescriptorEntryID: String,
            targetEntryID: String,
            kind: NewswireEditorialActionKind,
            reason: String?,
            correctionText: String?
        ) throws -> NewswireSignedRecord {
            try profile.createNewswireEditorialAction(input: NewswireEditorialActionInput(
                spaceDescriptorEntryId: spaceDescriptorEntryID,
                targetEntryId: targetEntryID,
                kind: kind,
                reason: reason,
                correctionText: correctionText
            ))
        }
    }

    private func spaceInput(_ name: String, roster: [String] = []) -> NewswireSpaceInput {
        NewswireSpaceInput(
            name: name,
            summary: "Community newswire.",
            languages: ["en"],
            geographicTags: [],
            topicTags: [],
            editorialRoster: roster
        )
    }

    private func postInput(_ spaceID: String, _ headline: String) -> NewswirePostInput {
        NewswirePostInput(
            spaceDescriptorEntryId: spaceID,
            headline: headline,
            body: "Body of the report.",
            language: "en",
            eventTimeUnixSeconds: nil,
            expiresAtUnixSeconds: nil,
            coarseLocation: nil,
            sourceClaims: [],
            operationalProfile: nil,
            aiAssisted: false
        )
    }

    /// A model whose authority is the LIVE core, keyed on the profile's REAL
    /// whoami id (the old `myKeyHex: "aa"*32` never mattered because the replaced
    /// static ignored the key for an empty roster — the predicate does not, so the
    /// real id is load-bearing now). The roster lives in the descriptor created via
    /// `spaceInput(_, roster:)`; the model reads it through the predicate, not a
    /// passed array.
    private func liveModel(profile: MobileProfile, spaceID: String) throws -> NewswireSurfaceModel {
        let live = LiveNewswire(profile)
        return NewswireSurfaceModel(
            projector: live,
            editor: live,
            authority: live,
            spaceDescriptorEntryID: spaceID,
            communityName: "Riverside",
            myKeyHex: RiotDirectoryRow.hex(try profile.profile().whoami().id)
        )
    }

    // MARK: - Closed field table (all six kinds)

    /// The exact closed table from the newswire design, asserted kind-by-kind.
    /// feature/verify forbid both fields; correct requires both; hide/tombstone/
    /// retract require a reason and forbid replacement text.
    func testClosedFieldRulesMatchTheDesignForAllSixKinds() {
        XCTAssertEqual(EditorialActionKind.feature.fieldRules,
                       EditorialFieldRules(reason: .forbidden, correctionText: .forbidden))
        XCTAssertEqual(EditorialActionKind.verify.fieldRules,
                       EditorialFieldRules(reason: .forbidden, correctionText: .forbidden))
        XCTAssertEqual(EditorialActionKind.correct.fieldRules,
                       EditorialFieldRules(reason: .required, correctionText: .required))
        XCTAssertEqual(EditorialActionKind.hide.fieldRules,
                       EditorialFieldRules(reason: .required, correctionText: .forbidden))
        XCTAssertEqual(EditorialActionKind.tombstone.fieldRules,
                       EditorialFieldRules(reason: .required, correctionText: .forbidden))
        XCTAssertEqual(EditorialActionKind.retract.fieldRules,
                       EditorialFieldRules(reason: .required, correctionText: .forbidden))
        // Every kind is covered — no kind is left without a rule.
        XCTAssertEqual(EditorialActionKind.allCases.count, 6)
    }

    func testFeatureAndVerifyRejectAnyReasonOrReplacementText() {
        for kind in [EditorialActionKind.feature, .verify] {
            XCTAssertEqual(
                EditorialActionValidator.validate(.init(kind: kind, reason: "because")),
                .failure(.reasonForbidden), "\(kind) must forbid a reason")
            XCTAssertEqual(
                EditorialActionValidator.validate(.init(kind: kind, correctionText: "new text")),
                .failure(.correctionForbidden), "\(kind) must forbid replacement text")
            // Both empty ⇒ valid, and both fields become nil for the wire.
            XCTAssertEqual(
                EditorialActionValidator.validate(.init(kind: kind)),
                .success(ValidatedEditorialAction(kind: kind, reason: nil, correctionText: nil)))
        }
    }

    func testCorrectRequiresBothReasonAndReplacementTextNonEmpty() {
        XCTAssertEqual(
            EditorialActionValidator.validate(.init(kind: .correct, reason: "", correctionText: "fix")),
            .failure(.reasonRequired))
        XCTAssertEqual(
            EditorialActionValidator.validate(.init(kind: .correct, reason: "wrong date", correctionText: "  ")),
            .failure(.correctionRequired), "whitespace-only replacement is not replacement text")
        XCTAssertEqual(
            EditorialActionValidator.validate(.init(kind: .correct, reason: "wrong date", correctionText: "May 2")),
            .success(ValidatedEditorialAction(kind: .correct, reason: "wrong date", correctionText: "May 2")))
    }

    func testHideTombstoneRetractRequireReasonAndForbidReplacementText() {
        for kind in [EditorialActionKind.hide, .tombstone, .retract] {
            XCTAssertEqual(
                EditorialActionValidator.validate(.init(kind: kind, reason: "   ")),
                .failure(.reasonRequired), "\(kind) requires a non-empty reason")
            XCTAssertEqual(
                EditorialActionValidator.validate(.init(kind: kind, reason: "unverified", correctionText: "x")),
                .failure(.correctionForbidden), "\(kind) forbids replacement text")
            XCTAssertEqual(
                EditorialActionValidator.validate(.init(kind: kind, reason: "unverified")),
                .success(ValidatedEditorialAction(kind: kind, reason: "unverified", correctionText: nil)))
        }
    }

    func testOnlyCorrectCarriesTheEditorialCorrectionLabel() {
        XCTAssertTrue(EditorialActionKind.correct.isEditorialCorrection)
        for kind in EditorialActionKind.allCases where kind != .correct {
            XCTAssertFalse(kind.isEditorialCorrection, "\(kind) must not read as a correction")
        }
        XCTAssertEqual(EditorialCorrectionLabel.text, "Editorial correction")
    }

    // MARK: - Immutable pre-signing review

    func testReviewShowsEveryCompleteIdentifierUntruncated() {
        let target = "cd".repeated(32)   // full 64-hex entry id
        let editorKey = "ef".repeated(32)
        guard case let .success(validated) =
            EditorialActionValidator.validate(.init(kind: .correct, reason: "typo", correctionText: "fixed")) else {
            return XCTFail("valid correction")
        }
        let review = EditorialActionReview(
            action: validated,
            targetEntryID: target,
            communityName: "Riverside",
            actingEditorKeyHex: editorKey
        )
        XCTAssertEqual(review.targetEntryID, target)          // not truncated
        XCTAssertEqual(review.actingEditorKeyHex, editorKey)  // not truncated
        XCTAssertEqual(review.communityName, "Riverside")
        XCTAssertEqual(review.kind, .correct)
        XCTAssertEqual(review.reason, "typo")
        XCTAssertEqual(review.replacementText, "fixed")

        let labels = review.rows.map(\.label)
        XCTAssertEqual(labels, ["Action", "Community", "Target entry", "Acting editor", "Reason", "Replacement text"])
        // The complete ids appear verbatim in the rows a person signs against.
        XCTAssertTrue(review.rows.contains { $0.value == target })
        XCTAssertTrue(review.rows.contains { $0.value == editorKey })
    }

    func testReviewOmitsForbiddenFieldsForFeature() {
        guard case let .success(validated) =
            EditorialActionValidator.validate(.init(kind: .feature)) else {
            return XCTFail("valid feature")
        }
        let review = EditorialActionReview(
            action: validated, targetEntryID: "01".repeated(32),
            communityName: "Riverside", actingEditorKeyHex: "02".repeated(32))
        XCTAssertNil(review.reason)
        XCTAssertNil(review.replacementText)
        XCTAssertEqual(review.rows.map(\.label), ["Action", "Community", "Target entry", "Acting editor"])
    }

    // MARK: - Three DISTINCT wire states

    func testEmptyWirePostsButNoFeatureAndOfflineStaleAreThreeDistinctStates() {
        let empty = NewswireWireState.from(projection(openWire: [], frontPage: []))
        XCTAssertEqual(empty, .emptyWire)

        let post = projectedPost(id: "a1", headline: "Report", treatment: .ordinary)
        let noFeature = NewswireWireState.from(projection(openWire: [post], frontPage: []))
        guard case .postsButNoFeature = noFeature else { return XCTFail("posts but no feature") }

        let featured = NewswireWireState.from(projection(openWire: [post], frontPage: [post]))
        guard case .featured = featured else { return XCTFail("featured") }

        // Distinct accessibility ids: the three never collapse to one view.
        let ids = Set([
            NewswireWireState.emptyWire.accessibilityID,
            noFeature.accessibilityID,
            NewswireWireState.offlineStale.accessibilityID,
            featured.accessibilityID,
        ])
        XCTAssertEqual(ids.count, 4)

        // Each of the three non-featured states has its own copy.
        XCTAssertNotEqual(NewswireWireCopy.emptyMessage, NewswireWireCopy.noFeatureMessage)
        XCTAssertNotEqual(NewswireWireCopy.noFeatureMessage, NewswireWireCopy.offlineMessage)
        XCTAssertNotEqual(NewswireWireCopy.emptyMessage, NewswireWireCopy.offlineMessage)
    }

    func testMissingDescriptorIsOfflineStaleNeverAFabricatedEmptyWire() {
        // No descriptor id ⇒ the surface cannot honestly claim the wire is empty.
        let model = NewswireSurfaceModel(
            projector: ThrowingProjector(), editor: ThrowingEditor(), authority: ThrowingEditor(),
            spaceDescriptorEntryID: "", communityName: "Riverside",
            myKeyHex: "aa".repeated(32))
        model.load()
        XCTAssertEqual(model.wire, .offlineStale)
    }

    func testProjectionFailureIsOfflineStaleNeverARawError() {
        let model = NewswireSurfaceModel(
            projector: ThrowingProjector(), editor: ThrowingEditor(), authority: ThrowingEditor(),
            spaceDescriptorEntryID: "desc", communityName: "Riverside",
            myKeyHex: "aa".repeated(32))
        model.load()
        XCTAssertEqual(model.wire, .offlineStale)
    }

    // MARK: - Treatment rendering

    func testHiddenPostRendersTheWarningInterstitialAndDropsThePayload() {
        let hidden = projectedPost(id: "h1", headline: nil, treatment: .hidden)
        let row = NewswirePostRow(hidden)
        XCTAssertEqual(row.display, .hiddenInterstitial)
        XCTAssertNil(row.headline, "a hidden post shows no headline; the interstitial stands in for it")
    }

    func testTombstonedPostRendersTheTombstoneTreatment() {
        let tomb = projectedPost(id: "t1", headline: nil, treatment: .tombstoned)
        XCTAssertEqual(NewswirePostRow(tomb).display, .tombstoned)
        XCTAssertNotEqual(NewswireTreatmentCopy.hiddenBody, NewswireTreatmentCopy.tombstoneBody)
    }

    func testCorrectionOnAPostShowsTheEditorialCorrectionLabel() {
        let corrected = projectedPost(id: "c1", headline: "Report", treatment: .ordinary, correctionIDs: ["x"])
        XCTAssertTrue(NewswirePostRow(corrected).hasCorrection)
        // A retraction in history exposes no correction label; a correction does.
        let action = projectedAction(id: "act", kind: .correct, active: true)
        XCTAssertEqual(EditorialHistoryRow(action).correctionLabel, "Editorial correction")
        let feature = projectedAction(id: "f", kind: .feature, active: true)
        XCTAssertNil(EditorialHistoryRow(feature).correctionLabel)
    }

    // MARK: - Editor visibility (UI hint ONLY — never the authorization check)

    func testEditorVisibilityIsAPureHintDecoupledFromAuthorization() {
        // Unknown roster (a joined/loaded community) ⇒ never offered a control.
        XCTAssertFalse(EditorialAuthority.isRecognizedEditor(myKeyHex: "aa".repeated(32), roster: nil))
        // Empty roster ⇒ core's founder-alone default ⇒ the founder is an editor.
        XCTAssertTrue(EditorialAuthority.isRecognizedEditor(myKeyHex: "aa".repeated(32), roster: []))
        // Named ⇒ editor iff the key is in the roster.
        XCTAssertTrue(EditorialAuthority.isRecognizedEditor(
            myKeyHex: "aa".repeated(32), roster: ["AA".repeated(32)]))  // case-insensitive
        XCTAssertFalse(EditorialAuthority.isRecognizedEditor(
            myKeyHex: "aa".repeated(32), roster: ["11".repeated(32)]))
        // An empty key is never an editor.
        XCTAssertFalse(EditorialAuthority.isRecognizedEditor(myKeyHex: "", roster: []))
    }

    // MARK: - Predicate-driven visibility (Unit 4b)

    func testFounderInTheStoredRosterIsOfferedControlsViaTheCorePredicate() throws {
        let profile = try openLocalProfile()
        let mineHex = RiotDirectoryRow.hex(try profile.profile().whoami().id)
        let space = try profile.createNewswireSpace(input: spaceInput("Mine", roster: [mineHex]))
        let model = try liveModel(profile: profile, spaceID: space.entryId)
        model.load()
        XCTAssertTrue(model.canOfferEditorialControls, "a roster member is offered controls")
        XCTAssertNil(model.editorialControlsPendingNote, "an editor sees no pending note")
    }

    func testNonMemberIsNotOfferedControlsAndSeesNoMisleadingPendingNoteWhenSynced() throws {
        let profile = try openLocalProfile()
        let space = try profile.createNewswireSpace(input: spaceInput("Others", roster: ["11".repeated(32)]))
        _ = try profile.createNewswirePost(input: postInput(space.entryId, "Report"))  // wire has content ⇒ synced
        let model = try liveModel(profile: profile, spaceID: space.entryId)             // my key ∉ roster
        model.load()
        XCTAssertFalse(model.canOfferEditorialControls, "a non-member is not offered controls")
        XCTAssertNil(model.editorialControlsPendingNote,
                     "a synced non-editor is a reader, not told controls 'appear after sync'")
    }

    func testUnknownDescriptorShowsThePendingSyncNoteNotABareEmptyView() throws {
        let profile = try openLocalProfile()
        let mineHex = RiotDirectoryRow.hex(try profile.profile().whoami().id)
        // A descriptor id we hold no descriptor for (a joined community pre-first-sync).
        let live = LiveNewswire(profile)
        let model = NewswireSurfaceModel(projector: live, editor: live, authority: live,
            spaceDescriptorEntryID: "ab".repeated(32), communityName: "Pending", myKeyHex: mineHex)
        model.load()  // projection fails ⇒ wire == .offlineStale; predicate ⇒ false
        XCTAssertFalse(model.canOfferEditorialControls)
        XCTAssertEqual(model.editorialControlsPendingNote,
                       "Editorial controls appear after this community's first sync.")
    }

    func testEmptyDescriptorIdIsNeverAnEditorAndShowsNoNote() throws {
        let profile = try openLocalProfile()
        let live = LiveNewswire(profile)
        let model = NewswireSurfaceModel(projector: live, editor: live, authority: live,
            spaceDescriptorEntryID: "", communityName: "None",
            myKeyHex: RiotDirectoryRow.hex(try profile.profile().whoami().id))
        model.load()
        XCTAssertFalse(model.canOfferEditorialControls)
        XCTAssertNil(model.editorialControlsPendingNote, "no descriptor id at all ⇒ no editorial affordance or note")
    }

    // MARK: - REAL authorization end-to-end (through core)

    /// A recognized editor (empty founding roster ⇒ the founder) can sign each of
    /// the six kinds through the model, and each takes effect in the projection.
    func testRecognizedEditorCanSignAllSixKindsAndEachTakesEffect() throws {
        let profile = try openLocalProfile()
        let mineHex = RiotDirectoryRow.hex(try profile.profile().whoami().id)
        let space = try profile.createNewswireSpace(input: spaceInput("Six Kinds", roster: [mineHex]))
        let model = try liveModel(profile: profile, spaceID: space.entryId)
        model.load()
        XCTAssertTrue(model.canOfferEditorialControls, "the founder in the stored roster is an editor")

        // feature ⇒ the post reaches the front page.
        let featurePost = try profile.createNewswirePost(input: postInput(space.entryId, "Featured"))
        model.draft = EditorialActionDraft(kind: .feature)
        let featureOutcome = model.sign(targetEntryID: featurePost.entryId)
        guard case let .signed(featureActionID) = featureOutcome else {
            return XCTFail("feature should sign, got \(featureOutcome)")
        }
        if case let .featured(frontPage, _) = model.wire {
            XCTAssertTrue(frontPage.contains { $0.id == featurePost.entryId })
        } else {
            XCTFail("a featured post should put the wire in the featured state, got \(model.wire)")
        }

        // verify ⇒ signs and appears in history.
        let verifyPost = try profile.createNewswirePost(input: postInput(space.entryId, "Verified"))
        model.draft = EditorialActionDraft(kind: .verify)
        XCTAssertSigned(model.sign(targetEntryID: verifyPost.entryId))

        // correct ⇒ requires reason + replacement, and marks the post corrected.
        let correctPost = try profile.createNewswirePost(input: postInput(space.entryId, "Corrected"))
        model.draft = EditorialActionDraft(kind: .correct, reason: "wrong date", correctionText: "May 2")
        XCTAssertSigned(model.sign(targetEntryID: correctPost.entryId))

        // hide ⇒ the post is redacted to Hidden with no headline.
        let hidePost = try profile.createNewswirePost(input: postInput(space.entryId, "Hidden"))
        model.draft = EditorialActionDraft(kind: .hide, reason: "unverified")
        XCTAssertSigned(model.sign(targetEntryID: hidePost.entryId))
        let afterHide = try profile.projectNewswireSpace(spaceDescriptorEntryId: space.entryId)
        let hiddenRow = try XCTUnwrap(afterHide.openWire.first { $0.entryId == hidePost.entryId })
        XCTAssertEqual(hiddenRow.treatment, .hidden)
        XCTAssertNil(hiddenRow.headline)

        // tombstone ⇒ the post is redacted to Tombstoned.
        let tombPost = try profile.createNewswirePost(input: postInput(space.entryId, "Tombstoned"))
        model.draft = EditorialActionDraft(kind: .tombstone, reason: "doxxing")
        XCTAssertSigned(model.sign(targetEntryID: tombPost.entryId))
        let afterTomb = try profile.projectNewswireSpace(spaceDescriptorEntryId: space.entryId)
        let tombRow = try XCTUnwrap(afterTomb.openWire.first { $0.entryId == tombPost.entryId })
        XCTAssertEqual(tombRow.treatment, .tombstoned)

        // retract ⇒ targets a prior editorial action (the feature), and signs.
        model.draft = EditorialActionDraft(kind: .retract, reason: "featured in error")
        XCTAssertSigned(model.sign(targetEntryID: featureActionID))
        XCTAssertTrue(model.history.contains { $0.id == featureActionID && $0.kind == .feature })
    }

    /// THE authorization property. A profile that signed a founding roster which
    /// EXCLUDES its own key is NOT an editor. Its attempt to hide a post is
    /// rejected by core, the draft is preserved, and — the point — the post's
    /// treatment is UNCHANGED: the action had NO EFFECT, not merely a hidden
    /// button. (Mirrors the Rust
    /// `a_founding_roster_that_excludes_the_founder_denies_them_editorial_authority`.)
    func testANonEditorsActionIsIgnoredTheEffectIsAbsentNotJustTheControl() throws {
        let stranger = "11".repeated(32)
        let profile = try openLocalProfile()
        let space = try profile.createNewswireSpace(input: spaceInput("Delegated", roster: [stranger]))
        let post = try profile.createNewswirePost(input: postInput(space.entryId, "Standing report"))

        let model = try liveModel(profile: profile, spaceID: space.entryId)

        model.draft = EditorialActionDraft(kind: .hide, reason: "I want this gone")
        let outcome = model.sign(targetEntryID: post.entryId)

        // Core refused to sign — the app surfaces the rejection…
        XCTAssertEqual(outcome, .rejected)
        // …the draft is preserved so the person loses nothing…
        XCTAssertEqual(model.draft.reason, "I want this gone")
        // …and, decisively, the post is UNCHANGED: still ordinary, headline intact.
        let projection = try profile.projectNewswireSpace(spaceDescriptorEntryId: space.entryId)
        let row = try XCTUnwrap(projection.openWire.first { $0.entryId == post.entryId })
        XCTAssertEqual(row.treatment, .ordinary, "a non-editor's hide must not hide the post")
        XCTAssertEqual(row.headline, "Standing report", "the payload must survive an unauthorized hide")
    }

    /// "UI visibility is never an authorization check." The two halves are proven
    /// INDEPENDENTLY on the same non-editor: the control is hidden AND, even when
    /// the sign is called directly (bypassing the hidden control), the post's
    /// treatment is unchanged. The hidden button is a courtesy; core is the gate.
    func testHiddenControlAndRejectedActionAreIndependent() throws {
        let stranger = "22".repeated(32)
        let profile = try openLocalProfile()
        let space = try profile.createNewswireSpace(input: spaceInput("Independence", roster: [stranger]))
        let post = try profile.createNewswirePost(input: postInput(space.entryId, "Untouched"))
        let model = try liveModel(profile: profile, spaceID: space.entryId)
        model.load()

        // Half 1 — the control is not offered.
        XCTAssertFalse(model.canOfferEditorialControls)

        // Half 2 — INDEPENDENTLY, call sign() anyway (as if the control existed).
        // The effect must still be absent: authorization is not the button.
        model.draft = EditorialActionDraft(kind: .feature)
        XCTAssertEqual(model.sign(targetEntryID: post.entryId), .rejected)
        let projection = try profile.projectNewswireSpace(spaceDescriptorEntryId: space.entryId)
        XCTAssertTrue(projection.frontPage.isEmpty, "a non-editor's feature must not reach the front page")
    }

    /// An invalid draft (violating the closed table) never reaches core: the model
    /// reports the field violation and signs nothing.
    func testAnInvalidDraftIsRejectedBeforeItEverReachesCore() throws {
        let profile = try openLocalProfile()
        let mineHex = RiotDirectoryRow.hex(try profile.profile().whoami().id)
        let space = try profile.createNewswireSpace(input: spaceInput("Validation", roster: [mineHex]))
        let post = try profile.createNewswirePost(input: postInput(space.entryId, "Report"))
        let model = try liveModel(profile: profile, spaceID: space.entryId)

        // feature with a reason ⇒ the closed table rejects it up front.
        model.draft = EditorialActionDraft(kind: .feature, reason: "not allowed")
        XCTAssertEqual(model.sign(targetEntryID: post.entryId), .invalid(.reasonForbidden))
        let projection = try profile.projectNewswireSpace(spaceDescriptorEntryId: space.entryId)
        XCTAssertTrue(projection.frontPage.isEmpty, "an invalid feature never took effect")
    }

    /// Cross-platform identity: the surface reads core's ALREADY-SPLIT front page
    /// and open wire verbatim — it never re-orders or re-selects — so every
    /// platform derives the identical views from the identical records. (The
    /// Android `RiotControllerNewswireTest` mirrors this derivation.)
    func testWireStateReadsCoreProjectionVerbatimWithoutReDeriving() throws {
        let profile = try openLocalProfile()
        let space = try profile.createNewswireSpace(input: spaceInput("Deterministic"))
        let p1 = try profile.createNewswirePost(input: postInput(space.entryId, "First"))
        let p2 = try profile.createNewswirePost(input: postInput(space.entryId, "Second"))
        // Feature p1 so the front page is non-empty and distinct from the wire.
        _ = try profile.createNewswireEditorialAction(input: NewswireEditorialActionInput(
            spaceDescriptorEntryId: space.entryId, targetEntryId: p1.entryId,
            kind: .feature, reason: nil, correctionText: nil))

        let projection = try profile.projectNewswireSpace(spaceDescriptorEntryId: space.entryId)
        let state = NewswireWireState.from(projection)
        guard case let .featured(frontPage, openWire) = state else {
            return XCTFail("a featured post should yield the featured state")
        }
        // The app's lists are core's lists, same ids in the same order — no re-sort.
        XCTAssertEqual(frontPage.map(\.id), projection.frontPage.map(\.entryId))
        XCTAssertEqual(openWire.map(\.id), projection.openWire.map(\.entryId))
        XCTAssertTrue(frontPage.contains { $0.id == p1.entryId })
        XCTAssertEqual(openWire.count, 2)
        XCTAssertTrue(openWire.contains { $0.id == p2.entryId })
    }

    // MARK: - Repository authority wrapper (Unit 4b, consumes Unit 4a)

    private func openRepository() throws -> RiotProfileRepository {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("newswire-editor-\(UUID().uuidString)", isDirectory: true)
        let storage = try ProtectedProfileStorage(
            fileURL: directory.appendingPathComponent("profile.json"))
        return try RiotProfileRepository.open(storage: storage, keyStore: TestWrappingKeyStore())
    }

    /// The live repository wrapper answers with core's descriptor-authenticated
    /// roster (Unit 4a): a roster member is an editor, a stranger is not, and an
    /// unknown / not-yet-synced descriptor answers `false`, never a throw (that
    /// defined false is what drives the pending-sync note in the model).
    func testRepositoryWrapperMatchesTheCoreAuthorityForMemberAndNonMember() throws {
        let repo = try openRepository()
        let mineHex = try repo.me().id   // the founder's real subspace id, hex
        // Founding roster = [me] (what ConferenceShellView seeds): I edit my own community.
        let record = try repo.createNewswireSpace(
            name: "Wrapped", summary: "News.", editorialRoster: [mineHex])
        XCTAssertTrue(try repo.newswireIsEditor(
            spaceDescriptorEntryID: record.entryId, subjectID: mineHex))
        // A stranger key is NOT an editor.
        XCTAssertFalse(try repo.newswireIsEditor(
            spaceDescriptorEntryID: record.entryId, subjectID: "11".repeated(32)))
        // An unknown / not-yet-synced descriptor id → false, NOT a throw.
        XCTAssertFalse(try repo.newswireIsEditor(
            spaceDescriptorEntryID: "ab".repeated(32), subjectID: mineHex))
    }

    // MARK: - Fixtures & helpers

    private func projection(
        openWire: [NewswireProjectedPost], frontPage: [NewswireProjectedPost]
    ) -> NewswireProjectionView {
        NewswireProjectionView(
            openWire: openWire, frontPage: frontPage, earlier: [],
            editorialHistory: [], futureQuarantine: [])
    }

    private func author(_ key: String = "ab".repeated(32)) -> NewswireAuthor {
        NewswireAuthor(id: key, displayName: "Ana", tag: String(key.prefix(8)),
                       rendered: "Ana · \(key.prefix(8))")
    }

    private func projectedPost(
        id: String, headline: String?, treatment: NewswirePostTreatment,
        correctionIDs: [String] = []
    ) -> NewswireProjectedPost {
        NewswireProjectedPost(
            entryId: id, author: author(), taiJ2000Micros: 1,
            headline: headline, body: headline == nil ? nil : "body", language: "en",
            coarseLocation: nil, eventTimeUnixSeconds: nil, expiresAtUnixSeconds: nil,
            sourceClaims: [], operationalProfile: nil, aiAssisted: false,
            verificationIds: [], correctionIds: correctionIDs, treatment: treatment)
    }

    private func projectedAction(
        id: String, kind: NewswireEditorialActionKind, active: Bool
    ) -> NewswireProjectedEditorialAction {
        NewswireProjectedEditorialAction(
            entryId: id, signer: author(), taiJ2000Micros: 1, targetEntryId: "t",
            kind: kind, reason: kind == .feature ? nil : "reason",
            correctionText: kind == .correct ? "new" : nil, active: active)
    }

    private func XCTAssertSigned(
        _ outcome: EditorialSignOutcome, file: StaticString = #filePath, line: UInt = #line
    ) {
        if case .signed = outcome { return }
        XCTFail("expected .signed, got \(outcome)", file: file, line: line)
    }

    // Stub seams for the pure offline/stale tests — they always throw, so the
    // model must degrade to the honest offline state (never a raw error).
    private struct ThrowingProjector: NewswireProjecting {
        func projectNewswire(spaceDescriptorEntryID: String) throws -> NewswireProjectionView {
            throw NSError(domain: "test", code: 1)
        }
    }
    private struct ThrowingEditor: NewswireEditorialActing, NewswireEditorAuthorityChecking {
        func createNewswireEditorialAction(
            spaceDescriptorEntryID: String, targetEntryID: String,
            kind: NewswireEditorialActionKind, reason: String?, correctionText: String?
        ) throws -> NewswireSignedRecord {
            throw NSError(domain: "test", code: 1)
        }
        func newswireIsEditor(spaceDescriptorEntryID: String, subjectID: String) throws -> Bool {
            throw NSError(domain: "test", code: 1)   // load() maps the throw to isEditor == false
        }
    }

    private final class TestWrappingKeyStore: WrappingKeyStore {
        private var key: Data?
        func loadOrCreateWrappingKey() throws -> Data {
            if let key { return key }
            let created = Data(repeating: 0x5a, count: 32)
            key = created
            return created
        }
    }
}

private extension String {
    /// Repeats a two-char hex unit `count` times — a readable full 32-byte id.
    func repeated(_ count: Int) -> String { String(repeating: self, count: count) }
}
