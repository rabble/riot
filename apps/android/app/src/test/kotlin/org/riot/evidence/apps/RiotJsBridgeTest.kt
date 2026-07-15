package org.riot.evidence.apps

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

private class FakeAppDataPort : AppDataPort {
    val stored = mutableMapOf<String, ByteArray>()
    var failNext: Boolean = false
    // Unit 0C: drive the §4.7 branch. `valid` is what the port reports for
    // `isValid()`; `failReads` makes get/list throw like a revoked read would.
    var valid: Boolean = true
    var failReads: Boolean = false
    var destroyed: Boolean = false
    override fun put(key: String, value: ByteArray) {
        if (failNext) throw IllegalStateException("boom")
        stored[key] = value
    }
    override fun get(key: String): ByteArray? {
        if (failReads) throw IllegalStateException("boom")
        return stored[key]
    }
    override fun list(prefix: String): List<Pair<String, ByteArray>> {
        if (failReads) throw IllegalStateException("boom")
        return stored.filterKeys { it.startsWith(prefix) }.map { it.key to it.value }.sortedBy { it.first }
    }
    override fun destroy() { destroyed = true }
    override fun isValid(): Boolean = valid
}

/**
 * Stands in for the profile FFI. `profileFor` mirrors the real contract: an id
 * it has never seen is NOT a failure — it resolves to the `member` fallback, so
 * a row by a peer whose profile hasn't synced yet still draws. Only a malformed
 * id is null.
 */
private class FakeProfilePort : ProfilePort {
    val me = BridgeProfile(idHex = "ab".repeat(32), displayName = "member", tag = "abababab")
    val known = mutableMapOf<String, String>()

    override fun whoami(): BridgeProfile = me

    override fun profileFor(idHex: String): BridgeProfile? {
        if (idHex.length != 64 || !idHex.all { it in "0123456789abcdef" }) return null
        return BridgeProfile(idHex, known[idHex] ?: "member", idHex.take(8))
    }
}

class RiotJsBridgeTest {
    private val port = FakeAppDataPort()
    private val profiles = FakeProfilePort()
    private val bridge = RiotJsBridge(port, profiles)

    @Test
    fun putStoresJsonTextAndReturnsOkEnvelope() {
        assertEquals("""{"ok":true,"value":null}""", bridge.riotPut("items/x", """{"text":"water"}"""))
        assertEquals("""{"text":"water"}""", port.stored["items/x"]!!.decodeToString())
    }

    @Test
    fun putRejectsOversizedValuesBeforeTouchingThePort() {
        val result = bridge.riotPut("items/x", "a".repeat(RiotJsBridge.MAX_MESSAGE_BYTES + 1))
        assertEquals("""{"ok":false,"error":"Couldn't save that — try again"}""", result)
        assertTrue(port.stored.isEmpty())
    }

    @Test
    fun putRejectsMissingOrEmptyKeysBeforeTouchingThePort() {
        assertTrue(bridge.riotPut(null, "{}").contains("\"ok\":false"))
        assertTrue(bridge.riotPut("", "{}").contains("\"ok\":false"))
        assertTrue(bridge.riotPut("items/x", null).contains("\"ok\":false"))
        assertTrue(port.stored.isEmpty())
    }

    @Test
    fun portFailuresBecomePlainLanguageErrorEnvelopes() {
        port.failNext = true
        assertEquals(
            """{"ok":false,"error":"Couldn't save that — try again"}""",
            bridge.riotPut("items/x", "{}"),
        )
    }

    @Test
    fun getReturnsNullEnvelopeWhenAbsentAndEscapedTextWhenPresent() {
        assertEquals("""{"ok":true,"value":null}""", bridge.riotGet("items/none"))
        // NOT a raw string: \" here is a real quote character, so the value
        // genuinely contains quotes for jsonQuote to escape. (The plan's raw
        // string stored literal backslashes instead — a Kotlin gotcha.)
        port.stored["items/q"] = "{\"text\":\"say \"hi\"\"}".toByteArray()
        val envelope = bridge.riotGet("items/q")
        assertTrue(envelope.startsWith("""{"ok":true,"value":""""))
        assertTrue(envelope.contains("\\\"hi\\\""))
    }

    @Test
    fun listReturnsKeyValueRowsAsEscapedJson() {
        port.stored["items/a"] = """{"done":false}""".toByteArray()
        port.stored["items/b"] = """{"done":true}""".toByteArray()
        assertEquals(
            """{"ok":true,"value":[{"key":"items/a","value":"{\"done\":false}"},{"key":"items/b","value":"{\"done\":true}"}]}""",
            bridge.riotList("items"),
        )
    }

    /**
     * The id is the field that matters: it is what an app STORES. The name and
     * tag beside it are only what to draw right now, and the app must re-resolve
     * them through `riotProfile` at render time.
     *
     * Byte-identical to the iOS envelope — the two bridges must not drift.
     */
    @Test
    fun whoamiCarriesTheStableIdAlongsideTheTwoHalvesToDrawIt() {
        assertEquals(
            """{"ok":true,"value":{"id":"${"ab".repeat(32)}","displayName":"member","tag":"abababab"}}""",
            bridge.riotWhoami(),
        )
    }

    @Test
    fun profileResolvesAStoredIdToTheCurrentName() {
        val id = "cd".repeat(32)
        profiles.known[id] = "Ana"
        assertEquals(
            """{"ok":true,"value":{"displayName":"Ana","tag":"cdcdcdcd"}}""",
            bridge.riotProfile(id),
        )
    }

    /** An author whose profile hasn't synced here yet still has to draw. */
    @Test
    fun profileFallsBackToMemberForAnIdItHasNeverSeen() {
        assertEquals(
            """{"ok":true,"value":{"displayName":"member","tag":"efefefef"}}""",
            bridge.riotProfile("ef".repeat(32)),
        )
    }

    @Test
    fun profileRejectsMalformedIds() {
        assertEquals("""{"ok":false,"error":"Couldn't load that"}""", bridge.riotProfile("not-hex"))
        assertEquals("""{"ok":false,"error":"Couldn't load that"}""", bridge.riotProfile("abcd"))
        assertEquals("""{"ok":false,"error":"Couldn't load that"}""", bridge.riotProfile(null))
    }

    // Unit 0C — §4.7: an invalidated-session failure routes to Return to Tools;
    // an ordinary per-op failure stays inline.

    @Test
    fun anInvalidatedSessionFailureRoutesToReturnToTools() {
        var invalidated = false
        bridge.onInvalidated = { invalidated = true }
        port.failReads = true
        port.valid = false // the session was revoked out from under the app

        val result = bridge.riotGet("items/x")
        assertEquals("""{"ok":false,"error":"${RiotJsBridge.REVOKED_MESSAGE}"}""", result)
        assertTrue("an invalidated session must route to Return to Tools", invalidated)
    }

    @Test
    fun aPerOpFailureOnAValidSessionStaysInlineAndDoesNotRoute() {
        var invalidated = false
        bridge.onInvalidated = { invalidated = true }
        port.failNext = true
        port.valid = true // the session is fine; this write just failed

        val result = bridge.riotPut("items/x", "{}")
        assertEquals("""{"ok":false,"error":"Couldn't save that — try again"}""", result)
        assertTrue("a per-op failure must not route to Return to Tools", !invalidated)
    }

    @Test
    fun teardownDestroysTheUnderlyingSession() {
        assertTrue(!port.destroyed)
        bridge.teardown()
        assertTrue("teardown must destroy the execution session", port.destroyed)
    }
}
