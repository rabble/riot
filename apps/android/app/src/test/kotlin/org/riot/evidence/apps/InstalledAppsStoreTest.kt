package org.riot.evidence.apps

import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.riot_ffi.InstalledAppRecord

class InstalledAppsStoreTest {
    private fun record(id: String) = InstalledAppRecord(
        appId = id, name = "Checklist", description = "d", version = "1.0.0",
        entryPoint = "index.html", permissions = listOf("Keep its own notes in this space"),
    )
    private fun bundle() = DecodedAppBundle(
        "index.html", listOf(AppResource("index.html", "text/html", ByteArray(1))),
    )

    @Test
    fun registeringTwiceKeepsOneRowPerAppId() {
        val store = InstalledAppsStore()
        store.register(record("aa".repeat(32)), bundle())
        store.register(record("aa".repeat(32)), bundle())
        assertEquals(1, store.all().size)
    }

    @Test
    fun findsByAppId() {
        val store = InstalledAppsStore()
        store.register(record("aa".repeat(32)), bundle())
        assertEquals("Checklist", store.find("aa".repeat(32))!!.record.name)
        assertEquals(null, store.find("bb".repeat(32)))
    }
}
