package org.riot.evidence

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.riot_ffi.NewswireAuthor
import uniffi.riot_ffi.NewswireEditorialActionKind
import uniffi.riot_ffi.NewswirePostTreatment
import uniffi.riot_ffi.NewswireProjectedEditorialAction
import uniffi.riot_ffi.NewswireProjectedPost
import uniffi.riot_ffi.NewswireProjectionView

/**
 * Unit 2C — editorial actions, front page & open wire (Android surface).
 *
 * Honesty note (same constraint as `NewswireImportTest`): the app module's
 * `testDebugUnitTest` runs on a host JVM that CANNOT load `libriot_ffi`, so no
 * unit test here calls a native FFI function — only the generated record/enum
 * types. So the AUTHORIZATION property (core refuses a non-editor's action) is
 * proven in Rust (`newswire_contract.rs`) and iOS (`NewswireSurfaceTests`, real
 * `MobileProfile`), not here.
 *
 * What this test DOES prove, and why it matters for cross-platform identity: the
 * Android surface derives its front page, open wire, editorial history, closed
 * field table, treatment rendering, pre-signing review, and editor-visibility
 * hint with logic identical to the iOS twin — asserted against generated records
 * constructed on the JVM. Because both platforms read core's ALREADY-SPLIT
 * projection verbatim and share these exact rules, they surface the identical
 * result for identical records.
 */
class RiotControllerNewswireTest {

    // MARK: - Closed field table (all six kinds)

    @Test
    fun closedFieldRulesMatchTheDesignForAllSixKinds() {
        assertEquals(
            EditorialFieldRules(EditorialFieldRequirement.FORBIDDEN, EditorialFieldRequirement.FORBIDDEN),
            NewswireEditorialActionKind.FEATURE.fieldRules,
        )
        assertEquals(
            EditorialFieldRules(EditorialFieldRequirement.FORBIDDEN, EditorialFieldRequirement.FORBIDDEN),
            NewswireEditorialActionKind.VERIFY.fieldRules,
        )
        assertEquals(
            EditorialFieldRules(EditorialFieldRequirement.REQUIRED, EditorialFieldRequirement.REQUIRED),
            NewswireEditorialActionKind.CORRECT.fieldRules,
        )
        for (kind in listOf(
            NewswireEditorialActionKind.HIDE,
            NewswireEditorialActionKind.TOMBSTONE,
            NewswireEditorialActionKind.RETRACT,
        )) {
            assertEquals(
                EditorialFieldRules(EditorialFieldRequirement.REQUIRED, EditorialFieldRequirement.FORBIDDEN),
                kind.fieldRules,
            )
        }
        // Every kind has a rule — the enum is fully covered.
        assertEquals(6, NewswireEditorialActionKind.values().size)
    }

    @Test
    fun featureAndVerifyRejectAnyReasonOrReplacementText() {
        for (kind in listOf(NewswireEditorialActionKind.FEATURE, NewswireEditorialActionKind.VERIFY)) {
            assertEquals(
                EditorialFieldViolation.REASON_FORBIDDEN,
                invalid(EditorialActionDraft(kind, reason = "because")),
            )
            assertEquals(
                EditorialFieldViolation.CORRECTION_FORBIDDEN,
                invalid(EditorialActionDraft(kind, correctionText = "new")),
            )
            val valid = valid(EditorialActionDraft(kind))
            assertNull(valid.reason)
            assertNull(valid.correctionText)
        }
    }

    @Test
    fun correctRequiresBothReasonAndReplacementTextNonEmpty() {
        assertEquals(
            EditorialFieldViolation.REASON_REQUIRED,
            invalid(EditorialActionDraft(NewswireEditorialActionKind.CORRECT, reason = "", correctionText = "fix")),
        )
        assertEquals(
            EditorialFieldViolation.CORRECTION_REQUIRED,
            invalid(EditorialActionDraft(NewswireEditorialActionKind.CORRECT, reason = "wrong", correctionText = "   ")),
        )
        val ok = valid(EditorialActionDraft(NewswireEditorialActionKind.CORRECT, reason = "wrong date", correctionText = "May 2"))
        assertEquals("wrong date", ok.reason)
        assertEquals("May 2", ok.correctionText)
    }

    @Test
    fun hideTombstoneRetractRequireReasonAndForbidReplacementText() {
        for (kind in listOf(
            NewswireEditorialActionKind.HIDE,
            NewswireEditorialActionKind.TOMBSTONE,
            NewswireEditorialActionKind.RETRACT,
        )) {
            assertEquals(
                EditorialFieldViolation.REASON_REQUIRED,
                invalid(EditorialActionDraft(kind, reason = "   ")),
            )
            assertEquals(
                EditorialFieldViolation.CORRECTION_FORBIDDEN,
                invalid(EditorialActionDraft(kind, reason = "unverified", correctionText = "x")),
            )
            val ok = valid(EditorialActionDraft(kind, reason = "unverified"))
            assertEquals("unverified", ok.reason)
            assertNull(ok.correctionText)
        }
    }

