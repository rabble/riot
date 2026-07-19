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
        reactions = emptyList(),
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
        val surface = NewswireScreen.resolve(null, cursor = null) { called = true; projection(emptyList(), emptyList()) }
        assertEquals(NewswireWireState.OfflineStale, surface.wire)
        assertTrue(surface.commentsByParent.isEmpty())
        assertFalse("a null descriptor must not attempt a projection", called)
    }

    @Test
    fun blankDescriptorIsOfflineStale() {
        val surface = NewswireScreen.resolve("   ", cursor = null) { projection(emptyList(), emptyList()) }
        assertEquals(NewswireWireState.OfflineStale, surface.wire)
    }

    @Test
    fun projectionThatThrowsIsOfflineStale() {
        val surface = NewswireScreen.resolve("desc", cursor = null) { error("core refused to project") }
        assertEquals(NewswireWireState.OfflineStale, surface.wire)
        assertTrue(surface.commentsByParent.isEmpty())
    }

    @Test
    fun emptyProjectionIsEmptyWire() {
        val surface = NewswireScreen.resolve("desc", cursor = null) { projection(emptyList(), emptyList()) }
        assertEquals(NewswireWireState.EmptyWire, surface.wire)
    }

    @Test
    fun postsWithNoFrontPageIsPostsButNoFeature() {
        val surface = NewswireScreen.resolve("desc", cursor = null) {
            projection(openWire = listOf(post("a", "Report", NewswirePostTreatment.ORDINARY)), frontPage = emptyList())
        }
        assertTrue(surface.wire is NewswireWireState.PostsButNoFeature)
        assertEquals(1, (surface.wire as NewswireWireState.PostsButNoFeature).openWire.size)
    }

    @Test
    fun frontPageAndOpenWireIsFeatured() {
        val featured = post("f", "Featured", NewswirePostTreatment.ORDINARY)
        val surface = NewswireScreen.resolve("desc", cursor = null) {
            projection(openWire = listOf(featured), frontPage = listOf(featured))
        }
        assertTrue(surface.wire is NewswireWireState.Featured)
        assertEquals("f", (surface.wire as NewswireWireState.Featured).frontPage.single().id)
    }

    @Test
    fun aFeaturedPostRepeatedOnTheWireDoesNotOwnItsThreadInTheFeaturedSection() {
        // Core always re-lists a featured post on the open wire (featured ⊆ open
        // wire), so its thread renders once — on the canonical open-wire row — and
        // the Featured highlight is headline-only.
        val featured = NewswirePostRow.of(post("f", "Featured", NewswirePostTreatment.ORDINARY))
        val state = NewswireWireState.Featured(frontPage = listOf(featured), openWire = listOf(featured))
        assertTrue(state.featuredOnlyIds.isEmpty())
    }

    @Test
    fun aFeaturedPostAbsentFromTheOpenWireKeepsItsThreadInFeatured() {
        val onlyFeatured = NewswirePostRow.of(post("f", "Featured", NewswirePostTreatment.ORDINARY))
        val onWire = NewswirePostRow.of(post("w", "Wire", NewswirePostTreatment.ORDINARY))
        val state = NewswireWireState.Featured(frontPage = listOf(onlyFeatured), openWire = listOf(onWire))
        assertEquals(setOf("f"), state.featuredOnlyIds)
    }

    @Test
    fun commentsSurviveIntoTheSurfaceGroupedUnderTheirParent() {
        val surface = NewswireScreen.resolve("desc", cursor = null) {
            projection(
                openWire = listOf(post("a", "Report", NewswirePostTreatment.ORDINARY)),
                frontPage = emptyList(),
                comments = listOf(comment("c1", "a"), comment("c2", "a")),
            )
        }
        assertEquals(listOf("c1", "c2"), surface.comments("a").map { it.id })
        assertTrue("a post with no replies has none", surface.comments("other").isEmpty())
    }

    @Test
    fun surfaceCarriesUnreadComputedAgainstTheCursor() {
        // The post helper stamps tai = 1, so a null cursor makes it unread and a
        // cursor at 1 makes it caught up — one projection yields wire + unread.
        val fresh = NewswireScreen.resolve("desc", cursor = null) {
            projection(openWire = listOf(post("a", "Report", NewswirePostTreatment.ORDINARY)), frontPage = emptyList())
        }
        assertEquals(1, fresh.unread.count)

        val caughtUp = NewswireScreen.resolve("desc", cursor = 1u) {
            projection(openWire = listOf(post("a", "Report", NewswirePostTreatment.ORDINARY)), frontPage = emptyList())
        }
        assertEquals(0, caughtUp.unread.count)
    }

    @Test
    fun offlineSurfaceHasNoUnread() {
        val surface = NewswireScreen.resolve("desc", cursor = null) { error("offline") }
        assertEquals(NewswireUnread.NONE, surface.unread)
    }
}
