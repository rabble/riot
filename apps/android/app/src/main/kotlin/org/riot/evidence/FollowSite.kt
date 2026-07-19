package org.riot.evidence

import uniffi.riot_ffi.FollowedSiteRow

/**
 * Follow a composite indymedia site by pasting its `riot://site/v1/...` ticket,
 * and project the followed-sites list — in pure Kotlin so it is host-JVM
 * testable (the FFI does not load off-device; the real trust check lives in the
 * core `follow_site`). Mirrors the iOS `FollowSiteModel.swift` so both platforms
 * render the same plain language and make the same honest fetch-gate decision.
 */

/**
 * Why a pasted/scanned string was refused as a site follow ticket, mapped to
 * actionable copy by the screen. Parallel to a community join reference — but a
 * SITE ticket's real trust check (signature + expiry) is the core's
 * `follow_site`, so this layer only screens the scheme and length before the FFI
 * verify.
 */
enum class FollowSiteError {
    /** A scanned/pasted payload that is not a `riot://site/v1/...` ticket at all. */
    NOT_A_SITE_TICKET,

    /**
     * Exceeds the sane bound — refused before any FFI work so a hostile QR code
     * (or pasted blob) cannot make the app chew on megabytes.
     */
    TOO_LONG,
}

/** Thrown by [FollowSiteModel.screen] when a payload fails the scheme/length screen. */
class FollowSiteScreenException(val reason: FollowSiteError) : Exception(reason.name)

/**
 * Pure, camera-free screening for the follow-a-site-by-ticket flow. Unlike a
 * community join reference (decoded locally), a SITE ticket's signature and
 * expiry are verified in the core `follow_site` FFI — so this model only enforces
 * the `riot://site/v1/` scheme and a length bound before the string is handed to
 * the FFI. Unit-testable without a device.
 */
object FollowSiteModel {
    const val SITE_SCHEME = "riot://site/v1/"

    /**
     * A canonical ticket is well under this; the bound only exists to refuse a
     * hostile oversize payload before the FFI ever sees it.
     */
    const val MAX_LENGTH = 4096

    /**
     * Screen a pasted/scanned ticket BEFORE the FFI verify. Enforces the scheme
     * and length bound; the real trust check (signature, expiry) is the core's.
     * Returns the trimmed ticket ready for `follow_site`, or throws
     * [FollowSiteScreenException].
     */
    fun screen(ticket: String): String {
        val trimmed = ticket.trim()
        if (trimmed.length > MAX_LENGTH) throw FollowSiteScreenException(FollowSiteError.TOO_LONG)
        if (!trimmed.startsWith(SITE_SCHEME)) {
            throw FollowSiteScreenException(FollowSiteError.NOT_A_SITE_TICKET)
        }
        return trimmed
    }

    /**
     * Decode a 64-char lowercase-hex site root to its 32 bytes, or `null` if it is
     * not exactly 32 bytes of hex. Used to hand `import_followed_site_bundle` the
     * followed root it re-verifies the pulled bytes against.
     */
    fun hexBytes(hex: String): ByteArray? {
        if (hex.length != 64) return null
        val out = ByteArray(32)
        var i = 0
        while (i < 64) {
            val hi = Character.digit(hex[i], 16)
            val lo = Character.digit(hex[i + 1], 16)
            if (hi < 0 || lo < 0) return null
            out[i / 2] = ((hi shl 4) or lo).toByte()
            i += 2
        }
        return out
    }
}

/**
 * One row on the followed-sites list, projected from core's [FollowedSiteRow].
 * Pure + a data class so the list is unit-testable without a device: the view
 * renders exactly these fields and makes no trust decision of its own — in
 * particular it HONORS the core's fetch-time arti gate rather than re-deciding it.
 */
data class FollowedSiteDisplay(
    val root: String,
    val title: String,
    val stateLabel: String,
    /**
     * True iff the site requires an unavailable transport (`require:arti`). The
     * row then shows "Requires Tor — unavailable" and offers NO fetch button.
     */
    val transportBlocked: Boolean,
    /**
     * The HTTPS URL to pull the owner-signed bundle from, or `null`. A blocked row
     * always projects `null` here (the core gate nulls it), so [canRefresh] is
     * false and no clearnet IP can leak to a mirror.
     */
    val fetchUrl: String?,
) {
    /**
     * Whether a "Refresh from site" action is offered: only when the site is not
     * transport-blocked AND carries a URL to pull from.
     */
    val canRefresh: Boolean
        get() = !transportBlocked && fetchUrl != null

    companion object {
        /**
         * Project a core [FollowedSiteRow]. A transport-blocked row keeps NO fetch
         * URL locally either — belt and suspenders over the core gate.
         */
        fun from(row: FollowedSiteRow): FollowedSiteDisplay =
            FollowedSiteDisplay(
                root = row.root,
                title = row.title,
                stateLabel = label(row.state),
                transportBlocked = row.transportBlocked,
                fetchUrl = if (row.transportBlocked) null else row.fetchUrl,
            )

        /** Human copy for the honest row-state token core hands back. */
        fun label(state: String): String =
            when (state) {
                "available" -> "Up to date"
                "pending-first-sync" -> "Waiting for first sync"
                "transport-blocked" -> "Requires Tor — unavailable"
                "degraded" -> "Needs attention"
                else -> state
            }

        /** Honest "Imported N record(s)" feedback after a successful refresh-import. */
        fun importedSummary(count: Int): String =
            "Imported $count record${if (count == 1) "" else "s"}"
    }
}
