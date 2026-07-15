package org.riot.evidence.apps

import uniffi.riot_ffi.AppExecutionSession
import uniffi.riot_ffi.ProfileSession
import uniffi.riot_ffi.WhoAmI

/**
 * The bridge's only I/O surface. Kept as a plain interface so JVM tests
 * drive the bridge without Android or the FFI. The real security boundary
 * — prefix scoping, value caps, key validation, AND (Unit 0C) revocation /
 * namespace / generation revalidation — lives in Rust, not here.
 */
interface AppDataPort {
    fun put(key: String, value: ByteArray)
    fun get(key: String): ByteArray?
    fun list(prefix: String): List<Pair<String, ByteArray>>

    /**
     * Tear the underlying Rust execution session down (Unit 0C). After this,
     * every read/commit fails closed. Default is a no-op for the test doubles
     * and non-session ports; only [UniffiAppDataPort] overrides it.
     */
    fun destroy() {}

    /**
     * Whether the underlying execution session is still valid RIGHT NOW — used
     * to tell a §4.7 invalidation (revoked / namespace-swapped / stale
     * generation → close to "Return to Tools") apart from an ordinary per-op
     * rejection (a malformed key → inline error). Both throw the same error.
     */
    fun isValid(): Boolean = true
}

/**
 * Thin adapter over the Rust-owned [AppExecutionSession] (Unit 0C). Every read
 * and commit goes through the gated session, which revalidates the app's
 * authority (trust / namespace / generation / not-destroyed) BEFORE it touches
 * data — so a revoked, namespace-swapped, re-approved, or torn-down app cannot
 * read or write even though this port still exists.
 *
 * `put` uses the receipt-returning variant so the host can persist the committed
 * signed bundle bytes and replay them on the next open — `onCommitted` hands
 * those bytes to the persistence layer. A persistence failure surfaces to the
 * caller (the bridge reports "couldn't save"), matching how the alert and trust
 * paths propagate a failed durable write.
 */
class UniffiAppDataPort(
    private val execution: AppExecutionSession,
    private val onCommitted: (key: String, bundleBytes: ByteArray) -> Unit = { _, _ -> },
) : AppDataPort {
    override fun put(key: String, value: ByteArray) {
        val bundle = execution.appDataPutWithReceipt(key, value)
        onCommitted(key, bundle)
    }

    override fun get(key: String): ByteArray? = execution.appDataGet(key)
    override fun list(prefix: String): List<Pair<String, ByteArray>> =
        execution.appDataList(prefix).map { it.key to it.value }

    override fun destroy() = execution.invalidate()
    override fun isValid(): Boolean = execution.isValid()
}

/**
 * One person as an app sees them: the stable **id** it stores, plus the two
 * halves it draws. Identical to iOS's `BridgeProfile` — the two bridges must
 * not drift, so the shapes, the names, and the hex convention are the same on
 * both.
 *
 * The id crosses as LOWERCASE HEX, not bytes: JS has no byte array over the
 * `@JavascriptInterface` boundary, and hex is already how app ids cross it. It
 * is the FFI `WhoAmI.id` (raw 32 bytes) hex-encoded, nothing more.
 *
 * `displayName` arrives from core ALREADY SANITIZED — no separator, no bidi or
 * control characters — which is what makes it safe for the page to flatten the
 * pair into `"{displayName} · {tag}"`. Nothing here re-sanitizes it; core is the
 * single enforcement point.
 */
data class BridgeProfile(val idHex: String, val displayName: String, val tag: String)

/**
 * The display-name surface behind `riot.whoami()` / `riot.profile(id)`. A plain
 * interface for the same reason `AppDataPort` is one: JVM tests drive the bridge
 * without Android or the FFI.
 */
interface ProfilePort {
    /**
     * Who the current person is. An app stores `idHex` and NEVER the name: a
     * name is a claim that can change, and a stored name is a snapshot no later
     * rename can ever repair.
     */
    fun whoami(): BridgeProfile

    /**
     * Resolves a stored id back to something drawable, at render time.
     *
     * An id this device has never seen a profile for is NOT a failure — core
     * resolves it to the `member` fallback, so a row authored by a peer whose
     * profile has not synced yet still draws. `null` means the id itself was
     * malformed (not hex), which is a caller bug.
     */
    fun profileFor(idHex: String): BridgeProfile?
}

/**
 * Thin adapter over the profile FFI. Names are resolved on EVERY call rather
 * than cached: a cached name would go stale the moment someone renames or a
 * peer's profile card finally syncs in, which is exactly the staleness that
 * storing the id exists to eliminate.
 */
class UniffiProfilePort(private val session: ProfileSession) : ProfilePort {
    override fun whoami(): BridgeProfile =
        runCatching { session.whoami().toBridgeProfile() }
            .getOrElse { BridgeProfile(idHex = "", displayName = "member", tag = "") }

    override fun profileFor(idHex: String): BridgeProfile? {
        val id = hexToBytes(idHex) ?: return null
        // An unknown id is not an error — Rust returns the `member` fallback.
        // Only a wrong-length id throws, and that is the caller bug both this
        // null and the one above report.
        return runCatching { session.profileFor(id).toBridgeProfile() }.getOrNull()
    }
}

private fun WhoAmI.toBridgeProfile() =
    BridgeProfile(idHex = id.toHexString(), displayName = displayName, tag = tag)

/** `Byte` is signed, so mask before formatting or -1 would not render as "ff". */
private fun ByteArray.toHexString(): String =
    joinToString("") { "%02x".format(it.toInt() and 0xff) }

/**
 * Strict hex decode: ASCII hex digits only, even length. The 32-byte length rule
 * for a subspace id stays in Rust, its one enforcement point.
 *
 * File-private on purpose: the test sources already carry their own top-level
 * `hexToBytes`, and a second one in this package would collide.
 */
private fun hexToBytes(hex: String): ByteArray? {
    if (hex.isEmpty() || hex.length % 2 != 0) return null
    val out = ByteArray(hex.length / 2)
    for (i in out.indices) {
        val high = nibble(hex[i * 2]) ?: return null
        val low = nibble(hex[i * 2 + 1]) ?: return null
        out[i] = ((high shl 4) or low).toByte()
    }
    return out
}

private fun nibble(c: Char): Int? = when (c) {
    in '0'..'9' -> c - '0'
    in 'a'..'f' -> c - 'a' + 10
    in 'A'..'F' -> c - 'A' + 10
    else -> null
}
