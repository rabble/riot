package net.protest.riot.design

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import net.protest.riot.R

/**
 * Riot's design system, ported from the iOS `RiotTheme` so both platforms draw
 * the same app rather than two apps that share a protocol.
 *
 * FULLY CUSTOM, NOT MATERIAL. The app depends on compose-foundation, not
 * material3: Riot has its own colour roles, its own type scale, and its own
 * components, so pulling in Material would only mean overriding it everywhere
 * and inheriting its defaults wherever we forgot. This is the "fully custom
 * design system" path Compose explicitly supports, and it mirrors what SwiftUI
 * does on iOS.
 *
 * Tokens travel by CompositionLocal, so a composable reads `RiotTheme.colors`
 * ambiently instead of threading a theme parameter through every call — the same
 * ergonomics `MaterialTheme` has, without Material.
 *
 * EVERY COLOUR HERE IS THE SAME HEX AS iOS. `RiotThemeParityTest` pins them, so
 * a change on one platform that is not made on the other fails a test rather
 * than quietly drifting the two apps apart.
 */
@Immutable
data class RiotColors(
    val paper: Color,
    val paper2: Color,
    val ink: Color,
    val inkSoft: Color,
    val blue: Color,
    val pink: Color,
    val line: Color,
    val lineStrong: Color,
    val card: Color,
    val accent: Color,
    val onAccent: Color,
    val isDark: Boolean,
)

private fun hex(value: Long) = Color(0xFF000000 or value)

val RiotLightColors =
    RiotColors(
        paper = hex(0xEAE6DA),
        paper2 = hex(0xE1DCCB),
        ink = hex(0x17160F),
        inkSoft = hex(0x4A473B),
        blue = hex(0x22399F),
        pink = hex(0xD1216E),
        line = hex(0x17160F).copy(alpha = 0.18f),
        lineStrong = hex(0x17160F).copy(alpha = 0.4f),
        card = hex(0xFCFAF4),
        accent = hex(0x1E6B4F),
        onAccent = hex(0xF6F2E9),
        isDark = false,
    )

val RiotDarkColors =
    RiotColors(
        paper = hex(0x131209),
        paper2 = hex(0x1C1A10),
        ink = hex(0xEFE9D8),
        inkSoft = hex(0xBEB69E),
        blue = hex(0x6D84FF),
        pink = hex(0xFF5F9E),
        line = hex(0xEFE9D8).copy(alpha = 0.16f),
        lineStrong = hex(0xEFE9D8).copy(alpha = 0.36f),
        card = hex(0x201E16),
        accent = hex(0x34A06E),
        onAccent = hex(0xF6F2E9),
        isDark = true,
    )

/**
 * A stable, key-derived disc colour for a person's initials avatar.
 *
 * Not decoration: two people who both call themselves "Ana" get different discs
 * because the colour is a pure function of their key, so the eye can tell them
 * apart the same way the tag does. The hash is a summed scalar — NEVER
 * `String.hashCode()`, which is stable on the JVM but is not the algorithm iOS
 * uses; both platforms must land on the same disc for the same key.
 */
fun riotAvatarColor(key: String): Color {
    val palette = listOf(0xC8791FL, 0x1E6B4FL, 0x2B6CB0L, 0x7A4F9EL, 0xB0522EL)
    var sum = 0
    for (character in key) sum += character.code
    return hex(palette[Math.floorMod(sum, palette.size)])
}

/** The four faces, matching the iOS `RiotFontRole` set. */
@Immutable
data class RiotTypography(
    val poster: FontFamily,
    val body: FontFamily,
    val mono: FontFamily,
    val serif: FontFamily,
)

private val AntonFamily = FontFamily(Font(R.font.anton_regular))
private val WorkSansFamily = FontFamily(Font(R.font.work_sans_variable))
private val SpaceMonoFamily =
    FontFamily(
        Font(R.font.space_mono_regular, FontWeight.Normal),
        Font(R.font.space_mono_bold, FontWeight.Bold),
    )

val RiotFonts =
    RiotTypography(
        poster = AntonFamily,
        body = WorkSansFamily,
        mono = SpaceMonoFamily,
        // The editorial serif is used only for headings read AS writing — a
        // report headline, a community's name — never for chrome or data. iOS
        // asks for "Iowan Old Style"; Android has no equivalent bundled, so this
        // resolves to the platform serif rather than shipping a fifth font file.
        serif = FontFamily.Serif,
    )

/**
 * Spacing scale. Named rather than numeric so a screen says what it means and a
 * later change lands everywhere at once.
 */
@Immutable
data class RiotSpacing(
    val hairline: Dp = 1.dp,
    val tight: Dp = 4.dp,
    val snug: Dp = 8.dp,
    val cozy: Dp = 12.dp,
    val comfortable: Dp = 16.dp,
    val roomy: Dp = 20.dp,
    val loose: Dp = 28.dp,
    /**
     * The minimum touch target. 48dp is the Android floor (iOS uses 44pt), and
     * it is a token rather than a literal so no control can quietly ship
     * smaller.
     */
    val touchTarget: Dp = 48.dp,
    val cardCorner: Dp = 14.dp,
    val pillCorner: Dp = 999.dp,
)

private val LocalRiotColors = staticCompositionLocalOf { RiotLightColors }
private val LocalRiotTypography = staticCompositionLocalOf { RiotFonts }
private val LocalRiotSpacing = staticCompositionLocalOf { RiotSpacing() }

object RiotTheme {
    val colors: RiotColors
        @Composable @ReadOnlyComposable get() = LocalRiotColors.current

    val fonts: RiotTypography
        @Composable @ReadOnlyComposable get() = LocalRiotTypography.current

    val spacing: RiotSpacing
        @Composable @ReadOnlyComposable get() = LocalRiotSpacing.current
}

@Composable
fun RiotTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    CompositionLocalProvider(
        LocalRiotColors provides if (darkTheme) RiotDarkColors else RiotLightColors,
        LocalRiotTypography provides RiotFonts,
        LocalRiotSpacing provides RiotSpacing(),
        content = content,
    )
}

/**
 * The type scale, as functions rather than a fixed set of named styles, because
 * the iOS side sizes per call site (`.riot(.mono, size: 11)`). Keeping the same
 * shape makes the two ports readable side by side.
 */
object RiotType {
    @Composable
    fun poster(size: Int) =
        TextStyle(fontFamily = RiotTheme.fonts.poster, fontSize = size.sp)

    @Composable
    fun body(size: Int) = TextStyle(fontFamily = RiotTheme.fonts.body, fontSize = size.sp)

    @Composable
    fun serif(size: Int) = TextStyle(fontFamily = RiotTheme.fonts.serif, fontSize = size.sp)

    /** Chrome, tags, counts. `tracking` matches the iOS 0.5pt letter spacing. */
    @Composable
    fun mono(size: Int, bold: Boolean = false) =
        TextStyle(
            fontFamily = RiotTheme.fonts.mono,
            fontSize = size.sp,
            fontWeight = if (bold) FontWeight.Bold else FontWeight.Normal,
            letterSpacing = 0.5.sp,
        )
}
