package org.riot.evidence

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.riot_ffi.NewswireAuthor
import uniffi.riot_ffi.NewswirePostTreatment
import uniffi.riot_ffi.NewswireProjectedComment

/**
 * Host-JVM tests for the pure comment surface: [NewswireCommentRow.of] field +
 * redaction mapping and [NewswireCommentRow.group] grouping. Constructs core's
 * projected records directly (no native library) — the same honesty constraint as
 * the post surface tests: the submit → sign → reproject round-trip is proven in
 * Rust and iOS with a real profile, not here. Mirrors the semantics of
 * `NewswireSurfaceTests.swift` (group under parent; a tombstoned reply surrenders
 * its body while identity survives).
 */
class NewswireCommentTest {
    private fun author(key: String = "cd".repeat(32)) =
        NewswireAuthor(id = key, displayName = "Bo", tag = key.take(8), rendered = "Bo · ${key.take(8)}")

    private fun comment(
        id: String,
        parent: String,
        body: String?,
        treatment: NewswirePostTreatment = NewswirePostTreatment.ORDINARY,
    ) = NewswireProjectedComment(
        entryId = id,
        parentEntryId = parent,
        author = author(),
        taiJ2000Micros = 1u,
        body = body,
        language = "en",
        treatment = treatment,
    )

    @Test
    fun commentRowMapsEveryField() {
        val row = NewswireCommentRow.of(comment("c1", "postA", "I was there."))
        assertEquals("c1", row.id)
        assertEquals("postA", row.parentId)
        assertEquals("Bo · cdcdcdcd", row.author)
        assertEquals("cd".repeat(32), row.authorKeyHex)
        assertEquals("I was there.", row.body)
        assertEquals(NewswirePostDisplay.ORDINARY, row.display)
    }

    @Test
    fun hiddenCommentRedactsToInterstitial() {
        val row = NewswireCommentRow.of(comment("c2", "postA", null, NewswirePostTreatment.HIDDEN))
        assertEquals(NewswirePostDisplay.HIDDEN_INTERSTITIAL, row.display)
        assertNull(row.body)
    }

    @Test
    fun tombstonedCommentSurrendersBodyButKeepsIdentity() {
        val row = NewswireCommentRow.of(comment("c3", "postA", null, NewswirePostTreatment.TOMBSTONED))
        assertEquals(NewswirePostDisplay.TOMBSTONED, row.display)
        assertNull("a tombstoned reply surrenders its body", row.body)
        assertEquals("postA", row.parentId)
    }

    @Test
    fun groupGroupsByParentPreservingCoreOrder() {
        // Core returns a flat, already-time-sorted list; grouping must not re-sort.
        val grouped = NewswireCommentRow.group(
            listOf(
                comment("c1", "A", "first under A"),
                comment("c2", "B", "only under B"),
                comment("c3", "A", "second under A"),
            ),
        )
        assertEquals(listOf("c1", "c3"), grouped["A"]?.map { it.id })
        assertEquals(listOf("c2"), grouped["B"]?.map { it.id })
    }

    @Test
    fun groupOfEmptyIsEmpty() {
        assertTrue(NewswireCommentRow.group(emptyList()).isEmpty())
    }

    // MARK: - Pre-submit validation (the one rule the surface enforces before the
    // native sign — the twin of iOS submitComment's `.empty` guard).

    @Test
    fun emptyOrWhitespaceReplyIsNotSubmittable() {
        assertFalse(NewswireCommentValidator.isSubmittable(""))
        assertFalse(NewswireCommentValidator.isSubmittable("   \n\t"))
    }

    @Test
    fun nonEmptyReplyIsSubmittable() {
        assertTrue(NewswireCommentValidator.isSubmittable("I was there."))
        assertTrue(NewswireCommentValidator.isSubmittable("   padded  "))
    }
}
