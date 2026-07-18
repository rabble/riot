package org.riot.evidence

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.riot_ffi.NewswireAuthor
import uniffi.riot_ffi.NewswirePostTreatment
import uniffi.riot_ffi.NewswireProjectedPost
import uniffi.riot_ffi.NewswireProjectionView

/**
 * Host-JVM tests for [NewswireScreen.resolve] — the pure seam the newswire screen
 * uses to turn "active community + its (possibly failing) projection" into a
 * [NewswireWireState]. Mirrors iOS's `try? projectNewswire(...)` → offlineStale
 * fallback. The state → rows mapping ([NewswireWireState.from]) is already covered
 * by `RiotControllerNewswireTest`; this only proves the resolve/fallback wiring.
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

    private fun projection(
        openWire: List<NewswireProjectedPost>,
        frontPage: List<NewswireProjectedPost>,
    ) = NewswireProjectionView(
        openWire = openWire,
        frontPage = frontPage,
        earlier = emptyList(),
        comments = emptyList(),
        editorialHistory = emptyList(),
        futureQuarantine = emptyList(),
    )

    @Test
    fun nullDescriptorIsOfflineStaleAndNeverProjects() {
        var called = false
        val state = NewswireScreen.resolve(null) { called = true; projection(emptyList(), emptyList()) }
        assertEquals(NewswireWireState.OfflineStale, state)
        assertFalse("a null descriptor must not attempt a projection", called)
    }

    @Test
    fun blankDescriptorIsOfflineStale() {
        val state = NewswireScreen.resolve("   ") { projection(emptyList(), emptyList()) }
        assertEquals(NewswireWireState.OfflineStale, state)
    }

    @Test
    fun projectionThatThrowsIsOfflineStale() {
        val state = NewswireScreen.resolve("desc") { error("core refused to project") }
        assertEquals(NewswireWireState.OfflineStale, state)
    }

    @Test
    fun emptyProjectionIsEmptyWire() {
        val state = NewswireScreen.resolve("desc") { projection(emptyList(), emptyList()) }
        assertEquals(NewswireWireState.EmptyWire, state)
    }

    @Test
    fun postsWithNoFrontPageIsPostsButNoFeature() {
        val state = NewswireScreen.resolve("desc") {
            projection(openWire = listOf(post("a", "Report", NewswirePostTreatment.ORDINARY)), frontPage = emptyList())
        }
        assertTrue(state is NewswireWireState.PostsButNoFeature)
        assertEquals(1, (state as NewswireWireState.PostsButNoFeature).openWire.size)
    }

    @Test
    fun frontPageAndOpenWireIsFeatured() {
        val featured = post("f", "Featured", NewswirePostTreatment.ORDINARY)
        val state = NewswireScreen.resolve("desc") {
            projection(openWire = listOf(featured), frontPage = listOf(featured))
        }
        assertTrue(state is NewswireWireState.Featured)
        assertEquals("f", (state as NewswireWireState.Featured).frontPage.single().id)
    }
}
