package org.riot.evidence

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.riot_ffi.NewswireAuthor
import uniffi.riot_ffi.NewswirePostTreatment
import uniffi.riot_ffi.NewswireProjectedComment
import uniffi.riot_ffi.NewswireProjectedPost
import uniffi.riot_ffi.NewswireProjectionView

/**
 * Host-JVM tests for [NewswireScreen.resolve] — the pure seam the newswire screen
 * uses to turn "active community + its (possibly failing) projection" into a
 * [NewswireSurface] (wire state + comments grouped under their parent). Mirrors
 * iOS's `try? projectNewswire(...)` → offlineStale fallback. The state → rows
 * mapping ([NewswireWireState.from]) and comment grouping ([NewswireCommentRow])
 * are covered elsewhere; this proves the resolve/fallback wiring and that comments
 * survive into the surface.
 */
class NewswireScreenTest {
    private fun author(key: String = "ab".repeat(32)) =
        NewswireAuthor(id = key, displayName = "Ana", tag = key.take(8), rendered = "Ana · ${key.take(8)}")

    private fun post(id: String, headline: String?, treatment: NewswirePostTreatment) = NewswireProjectedPost(
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
        correctionIds = emptyList(),
        treatment = treatment,
    )

    private fun comment(id: String, parent: String) = NewswireProjectedComment(
        entryId = id,
        parentEntryId = parent,
        author = author(),
        taiJ2000Micros = 1u,
        body = "reply $id",
        language = "en",
        treatment = NewswirePostTreatment.ORDINARY,
    )

    private fun projection(
        openWire: List<NewswireProjectedPost>,
        frontPage: List<NewswireProjectedPost>,
        comments: List<NewswireProjectedComment> = emptyList(),
    ) = NewswireProjectionView(
        openWire = openWire,
        frontPage = frontPage,
        earlier = emptyList(),
        comments = comments,
        editorialHistory = emptyList(),
        futureQuarantine = emptyList(),
    )

    @Test
    fun nullDescriptorIsOfflineStaleAndNeverProjects() {
        var called = false
        val surface = NewswireScreen.resolve(null) { called = true; projection(emptyList(), emptyList()) }
        assertEquals(NewswireWireState.OfflineStale, surface.wire)
        assertTrue(surface.commentsByParent.isEmpty())
        assertFalse("a null descriptor must not attempt a projection", called)
    }

    @Test
    fun blankDescriptorIsOfflineStale() {
        val surface = NewswireScreen.resolve("   ") { projection(emptyList(), emptyList()) }
        assertEquals(NewswireWireState.OfflineStale, surface.wire)
    }

    @Test
    fun projectionThatThrowsIsOfflineStale() {
        val surface = NewswireScreen.resolve("desc") { error("core refused to project") }
        assertEquals(NewswireWireState.OfflineStale, surface.wire)
        assertTrue(surface.commentsByParent.isEmpty())
    }

    @Test
    fun emptyProjectionIsEmptyWire() {
        val surface = NewswireScreen.resolve("desc") { projection(emptyList(), emptyList()) }
        assertEquals(NewswireWireState.EmptyWire, surface.wire)
    }

    @Test
    fun postsWithNoFrontPageIsPostsButNoFeature() {
        val surface = NewswireScreen.resolve("desc") {
            projection(openWire = listOf(post("a", "Report", NewswirePostTreatment.ORDINARY)), frontPage = emptyList())
        }
        assertTrue(surface.wire is NewswireWireState.PostsButNoFeature)
        assertEquals(1, (surface.wire as NewswireWireState.PostsButNoFeature).openWire.size)
    }

    @Test
    fun frontPageAndOpenWireIsFeatured() {
        val featured = post("f", "Featured", NewswirePostTreatment.ORDINARY)
        val surface = NewswireScreen.resolve("desc") {
            projection(openWire = listOf(featured), frontPage = listOf(featured))
        }
        assertTrue(surface.wire is NewswireWireState.Featured)
        assertEquals("f", (surface.wire as NewswireWireState.Featured).frontPage.single().id)
    }

    @Test
    fun commentsSurviveIntoTheSurfaceGroupedUnderTheirParent() {
        val surface = NewswireScreen.resolve("desc") {
            projection(
                openWire = listOf(post("a", "Report", NewswirePostTreatment.ORDINARY)),
                frontPage = emptyList(),
                comments = listOf(comment("c1", "a"), comment("c2", "a")),
            )
        }
        assertEquals(listOf("c1", "c2"), surface.comments("a").map { it.id })
        assertTrue("a post with no replies has none", surface.comments("other").isEmpty())
    }
}
