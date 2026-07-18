package org.riot.evidence

import uniffi.riot_ffi.NewswireEditorialActionKind
import uniffi.riot_ffi.NewswirePostTreatment
import uniffi.riot_ffi.NewswireProjectedEditorialAction
import uniffi.riot_ffi.NewswireProjectedPost
import uniffi.riot_ffi.NewswireProjectionView

/**
 * Unit 2C — editorial actions, front page & open wire (Android surface logic).
 *
 * This is the pure-Kotlin TWIN of iOS `NewswireEditorial.swift`: the closed
 * editorial field table, the three distinct wire states, treatment rendering, the
 * immutable pre-signing review, and the editor-visibility hint. Both platforms
 * read core's ALREADY-SPLIT projection (`frontPage` / `openWire` /
 * `editorialHistory`) verbatim — never re-deriving — so every client shows the
 * identical surface for identical records. The rules here match the Swift twin
 * one-for-one; `RiotControllerNewswireTest` asserts them on a host JVM (no native
 * library needed — only the generated record/enum types).
 *
 * Authorization is NOT here: core refuses to sign an action from a key outside
 * the descriptor's roster. This file only decides what to SHOW.
 */

/** Person-facing label + closed-table rule for one editorial action. */
val NewswireEditorialActionKind.label: String
    get() = when (this) {
        NewswireEditorialActionKind.FEATURE -> "Feature"
        NewswireEditorialActionKind.VERIFY -> "Verify"
        NewswireEditorialActionKind.CORRECT -> "Correct"
        NewswireEditorialActionKind.HIDE -> "Hide"
        NewswireEditorialActionKind.TOMBSTONE -> "Safety tombstone"
        NewswireEditorialActionKind.RETRACT -> "Retract"
    }

/** `correct` alone carries the mandatory "Editorial correction" label so it is
 *  never mistaken for an author's own revision. */
val NewswireEditorialActionKind.isEditorialCorrection: Boolean
    get() = this == NewswireEditorialActionKind.CORRECT

enum class EditorialFieldRequirement { FORBIDDEN, REQUIRED }

data class EditorialFieldRules(
    val reason: EditorialFieldRequirement,
    val correctionText: EditorialFieldRequirement,
)

/**
 * The closed field table (identical to the Swift twin and to core's `validate`):
 *
 * | Action                    | Reason            | Correction text   |
 * | feature, verify           | forbidden         | forbidden         |
 * | correct                   | required non-empty| required non-empty|
 * | hide, tombstone, retract  | required non-empty| forbidden         |
 */
val NewswireEditorialActionKind.fieldRules: EditorialFieldRules
    get() = when (this) {
        NewswireEditorialActionKind.FEATURE, NewswireEditorialActionKind.VERIFY ->
            EditorialFieldRules(EditorialFieldRequirement.FORBIDDEN, EditorialFieldRequirement.FORBIDDEN)
        NewswireEditorialActionKind.CORRECT ->
            EditorialFieldRules(EditorialFieldRequirement.REQUIRED, EditorialFieldRequirement.REQUIRED)
        NewswireEditorialActionKind.HIDE,
        NewswireEditorialActionKind.TOMBSTONE,
        NewswireEditorialActionKind.RETRACT ->
            EditorialFieldRules(EditorialFieldRequirement.REQUIRED, EditorialFieldRequirement.FORBIDDEN)
    }

object EditorialCorrectionLabel {
    const val TEXT = "Editorial correction"
}

/** The ONE field a draft violates, so the composer can point at it precisely. */
enum class EditorialFieldViolation(val message: String) {
    REASON_FORBIDDEN("This action does not take a reason."),
    REASON_REQUIRED("A reason is required for this action."),
    CORRECTION_FORBIDDEN("This action does not take replacement text."),
    CORRECTION_REQUIRED("Replacement text is required for a correction."),
}

/** A drafted, unvalidated action — free text exactly as typed, so a failed sign
 *  can preserve it verbatim. */
data class EditorialActionDraft(
    val kind: NewswireEditorialActionKind,
    val reason: String = "",
    val correctionText: String = "",
) {
    val isEmpty: Boolean
        get() = reason.trim().isEmpty() && correctionText.trim().isEmpty()
}

/** A draft that passed the closed table: normalized fields ready for the FFI,
 *  with a forbidden field forced to null. The ONLY thing the surface signs. */
data class ValidatedEditorialAction(
    val kind: NewswireEditorialActionKind,
    val reason: String?,
    val correctionText: String?,
)

