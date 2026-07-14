package org.riot.evidence.apps

import uniffi.riot_ffi.DirectoryListing
import uniffi.riot_ffi.PublicSpace

/** The computed app directory as the storefront reads and writes it. */
interface DirectoryPort {
    fun listings(): List<DirectoryListing>
    fun endorse(appId: ByteArray, note: String, retract: Boolean)
    fun share(appId: ByteArray, space: PublicSpace)

    /**
     * This profile's own 32-byte subspace id — the id it signs endorsements
     * with, and the one to look for among a listing's endorsers to know whether
     * a recommendation is this person's to take back.
     */
    fun mySubspaceId(): ByteArray
}

/**
 * The installed-tool operations the storefront needs: install a built-in and
 * match a directory row back to a locally-held bundle. Implemented by
 * [RiotAppsController]; faked in unit tests.
 */
interface InstalledAppsAccess {
    fun install(manifestBytes: ByteArray, bundleBytes: ByteArray): InstalledApp
    fun find(appIdHex: String): InstalledApp?

    /** Takes up an app this profile already holds, from the store's own bytes. */
    fun getFromDirectory(appIdBytes: ByteArray): InstalledApp
}

/** Bytes of a built-in app shipped with the binary. */
interface StarterCatalog {
    /**
     * (manifestBytes, bundleBytes), or null when the shipped assets can't be
     * read. Null is not fatal: the built-in is compiled into the core so it
     * still appears in listings, it just can't be opened until its bytes are
     * available.
     */
    fun read(): Pair<ByteArray, ByteArray>?
}

/**
 * Storefront logic with no Android or FFI types of its own — it reaches the
 * directory surface, the installed-tool store, and the shipped built-ins only
 * through the three ports above, so it runs entirely in JVM tests (same shape
 * as [RiotJsBridge]).
 */
class DirectoryController(
    private val port: DirectoryPort,
    private val installed: InstalledAppsAccess,
    private val starter: StarterCatalog,
) {
    private var starterAttempted = false

    /**
     * Installs the built-in checklist so it appears under Tools and can be
     * opened from the directory. `install` is idempotent, but re-reading the
     * assets on every render is not free — this runs the attempt at most once.
     */
    fun ensureStarterInstalled() {
        if (starterAttempted) return
        starterAttempted = true
        val bytes = starter.read() ?: return
        installed.install(bytes.first, bytes.second)
    }

    fun listings(): List<DirectoryListing> = port.listings()

    /**
     * The locally-held tool for a row, or null when its bytes haven't arrived
     * yet — a carried app this profile can list but not open until sync brings
     * the bundle.
     */
    fun installedFor(listing: DirectoryListing): InstalledApp? =
        installed.find(hex(listing.appId))

    /**
     * Whether this row can be taken up: an app someone in your community is
     * carrying, whose pages have finished arriving, that this profile does not
     * hold yet. While the bundle is still in flight there is nothing to open,
     * so the row offers no action and says so instead.
     */
    fun canGet(listing: DirectoryListing): Boolean =
        installedFor(listing) == null && listing.bundlePresent

    /**
     * Takes up a carried app so it can be reviewed and opened. Rust re-derives
     * the app id from its own stored bytes and refuses anything that does not
     * match, so the failure here is not a user error to explain in detail —
     * whatever went wrong, the honest answer is that the app is not all here
     * yet. Callers surface the message as-is.
     */
    fun get(listing: DirectoryListing): InstalledApp =
        try {
            installed.getFromDirectory(listing.appId)
        } catch (error: Exception) {
            throw IllegalStateException(
                "${listing.name} hasn't finished arriving from your group. " +
                    "Try again the next time you're together.",
                error,
            )
        }

    /**
     * True when a recognized organizer of the current space trusts this app —
     * the signal that flips a row from "Review" to "Open".
     */
    fun trustedInCurrentSpace(listing: DirectoryListing, space: PublicSpace?): Boolean {
        if (space == null) return false
        return listing.trustedInSpaces.any { hex(it) == space.namespaceId }
    }

    /**
     * Whether this profile may recommend the app: endorsement speaks for a
     * space that already trusts the app (design spec), so it is offered only
     * where the app is on in the current space — and not on a row this person
     * has already recommended, which offers the take-back instead. The two
     * controls are exclusive HERE rather than in the surface, so no view can
     * draw both by getting its branches wrong.
     */
    fun canRecommend(listing: DirectoryListing, space: PublicSpace?): Boolean =
        trustedInCurrentSpace(listing, space) && !endorsedByMe(listing)

    fun recommend(listing: DirectoryListing, note: String) =
        port.endorse(listing.appId, note, false)

    /**
     * True when THIS profile is among the app's endorsers.
     *
     * There is no local list of what this device recommended, and there does not
     * need to be: an endorsement is a signed entry, so the directory already
     * knows. Asking it — rather than mirroring the decision on the side — means
     * the answer is right after a relaunch, and a recommendation withdrawn
     * anywhere stops being offered for take-back here.
     */
    fun endorsedByMe(listing: DirectoryListing): Boolean {
        val me = port.mySubspaceId()
        return listing.endorsingMetSubspaces.any { it.contentEquals(me) }
    }

    /**
     * Whether to offer the take-back. Exclusive with [canRecommend]: you either
     * recommended this app or you did not, and only one of the two controls can
     * be true of a row at a time.
     */
    fun canRetract(listing: DirectoryListing): Boolean = endorsedByMe(listing)

    /** Withdraws this profile's recommendation of an app. */
    fun retractRecommendation(listing: DirectoryListing) =
        port.endorse(listing.appId, "", true)

    fun share(listing: DirectoryListing, space: PublicSpace) =
        port.share(listing.appId, space)

    companion object {
        private val HEX = "0123456789abcdef".toCharArray()

        /** Lowercase hex, matching the Rust FFI's own encoding of app ids. */
        fun hex(bytes: ByteArray): String {
            val out = CharArray(bytes.size * 2)
            for (i in bytes.indices) {
                val v = bytes[i].toInt() and 0xff
                out[i * 2] = HEX[v ushr 4]
                out[i * 2 + 1] = HEX[v and 0x0f]
            }
            return String(out)
        }
    }
}
