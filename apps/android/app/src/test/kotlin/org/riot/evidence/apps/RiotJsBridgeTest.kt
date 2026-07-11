package org.riot.evidence.apps

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

private class FakeAppDataPort : AppDataPort {
    val stored = mutableMapOf<String, ByteArray>()
    var failNext: Boolean = false
    override fun put(key: String, value: ByteArray) {
        if (failNext) throw IllegalStateException("boom")
        stored[key] = value
    }
    override fun get(key: String): ByteArray? = stored[key]
    override fun list(prefix: String): List<Pair<String, ByteArray>> =
        stored.filterKeys { it.startsWith(prefix) }.map { it.key to it.value }.sortedBy { it.first }
}

class RiotJsBridgeTest {
    private val port = FakeAppDataPort()
    private val bridge = RiotJsBridge(port, displayName = "member-ab12cd34")

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

    @Test
    fun whoamiReturnsDisplayNameOnly() {
        assertEquals("""{"ok":true,"value":{"displayName":"member-ab12cd34"}}""", bridge.riotWhoami())
    }
}
