package net.protest.riot

import androidx.compose.ui.graphics.Color
import net.protest.riot.design.RiotDarkColors
import net.protest.riot.design.RiotLightColors
import net.protest.riot.design.riotAvatarColor
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

/**
 * Pins the Android design tokens to the iOS ones.
 *
 * WHY THIS TEST EXISTS: the two apps are supposed to BE the same app. Nothing in
 * the compiler stops someone nudging a colour on one platform and not the other,
 * and the drift is invisible until you put two screenshots side by side. These
 * hexes are copied from apps/ios/Riot/Design/RiotTheme.swift; changing one
 * without changing the other fails here.
 */
class RiotThemeParityTest {
    private fun hex(value: Long) = Color(0xFF000000 or value)

    @Test
    fun `light colours match the iOS light scheme`() {
        assertEquals(hex(0xEAE6DA), RiotLightColors.paper)
        assertEquals(hex(0xE1DCCB), RiotLightColors.paper2)
        assertEquals(hex(0x17160F), RiotLightColors.ink)
        assertEquals(hex(0x4A473B), RiotLightColors.inkSoft)
        assertEquals(hex(0x22399F), RiotLightColors.blue)
        assertEquals(hex(0xD1216E), RiotLightColors.pink)
        assertEquals(hex(0xFCFAF4), RiotLightColors.card)
        assertEquals(hex(0x1E6B4F), RiotLightColors.accent)
        assertEquals(hex(0xF6F2E9), RiotLightColors.onAccent)
    }

    @Test
    fun `dark colours match the iOS dark scheme`() {
        assertEquals(hex(0x131209), RiotDarkColors.paper)
        assertEquals(hex(0x1C1A10), RiotDarkColors.paper2)
        assertEquals(hex(0xEFE9D8), RiotDarkColors.ink)
        assertEquals(hex(0xBEB69E), RiotDarkColors.inkSoft)
        assertEquals(hex(0x6D84FF), RiotDarkColors.blue)
        assertEquals(hex(0xFF5F9E), RiotDarkColors.pink)
        assertEquals(hex(0x201E16), RiotDarkColors.card)
        assertEquals(hex(0x34A06E), RiotDarkColors.accent)
        assertEquals(hex(0xF6F2E9), RiotDarkColors.onAccent)
    }

    /** The hairline and its stronger sibling are derived, so pin the alphas too. */
    @Test
    fun `line alphas match iOS`() {
        assertEquals(0.18f, RiotLightColors.line.alpha, 0.001f)
        assertEquals(0.40f, RiotLightColors.lineStrong.alpha, 0.001f)
        assertEquals(0.16f, RiotDarkColors.line.alpha, 0.001f)
        assertEquals(0.36f, RiotDarkColors.lineStrong.alpha, 0.001f)
    }

    /**
     * The avatar disc must be the SAME colour for the same key on both
     * platforms, or one person is two different discs depending on the phone
     * they are looked at from. iOS sums unicode scalars and indexes a 5-colour
     * palette; this reproduces that exactly — notably NOT `String.hashCode()`.
     */
    @Test
    fun `avatar colour is the iOS summed-scalar hash`() {
        val palette = listOf(0xC8791FL, 0x1E6B4FL, 0x2B6CB0L, 0x7A4F9EL, 0xB0522EL)
        for (key in listOf("aa11", "ana", "", "eeeea01163a151165e91d05d5528c932", "Ω")) {
            val expected = hex(palette[key.sumOf { it.code } % palette.size])
            assertEquals("key=$key", expected, riotAvatarColor(key))
        }
    }

    /** Deterministic across calls — the disc cannot flicker between launches. */
    @Test
    fun `avatar colour is stable and distinguishes different keys`() {
        assertEquals(riotAvatarColor("ana"), riotAvatarColor("ana"))
        assertNotEquals(riotAvatarColor("a"), riotAvatarColor("b"))
    }
}