sealed class EditorialValidation {
    data class Valid(val action: ValidatedEditorialAction) : EditorialValidation()
    data class Invalid(val violation: EditorialFieldViolation) : EditorialValidation()
}

/** Validates a draft against the closed field table. Non-empty means non-empty
 *  AFTER trimming, so a reason of spaces is no reason. Pure and deterministic —
 *  the exact mirror of iOS `EditorialActionValidator`. */
object EditorialActionValidator {
    fun validate(draft: EditorialActionDraft): EditorialValidation {
        val reason = draft.reason.trim()
        val correction = draft.correctionText.trim()
        val rules = draft.kind.fieldRules

        when (rules.reason) {
            EditorialFieldRequirement.FORBIDDEN ->
                if (reason.isNotEmpty()) return EditorialValidation.Invalid(EditorialFieldViolation.REASON_FORBIDDEN)
            EditorialFieldRequirement.REQUIRED ->
                if (reason.isEmpty()) return EditorialValidation.Invalid(EditorialFieldViolation.REASON_REQUIRED)
        }
        when (rules.correctionText) {
            EditorialFieldRequirement.FORBIDDEN ->
                if (correction.isNotEmpty()) return EditorialValidation.Invalid(EditorialFieldViolation.CORRECTION_FORBIDDEN)
            EditorialFieldRequirement.REQUIRED ->
                if (correction.isEmpty()) return EditorialValidation.Invalid(EditorialFieldViolation.CORRECTION_REQUIRED)
        }

        return EditorialValidation.Valid(
            ValidatedEditorialAction(
                kind = draft.kind,
                reason = if (rules.reason == EditorialFieldRequirement.FORBIDDEN) null else reason,
                correctionText = if (rules.correctionText == EditorialFieldRequirement.FORBIDDEN) null else correction,
            ),
        )
    }
}

/**
 * Whether a profile should be OFFERED an editorial control — UI visibility only,
 * NEVER the authorization check. Core (which refuses to sign an action from a key
 * outside the roster) is the boundary. `roster` is `null` for a joined/loaded
 * community whose roster this device does not know (Risk 11): unknown is never an
 * editor here, so the control stays hidden and a real attempt fails closed. An
 * empty roster is core's founder-alone default.
 */
object EditorialAuthority {
    fun isRecognizedEditor(myKeyHex: String, roster: List<String>?): Boolean {
        val me = myKeyHex.lowercase()
        if (me.isEmpty()) return false
        if (roster == null) return false
        if (roster.isEmpty()) return true
        return roster.any { it.lowercase() == me }
    }
}

/**
 * The immutable pre-signing review: complete target entry id, community, acting
 * editor key, action, reason, and replacement text — every identifier UNTRUNCATED,
 * because this is the signing surface.
 */
data class EditorialActionReview(
    val targetEntryId: String,
    val communityName: String,
    val actingEditorKeyHex: String,
    val kind: NewswireEditorialActionKind,
    val reason: String?,
    val replacementText: String?,
) {
    val rows: List<Pair<String, String>>
        get() = buildList {
            add("Action" to kind.label)
            add("Community" to communityName)
            add("Target entry" to targetEntryId)
            add("Acting editor" to actingEditorKeyHex)
            reason?.let { add("Reason" to it) }
            replacementText?.let { add("Replacement text" to it) }
        }

    companion object {
        fun of(
            action: ValidatedEditorialAction,
            targetEntryId: String,
            communityName: String,
            actingEditorKeyHex: String,
        ) = EditorialActionReview(
            targetEntryId = targetEntryId,
            communityName = communityName,
            actingEditorKeyHex = actingEditorKeyHex,
            kind = action.kind,
            reason = action.reason,
            replacementText = action.correctionText,
        )
    }
}

/** How one projected post renders — read straight from core's treatment. */
enum class NewswirePostDisplay {
    ORDINARY,
    HIDDEN_INTERSTITIAL,
    TOMBSTONED,
    ;

    companion object {
        fun from(treatment: NewswirePostTreatment) = when (treatment) {
            NewswirePostTreatment.ORDINARY -> ORDINARY
            NewswirePostTreatment.HIDDEN -> HIDDEN_INTERSTITIAL
            NewswirePostTreatment.TOMBSTONED -> TOMBSTONED
        }
    }
}

object NewswireTreatmentCopy {
    const val HIDDEN_TITLE = "Hidden by the editorial collective"
    const val HIDDEN_BODY =
        "The collective hid this report. You can still inspect the original and the signed actions."
    const val TOMBSTONE_TITLE = "Removed for safety"
    const val TOMBSTONE_BODY =
        "The collective tombstoned this report. Its content is withheld; the signed record of the act remains."
}

