package org.riot.evidence

import org.junit.Assert.assertEquals
import org.junit.Test

class ConferenceSurfaceTest {
    @Test
    fun shellHasExactlyTheFiveConferenceSurfaces() {
        assertEquals(
            listOf("Spaces", "Incident board", "Compose & sign", "Import preview", "Connection"),
            ConferenceSurface.entries.map { it.label },
        )
    }
}
