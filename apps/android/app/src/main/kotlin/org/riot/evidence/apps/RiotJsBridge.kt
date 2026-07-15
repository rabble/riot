package org.riot.evidence.apps

import android.webkit.JavascriptInterface

/**
 * The `@JavascriptInterface` object behind `window.riot`. Methods are
 * synchronous and run on WebView's dedicated bridge thread; `AppDataPort`
 * (backed by `AppRuntimeSession`) is safe there since Rust serializes over
 * the shared profile mutex. Nothing here touches `PersistedProfile` —
 * persistence is gated, and thread ownership must be re-checked when it
 * lands.
 *
 * Envelopes are hand-rolled JSON: `org.json` is a stub on the JVM test
 * classpath, so the wire format is spelled out here and covered by JVM
 * tests directly.
 */
class RiotJsBridge(private val port: AppDataPort, private val profiles: ProfilePort) {
    companion object {
        /** Total message budget; individual values are further capped in Rust. */
        const val MAX_MESSAGE_BYTES = 262_144
        private const val SAVE_ERROR = "Couldn't save that — try again"
        private const val LOAD_ERROR = "Couldn't load that"

        /**
         * Fixed §4.7 copy shown once when a running app's access is invalidated
         * (trust revoked, namespace swapped, approval changed). Identical intent
         * to iOS's `AppBridgeController.revokedMessage`.
         */
        const val REVOKED_MESSAGE = "Your access to this tool was turned off. Return to Tools."
    }

    /**
     * Called on the WebView's JS thread when a read/commit fails BECAUSE the
     * execution session was invalidated (§4.7). The host posts to the UI thread
     * and closes the app to "Return to Tools" — it never keeps trying against a
     * dead session. Distinct from an ordinary per-op failure (a malformed key),
     * which leaves the session valid and stays inline.
     */
    var onInvalidated: (() -> Unit)? = null

    /** Tear the underlying execution session down. After this every op fails. */
    fun teardown() = port.destroy()

    /**
     * Route a caught bridge failure: if the session is no longer valid this is a
     * §4.7 invalidation — fire [onInvalidated] and return the fixed revoked copy;
     * otherwise return the caller's inline message and leave the app open.
     */
    private fun errorOrInvalidate(fallback: String): String =
        if (!port.isValid()) {
            onInvalidated?.invoke()
            error(REVOKED_MESSAGE)
        } else {
            error(fallback)
        }

    @JavascriptInterface
    fun riotPut(key: String?, valueJson: String?): String {
        if (key.isNullOrEmpty() || valueJson == null) return error(SAVE_ERROR)
        if (key.toByteArray().size + valueJson.toByteArray().size > MAX_MESSAGE_BYTES) {
            return error(SAVE_ERROR)
        }
        return runCatching { port.put(key, valueJson.toByteArray()) }
            .fold({ """{"ok":true,"value":null}""" }, { errorOrInvalidate(SAVE_ERROR) })
    }

    @JavascriptInterface
    fun riotGet(key: String?): String {
        if (key.isNullOrEmpty() || key.toByteArray().size > MAX_MESSAGE_BYTES) return error(LOAD_ERROR)
        return runCatching { port.get(key) }.fold(
            { value ->
                if (value == null) """{"ok":true,"value":null}"""
                else """{"ok":true,"value":${jsonQuote(value.decodeToString())}}"""
            },
            { errorOrInvalidate(LOAD_ERROR) },
        )
    }

    @JavascriptInterface
    fun riotList(prefix: String?): String {
        if (prefix == null || prefix.toByteArray().size > MAX_MESSAGE_BYTES) return error(LOAD_ERROR)
        return runCatching { port.list(prefix) }.fold(
            { rows ->
                val encoded = rows.joinToString(",") { (key, value) ->
                    """{"key":${jsonQuote(key)},"value":${jsonQuote(value.decodeToString())}}"""
                }
                """{"ok":true,"value":[$encoded]}"""
            },
            { errorOrInvalidate(LOAD_ERROR) },
        )
    }

    /**
     * `{ id, displayName, tag }`. The id is what the app STORES; displayName and
     * tag are only what it draws right now, and it must re-resolve them through
     * [riotProfile] on every render — otherwise a rename can never repair the
     * rows already written.
     */
    @JavascriptInterface
    fun riotWhoami(): String {
        val me = profiles.whoami()
        return """{"ok":true,"value":{"id":${jsonQuote(me.idHex)},""" +
            """"displayName":${jsonQuote(me.displayName)},"tag":${jsonQuote(me.tag)}}}"""
    }

    /**
     * Resolves a stored id to `{ displayName, tag }` — the two halves the page
     * flattens into `"{displayName} · {tag}"`. Core has already guaranteed the
     * name cannot contain the separator, so that flattening cannot forge a
     * second tag; nothing is re-sanitized here.
     *
     * An id with no profile yet is NOT an error (it resolves to the `member`
     * fallback). Only a malformed id fails.
     */
    @JavascriptInterface
    fun riotProfile(idHex: String?): String {
        if (idHex == null || idHex.toByteArray().size > MAX_MESSAGE_BYTES) return error(LOAD_ERROR)
        val who = profiles.profileFor(idHex) ?: return error(LOAD_ERROR)
        return """{"ok":true,"value":{"displayName":${jsonQuote(who.displayName)},""" +
            """"tag":${jsonQuote(who.tag)}}}"""
    }

    private fun error(message: String) = """{"ok":false,"error":${jsonQuote(message)}}"""

    private fun jsonQuote(value: String): String = buildString {
        append('"')
        value.forEach { c ->
            when {
                c == '"' -> append("\\\"")
                c == '\\' -> append("\\\\")
                c == '\n' -> append("\\n")
                c == '\r' -> append("\\r")
                c == '\t' -> append("\\t")
                c < ' ' -> append("\\u%04x".format(c.code))
                else -> append(c)
            }
        }
        append('"')
    }
}