data class NewswirePostRow(
    val id: String,
    val author: String,
    val authorKeyHex: String,
    val headline: String?,
    val display: NewswirePostDisplay,
    val hasCorrection: Boolean,
    val verificationCount: Int,
    val aiAssisted: Boolean,
) {
    companion object {
        fun of(post: NewswireProjectedPost) = NewswirePostRow(
            id = post.entryId,
            author = post.author.rendered,
            authorKeyHex = post.author.id,
            headline = post.headline,
            display = NewswirePostDisplay.from(post.treatment),
            hasCorrection = post.correctionIds.isNotEmpty(),
            verificationCount = post.verificationIds.size,
            aiAssisted = post.aiAssisted,
        )
    }
}

data class EditorialHistoryRow(
    val id: String,
    val signer: String,
    val kind: NewswireEditorialActionKind,
    val targetEntryId: String,
    val reason: String?,
    val replacementText: String?,
    val isActive: Boolean,
) {
    /** The mandatory correction label, present only for a correction. */
    val correctionLabel: String?
        get() = if (kind.isEditorialCorrection) EditorialCorrectionLabel.TEXT else null

    companion object {
        fun of(action: NewswireProjectedEditorialAction) = EditorialHistoryRow(
            id = action.entryId,
            signer = action.signer.rendered,
            kind = action.kind,
            targetEntryId = action.targetEntryId,
            reason = action.reason,
            replacementText = action.correctionText,
            isActive = action.active,
        )
    }
}

object NewswireWireCopy {
    const val EMPTY_TITLE = "No reports yet"
    const val EMPTY_MESSAGE = "No reports have arrived on this wire yet."
    const val NO_FEATURE_TITLE = "Nothing featured yet"
    const val NO_FEATURE_MESSAGE =
        "The collective has not selected a feature. See the open wire for every report."
    const val NO_FEATURE_LINK = "Open wire"
    const val OFFLINE_TITLE = "Updates unavailable"
    const val OFFLINE_MESSAGE =
        "This community's wire is offline or has not synced yet. What you already have is still here."
}

/**
 * The three non-featured states are DISTINCT (never collapsed): an empty wire, a
 * wire with posts but no collective feature, and an offline/stale projection.
 * Front page and open wire are read from core's split verbatim.
 */
sealed class NewswireWireState(val accessibilityId: String) {
    object OfflineStale : NewswireWireState("newswire-offline-stale")
    object EmptyWire : NewswireWireState("newswire-empty-wire")
    data class PostsButNoFeature(val openWire: List<NewswirePostRow>) :
        NewswireWireState("newswire-no-feature")
    data class Featured(val frontPage: List<NewswirePostRow>, val openWire: List<NewswirePostRow>) :
        NewswireWireState("newswire-featured")

    companion object {
        fun from(projection: NewswireProjectionView): NewswireWireState {
            val openWire = projection.openWire.map(NewswirePostRow::of)
            val frontPage = projection.frontPage.map(NewswirePostRow::of)
            return when {
                openWire.isEmpty() -> EmptyWire
                frontPage.isEmpty() -> PostsButNoFeature(openWire)
                else -> Featured(frontPage, openWire)
            }
        }
    }
}

/** Projects a community's newswire by its descriptor entry id. A one-method seam
 *  (`RiotController::projectNewswire`) so the screen resolver stays pure and
 *  host-JVM testable without a native profile. */
fun interface NewswireProjector {
    fun project(descriptorEntryId: String): NewswireProjectionView
}

/**
 * The screen-level resolver: turns the active community's descriptor (which may be
 * null for a legacy space, or point at a wire that fails to project) into the
 * [NewswireWireState] the surface renders. A null/blank descriptor or a projection
 * that throws yields [NewswireWireState.OfflineStale] — the exact mirror of iOS's
 * `try? projectNewswire(...)` → offlineStale fallback. Otherwise it delegates to
 * the already-verified [NewswireWireState.from]. Pure given the projector.
 */
object NewswireScreen {
    fun resolve(descriptorEntryId: String?, projector: NewswireProjector): NewswireWireState {
        if (descriptorEntryId.isNullOrBlank()) return NewswireWireState.OfflineStale
        // Any projection failure (an unavailable/stale wire, a core refusal) is an
        // offline-stale surface, never a crash — matches iOS's `try?` fallback.
        return try {
            NewswireWireState.from(projector.project(descriptorEntryId))
        } catch (_: Exception) {
            NewswireWireState.OfflineStale
        }
    }
}
