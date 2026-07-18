package org.riot.evidence

import android.content.SharedPreferences
import uniffi.riot_ffi.NewswireProjectionView

/**
 * What's-new / unread — per-device seen state. The Android twin of iOS
 * `WhatsNew.swift`. Whether a reader has caught up on a community's wire is a
 * per-DEVICE fact, never a signed record: it says nothing about the collective and
 * must never touch the Willow store or cross the FFI. It lives in SharedPreferences
 * keyed per community. The unread math is a pure function of the posts a projection
 * shows and the highest order key this device has already seen (the cursor).
 */

/** The backing store [SeenCursorStore] persists to. SharedPreferences conforms in
 *  production; tests inject an in-memory double. The value is a decimal string so
 *  the full `ULong` order key round-trips — SharedPreferences has no unsigned-64
 *  type, and its numeric accessors would lose precision. */
interface SeenStateStore {
    fun seenValue(key: String): String?
    fun setSeenValue(key: String, value: String?)
}

/** The production [SeenStateStore]: a SharedPreferences file of decimal cursors. */
class SharedPreferencesSeenStore(private val prefs: SharedPreferences) : SeenStateStore {
    override fun seenValue(key: String): String? = prefs.getString(key, null)

    override fun setSeenValue(key: String, value: String?) {
        prefs.edit().apply {
            if (value != null) putString(key, value) else remove(key)
        }.apply()
    }
}

/**
 * Persists, per community, the highest order key this device has viewed. The cursor
 * only ever moves FORWARD: [advance] never lowers it, so a stale reload or an
 * out-of-order call can never resurrect already-seen posts as unread.
 *
 * The community key is the newswire space descriptor entry id — stable for the life
 * of the community and distinct per community, which keeps one community's seen
 * state from bleeding into another's. An empty key (a community with no descriptor
 * yet) is inert: there is nothing to track until the wire has an identity.
 */
class SeenCursorStore(
    private val store: SeenStateStore,
    private val keyPrefix: String = "riot.newswire.seen.",
) {
    private fun storageKey(community: String): String? =
        if (community.isEmpty()) null else keyPrefix + community

    /** The highest order key this device has marked seen for the community, or null
     *  when it has never looked (a fresh community — everything is unread). */
    fun cursor(community: String): ULong? =
        storageKey(community)?.let { store.seenValue(it) }?.toULongOrNull()

    /** Move the community's cursor up to [value]. A no-op when [value] is not
     *  strictly greater than the stored cursor (monotonic) or the community has no
     *  key yet, so it is always safe to call with the max of whatever is on screen —
     *  it can never mark seen something older than the reader saw. */
    fun advance(community: String, value: ULong) {
        val key = storageKey(community) ?: return
        val existing = cursor(community)
        if (existing != null && existing >= value) return
        store.setSeenValue(key, value.toString())
    }
}

/**
 * The unread state for a community's wire, derived from a projection and the stored
 * cursor. Dedups `open_wire + front_page` by entry id — a featured post is also on
 * the wire, so keying by id counts each post once and the total matches what the
 * reader can actually see. Twin of iOS `NewswireSurfaceModel.seenRefs` + the
 * `NewswireUnread(posts:cursor:)` construction.
 */
fun NewswireUnread.Companion.of(projection: NewswireProjectionView, cursor: ULong?): NewswireUnread {
    val byId = linkedMapOf<String, SeenPostRef>()
    for (post in projection.openWire + projection.frontPage) {
        byId[post.entryId] = SeenPostRef(post.entryId, post.taiJ2000Micros)
    }
    return NewswireUnread(posts = byId.values.toList(), cursor = cursor)
}
