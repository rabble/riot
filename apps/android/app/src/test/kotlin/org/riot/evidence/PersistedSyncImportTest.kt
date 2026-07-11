package org.riot.evidence

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.riot_ffi.AlertFreshness
import uniffi.riot_ffi.CurrentEntry

class PersistedSyncImportTest {
    @Test
    fun mergeKeepsTheExactBundleAndDeduplicatesExistingEntryIds() {
        val bundle = byteArrayOf(4, 5, 6)
        val existing = entry("existing").toPersisted(byteArrayOf(1))
        val profile = PersistedProfile(PersistedSpace("space", "Test"), listOf(existing))

        val merged = mergeAcceptedSync(
            profile,
            bundle,
            listOf(entry("existing"), entry("new")),
        )

        assertEquals(listOf("existing", "new"), merged.alerts.map { it.entryId })
        assertArrayEquals(bundle, merged.alerts.last().bundleBytes)
    }
}

private fun entry(id: String) = CurrentEntry(
    entryId = id,
    namespaceId = "space",
    signerId = "signer",
    headline = "Headline $id",
    freshness = AlertFreshness(1u, null, 2u),
    aiAssisted = false,
)
