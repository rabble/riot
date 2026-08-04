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
        NewswireEditorAuthorityChecking, NewswireCommenting, NewswireReacting {
        let profile: MobileProfile
        init(_ profile: MobileProfile) { self.profile = profile }

        func projectNewswire(spaceDescriptorEntryID: String) throws -> NewswireProjectionView {
            try profile.projectNewswireSpace(spaceDescriptorEntryId: spaceDescriptorEntryID)
        }

        /// The genuine communal-reaction signer — identical to the shipping
        /// `RiotProfileRepository` wrapper, so an admission is genuinely core's.
        func toggleNewswireReaction(
            spaceDescriptorEntryID: String,
            parentEntryID: String,
            kind: String,
            active: Bool
        ) throws -> NewswireSignedRecord {
            try profile.toggleNewswireReaction(
                spaceDescriptorEntryId: spaceDescriptorEntryID,
                parentEntryId: parentEntryID,
                kind: kind,
                active: active
            )
        }

        /// The genuine communal-reply signer — identical to the shipping
        /// `RiotProfileRepository` wrapper, so an admission is genuinely core's.
        func createNewswireComment(
            spaceDescriptorEntryID: String,
            parentEntryID: String,
            body: String,
            language: String
        ) throws -> NewswireSignedRecord {
            try profile.createNewswireComment(
                spaceDescriptorEntryId: spaceDescriptorEntryID,
                parentEntryId: parentEntryID,
                body: body,
                language: language
            )
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
    private func liveModel(
        profile: MobileProfile,
        spaceID: String,
        withCommenter: Bool = true,
        withReactor: Bool = true
    ) throws -> NewswireSurfaceModel {
        let live = LiveNewswire(profile)
        return NewswireSurfaceModel(
            projector: live,
            editor: live,
            authority: live,
            spaceDescriptorEntryID: spaceID,
            communityName: "Riverside",
            myKeyHex: RiotDirectoryRow.hex(try profile.profile().whoami().id),
            commenter: withCommenter ? live : nil,
            reactor: withReactor ? live : nil,
            reactionWriter: withReactor ? MobileProfileReactionWriter(profile: profile) : nil
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
        XCTAssertFalse(NewswireWireState.offlineStale.hasPosts)
        XCTAssertFalse(empty.hasPosts)
        XCTAssertTrue(noFeature.hasPosts)
        XCTAssertTrue(featured.hasPosts)

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

    // MARK: - Dead-end fixes (Unit 7)

    func testPostsButNoFeatureOffersNoDeadButtonTheOpenWireIsTheNextAction() {
        let post = projectedPost(id: "a1", headline: "Report", treatment: .ordinary)
        let model = NewswireSurfaceModel(
            projector: FixedProjector(projection(openWire: [post], frontPage: [])),
            editor: ThrowingEditor(),
            authority: StubAuthority(),                 // 4b seam; editor status irrelevant here
            spaceDescriptorEntryID: "desc", communityName: "Riverside",
            myKeyHex: "aa".repeated(32))
        model.load()
        guard case let .postsButNoFeature(openWire) = model.wire else {
            return XCTFail("posts but no feature")
        }
        // The next action is the visible open wire, not a button.
        XCTAssertFalse(openWire.isEmpty, "the open wire content IS the reachable next action")
        XCTAssertTrue(model.forwardActions.isEmpty, "the dead 'Open wire' no-op button is gone")
    }

    func testEveryTerminalWireStateHasAReachableNextActionAndNoDeadNoOp() {
        // emptyWire → post the first update; offlineStale → a forward path; the two
        // content states → the content itself. No state offers a no-op button.
        let empty = NewswireSurfaceModel(
            projector: FixedProjector(projection(openWire: [], frontPage: [])),
            editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "desc",
            communityName: "R", myKeyHex: "aa".repeated(32))
        empty.load()
        XCTAssertEqual(empty.wire, .emptyWire)
        XCTAssertEqual(empty.forwardActions, [.postFirstUpdate])
    }

    func testOfflineStaleReDerivesADescriptorThatLandedInsteadOfLooping() {
        // Built with "" (the shell's pre-sync case), but the registry now HAS a
        // descriptor (a joined/switched community whose sync just landed). load()
        // must pick it up and project — not re-loop on the empty id.
        let post = projectedPost(id: "a1", headline: "Landed", treatment: .ordinary)
        let model = NewswireSurfaceModel(
            projector: FixedProjector(projection(openWire: [post], frontPage: [])),
            editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "", communityName: "Riverside",
            myKeyHex: "aa".repeated(32),
            descriptorResolver: { "desc-that-just-synced" })
        model.load()
        guard case .postsButNoFeature = model.wire else {
            return XCTFail("a re-derived descriptor must project, not stay offlineStale")
        }
    }

    func testOfflineStaleWithNoDerivableDescriptorOffersAForwardPathNotASilentLoop() {
        // A nearby-joined community: it never gets a descriptorEntryId, so the
        // resolver yields nil. The state must offer real forward paths and MUST NOT
        // offer the silent .retry re-loop.
        let model = NewswireSurfaceModel(
            projector: ThrowingProjector(), editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "", communityName: "Riverside",
            myKeyHex: "aa".repeated(32),
            descriptorResolver: { nil })
        model.load()
        XCTAssertEqual(model.wire, .offlineStale)
        XCTAssertFalse(model.forwardActions.contains(.retry),
                       "no silent re-loop when there is nothing to re-derive")
        XCTAssertFalse(model.forwardActions.isEmpty, "offlineStale is never a dead end")
    }

    func testKnownDescriptorThatIsMerelyOfflineStillOffersRetry() {
        // A descriptor we DO have, but projection throws (transient offline). Retry
        // is the honest action here — reproject the id we already hold.
        let model = NewswireSurfaceModel(
            projector: ThrowingProjector(), editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "desc", communityName: "Riverside",
            myKeyHex: "aa".repeated(32))
        model.load()
        XCTAssertEqual(model.wire, .offlineStale)
        XCTAssertEqual(model.forwardActions, [.retry])
    }

    func testPendingFirstSyncLeadsWithVerifiedPathAndKeepsNearbySecondary() {
        let model = NewswireSurfaceModel(
            projector: ThrowingProjector(), editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "", communityName: "Riverside",
            myKeyHex: "aa".repeated(32),
            descriptorResolver: { nil })
        model.load()
        let actions = model.forwardActions
        XCTAssertEqual(actions.first, .rejoinWithLink, "the verified-working path is the headline")
        XCTAssertFalse(actions.first?.isNearbyPath ?? true, "the red Nearby path is never first")
        XCTAssertEqual(actions.last, .syncWithPeer, "Nearby is offered but secondary")
        XCTAssertEqual(actions, [.rejoinWithLink, .syncWithPeer])
    }

    func testPendingSyncCopyIsHonestAndDistinctFromTransientOfflineCopy() {
        // The pending-first-sync message explains WHY there is nothing yet and names
        // the forward paths; it is not the same string as the transient-offline copy.
        XCTAssertTrue(
            NewswireWireCopy.pendingSyncMessage.localizedCaseInsensitiveContains("sync")
            || NewswireWireCopy.pendingSyncMessage.localizedCaseInsensitiveContains("peer"))
        XCTAssertNotEqual(NewswireWireCopy.pendingSyncMessage, NewswireWireCopy.offlineMessage)
        XCTAssertNotEqual(NewswireWireCopy.pendingSyncTitle, NewswireWireCopy.offlineTitle)
    }

    func testOfflineStaleCopySelectionFollowsRecoverability() {
        // Unrecoverable (pending first sync) → pending copy; recoverable (transient
        // offline of a known descriptor) → the existing offline copy.
        let pending = NewswireSurfaceModel(
            projector: ThrowingProjector(), editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "", communityName: "R",
            myKeyHex: "aa".repeated(32), descriptorResolver: { nil })
        pending.load()
        XCTAssertEqual(pending.offlineTitle, NewswireWireCopy.pendingSyncTitle)
        XCTAssertEqual(pending.offlineMessage, NewswireWireCopy.pendingSyncMessage)

        let offline = NewswireSurfaceModel(
            projector: ThrowingProjector(), editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "desc", communityName: "R",
            myKeyHex: "aa".repeated(32))
        offline.load()
        XCTAssertEqual(offline.offlineTitle, NewswireWireCopy.offlineTitle)
        XCTAssertEqual(offline.offlineMessage, NewswireWireCopy.offlineMessage)
    }

    // MARK: - Treatment rendering

    func testHiddenPostRendersTheWarningInterstitialAndDropsThePayload() {
        let hidden = projectedPost(
            id: "h1",
            headline: "must disappear",
            treatment: .hidden,
            body: "secret body",
            sourceClaims: ["eyewitness"],
            coarseLocation: "north bridge",
            operationalProfile: alertProfile
        )
        let row = NewswirePostRow(hidden)
        XCTAssertEqual(row.display, .hiddenInterstitial)
        XCTAssertNil(row.headline, "a hidden post shows no headline; the interstitial stands in for it")
        XCTAssertNil(row.body)
        XCTAssertEqual(row.sourceClaims, [])
        XCTAssertNil(row.coarseLocation)
        XCTAssertNil(row.operationalProfile)
        XCTAssertEqual(
            NewswireTreatmentCopy.hiddenBody,
            "The collective hid this report. Its signed treatment record remains available."
        )
    }

    func testTombstonedPostRendersTheTombstoneTreatment() {
        let tomb = projectedPost(
            id: "t1",
            headline: "must disappear",
            treatment: .tombstoned,
            body: "private payload",
            sourceClaims: ["source"],
            coarseLocation: "private place",
            operationalProfile: alertProfile
        )
        let row = NewswirePostRow(tomb)
        XCTAssertEqual(row.display, .tombstoned)
        XCTAssertNil(row.headline)
        XCTAssertNil(row.body)
        XCTAssertEqual(row.sourceClaims, [])
        XCTAssertNil(row.coarseLocation)
        XCTAssertNil(row.operationalProfile)
        XCTAssertNotEqual(NewswireTreatmentCopy.hiddenBody, NewswireTreatmentCopy.tombstoneBody)
    }

    func testOrdinaryRowCarriesReadableAndOperationalFields() {
        let row = NewswirePostRow(projectedPost(
            id: "ordinary",
            headline: "Bridge update",
            treatment: .ordinary,
            body: "Full body",
            taiJ2000Micros: 42,
            sourceClaims: ["eyewitness"],
            coarseLocation: "north bridge",
            eventTimeUnixSeconds: 1_000,
            expiresAtUnixSeconds: 2_000,
            operationalProfile: alertProfile
        ))

        XCTAssertEqual(row.body, "Full body")
        XCTAssertEqual(row.sourceClaims, ["eyewitness"])
        XCTAssertEqual(row.coarseLocation, "north bridge")
        XCTAssertEqual(row.eventTimeUnixSeconds, 1_000)
        XCTAssertEqual(row.expiresAtUnixSeconds, 2_000)
        XCTAssertEqual(row.operationalProfile, alertProfile)
        XCTAssertEqual(row.taiJ2000Micros, 42)
        XCTAssertEqual(row.readAccessibilityLabel, "Read Bridge update")
        XCTAssertNotEqual(
            NewswireReportTrigger(surface: "front-page", reportID: row.id),
            NewswireReportTrigger(surface: "open-wire", reportID: row.id),
            "duplicate projections of one report must restore to the opening surface"
        )
    }

    func testExactTrustCopyAndMissingEventTimeAreHonest() {
        XCTAssertEqual(
            NewswireTrustCopy.signatureBody,
            "This key authored this report. A signature does not prove the report is true. Display names are self-claimed."
        )
        XCTAssertEqual(
            NewswireTrustCopy.editorialChecksBody,
            "Signed editorial judgments from this community’s current editorial roster. They are evidence notes, not proof of truth."
        )
        XCTAssertEqual(
            NewswireTrustCopy.correctionBody,
            "Editors signed a correction. Review the signed history."
        )
        XCTAssertEqual(
            NewswireTrustCopy.aiAssisted,
            "AI-assisted · human reviewed and signed"
        )
        let row = NewswirePostRow(projectedPost(
            id: "no-event",
            headline: "Report",
            treatment: .ordinary,
            eventTimeUnixSeconds: nil
        ))
        XCTAssertEqual(NewswirePostDetail(row: row).eventTimeText, "Event time not provided")
    }

    func testEditorialLineageIncludesDirectActionsAndTransitiveRetractionsOnly() {
        let rows = [
            projectedAction(id: "hide", target: "post", kind: .hide, active: false, tai: 10),
            projectedAction(id: "undo", target: "hide", kind: .retract, active: false, tai: 11),
            projectedAction(id: "undo-undo", target: "undo", kind: .retract, active: true, tai: 12),
            projectedAction(id: "other", target: "elsewhere", kind: .verify, active: true, tai: 13)
        ].map(EditorialHistoryRow.init)

        let lineage = EditorialActionLineage.forReport("post", in: rows)

        XCTAssertEqual(lineage.map(\.id), ["hide", "undo", "undo-undo"])
        XCTAssertEqual(lineage.map(\.taiJ2000Micros), [10, 11, 12])
        XCTAssertFalse(lineage.contains { $0.id == "other" })
        XCTAssertEqual(
            EditorialHistoryRow.signedOrderingLabel,
            "Signed ordering value (TAI-J2000 microseconds)"
        )
    }

    func testTreatmentDetailContainsAccountabilityButNoPayload() {
        let treated = NewswirePostRow(projectedPost(
            id: "treated",
            headline: "redact me",
            treatment: .hidden,
            body: "redact me too",
            taiJ2000Micros: 77,
            sourceClaims: ["redact"],
            coarseLocation: "redact",
            operationalProfile: alertProfile
        ))
        let action = EditorialHistoryRow(projectedAction(
            id: "hide", target: "treated", kind: .hide, active: true, tai: 88
        ))
        let detail = NewswireTreatmentDetail(
            row: treated,
            lineage: EditorialActionLineage.forReport("treated", in: [action])
        )

        XCTAssertEqual(detail.reportID, "treated")
        XCTAssertEqual(detail.taiJ2000Micros, 77)
        XCTAssertEqual(detail.lineage.map(\.id), ["hide"])
        XCTAssertEqual(detail.eventTimeText, "Event time not provided")
        XCTAssertFalse(detail.exposesPayload)
        XCTAssertEqual(
            EditorialActionTarget.retraction(of: action).entryID,
            "hide",
            "Retract must sign against the selected action, never the report"
        )
        XCTAssertEqual(
            EditorialActionTarget.retraction(of: action).availableKinds,
            [.retract]
        )
        XCTAssertFalse(
            EditorialActionTarget.report(treated).availableKinds.contains(.retract),
            "report-scoped controls must not offer a retraction against a report ID"
        )
        let staleRetraction = EditorialActionDraft(
            kind: .retract,
            reason: "must not cross into report-scoped signing"
        )
        let prepared = EditorialActionTarget.report(treated)
            .preparedDraft(from: staleRetraction)
        XCTAssertNotEqual(prepared.kind, .retract)
        XCTAssertTrue(prepared.reason.isEmpty)
    }

    func testActionScopedRetractionSignsTheSelectedActionID() {
        let editor = RecordingEditor()
        let model = NewswireSurfaceModel(
            projector: FixedProjector(projection(openWire: [], frontPage: [])),
            editor: editor,
            authority: StubAuthority(),
            spaceDescriptorEntryID: "space",
            communityName: "Riverside",
            myKeyHex: "aa".repeated(32)
        )
        let selected = EditorialHistoryRow(projectedAction(
            id: "selected-action",
            target: "report",
            kind: .hide,
            active: true
        ))
        let target = EditorialActionTarget.retraction(of: selected)
        model.draft = EditorialActionDraft(kind: .retract, reason: "signed in error")

        XCTAssertSigned(model.sign(targetEntryID: target.entryID))
        XCTAssertEqual(editor.targetEntryID, "selected-action")
        XCTAssertEqual(editor.kind, .retract)

        let report = NewswirePostRow(projectedPost(
            id: "report",
            headline: "Report",
            treatment: .ordinary
        ))
        model.draft = EditorialActionDraft(
            kind: .retract,
            reason: "stale canceled retraction"
        )
        let reportTarget = EditorialActionTarget.report(report)
        model.draft = reportTarget.preparedDraft(from: model.draft)
        guard case let .success(review) = model.review(
            targetEntryID: reportTarget.entryID
        ) else {
            return XCTFail("normalized report action should be reviewable")
        }
        XCTAssertNotEqual(review.kind, .retract)
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
                       "Editorial controls appear once this community's details reach you.")
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

    /// Even if a bug FORCED the editorial control visible (authority seam stubbed to
    /// true), the core still refuses a non-roster author's action and the post is
    /// UNCHANGED — the display predicate is never the security boundary (design §4
    /// defense-in-depth).
    func testForcingControlsVisibleDoesNotLetANonEditorChangeAnything() throws {
        struct AlwaysEditor: NewswireEditorAuthorityChecking {
            func newswireIsEditor(spaceDescriptorEntryID: String, subjectID: String) throws -> Bool { true }
        }
        let stranger = "33".repeated(32)
        let profile = try openLocalProfile()                       // my key ∉ roster
        let space = try profile.createNewswireSpace(input: spaceInput("Forced", roster: [stranger]))
        let post = try profile.createNewswirePost(input: postInput(space.entryId, "Untouched"))
        let live = LiveNewswire(profile)
        let model = NewswireSurfaceModel(projector: live, editor: live, authority: AlwaysEditor(),
            spaceDescriptorEntryID: space.entryId, communityName: "Forced",
            myKeyHex: RiotDirectoryRow.hex(try profile.profile().whoami().id))
        model.load()
        XCTAssertTrue(model.canOfferEditorialControls, "seam forced true ⇒ control shown (the bug we defend against)")

        // …yet the action still fails at core and the post is unchanged.
        model.draft = EditorialActionDraft(kind: .hide, reason: "force it")
        XCTAssertEqual(model.sign(targetEntryID: post.entryId), .rejected)
        let projection = try profile.projectNewswireSpace(spaceDescriptorEntryId: space.entryId)
        let row = try XCTUnwrap(projection.openWire.first { $0.entryId == post.entryId })
        XCTAssertEqual(row.treatment, .ordinary, "core, not the UI, is the gate")
        XCTAssertEqual(row.headline, "Untouched")
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

    // MARK: - Communal replies (comments)

    /// A reply composed through the model reaches core, and on reload the surface
    /// groups it under its parent post — carrying the body and a rendered author,
    /// exactly as a post row does. A reply is communal, so the founder (or any
    /// member) may post one; `canComment` is independent of editor status.
    func testAReplyIsSignedAndGroupedUnderItsParentPost() throws {
        let profile = try openLocalProfile()
        try profile.profile().setDisplayName(name: "Bo")
        let space = try profile.createNewswireSpace(input: spaceInput("Discussion"))
        let post = try profile.createNewswirePost(input: postInput(space.entryId, "What did you see?"))
        let model = try liveModel(profile: profile, spaceID: space.entryId)
        model.load()
        XCTAssertTrue(model.canComment, "a wired commenter + descriptor ⇒ the reply affordance is offered")

        let outcome = model.submitComment(parentEntryID: post.entryId, body: "I was on the east side.")
        guard case .posted = outcome else {
            return XCTFail("a communal reply should post, got \(outcome)")
        }

        let replies = model.comments(under: post.entryId)
        XCTAssertEqual(replies.count, 1)
        let reply = try XCTUnwrap(replies.first)
        XCTAssertEqual(reply.parentID, post.entryId)
        XCTAssertEqual(reply.body, "I was on the east side.")
        XCTAssertEqual(reply.display, .ordinary)
        XCTAssertTrue(
            reply.author.hasPrefix("Bo · "),
            "the reply author is rendered by the same name path as a post author, got \(reply.author)")
    }

    /// A reply whose parent post is not held is dropped from the projection — the
    /// surface never shows an orphan reply. Core signs it (it is a valid communal
    /// record); projection is where the dangling parent is resolved away.
    func testAReplyWithNoHeldParentNeverAppearsInTheSurface() throws {
        let profile = try openLocalProfile()
        let space = try profile.createNewswireSpace(input: spaceInput("Danglers"))
        let model = try liveModel(profile: profile, spaceID: space.entryId)

        let outcome = model.submitComment(parentEntryID: "ab".repeated(32), body: "Into the void.")
        guard case .posted = outcome else {
            return XCTFail("a well-formed reply signs even when its parent is unheld, got \(outcome)")
        }
        model.load()
        XCTAssertTrue(
            model.commentsByParent.isEmpty,
            "a reply with no held parent must not surface")
    }

    /// An empty reply never reaches core, and a model with no commenter wired hides
    /// the affordance and answers `.unavailable` if asked to submit anyway.
    func testEmptyReplyIsBlockedAndAbsentCommenterHidesTheAffordance() throws {
        let profile = try openLocalProfile()
        let space = try profile.createNewswireSpace(input: spaceInput("Guards"))
        let post = try profile.createNewswirePost(input: postInput(space.entryId, "Prompt"))

        let model = try liveModel(profile: profile, spaceID: space.entryId)
        model.load()
        XCTAssertEqual(model.submitComment(parentEntryID: post.entryId, body: "   \n"), .empty)
        XCTAssertTrue(model.comments(under: post.entryId).isEmpty)

        let noCommenter = try liveModel(profile: profile, spaceID: space.entryId, withCommenter: false)
        noCommenter.load()
        XCTAssertFalse(noCommenter.canComment, "no commenter wired ⇒ the reply affordance is hidden")
        XCTAssertEqual(
            noCommenter.submitComment(parentEntryID: post.entryId, body: "hi"), .unavailable)
    }

    /// An editor tombstoning a reply redacts its body while keeping identity and
    /// ordering — the same per-content moderation a post receives, never per person.
    /// (Mirrors the Rust `editor_tombstone_redacts_a_comment_body`.)
    func testAnEditorTombstoneRedactsAReplyBody() throws {
        let profile = try openLocalProfile()
        let mineHex = RiotDirectoryRow.hex(try profile.profile().whoami().id)
        let space = try profile.createNewswireSpace(input: spaceInput("Moderated", roster: [mineHex]))
        let post = try profile.createNewswirePost(input: postInput(space.entryId, "Open thread"))
        let model = try liveModel(profile: profile, spaceID: space.entryId)
        model.load()

        guard case let .posted(commentID) = model.submitComment(
            parentEntryID: post.entryId, body: "Names a private individual.") else {
            return XCTFail("the reply should post")
        }

        // The founder is the sole editor and moderates the reply per content.
        model.draft = EditorialActionDraft(kind: .tombstone, reason: "Names a private individual.")
        XCTAssertSigned(model.sign(targetEntryID: commentID))
        model.load()

        let reply = try XCTUnwrap(model.comments(under: post.entryId).first { $0.id == commentID })
        XCTAssertEqual(reply.display, .tombstoned)
        XCTAssertNil(reply.body, "a tombstoned reply surrenders its body")
        XCTAssertEqual(reply.parentID, post.entryId, "identity survives the redaction")
    }

    // MARK: - Communal reactions (Layer 3)

    func testAsyncReactionPendingIsSharedAndSuppressesDuplicateSurfaceTap() async {
        let writer = ControllableReactionWriter()
        let clock = ControllableReactionStallClock()
        let post = projectedPost(id: "p1", headline: "Report", treatment: .ordinary)
        let model = NewswireSurfaceModel(
            projector: FixedProjector(projection(openWire: [post], frontPage: [])),
            editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "desc", communityName: "R", myKeyHex: "aa".repeated(32),
            reactionWriter: writer, reactionStallClock: clock)
        model.load()
        let row = NewswirePostRow(post)
        let key = ReactionKey(postID: row.id, kind: .support)
        let other = ReactionKey(postID: row.id, kind: .grief)

        model.toggleReaction(post: row, kind: .support, surface: .openWire)
        model.toggleReaction(post: row, kind: .support, surface: .frontPage)

        XCTAssertTrue(model.isPending(key))
        XCTAssertFalse(model.isPending(other))
        await waitForWriterCalls(writer, count: 1)
        let callCount = await writer.callCount()
        XCTAssertEqual(callCount, 1, "duplicate renderings share one logical pending key")

        await writer.completeNext(.accepted(.init(
            projection: projection(openWire: [projectedPost(
                id: "p1", headline: "Report", treatment: .ordinary,
                reactions: [NewswireReactionTally(kind: "support", count: 1, reactedByMe: true)]
            )], frontPage: []),
            revision: 1
        )))
        await waitForModel { !model.isPending(key) }
        XCTAssertTrue(model.isReacted(post: row.id, kind: .support))
        XCTAssertEqual(model.reactionCount(post: row.id, kind: .support), 1)
        XCTAssertEqual(model.reactionAnnouncements.count, 1)
        XCTAssertEqual(model.reactionAnnouncements.first?.surface, .openWire)
        XCTAssertEqual(model.reactionAnnouncements.first?.message, "Support reaction added")
    }

    func testDifferentReactionKeysRemainIndependent() async {
        let writer = ControllableReactionWriter()
        let post = projectedPost(id: "p1", headline: "Report", treatment: .ordinary)
        let model = asyncReactionModel(post: post, writer: writer)
        let row = NewswirePostRow(post)
        let support = ReactionKey(postID: row.id, kind: .support)
        let grief = ReactionKey(postID: row.id, kind: .grief)

        model.toggleReaction(post: row, kind: .support, surface: .openWire)
        model.toggleReaction(post: row, kind: .grief, surface: .openWire)

        XCTAssertTrue(model.isPending(support))
        XCTAssertTrue(model.isPending(grief))
        await waitForWriterCalls(writer, count: 1)
        await writer.completeNext(.accepted(.init(
            projection: projection(openWire: [post], frontPage: []), revision: 1)))
        await waitForWriterCalls(writer, count: 2)
        await writer.completeNext(.accepted(.init(
            projection: projection(openWire: [post], frontPage: []), revision: 2)))
        await waitForModel { !model.isPending(support) && !model.isPending(grief) }
    }

    func testRejectedReactionUsesTypedScopeAndRedactedDiagnostics() async {
        let writer = ControllableReactionWriter()
        let reporter = CapturingReactionReporter()
        let post = projectedPost(id: "private-post-id", headline: "secret body", treatment: .ordinary)
        let model = asyncReactionModel(post: post, writer: writer, reporter: reporter)
        let row = NewswirePostRow(post)
        let key = ReactionKey(postID: row.id, kind: .support)
        let failure = ReactionFailure(
            kind: .retryablePersistence,
            publicCode: "reaction_persistence",
            message: "Couldn’t save your reaction. Try again.")

        model.toggleReaction(post: row, kind: .support, surface: .openWire)
        await waitForWriterCalls(writer, count: 1)
        await writer.completeNext(.rejected(failure))
        await waitForModel { !model.isPending(key) }

        XCTAssertEqual(model.failure(for: key, surface: .openWire)?.message, failure.message)
        XCTAssertNil(model.failure(for: key, surface: .frontPage))
        let captured = await reporter.capturedText()
        for sentinel in ["private-post-id", "/Users/person/profile.sqlite", "deadbeef", "secret body"] {
            XCTAssertFalse(captured.contains(sentinel))
            XCTAssertFalse(model.reactionAnnouncements.map(\.message).joined().contains(sentinel))
        }
        XCTAssertTrue(captured.contains("reaction_persistence"))
    }

    func testRawErrorSentinelsCannotCrossTheReactionFailureBoundary() async {
        let raw = SentinelReactionError(
            description: [
                "private-post-id",
                "/Users/person/profile.sqlite",
                "signed-bytes-deadbeef",
                "secret post body",
            ].joined(separator: "|"))
        let failure = ReactionFailure(raw)
        let reporter = CapturingReactionReporter()
        await reporter.report(failure, phase: .persistence)

        let exposed = [
            failure.publicCode,
            failure.message,
            await reporter.capturedText(),
        ].joined(separator: "|")
        XCTAssertEqual(failure.kind, .authorityOrInput)
        XCTAssertEqual(failure.publicCode, "reaction_authority_or_input")
        for sentinel in [
            "private-post-id",
            "/Users/person/profile.sqlite",
            "signed-bytes-deadbeef",
            "secret post body",
        ] {
            XCTAssertFalse(exposed.contains(sentinel))
        }
    }

    func testMobileErrorsMapToClosedReactionFailureCategories() {
        XCTAssertEqual(ReactionFailure(MobileError.Database).kind, .retryablePersistence)
        XCTAssertEqual(ReactionFailure(MobileError.SessionFailed).publicCode, "reaction_persistence")
        XCTAssertEqual(ReactionFailure(MobileError.StoreFull).kind, .capacity)
        XCTAssertEqual(ReactionFailure(MobileError.SessionLimit).publicCode, "reaction_capacity")
        XCTAssertEqual(ReactionFailure(MobileError.ClockUnavailable).kind, .clock)
        XCTAssertEqual(ReactionFailure(MobileError.InvalidInput).kind, .authorityOrInput)
        XCTAssertEqual(ReactionFailure(MobileError.Internal).publicCode, "reaction_authority_or_input")
    }

    func testFailureCategoriesDisableTheIntendedScope() async {
        let writer = ControllableReactionWriter()
        let post = projectedPost(id: "p1", headline: "Report", treatment: .ordinary)
        let model = asyncReactionModel(post: post, writer: writer)
        let row = NewswirePostRow(post)
        let support = ReactionKey(postID: row.id, kind: .support)

        model.toggleReaction(post: row, kind: .support, surface: .openWire)
        await waitForWriterCalls(writer, count: 1)
        await writer.completeNext(.rejected(.init(
            kind: .capacity,
            publicCode: "reaction_capacity",
            message: "This community can’t hold another reaction right now."
        )))
        await waitForModel { !model.isPending(support) }
        XCTAssertTrue(model.isReactionDisabled(support))
        XCTAssertFalse(model.isReactionDisabled(.init(postID: row.id, kind: .grief)))

        let secondWriter = ControllableReactionWriter()
        let second = asyncReactionModel(post: post, writer: secondWriter)
        second.toggleReaction(post: row, kind: .grief, surface: .openWire)
        await waitForWriterCalls(secondWriter, count: 1)
        await secondWriter.completeNext(.rejected(.init(
            kind: .authorityOrInput,
            publicCode: "reaction_authority_or_input",
            message: "Reactions aren’t available for this post."
        )))
        await waitForModel { !second.isPending(.init(postID: row.id, kind: .grief)) }
        XCTAssertTrue(second.isReactionRowDisabled(postID: row.id))
        XCTAssertTrue(second.isReactionDisabled(.init(postID: row.id, kind: .support)))
    }

    func testClockFailureDisablesOnlyItsKeyAndRetryClearsOnlyThatKeysCopy() async {
        let writer = ControllableReactionWriter()
        let post = projectedPost(id: "p1", headline: "Report", treatment: .ordinary)
        let model = asyncReactionModel(post: post, writer: writer)
        let row = NewswirePostRow(post)
        let key = ReactionKey(postID: row.id, kind: .support)
        let other = ReactionKey(postID: row.id, kind: .grief)

        model.toggleReaction(post: row, kind: .support, surface: .openWire)
        await waitForWriterCalls(writer, count: 1)
        await writer.completeNext(.rejected(.init(
            kind: .retryablePersistence,
            publicCode: "reaction_persistence",
            message: "Couldn’t save your reaction. Try again."
        )))
        await waitForModel { !model.isPending(key) }
        XCTAssertNotNil(model.failure(for: key, surface: .openWire))

        model.toggleReaction(post: row, kind: .support, surface: .openWire)
        XCTAssertNil(model.failure(for: key, surface: .openWire), "retry clears its old copy immediately")
        await waitForWriterCalls(writer, count: 2)
        await writer.completeNext(.rejected(.init(
            kind: .clock,
            publicCode: "reaction_clock",
            message: "Check this device’s Date & Time before reacting."
        )))
        await waitForModel { !model.isPending(key) }
        XCTAssertTrue(model.isReactionDisabled(key))
        XCTAssertFalse(model.isReactionDisabled(other))
        XCTAssertFalse(model.isReactionRowDisabled(postID: row.id))
    }

    func testCommittedNeedsRefreshReconcilesWithNextAcceptedProjection() async {
        let writer = ControllableReactionWriter()
        let post = projectedPost(id: "p1", headline: "Report", treatment: .ordinary)
        let model = asyncReactionModel(post: post, writer: writer)
        let row = NewswirePostRow(post)
        let key = ReactionKey(postID: row.id, kind: .support)

        model.toggleReaction(post: row, kind: .support, surface: .openWire)
        await waitForWriterCalls(writer, count: 1)
        await writer.completeNext(.committedNeedsRefresh(active: true, revision: 4))
        await waitForModel { !model.isPending(key) }
        XCTAssertTrue(model.isReacted(post: row.id, kind: .support))
        XCTAssertEqual(model.reactionCount(post: row.id, kind: .support), 0)
        XCTAssertEqual(
            model.failure(for: key, surface: .openWire)?.message,
            "Saved. The count updates when new posts arrive.")

        model.toggleReaction(post: row, kind: .grief, surface: .openWire)
        await waitForWriterCalls(writer, count: 2)
        let refreshed = projectedPost(
            id: "p1", headline: "Report", treatment: .ordinary,
            reactions: [NewswireReactionTally(kind: "support", count: 1, reactedByMe: true)])
        await writer.completeNext(.accepted(.init(
            projection: projection(openWire: [refreshed], frontPage: []), revision: 5)))
        await waitForModel { !model.isPending(.init(postID: row.id, kind: .grief)) }
        XCTAssertTrue(model.isReacted(post: row.id, kind: .support))
        XCTAssertNil(model.failure(for: key, surface: .openWire))
    }

    func testOlderWriterRevisionCannotOverwriteNewerProjection() async {
        let writer = ControllableReactionWriter()
        let post = projectedPost(id: "p1", headline: "Report", treatment: .ordinary)
        let model = asyncReactionModel(post: post, writer: writer)
        let row = NewswirePostRow(post)

        model.applyReactionSnapshot(.init(
            projection: projection(openWire: [projectedPost(
                id: "p1", headline: "Report", treatment: .ordinary,
                reactions: [NewswireReactionTally(kind: "support", count: 2, reactedByMe: true)]
            )], frontPage: []),
            revision: 8
        ))
        model.applyReactionSnapshot(.init(
            projection: projection(openWire: [post], frontPage: []),
            revision: 7
        ))

        XCTAssertEqual(model.lastAppliedReactionRevision, 8)
        XCTAssertTrue(model.isReacted(post: row.id, kind: .support))
        XCTAssertEqual(model.reactionCount(post: row.id, kind: .support), 2)
    }

    func testTeardownCancelsQueuedWorkAndIgnoresLateCompletion() async {
        let writer = ControllableReactionWriter()
        let post = projectedPost(id: "p1", headline: "Report", treatment: .ordinary)
        let model = asyncReactionModel(post: post, writer: writer)
        let row = NewswirePostRow(post)
        let key = ReactionKey(postID: row.id, kind: .support)

        model.toggleReaction(post: row, kind: .support, surface: .openWire)
        await waitForWriterCalls(writer, count: 1)
        model.cancelReactionTasks()
        XCTAssertFalse(model.isPending(key))
        await writer.completeNext(.accepted(.init(
            projection: projection(openWire: [projectedPost(
                id: "p1", headline: "Report", treatment: .ordinary,
                reactions: [NewswireReactionTally(kind: "support", count: 1, reactedByMe: true)]
            )], frontPage: []),
            revision: 1
        )))
        await Task.yield()
        XCTAssertFalse(model.isReacted(post: row.id, kind: .support))
    }

    func testTeardownDuringDiagnosticSuspensionCannotPublishLatePresentation() async {
        let writer = ControllableReactionWriter()
        let reporter = SuspendingReactionReporter()
        let post = projectedPost(id: "p1", headline: "Report", treatment: .ordinary)
        let model = asyncReactionModel(post: post, writer: writer, reporter: reporter)
        let row = NewswirePostRow(post)
        let key = ReactionKey(postID: row.id, kind: .support)
        let failure = ReactionFailure(
            kind: .retryablePersistence,
            publicCode: "reaction_persistence",
            message: "Couldn’t save your reaction. Try again.")

        model.toggleReaction(post: row, kind: .support, surface: .openWire)
        await waitForWriterCalls(writer, count: 1)
        await writer.completeNext(.rejected(failure))
        await waitForReporterCalls(reporter, count: 1)

        model.cancelReactionTasks()
        XCTAssertFalse(model.isPending(key))
        XCTAssertNil(model.failure(for: key, surface: .openWire))
        XCTAssertTrue(model.reactionAnnouncements.isEmpty)

        await reporter.release()
        await Task.yield()
        await Task.yield()
        XCTAssertNil(model.failure(for: key, surface: .openWire))
        XCTAssertTrue(
            model.reactionAnnouncements.isEmpty,
            "a stale operation must not announce after teardown while diagnostics suspended")
        let captured = await reporter.capturedText()
        XCTAssertTrue(captured.contains("reaction_persistence"))
    }

    func testStartedWriteStillReportsRedactedFailureAfterTeardown() async {
        let writer = ControllableReactionWriter()
        let reporter = CapturingReactionReporter()
        let post = projectedPost(id: "p1", headline: "Report", treatment: .ordinary)
        let model = asyncReactionModel(post: post, writer: writer, reporter: reporter)
        let row = NewswirePostRow(post)
        let key = ReactionKey(postID: row.id, kind: .support)

        model.toggleReaction(post: row, kind: .support, surface: .openWire)
        await waitForWriterCalls(writer, count: 1)
        model.cancelReactionTasks()
        await writer.completeNext(.rejected(.init(
            kind: .capacity,
            publicCode: "reaction_capacity",
            message: "This community can’t hold another reaction right now."
        )))
        await waitForReporterText(reporter, containing: "reaction_capacity")

        XCTAssertFalse(model.isPending(key))
        XCTAssertNil(model.failure(for: key, surface: .openWire))
        XCTAssertTrue(model.reactionAnnouncements.isEmpty)
        XCTAssertFalse(model.isReactionDisabled(key))
    }

    func testTeardownCancelsAWriterRequestBeforeItBegins() async {
        let writer = SerialControllableReactionWriter()
        let post = projectedPost(id: "p1", headline: "Report", treatment: .ordinary)
        let model = NewswireSurfaceModel(
            projector: FixedProjector(projection(openWire: [post], frontPage: [])),
            editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "desc", communityName: "R", myKeyHex: "aa".repeated(32),
            reactionWriter: writer,
            reactionStallClock: ControllableReactionStallClock())
        model.load()
        let row = NewswirePostRow(post)

        model.toggleReaction(post: row, kind: .support, surface: .openWire)
        await waitForSerialWriterCalls(writer, count: 1)
        model.toggleReaction(post: row, kind: .grief, surface: .openWire)
        await Task.yield()
        let beforeCancel = await writer.observedCallCount()
        XCTAssertEqual(beforeCancel, 1, "second request is queued")

        model.cancelReactionTasks()
        await writer.completeCurrent(.accepted(.init(
            projection: projection(openWire: [post], frontPage: []),
            revision: 1)))
        await Task.yield()
        await Task.yield()
        let afterCancel = await writer.observedCallCount()
        XCTAssertEqual(afterCancel, 1, "cancelled queued request never starts")
    }

    func testNoWriterAndAcceptedEmptyProjectionStayFailClosed() async {
        let post = projectedPost(id: "p1", headline: "Report", treatment: .ordinary)
        let noWriter = NewswireSurfaceModel(
            projector: FixedProjector(projection(openWire: [post], frontPage: [])),
            editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "desc", communityName: "R", myKeyHex: "aa".repeated(32))
        noWriter.load()
        let row = NewswirePostRow(post)
        let key = ReactionKey(postID: row.id, kind: .support)
        noWriter.toggleReaction(post: row, kind: .support, surface: .openWire)
        XCTAssertFalse(noWriter.canReact)
        XCTAssertFalse(noWriter.isPending(key))

        let writer = ControllableReactionWriter()
        let model = asyncReactionModel(post: post, writer: writer)
        model.toggleReaction(post: row, kind: .support, surface: .openWire)
        await waitForWriterCalls(writer, count: 1)
        await writer.completeNext(.accepted(.init(
            projection: projection(openWire: [], frontPage: []),
            revision: 1)))
        await waitForModel { !model.isPending(key) }
        XCTAssertFalse(model.isReacted(post: row.id, kind: .support))
        XCTAssertEqual(model.reactionCount(post: row.id, kind: .support), 0)
    }

    func testStalledCopyAppearsAtTwoSecondsOnceWithoutOfferingRetry() async {
        let writer = ControllableReactionWriter()
        let clock = ControllableReactionStallClock()
        let post = projectedPost(id: "p1", headline: "Report", treatment: .ordinary)
        let model = NewswireSurfaceModel(
            projector: FixedProjector(projection(openWire: [post], frontPage: [])),
            editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "desc", communityName: "R", myKeyHex: "aa".repeated(32),
            reactionWriter: writer, reactionStallClock: clock)
        model.load()
        let row = NewswirePostRow(post)
        let key = ReactionKey(postID: row.id, kind: .support)

        model.toggleReaction(post: row, kind: .support, surface: .openWire)
        await waitForWriterCalls(writer, count: 1)
        XCTAssertNil(model.failure(for: key, surface: .openWire), "one second-equivalent: no stalled copy")
        await clock.fire()
        await waitForModel { model.failure(for: key, surface: .openWire) != nil }
        XCTAssertEqual(
            model.failure(for: key, surface: .openWire)?.message,
            "Still saving on this device…")
        let firstCount = model.reactionAnnouncements.count
        await clock.fire()
        await Task.yield()
        XCTAssertEqual(model.reactionAnnouncements.count, firstCount)
        XCTAssertTrue(model.isPending(key), "five seconds-equivalent: still pending, no retry")
    }

    /// The tally→bar mapping: the surface reads each projected post's `reactions`
    /// verbatim (core's ascending-by-kind order), and a kind with no tally reads as
    /// zero so the bar can draw every kind, not only the ones already reacted to.
    func testReactionTalliesMapToTheBarAndAbsentKindsReadZero() {
        let post = projectedPost(
            id: "p1", headline: "Report", treatment: .ordinary,
            reactions: [
                NewswireReactionTally(kind: "support", count: 3, reactedByMe: true),
                NewswireReactionTally(kind: "grief", count: 1, reactedByMe: false),
            ])
        let model = NewswireSurfaceModel(
            projector: FixedProjector(projection(openWire: [post], frontPage: [])),
            editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "desc", communityName: "R", myKeyHex: "aa".repeated(32))
        model.load()
        // Read verbatim, in core's order — the surface never re-tallies or re-sorts.
        XCTAssertEqual(model.reactions(under: "p1").map(\.kind), ["support", "grief"])
        XCTAssertEqual(model.reactionCount(post: "p1", kind: .support), 3)
        XCTAssertEqual(model.reactionCount(post: "p1", kind: .grief), 1)
        // Kinds with no tally read as zero so the bar draws all four.
        XCTAssertEqual(model.reactionCount(post: "p1", kind: .solidarity), 0)
        XCTAssertEqual(model.reactionCount(post: "p1", kind: .important), 0)
        // The four kinds are the closed set the bar iterates, in fixed order.
        XCTAssertEqual(ReactionKind.allCases.map(\.rawValue),
                       ["support", "solidarity", "important", "grief"])
        XCTAssertTrue(model.isReacted(post: "p1", kind: .support))
        XCTAssertFalse(model.isReacted(post: "p1", kind: .grief))
    }

    /// The bar draws an emoji glyph per kind while the wire name and the spoken
    /// label stay words: the glyph is presentation only, so core's closed
    /// `support`/`solidarity`/`important`/`grief` vocabulary is untouched and
    /// VoiceOver still reads a word, never an emoji name.
    func testEachReactionKindHasADistinctGlyphAndKeepsItsWordLabelAndWireName() {
        let glyphs = ReactionKind.allCases.map(\.glyph)
        XCTAssertEqual(glyphs.count, 4)
        for glyph in glyphs {
            XCTAssertFalse(glyph.isEmpty, "every kind draws a glyph")
        }
        XCTAssertEqual(Set(glyphs).count, 4, "each kind is visually distinct")
        // The wire name and the spoken label are unchanged by the glyph.
        XCTAssertEqual(ReactionKind.allCases.map(\.rawValue),
                       ["support", "solidarity", "important", "grief"])
        XCTAssertEqual(ReactionKind.allCases.map(\.label),
                       ["Support", "Solidarity", "Important", "Grief"])
    }

    /// `toggleReaction` calls the reactor with the tapped kind and `active: true`
    /// the first time, and `active: false` on the next tap of the same kind — the
    /// session-local active state flips only after core accepts, and drives the
    /// pink selection.
    func testToggleReactionCallsTheReactorWithTheRightKindAndActiveThenRetracts() {
        let reactor = RecordingReactor()
        let post = projectedPost(id: "p1", headline: "Report", treatment: .ordinary)
        let model = NewswireSurfaceModel(
            projector: FixedProjector(projection(openWire: [post], frontPage: [])),
            editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "desc", communityName: "R", myKeyHex: "aa".repeated(32),
            reactor: reactor)
        model.load()
        XCTAssertTrue(model.canReact, "a wired reactor + descriptor ⇒ the reaction bar is offered")
        let row = NewswirePostRow(post)

        // First tap ⇒ react (active: true) with the exact kind name, and the
        // session marks it active.
        XCTAssertEqual(model.toggleReaction(post: row, kind: .support), .reacted)
        XCTAssertEqual(reactor.calls.count, 1)
        XCTAssertEqual(reactor.calls.first?.kind, "support")
        XCTAssertEqual(reactor.calls.first?.active, true)
        XCTAssertEqual(reactor.calls.first?.parent, "p1")
        XCTAssertTrue(model.isReacted(post: "p1", kind: .support))

        // Second tap of the same kind ⇒ retract (active: false), and the session
        // clears it.
        XCTAssertEqual(model.toggleReaction(post: row, kind: .support), .retracted)
        XCTAssertEqual(reactor.calls.last?.active, false)
        XCTAssertFalse(model.isReacted(post: "p1", kind: .support))
        // A different kind is independent — still inactive, never toggled.
        XCTAssertFalse(model.isReacted(post: "p1", kind: .grief))
    }

    /// Selection and toggle direction come from core's viewer-aware projection,
    /// so rebuilding the model (an app relaunch) cannot turn an active reaction
    /// into an accidental second "add".
    func testProjectedViewerReactionSurvivesFreshModelAndNextTapRetracts() {
        let reactor = RecordingReactor()
        let post = projectedPost(
            id: "p1", headline: "Report", treatment: .ordinary,
            reactions: [
                NewswireReactionTally(kind: "support", count: 1, reactedByMe: true),
            ])
        let model = NewswireSurfaceModel(
            projector: FixedProjector(projection(openWire: [post], frontPage: [])),
            editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "desc", communityName: "R", myKeyHex: "aa".repeated(32),
            reactor: reactor)

        model.load()
        XCTAssertTrue(model.isReacted(post: "p1", kind: .support))
        XCTAssertEqual(
            model.toggleReaction(post: NewswirePostRow(post), kind: .support),
            .retracted)
        XCTAssertEqual(reactor.calls.map(\.active), [false])
    }

    /// `toggleReaction` derives its direction from the viewer-aware projection:
    /// an absent reaction adds, while a freshly reconstructed model with the
    /// selected tally retracts.
    func testToggleReactionCallsTheReactorWithDirectionFromProjection() {
        let reactor = RecordingReactor()
        let post = projectedPost(id: "p1", headline: "Report", treatment: .ordinary)
        let inactiveModel = NewswireSurfaceModel(
            projector: FixedProjector(projection(openWire: [post], frontPage: [])),
            editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "desc", communityName: "R", myKeyHex: "aa".repeated(32),
            reactor: reactor)
        inactiveModel.load()
        XCTAssertFalse(
            inactiveModel.canReact,
            "a legacy synchronous reactor alone must not expose buttons that require the async writer")
        let row = NewswirePostRow(post)

        XCTAssertEqual(inactiveModel.toggleReaction(post: row, kind: .support), .reacted)
        XCTAssertEqual(reactor.calls.count, 1)
        XCTAssertEqual(reactor.calls.first?.kind, "support")
        XCTAssertEqual(reactor.calls.first?.active, true)
        XCTAssertEqual(reactor.calls.first?.parent, "p1")

        let selectedPost = projectedPost(
            id: "p1", headline: "Report", treatment: .ordinary,
            reactions: [
                NewswireReactionTally(kind: "support", count: 1, reactedByMe: true),
            ])
        let selectedModel = NewswireSurfaceModel(
            projector: FixedProjector(projection(openWire: [selectedPost], frontPage: [])),
            editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "desc", communityName: "R", myKeyHex: "aa".repeated(32),
            reactor: reactor)
        selectedModel.load()
        XCTAssertTrue(selectedModel.isReacted(post: "p1", kind: .support))
        XCTAssertEqual(
            selectedModel.toggleReaction(post: NewswirePostRow(selectedPost), kind: .support),
            .retracted)
        XCTAssertEqual(reactor.calls.last?.active, false)
        XCTAssertFalse(selectedModel.isReacted(post: "p1", kind: .grief))
    }

    /// Without a wired reactor the bar is hidden and a direct toggle is a defensive
    /// no-op — the reaction analogue of the absent-commenter reply guard.
    func testReactionBarIsHiddenWithoutAWiredReactor() {
        let model = NewswireSurfaceModel(
            projector: FixedProjector(projection(openWire: [], frontPage: [])),
            editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "desc", communityName: "R", myKeyHex: "aa".repeated(32))
        model.load()
        XCTAssertFalse(model.canReact, "no reactor wired ⇒ the reaction bar is hidden")
        let row = NewswirePostRow(projectedPost(id: "p1", headline: "x", treatment: .ordinary))
        XCTAssertEqual(model.toggleReaction(post: row, kind: .support), .unavailable)
    }

    /// A reaction toggled through the model reaches core, and on reload the post's
    /// tally reflects it; retracting removes it. Driven through the REAL core (like
    /// the reply end-to-end test), so the round-trip is genuinely core's, not a
    /// stub's. A reaction is communal, so the founder (or any member) may react —
    /// `canReact` is independent of editor status.
    func testAReactionIsSignedThroughCoreAndTheTallyReflectsItThenRetracts() throws {
        let profile = try openLocalProfile()
        let space = try profile.createNewswireSpace(input: spaceInput("Reactions"))
        let post = try profile.createNewswirePost(input: postInput(space.entryId, "What did you see?"))
        let model = try liveModel(profile: profile, spaceID: space.entryId)
        model.load()
        XCTAssertTrue(model.canReact, "a wired reactor + descriptor ⇒ the reaction bar is offered")

        guard case let .postsButNoFeature(openWire) = model.wire,
              let row = openWire.first(where: { $0.id == post.entryId }) else {
            return XCTFail("the post should be on the open wire")
        }
        XCTAssertEqual(model.reactionCount(post: row.id, kind: .support), 0, "no reaction yet")

        // React ⇒ the tally reflects one supporter and the session marks it active.
        XCTAssertEqual(model.toggleReaction(post: row, kind: .support), .reacted)
        XCTAssertEqual(model.reactionCount(post: row.id, kind: .support), 1,
                       "the tally reflects the reaction through core")
        XCTAssertTrue(model.isReacted(post: row.id, kind: .support))

        // Retract ⇒ the tally drops back to zero (latest-wins per author).
        XCTAssertEqual(model.toggleReaction(post: row, kind: .support), .retracted)
        XCTAssertEqual(model.reactionCount(post: row.id, kind: .support), 0,
                       "retraction removes this author's reaction")
        XCTAssertFalse(model.isReacted(post: row.id, kind: .support))
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
            comments: [], editorialHistory: [], futureQuarantine: [])
    }

    private func author(_ key: String = "ab".repeated(32)) -> NewswireAuthor {
        NewswireAuthor(id: key, displayName: "Ana", tag: String(key.prefix(8)),
                       rendered: "Ana · \(key.prefix(8))")
    }

    private var alertProfile: NewswireOperationalProfile {
        .alert(profile: NewswireAlertProfile(
            urgency: .immediate,
            severity: .severe,
            certainty: .observed,
            validFromUnixSeconds: nil
        ))
    }

    private func projectedPost(
        id: String,
        headline: String?,
        treatment: NewswirePostTreatment,
        correctionIDs: [String] = [],
        body: String? = nil,
        taiJ2000Micros: UInt64 = 1,
        createdAtUnixSeconds: UInt64? = nil,
        sourceClaims: [String] = [],
        coarseLocation: String? = nil,
        eventTimeUnixSeconds: UInt64? = nil,
        expiresAtUnixSeconds: UInt64? = nil,
        operationalProfile: NewswireOperationalProfile? = nil,
        reactions: [NewswireReactionTally] = []
    ) -> NewswireProjectedPost {
        NewswireProjectedPost(
            entryId: id, author: author(), taiJ2000Micros: taiJ2000Micros,
            createdAtUnixSeconds: createdAtUnixSeconds,
            headline: headline, body: body ?? (headline == nil ? nil : "body"), language: "en",
            coarseLocation: coarseLocation,
            eventTimeUnixSeconds: eventTimeUnixSeconds,
            expiresAtUnixSeconds: expiresAtUnixSeconds,
            sourceClaims: sourceClaims,
            operationalProfile: operationalProfile,
            aiAssisted: false,
            verificationIds: [], correctionIds: correctionIDs, treatment: treatment,
            reactions: reactions)
    }

    private func projectedAction(
        id: String,
        target: String = "t",
        kind: NewswireEditorialActionKind,
        active: Bool,
        tai: UInt64 = 1
    ) -> NewswireProjectedEditorialAction {
        NewswireProjectedEditorialAction(
            entryId: id, signer: author(), taiJ2000Micros: tai, targetEntryId: target,
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
    /// Returns a fixed projection so a wire-state can be driven without a store.
    private struct FixedProjector: NewswireProjecting {
        let projection: NewswireProjectionView
        init(_ projection: NewswireProjectionView) { self.projection = projection }
        func projectNewswire(spaceDescriptorEntryID: String) throws -> NewswireProjectionView {
            projection
        }
    }

    private final class RecordingEditor: NewswireEditorialActing {
        private(set) var targetEntryID: String?
        private(set) var kind: NewswireEditorialActionKind?

        func createNewswireEditorialAction(
            spaceDescriptorEntryID: String,
            targetEntryID: String,
            kind: NewswireEditorialActionKind,
            reason: String?,
            correctionText: String?
        ) throws -> NewswireSignedRecord {
            self.targetEntryID = targetEntryID
            self.kind = kind
            return NewswireSignedRecord(entryId: "signed", signedBytes: Data())
        }
    }
    /// Satisfies Unit 4b's `authority:` seam for wire-state tests that don't exercise
    /// editor gating — always "not an editor", so it never colors a wire-state result.
    private struct StubAuthority: NewswireEditorAuthorityChecking {
        func newswireIsEditor(spaceDescriptorEntryID: String, subjectID: String) throws -> Bool { false }
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

    /// Records each reaction toggle it is handed (parent, kind, active) and returns
    /// a canned signed record, so a test can assert the model calls core with the
    /// exact kind name and direction without a store.
    private final class RecordingReactor: NewswireReacting {
        var calls: [(parent: String, kind: String, active: Bool)] = []
        func toggleNewswireReaction(
            spaceDescriptorEntryID: String, parentEntryID: String, kind: String, active: Bool
        ) throws -> NewswireSignedRecord {
            calls.append((parentEntryID, kind, active))
            return NewswireSignedRecord(entryId: "11".repeated(32), signedBytes: Data([1, 2, 3]))
        }
    }

    private actor ControllableReactionWriter: NewswireReactionWriting {
        private struct Pending {
            let continuation: CheckedContinuation<ReactionWriteResult, Never>
        }

        private var calls: [(descriptor: String, post: String, kind: ReactionKind, active: Bool)] = []
        private var pending: [Pending] = []

        func setReaction(
            descriptorID: String,
            postID: String,
            kind: ReactionKind,
            active: Bool
        ) async -> ReactionWriteResult {
            guard !Task.isCancelled else { return .cancelled }
            calls.append((descriptorID, postID, kind, active))
            return await withCheckedContinuation { continuation in
                pending.append(Pending(continuation: continuation))
            }
        }

        func callCount() -> Int { calls.count }

        func completeNext(_ result: ReactionWriteResult) {
            guard !pending.isEmpty else { return }
            pending.removeFirst().continuation.resume(returning: result)
        }
    }

    /// Models the shipping writer actor's non-reentrant synchronous FFI section:
    /// one call is active and later requests wait before they count as started.
    private actor SerialControllableReactionWriter: NewswireReactionWriting {
        private var active = false
        private var turnWaiters: [CheckedContinuation<Void, Never>] = []
        private var resultContinuation: CheckedContinuation<ReactionWriteResult, Never>?
        private var calls = 0

        func setReaction(
            descriptorID: String,
            postID: String,
            kind: ReactionKind,
            active: Bool
        ) async -> ReactionWriteResult {
            if self.active {
                await withCheckedContinuation { turnWaiters.append($0) }
            }
            guard !Task.isCancelled else { return .cancelled }
            self.active = true
            calls += 1
            let result = await withCheckedContinuation {
                resultContinuation = $0
            }
            self.active = false
            if !turnWaiters.isEmpty {
                turnWaiters.removeFirst().resume()
            }
            return result
        }

        func observedCallCount() -> Int { calls }

        func completeCurrent(_ result: ReactionWriteResult) {
            let continuation = resultContinuation
            resultContinuation = nil
            continuation?.resume(returning: result)
        }
    }

    private actor ControllableReactionStallClock: ReactionStallClock {
        private var continuations: [CheckedContinuation<Void, Never>] = []

        func waitUntilStalled() async {
            await withCheckedContinuation { continuation in
                continuations.append(continuation)
            }
        }

        func fire() {
            let waiting = continuations
            continuations.removeAll()
            waiting.forEach { $0.resume() }
        }
    }

    private actor CapturingReactionReporter: ReactionDiagnosticReporting {
        private var lines: [String] = []

        func report(_ failure: ReactionFailure, phase: ReactionOperationPhase) {
            lines.append("\(phase.rawValue):\(failure.publicCode):\(failure.message)")
        }

        func capturedText() -> String { lines.joined(separator: "\n") }
    }

    private actor SuspendingReactionReporter: ReactionDiagnosticReporting {
        private var lines: [String] = []
        private var waiters: [CheckedContinuation<Void, Never>] = []

        func report(_ failure: ReactionFailure, phase: ReactionOperationPhase) async {
            lines.append("\(phase.rawValue):\(failure.publicCode):\(failure.message)")
            await withCheckedContinuation { waiters.append($0) }
        }

        func callCount() -> Int { lines.count }
        func capturedText() -> String { lines.joined(separator: "\n") }

        func release() {
            let waiting = waiters
            waiters.removeAll()
            waiting.forEach { $0.resume() }
        }
    }

    private struct SentinelReactionError: LocalizedError {
        let description: String
        var errorDescription: String? { description }
    }

    private func asyncReactionModel(
        post: NewswireProjectedPost,
        writer: ControllableReactionWriter,
        reporter: (any ReactionDiagnosticReporting)? = nil
    ) -> NewswireSurfaceModel {
        let model = NewswireSurfaceModel(
            projector: FixedProjector(projection(openWire: [post], frontPage: [])),
            editor: ThrowingEditor(), authority: StubAuthority(),
            spaceDescriptorEntryID: "desc", communityName: "R", myKeyHex: "aa".repeated(32),
            reactionWriter: writer,
            reactionStallClock: ControllableReactionStallClock(),
            reactionDiagnosticReporter: reporter)
        model.load()
        return model
    }

    private func waitForWriterCalls(
        _ writer: ControllableReactionWriter,
        count: Int,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        for _ in 0..<1_000 {
            if await writer.callCount() >= count { return }
            await Task.yield()
        }
        XCTFail("writer did not receive \(count) calls", file: file, line: line)
    }

    private func waitForSerialWriterCalls(
        _ writer: SerialControllableReactionWriter,
        count: Int,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        for _ in 0..<1_000 {
            if await writer.observedCallCount() >= count { return }
            await Task.yield()
        }
        XCTFail("serial writer did not receive \(count) calls", file: file, line: line)
    }

    private func waitForReporterCalls(
        _ reporter: SuspendingReactionReporter,
        count: Int,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        for _ in 0..<1_000 {
            if await reporter.callCount() >= count { return }
            await Task.yield()
        }
        XCTFail("reporter did not receive \(count) calls", file: file, line: line)
    }

    private func waitForReporterText(
        _ reporter: CapturingReactionReporter,
        containing needle: String,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        for _ in 0..<1_000 {
            if await reporter.capturedText().contains(needle) { return }
            await Task.yield()
        }
        XCTFail("reporter did not receive \(needle)", file: file, line: line)
    }

    private func waitForModel(
        _ condition: @MainActor () -> Bool,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        for _ in 0..<1_000 {
            if condition() { return }
            await Task.yield()
        }
        XCTFail("model condition did not become true", file: file, line: line)
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

/// Fix #4 — the composite-site moderation READ model. The security property under
/// test: a `.moderationLoading` verdict HOLDS the whole content surface (the banner
/// is a control, not decoration), and moderated items render as accountable
/// placeholders. Pure over core's `ResolvedCompositeSite` — no store, mirrors the
/// Rust `resolve_composite_site` contract the model consumes.
final class CompositeSiteReadModelTests: XCTestCase {
    private func item(
        _ id: String, _ treatment: SiteItemTreatment, tier: SiteTrustTier = .openWire
    ) -> ResolvedSiteItem {
        ResolvedSiteItem(
            entryId: id, authorSubspace: "ab".repeated(32), trustTier: tier, treatment: treatment)
    }

    private func resolved(
        _ degradation: SiteDegradation, items: [ResolvedSiteItem] = []
    ) -> ResolvedCompositeSite {
        ResolvedCompositeSite(
            root: "cd".repeated(32), degradation: degradation, transportStatus: "available",
            items: items, writerCapExpired: false)
    }

    /// THE security property: moderation-loading holds the surface. The whole
    /// content column is gated (`isContentHeld`) and the banner explains why — a
    /// present item is NEVER shown as trustworthy-clean under a hold.
    func testModerationLoadingHoldsTheWholeSurface() {
        let model = CompositeSiteReadModel.from(
            resolved(.moderationLoading, items: [item("11".repeated(32), .ordinary)]))
        XCTAssertTrue(model.isContentHeld, "moderation-loading must HOLD the content surface")
        XCTAssertEqual(model.hold, .held(reason: .moderationLoading))
        XCTAssertNotNil(model.bannerMessage, "the hold must be explained by a banner")
        // The item is still present as a row (accountable), but the surface is held.
        XCTAssertEqual(model.items.count, 1)
    }

    /// A fully-current site shows content: not held, no banner.
    func testCurrentSiteShowsContentWithoutABanner() {
        let model = CompositeSiteReadModel.from(
            resolved(.none, items: [item("11".repeated(32), .ordinary)]))
        XCTAssertFalse(model.isContentHeld)
        XCTAssertEqual(model.hold, .shown)
        XCTAssertNil(model.bannerMessage)
    }

    /// A tombstoned or hidden item renders as an accountable placeholder treatment,
    /// never a silent disappearance — even on a current site.
    func testModeratedItemsRenderAsAccountablePlaceholders() {
        let model = CompositeSiteReadModel.from(
            resolved(
                .none,
                items: [
                    item("11".repeated(32), .tombstoned),
                    item("22".repeated(32), .hidden),
                    item("33".repeated(32), .ordinary),
                ]))
        XCTAssertEqual(model.items.map(\.display), [.tombstoned, .hiddenInterstitial, .ordinary])
    }

    /// An invalid manifest also holds the surface (a more-severe honest state), with
    /// a banner — content is never shown as trustworthy without a valid manifest.
    func testInvalidManifestHoldsTheSurface() {
        let model = CompositeSiteReadModel.from(resolved(.manifestInvalid))
        XCTAssertTrue(model.isContentHeld)
        XCTAssertEqual(model.hold, .held(reason: .manifestInvalid))
        XCTAssertNotNil(model.bannerMessage)
    }

    /// The mild states show content WITH a notice — held is reserved for states
    /// where content must not be trusted, so a milder degradation never needlessly
    /// blanks the surface.
    func testMildDegradationShowsContentWithANotice() {
        for degradation in [SiteDegradation.editorialOnly, .memberUnverified] {
            let model = CompositeSiteReadModel.from(
                resolved(degradation, items: [item("11".repeated(32), .ordinary)]))
            XCTAssertFalse(model.isContentHeld, "\(degradation) should show content, not hold it")
            XCTAssertNotNil(model.bannerMessage, "\(degradation) still explains itself")
        }
    }

    // MARK: - Trust-tier visual style (anti-impersonation)
    //
    // `CompositeSiteTierStyle` is a SECURITY-relevant UI type, not decoration: an
    // open-wire or comment item must never be able to wear editorial's badge or
    // tint, so `for(_:)` is required to produce visually DISTINCT values per tier
    // — these assertions were migrated from the (now-deleted) parallel
    // `CompositeSiteSurfaceTests` suite, which duplicated this canonical surface.

    func testEditorialAndOpenWireProduceDistinctTierStyles() {
        let editorial = CompositeSiteTierStyle.for(.editorial)
        let openWire = CompositeSiteTierStyle.for(.openWire)

        XCTAssertNotEqual(editorial, openWire, "open-wire must not be styled like editorial")
        XCTAssertNotEqual(
            openWire.badgeSymbol, editorial.badgeSymbol,
            "an open-wire item must not carry the editorial badge symbol")
        XCTAssertNotEqual(
            openWire.tintToken, editorial.tintToken,
            "an open-wire item must not carry the editorial tint")
    }

    func testEditorialAndCommentProduceDistinctTierStyles() {
        let editorial = CompositeSiteTierStyle.for(.editorial)
        let comment = CompositeSiteTierStyle.for(.comment)

        XCTAssertNotEqual(comment, editorial, "a comment must not be styled like editorial")
        XCTAssertNotEqual(
            comment.badgeSymbol, editorial.badgeSymbol,
            "a comment must not carry the editorial badge symbol")
        XCTAssertNotEqual(
            comment.tintToken, editorial.tintToken, "a comment must not carry the editorial tint")
    }

    func testAllThreeTrustTiersProduceDistinctTierStyles() {
        let styles = [
            CompositeSiteTierStyle.for(.editorial),
            CompositeSiteTierStyle.for(.openWire),
            CompositeSiteTierStyle.for(.comment),
        ]
        XCTAssertEqual(Set(styles).count, 3, "each trust tier must have a visually distinct style")
        XCTAssertEqual(
            Set(styles.map(\.badgeSymbol)).count, 3, "each trust tier must have a distinct badge symbol")
        XCTAssertEqual(
            Set(styles.map(\.tintToken)).count, 3, "each trust tier must have a distinct tint")
    }

    func testTierStylesHaveNonEmptyBadgeAndTint() {
        for tier: SiteTrustTier in [.editorial, .openWire, .comment] {
            let style = CompositeSiteTierStyle.for(tier)
            XCTAssertFalse(style.badgeSymbol.isEmpty, "\(tier) must have a badge symbol")
            XCTAssertFalse(style.tintToken.isEmpty, "\(tier) must have a tint")
        }
    }
}

/// Fix #4 write path — the owner moderation authoring model. Validates drafts
/// into the FFI action, signs through the seam (core auto-publishes the coupled
/// heartbeat), and — critically — RETAINS the outcome so the signed bytes stay
/// reachable for propagation (owned-namespace /mod/ has no automatic follower sync
/// yet). Pure over a stub seam; the real end-to-end round-trip is Rust-tested.
@MainActor
final class SiteModerationModelTests: XCTestCase {
    private let root = "ab".repeated(32)
    private let id32 = "cd".repeated(32)

    /// A stub authoring seam: records the action it was handed and returns a canned
    /// outcome (with non-empty signed bytes), or throws to simulate a core refusal.
    private final class StubAuthoring: SiteModerationAuthoring {
        var lastAction: SiteModerationAction?
        var shouldThrow = false
        func authorSiteModeration(sealedRoot: Data, action: SiteModerationAction) throws
            -> SiteModerationOutcome
        {
            lastAction = action
            if shouldThrow { throw NSError(domain: "test", code: 1) }
            return SiteModerationOutcome(
                action: SiteModerationSignedRecord(
                    entryId: "11".repeated(32), signedBytes: Data([1, 2, 3])),
                epoch: SiteModerationSignedRecord(
                    entryId: "22".repeated(32), signedBytes: Data([4, 5, 6, 7])))
        }
    }

    private func model(_ stub: StubAuthoring, kind: SiteModerationTargetKind) -> SiteModerationModel {
        SiteModerationModel(
            siteRoot: root, sealedRoot: Data([0x9]), authoring: stub, initialKind: kind)
    }

    /// A revoke needs a well-formed author key; a tombstone needs both a namespace
    /// and an entry id. Malformed/empty targets are named, never reach core.
    func testValidatorNamesEachMissingOrMalformedField() {
        XCTAssertEqual(
            SiteModerationValidator.validate(SiteModerationDraft(kind: .revoke)),
            .failure(.authorKeyRequired))
        XCTAssertEqual(
            SiteModerationValidator.validate(SiteModerationDraft(kind: .revoke, authorKey: "nothex")),
            .failure(.authorKeyMalformed))
        XCTAssertEqual(
            SiteModerationValidator.validate(SiteModerationDraft(kind: .tombstone, targetEntry: id32)),
            .failure(.targetNamespaceRequired))
        XCTAssertEqual(
            SiteModerationValidator.validate(
                SiteModerationDraft(kind: .tombstone, targetNamespace: id32, targetEntry: "short")),
            .failure(.targetEntryMalformed))

        if case let .success(action) = SiteModerationValidator.validate(
            SiteModerationDraft(kind: .revoke, authorKey: id32.uppercased()))
        {
            XCTAssertEqual(action, .revoke(authorKey: id32))  // normalized lowercase
        } else {
            XCTFail("a 64-hex author key must validate")
        }
    }

    /// A valid draft signs through the seam and the outcome is RETAINED — the
    /// signed bytes of BOTH the action and the auto-published heartbeat stay
    /// reachable on the model for the app to propagate.
    func testSigningRetainsTheOutcomeAndItsSignedBytes() {
        let stub = StubAuthoring()
        let model = model(stub, kind: .tombstone)
        model.draft.targetNamespace = id32
        model.draft.targetEntry = id32

        guard case let .signed(outcome) = model.sign() else {
            return XCTFail("a valid tombstone draft should sign")
        }
        XCTAssertEqual(stub.lastAction, .tombstone(targetNamespace: id32, targetEntry: id32))
        // The bytes are the propagation payload — surfaced, not dropped.
        XCTAssertFalse(outcome.action.signedBytes.isEmpty)
        XCTAssertFalse(outcome.epoch.signedBytes.isEmpty)
        XCTAssertEqual(model.lastOutcome, outcome)
    }

    /// An invalid draft never reaches the seam.
    func testInvalidDraftNeverReachesCore() {
        let stub = StubAuthoring()
        let model = model(stub, kind: .revoke)  // authorKey empty
        XCTAssertEqual(model.sign(), .invalid(.authorKeyRequired))
        XCTAssertNil(stub.lastAction, "an invalid draft must not be signed")
        XCTAssertNil(model.lastOutcome)
    }

    /// A core refusal is surfaced and the draft is preserved.
    func testCoreRefusalIsSurfacedAndDraftPreserved() {
        let stub = StubAuthoring()
        stub.shouldThrow = true
        let model = model(stub, kind: .revoke)
        model.draft.authorKey = id32
        XCTAssertEqual(model.sign(), .rejected)
        XCTAssertEqual(model.draft.authorKey, id32, "a refusal preserves the draft")
        XCTAssertNil(model.lastOutcome)
    }

    /// The review shows the complete, untruncated identifiers — this is the signing
    /// surface, where truncation would be a defect.
    func testReviewShowsUntruncatedIdentifiers() {
        let stub = StubAuthoring()
        let model = model(stub, kind: .tombstone)
        model.draft.targetNamespace = id32
        model.draft.targetEntry = id32
        guard case let .success(review) = model.review() else {
            return XCTFail("a valid draft should review")
        }
        XCTAssertTrue(review.rows.contains { $0.value == id32 })
        XCTAssertTrue(review.rows.contains { $0.label == "Site" && $0.value == root })
    }
}
