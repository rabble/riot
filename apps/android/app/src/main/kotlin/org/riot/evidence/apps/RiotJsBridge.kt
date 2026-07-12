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
    }

    @JavascriptInterface
    fun riotPut(key: String?, valueJson: String?): String {
        if (key.isNullOrEmpty() || valueJson == null) return error(SAVE_ERROR)
        if (key.toByteArray().size + valueJson.toByteArray().size > MAX_MESSAGE_BYTES) {
            return error(SAVE_ERROR)
        }
        return runCatching { port.put(key, valueJson.toByteArray()) }
            .fold({ """{"ok":true,"value":null}""" }, { error(SAVE_ERROR) })
    }

    @JavascriptInterface
    fun riotGet(key: String?): String {
        if (key.isNullOrEmpty() || key.toByteArray().size > MAX_MESSAGE_BYTES) return error(LOAD_ERROR)
        return runCatching { port.get(key) }.fold(
            { value ->
                if (value == null) """{"ok":true,"value":null}"""
                else """{"ok":true,"value":${jsonQuote(value.decodeToString())}}"""
            },
            { error(LOAD_ERROR) },
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
            { error(LOAD_ERROR) },
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
