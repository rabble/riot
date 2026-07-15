package org.riot.evidence

import uniffi.riot_ffi.CommunityRelationship
import uniffi.riot_ffi.CommunityRow

/**
 * Multiple communities (Unit 3C) — the "Your communities" chooser, in pure
 * Kotlin so it is host-JVM testable (the FFI does not load off-device; the
 * switch itself is exercised on-device / at the FFI level, per Risk 10). Mirrors
 * the iOS `CommunityChooser.swift` models so both platforms render the same
 * plain language.
 */

/** Plain-language relationship, from core's derived [CommunityRelationship]. */
fun CommunityRelationship.plainLabel(): String =
    when (this) {
        CommunityRelationship.ORGANIZER -> "Organizer"
        CommunityRelationship.MEMBER -> "Member"
        CommunityRelationship.PUBLIC_READER -> "Public reader"
    }

/** Plain relative-time phrases for the chooser — never a raw timestamp. */
object CommunityRelativeTime {
    fun recentActivity(unixSeconds: ULong?, nowUnixSeconds: Long): String =
        if (unixSeconds == null) "No activity yet" else "Active ${phrase(unixSeconds, nowUnixSeconds)}"

    fun syncFreshness(unixSeconds: ULong?, nowUnixSeconds: Long): String =
        if (unixSeconds == null) "Not synced yet" else "Synced ${phrase(unixSeconds, nowUnixSeconds)}"

    fun phrase(unixSeconds: ULong, nowUnixSeconds: Long): String {
        val seconds = (nowUnixSeconds - unixSeconds.toLong()).coerceAtLeast(0)
        return when {
            seconds < 60 -> "just now"
            seconds < 3_600 -> plural(seconds / 60, "minute")
            seconds < 86_400 -> plural(seconds / 3_600, "hour")
            else -> plural(seconds / 86_400, "day")
        }
    }

    private fun plural(count: Long, unit: String): String =
        "$count $unit${if (count == 1L) "" else "s"} ago"
}

/**
 * One plain-language chooser row. Name and relationship lead; the namespace id
 * is carried for addressing (switch, recovery) but is never the visible label.
 */
data class CommunityChooserRow(
    val namespaceId: String,
    val name: String,
    val relationshipLabel: String,
    val recentActivity: String,
    val syncFreshness: String,
    val available: Boolean,
    val archived: Boolean,
    val quarantined: Boolean,
    /**
     * Joined but never synced (Unit 3D, manual share-reference join): a held
     * member community with no activity and no sync exchange yet — its descriptor
     * and content arrive on the first sync. A distinct HONEST state; the row says
     * so rather than fabricating a name or a feed. Mirrors iOS `pendingFirstSync`.
     */
    val pendingFirstSync: Boolean,
) {
    companion object {
        fun from(row: CommunityRow, nowUnixSeconds: Long): CommunityChooserRow =
            CommunityChooserRow(
                namespaceId = row.namespaceId,
                name = row.title,
                relationshipLabel = row.relationship.plainLabel(),
                recentActivity = CommunityRelativeTime.recentActivity(row.recentActivityUnixSeconds, nowUnixSeconds),
                syncFreshness = CommunityRelativeTime.syncFreshness(row.syncFreshnessUnixSeconds, nowUnixSeconds),
                available = row.available,
                archived = row.archived,
                quarantined = row.quarantined,
                pendingFirstSync = isPendingFirstSync(row),
            )

        /**
         * A community is "pending first sync" when it is a held, openable MEMBER
         * space that has received nothing yet — no local activity and no sync
         * exchange. An organizer's own space is never pending (its descriptor is
         * local from creation); any recorded activity or sync clears the state.
         * Derived entirely from core's [CommunityRow], never from a UI guess.
         */
        fun isPendingFirstSync(row: CommunityRow): Boolean =
            row.available &&
                !row.archived &&
                !row.quarantined &&
                row.relationship != CommunityRelationship.ORGANIZER &&
                row.recentActivityUnixSeconds == null &&
                row.syncFreshnessUnixSeconds == null
    }
}

/**
 * The manual, share-reference join path (Unit 3D). A person pastes a
 * `riot://newswire/join/v1/...` reference; Riot decodes it, joins the named
 * community as a fresh unlinkable member, and shows it "pending first sync" until
 * its descriptor and content arrive over sync. Mirrors iOS `CommunityShareJoin`.
 */
object CommunityShareJoin {
    /**
     * The provisional local label a joined community carries BEFORE its signed
     * descriptor arrives over sync and supplies the real name. The reference
     * carries only coordinates, never a name, so this is the honest placeholder;
     * a short namespace prefix keeps two pending joins distinguishable without
     * leading with a full technical id.
     */
    fun provisionalTitle(namespaceId: String): String = "New community · ${namespaceId.take(6)}"
}

/**
 * Returning-opens-last-available (nav design Slice 3): open the last available
 * community directly; if it can't open, open the chooser with its record
 * preserved for recovery; otherwise the chooser, or the no-community state.
 */
sealed interface CommunityReturnOutcome {
    data class OpenCommunity(val namespaceId: String) : CommunityReturnOutcome
    data class Unavailable(val name: String) : CommunityReturnOutcome
    object Chooser : CommunityReturnOutcome
    object NoCommunity : CommunityReturnOutcome

    companion object {
        fun decide(active: CommunityRow?, all: List<CommunityRow>): CommunityReturnOutcome {
            if (active != null) {
                return if (active.available && !active.archived) {
                    OpenCommunity(active.namespaceId)
                } else {
                    Unavailable(active.title)
                }
            }
            return if (all.none { !it.archived }) NoCommunity else Chooser
        }
    }
}
