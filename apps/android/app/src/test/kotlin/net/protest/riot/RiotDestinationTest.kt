package net.protest.riot

import net.protest.riot.design.RiotDestination
import net.protest.riot.design.pushedFrom
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pins the shell's shape and, more importantly, that the reshape did not quietly
 * delete a capability.
 *
 * Collapsing eight tabs into four is exactly the kind of change where a surface
 * stops being reachable and nobody notices for a month, because the code still
 * compiles and the screen still exists. These tests make "every surface has a
 * home" a property the build checks.
 */
class RiotDestinationTest {
    /** Four places, same labels and order as the iOS shell. */
    @Test
    fun `the shell has the four iOS destinations in order`() {
        assertEquals(
            listOf("Home", "Tools", "People", "Nearby"),
            RiotDestination.tabs.map { it.label },
        )
    }

    /**
     * THE ANTI-REGRESSION: every one of the original eight surfaces is either a
     * place of its own or is pushed from a named tab. A surface that is neither
     * is unreachable — that is what this test exists to catch.
     */
    @Test
    fun `every surface is either a place or reachable from one`() {
        val placedInATab =
            setOf(
                ConferenceSurface.NEWSWIRE,
                ConferenceSurface.INCIDENT_BOARD,
                ConferenceSurface.SPACES,
            )
        for (surface in ConferenceSurface.entries) {
            if (surface in placedInATab) {
                assertEquals(
                    "$surface is drawn inside a tab, so it must not also be a push",
                    null,
                    surface.pushedFrom,
                )
            } else {
                assertNotNull(
                    "$surface would be unreachable: no tab pushes it",
                    surface.pushedFrom,
                )
            }
        }
    }

    /** The pushes are spread across the shell rather than piled onto one tab. */
    @Test
    fun `pushed surfaces land on the tab they belong to`() {
        assertEquals(RiotDestination.TOOLS, ConferenceSurface.APP_DIRECTORY.pushedFrom)
        assertEquals(RiotDestination.TOOLS, ConferenceSurface.FOLLOW_SITES.pushedFrom)
        assertEquals(RiotDestination.HOME, ConferenceSurface.COMPOSE_AND_SIGN.pushedFrom)
        assertEquals(RiotDestination.NEARBY, ConferenceSurface.IMPORT_PREVIEW.pushedFrom)
        assertEquals(RiotDestination.NEARBY, ConferenceSurface.CONNECTION.pushedFrom)
    }

    /** Four labels are short enough that a phone tab bar can draw them in full. */
    @Test
    fun `tab labels stay short enough to render`() {
        assertTrue(RiotDestination.tabs.all { it.label.length <= 7 })
    }
}