    @Test
    fun onlyCorrectCarriesTheEditorialCorrectionLabel() {
        assertTrue(NewswireEditorialActionKind.CORRECT.isEditorialCorrection)
        for (kind in NewswireEditorialActionKind.values().filter { it != NewswireEditorialActionKind.CORRECT }) {
            assertFalse(kind.isEditorialCorrection)
        }
        assertEquals("Editorial correction", EditorialCorrectionLabel.TEXT)
    }

    // MARK: - Immutable pre-signing review

    @Test
    fun reviewShowsEveryCompleteIdentifierUntruncated() {
        val target = "cd".repeat(32)
        val editorKey = "ef".repeat(32)
        val validated = valid(EditorialActionDraft(NewswireEditorialActionKind.CORRECT, reason = "typo", correctionText = "fixed"))
        val review = EditorialActionReview.of(validated, target, "Riverside", editorKey)

        assertEquals(target, review.targetEntryId)         // not truncated
        assertEquals(editorKey, review.actingEditorKeyHex) // not truncated
        assertEquals(
            listOf("Action", "Community", "Target entry", "Acting editor", "Reason", "Replacement text"),
            review.rows.map { it.first },
        )
        assertTrue(review.rows.any { it.second == target })
        assertTrue(review.rows.any { it.second == editorKey })
    }

    @Test
    fun reviewOmitsForbiddenFieldsForFeature() {
        val validated = valid(EditorialActionDraft(NewswireEditorialActionKind.FEATURE))
        val review = EditorialActionReview.of(validated, "01".repeat(32), "Riverside", "02".repeat(32))
        assertNull(review.reason)
        assertNull(review.replacementText)
        assertEquals(listOf("Action", "Community", "Target entry", "Acting editor"), review.rows.map { it.first })
    }

    // MARK: - Three DISTINCT wire states

    @Test
    fun emptyWirePostsButNoFeatureAndOfflineStaleAreThreeDistinctStates() {
        assertEquals(NewswireWireState.EmptyWire, NewswireWireState.from(projection(emptyList(), emptyList())))

        val post = post("a1", headline = "Report", treatment = NewswirePostTreatment.ORDINARY)
        assertTrue(NewswireWireState.from(projection(listOf(post), emptyList())) is NewswireWireState.PostsButNoFeature)
        assertTrue(NewswireWireState.from(projection(listOf(post), listOf(post))) is NewswireWireState.Featured)

        val ids = setOf(
            NewswireWireState.EmptyWire.accessibilityId,
            NewswireWireState.PostsButNoFeature(emptyList()).accessibilityId,
            NewswireWireState.OfflineStale.accessibilityId,
            NewswireWireState.Featured(emptyList(), emptyList()).accessibilityId,
        )
        assertEquals(4, ids.size)

        // Distinct copy for the three non-featured states.
        assertNotEquals(NewswireWireCopy.EMPTY_MESSAGE, NewswireWireCopy.NO_FEATURE_MESSAGE)
        assertNotEquals(NewswireWireCopy.NO_FEATURE_MESSAGE, NewswireWireCopy.OFFLINE_MESSAGE)
        assertNotEquals(NewswireWireCopy.EMPTY_MESSAGE, NewswireWireCopy.OFFLINE_MESSAGE)
    }

    /**
     * Cross-platform identity: the surface reads core's already-split front page
     * and open wire VERBATIM (same ids, same order) — it never re-sorts or
     * re-selects — so Android and iOS derive identical views from identical
     * records. (iOS `testWireStateReadsCoreProjectionVerbatimWithoutReDeriving`
     * asserts the same against the real encoder.)
     */
    @Test
    fun wireStateReadsCoreProjectionVerbatimWithoutReDeriving() {
        val p1 = post("p1", headline = "First", treatment = NewswirePostTreatment.ORDINARY)
        val p2 = post("p2", headline = "Second", treatment = NewswirePostTreatment.ORDINARY)
        val projection = projection(openWire = listOf(p2, p1), frontPage = listOf(p1))

        val state = NewswireWireState.from(projection) as NewswireWireState.Featured
        assertEquals(projection.frontPage.map { it.entryId }, state.frontPage.map { it.id })
        assertEquals(projection.openWire.map { it.entryId }, state.openWire.map { it.id })
        assertEquals(listOf("p2", "p1"), state.openWire.map { it.id })
    }

