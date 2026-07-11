package org.riot.evidence

import org.junit.Assert.assertEquals
import org.junit.Test

class ConferenceSurfaceTest {
    @Test
    fun shellHasExactlyTheConferenceSurfaces() {
        assertEquals(
            listOf(
                "Spaces",
                "App directory",
                "Incident board",
                "Compose & sign",
                "Import preview",
                "Connection",
            ),
            ConferenceSurface.entries.map { it.label },
        )
    }
}
