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
            )
    }
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