    // MARK: - Treatment rendering

    @Test
    fun hiddenAndTombstonedPostsRenderRedactionTreatmentsWithNoPayload() {
        val hidden = NewswirePostRow.of(post("h", headline = null, treatment = NewswirePostTreatment.HIDDEN))
        assertEquals(NewswirePostDisplay.HIDDEN_INTERSTITIAL, hidden.display)
        assertNull(hidden.headline)

        val tomb = NewswirePostRow.of(post("t", headline = null, treatment = NewswirePostTreatment.TOMBSTONED))
        assertEquals(NewswirePostDisplay.TOMBSTONED, tomb.display)
        assertNotEquals(NewswireTreatmentCopy.HIDDEN_BODY, NewswireTreatmentCopy.TOMBSTONE_BODY)
    }

    @Test
    fun correctionRowCarriesTheEditorialCorrectionLabelAndAFeatureDoesNot() {
        val corrected = NewswirePostRow.of(
            post("c", headline = "Report", treatment = NewswirePostTreatment.ORDINARY, correctionIds = listOf("x")),
        )
        assertTrue(corrected.hasCorrection)
        assertEquals(
            "Editorial correction",
            EditorialHistoryRow.of(action("a", NewswireEditorialActionKind.CORRECT, active = true)).correctionLabel,
        )
        assertNull(EditorialHistoryRow.of(action("f", NewswireEditorialActionKind.FEATURE, active = true)).correctionLabel)
    }

    @Test
    fun retractedActionStaysInHistoryButIsMarkedInactive() {
        val row = EditorialHistoryRow.of(action("f", NewswireEditorialActionKind.FEATURE, active = false))
        assertFalse(row.isActive)
        assertEquals(NewswireEditorialActionKind.FEATURE, row.kind)
    }

    // MARK: - Editor visibility (UI hint ONLY — decoupled from authorization)

    @Test
    fun editorVisibilityIsAPureHintDecoupledFromAuthorization() {
        val me = "aa".repeat(32)
        // Unknown roster (joined/loaded community) ⇒ never offered a control.
        assertFalse(EditorialAuthority.isRecognizedEditor(me, null))
        // Empty roster ⇒ core's founder-alone default ⇒ editor.
        assertTrue(EditorialAuthority.isRecognizedEditor(me, emptyList()))
        // Named ⇒ editor iff present (case-insensitive).
        assertTrue(EditorialAuthority.isRecognizedEditor(me, listOf("AA".repeat(32))))
        assertFalse(EditorialAuthority.isRecognizedEditor(me, listOf("11".repeat(32))))
        // Empty key ⇒ never an editor.
        assertFalse(EditorialAuthority.isRecognizedEditor("", emptyList()))
    }

    // MARK: - Helpers

    private fun invalid(draft: EditorialActionDraft): EditorialFieldViolation =
        (EditorialActionValidator.validate(draft) as EditorialValidation.Invalid).violation

    private fun valid(draft: EditorialActionDraft): ValidatedEditorialAction =
        (EditorialActionValidator.validate(draft) as EditorialValidation.Valid).action

    private fun author(key: String = "ab".repeat(32)) =
        NewswireAuthor(id = key, displayName = "Ana", tag = key.take(8), rendered = "Ana · ${key.take(8)}")

    private fun post(
        id: String,
        headline: String?,
        treatment: NewswirePostTreatment,
        correctionIds: List<String> = emptyList(),
    ) = NewswireProjectedPost(
        entryId = id,
        author = author(),
        taiJ2000Micros = 1u,
        headline = headline,
        body = headline?.let { "body" },
        language = "en",
        coarseLocation = null,
        eventTimeUnixSeconds = null,
        expiresAtUnixSeconds = null,
        sourceClaims = emptyList(),
        operationalProfile = null,
        aiAssisted = false,
        verificationIds = emptyList(),
        correctionIds = correctionIds,
        treatment = treatment,
    )

    private fun action(id: String, kind: NewswireEditorialActionKind, active: Boolean) =
        NewswireProjectedEditorialAction(
            entryId = id,
            signer = author(),
            taiJ2000Micros = 1u,
            targetEntryId = "t",
            kind = kind,
            reason = if (kind == NewswireEditorialActionKind.FEATURE) null else "reason",
            correctionText = if (kind == NewswireEditorialActionKind.CORRECT) "new" else null,
            active = active,
        )

    private fun projection(
        openWire: List<NewswireProjectedPost>,
        frontPage: List<NewswireProjectedPost>,
    ) = NewswireProjectionView(
        openWire = openWire,
        frontPage = frontPage,
        earlier = emptyList(),
        editorialHistory = emptyList(),
        futureQuarantine = emptyList(),
    )
}
