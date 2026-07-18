package org.riot.evidence

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.riot_ffi.NewswireAuthor
import uniffi.riot_ffi.NewswirePostTreatment
import uniffi.riot_ffi.NewswireProjectedPost
import uniffi.riot_ffi.NewswireProjectionView

/**
 * Host-JVM twin of iOS `RiotTests/WhatsNewTests.swift`: the per-device seen cursor
 * ([SeenCursorStore]) and the unread computation off a projection
 * ([NewswireUnread.of]). Pure — an in-memory [SeenStateStore] double stands in for
 * SharedPreferences and projections are constructed directly (no native library).
 */
class WhatsNewTest {
    /** In-memory backing so "persists across reload" is a second store over the
     *  SAME map, and the cursor logic runs without SharedPreferences. */
    private class MemorySeenStore : SeenStateStore {
        private val values = mutableMapOf<String, String>()
        override fun seenValue(key: String): String? = values[key]
        override fun setSeenValue(key: String, value: String?) {
            if (value != null) values[key] = value else values.remove(key)
        }
    }

    private fun author() = NewswireAuthor(id = "ab", displayName = "Ana", tag = "ab", rendered = "Ana · ab")

    private fun post(id: String, tai: ULong) = NewswireProjectedPost(
        entryId = id,
        author = author(),
        taiJ2000Micros = tai,
        headline = "h-$id",
        body = "b",
        language = "en",
        coarseLocation = null,
        eventTimeUnixSeconds = null,
        expiresAtUnixSeconds = null,
        sourceClaims = emptyList(),
        operationalProfile = null,
        aiAssisted = false,
        verificationIds = emptyList(),
        correctionIds = emptyList(),
        treatment = NewswirePostTreatment.ORDINARY,
    )

    private fun projection(openWire: List<NewswireProjectedPost>, frontPage: List<NewswireProjectedPost> = emptyList()) =
        NewswireProjectionView(
            openWire = openWire,
            frontPage = frontPage,
            earlier = emptyList(),
            comments = emptyList(),
            editorialHistory = emptyList(),
            futureQuarantine = emptyList(),
        )

    // MARK: - Seen cursor persistence

    @Test
    fun cursorIsNullBeforeAnythingIsMarkedSeen() {
        assertNull(SeenCursorStore(MemorySeenStore()).cursor("space-1"))
    }

    @Test
    fun advanceThenReadReturnsTheAdvancedValue() {
        val store = SeenCursorStore(MemorySeenStore())
        store.advance("space-1", 42u)
        assertEquals(42uL, store.cursor("space-1"))
    }

    @Test
    fun advanceNeverMovesTheCursorBackward() {
        val store = SeenCursorStore(MemorySeenStore())
        store.advance("space-1", 100u)
        store.advance("space-1", 40u)
        assertEquals(100uL, store.cursor("space-1"))
    }

    @Test
    fun cursorPersistsAcrossStoreReload() {
        val backing = MemorySeenStore()
        SeenCursorStore(backing).advance("space-1", 77u)
        // A fresh store over the same backing is a new app launch.
        assertEquals(77uL, SeenCursorStore(backing).cursor("space-1"))
    }

    @Test
    fun oneCommunitysCursorDoesNotAffectAnother() {
        val store = SeenCursorStore(MemorySeenStore())
        store.advance("space-A", 500u)
        assertEquals(500uL, store.cursor("space-A"))
        assertNull(store.cursor("space-B"))
        store.advance("space-B", 10u)
        assertEquals(500uL, store.cursor("space-A"))
        assertEquals(10uL, store.cursor("space-B"))
    }

    @Test
    fun emptyCommunityKeyIsInert() {
        val store = SeenCursorStore(MemorySeenStore())
        store.advance("", 99u)
        assertNull(store.cursor(""))
    }

    @Test
    fun largeOrderKeyRoundTripsWithoutPrecisionLoss() {
        val store = SeenCursorStore(MemorySeenStore())
        val big = 9_000_000_000_000_000_123uL
        store.advance("space-1", big)
        assertEquals(big, store.cursor("space-1"))
    }

    // MARK: - Unread off a projection (dedup open wire + front page)

    @Test
    fun freshCommunityWithNoCursorMarksEveryPostUnread() {
        val unread = NewswireUnread.of(projection(listOf(post("a", 30u), post("b", 20u), post("c", 10u))), cursor = null)
        assertEquals(3, unread.count)
        assertTrue(unread.hasUnread)
        assertTrue(unread.isNew("a"))
        assertEquals(30uL, unread.latestTimestamp)
    }

    @Test
    fun cursorAtLatestMarksNothingUnread() {
        val unread = NewswireUnread.of(projection(listOf(post("a", 30u), post("b", 20u))), cursor = 30u)
        assertEquals(0, unread.count)
        assertFalse(unread.hasUnread)
    }

    @Test
    fun onlyPostsStrictlyNewerThanCursorAreUnread() {
        val unread = NewswireUnread.of(
            projection(listOf(post("new", 40u), post("seen", 25u), post("boundary", 25u))),
            cursor = 25u,
        )
        assertEquals(1, unread.count)
        assertTrue(unread.isNew("new"))
        assertFalse(unread.isNew("boundary"))
    }

    @Test
    fun frontPageAndOpenWireOverlapCountsEachPostOnce() {
        // A featured post is re-listed on the open wire; keying by entry id counts
        // it once so the total matches what the reader can see.
        val shared = post("feat", 50u)
        val unread = NewswireUnread.of(
            projection(openWire = listOf(shared, post("x", 40u)), frontPage = listOf(shared)),
            cursor = null,
        )
        assertEquals(2, unread.count)
    }

    @Test
    fun markSeenThenRecomputeShowsZeroThenAnyNewerPost() {
        val store = SeenCursorStore(MemorySeenStore())
        val posts = listOf(post("a", 30u), post("b", 20u), post("c", 10u))

        val firstVisit = NewswireUnread.of(projection(posts), store.cursor("s"))
        assertEquals(3, firstVisit.count)

        // Marking all seen advances the cursor to the newest shown post.
        store.advance("s", firstVisit.latestTimestamp ?: 0u)
        assertEquals(0, NewswireUnread.of(projection(posts), store.cursor("s")).count)

        // Only a post newer than the cursor reads as unread afterward.
        val third = NewswireUnread.of(projection(posts + post("d", 45u)), store.cursor("s"))
        assertEquals(1, third.count)
        assertTrue(third.isNew("d"))
    }
}
