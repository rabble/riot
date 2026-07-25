package net.protest.riot.design

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp

/**
 * The Riot component set, ported one-for-one from `apps/ios/Riot/Design`.
 *
 * Each of these exists on iOS as a SwiftUI view or style; keeping the names and
 * the visual contract identical is what stops the two apps drifting into
 * different products. Anything a screen needs that is not here should be added
 * here first, not inlined into the screen.
 */

/** Text, always themed. Foundation's `BasicText` takes no ambient colour, so this wrapper supplies it. */
@Composable
fun RiotText(
    text: String,
    style: TextStyle,
    modifier: Modifier = Modifier,
    color: Color? = null,
    textAlign: TextAlign? = null,
    maxLines: Int = Int.MAX_VALUE,
    softWrap: Boolean = true,
) {
    BasicText(
        text = text,
        modifier = modifier,
        style = style.copy(color = color ?: RiotTheme.colors.ink, textAlign = textAlign ?: TextAlign.Unspecified),
        maxLines = maxLines,
        softWrap = softWrap,
    )
}

/**
 * The calm paper card: a near-white sheet on the warm ground, a hairline border,
 * and a rounded corner. The workhorse container — a screen is a stack of these.
 */
@Composable
fun RiotCard(
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    val theme = RiotTheme.colors
    val spacing = RiotTheme.spacing
    Column(
        modifier =
            modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(spacing.cardCorner))
                .background(theme.card)
                .border(
                    BorderStroke(spacing.hairline, theme.line),
                    RoundedCornerShape(spacing.cardCorner),
                )
                .padding(spacing.comfortable),
        verticalArrangement = Arrangement.spacedBy(spacing.snug),
        content = content,
    )
}

/**
 * The section label above a block — uppercase mono, quiet. On iOS this is the
 * `eyebrow`; same word here so the two trees read alike.
 */
@Composable
fun RiotEyebrow(text: String, modifier: Modifier = Modifier) {
    RiotText(
        text = text.uppercase(),
        style = RiotType.mono(12),
        color = RiotTheme.colors.inkSoft,
        modifier = modifier,
    )
}

/** A screen's masthead: eyebrow over a serif title, the thing a person reads as writing. */
@Composable
fun RiotHeader(eyebrow: String, title: String, modifier: Modifier = Modifier) {
    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(RiotTheme.spacing.tight),
    ) {
        RiotEyebrow(eyebrow)
        RiotText(text = title, style = RiotType.serif(30))
    }
}

/** A small mono tag — a count, a state, a role. */
@Composable
fun RiotBadge(text: String, modifier: Modifier = Modifier, tint: Color? = null) {
    val theme = RiotTheme.colors
    val spacing = RiotTheme.spacing
    Box(
        modifier =
            modifier
                .clip(RoundedCornerShape(spacing.pillCorner))
                .background(theme.paper2)
                .padding(horizontal = spacing.snug, vertical = spacing.tight)
    ) {
        RiotText(text = text.uppercase(), style = RiotType.mono(11), color = tint ?: theme.inkSoft)
    }
}

/**
 * The one filled action per card — the grounded civic green. Everything else on
 * a screen stays quiet so this reads as THE thing to do.
 */
@Composable
fun RiotPrimaryButton(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    val theme = RiotTheme.colors
    val spacing = RiotTheme.spacing
    Box(
        modifier =
            modifier
                .fillMaxWidth()
                .defaultMinSize(minHeight = spacing.touchTarget)
                .clip(RoundedCornerShape(spacing.pillCorner))
                .background(if (enabled) theme.accent else theme.paper2)
                .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
                .padding(horizontal = spacing.roomy, vertical = spacing.cozy),
        contentAlignment = Alignment.Center,
    ) {
        RiotText(
            text = label,
            style = RiotType.body(17),
            color = if (enabled) theme.onAccent else theme.inkSoft,
            textAlign = TextAlign.Center,
        )
    }
}

/** The quiet action: outlined, no fill, for everything that is not THE action. */
@Composable
fun RiotSecondaryButton(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    destructive: Boolean = false,
) {
    val theme = RiotTheme.colors
    val spacing = RiotTheme.spacing
    val tint = if (destructive) theme.pink else theme.ink
    Box(
        modifier =
            modifier
                .fillMaxWidth()
                .defaultMinSize(minHeight = spacing.touchTarget)
                .clip(RoundedCornerShape(spacing.pillCorner))
                .border(BorderStroke(spacing.hairline, if (destructive) theme.pink else theme.lineStrong), RoundedCornerShape(spacing.pillCorner))
                .clickable(role = Role.Button, onClick = onClick)
                .padding(horizontal = spacing.roomy, vertical = spacing.cozy),
        contentAlignment = Alignment.Center,
    ) {
        RiotText(text = label, style = RiotType.body(17), color = tint, textAlign = TextAlign.Center)
    }
}

/**
 * What a surface says when it has nothing yet. An empty screen with no words is
 * indistinguishable from a broken one, so every list gets one of these.
 */
@Composable
fun RiotEmptyState(title: String, body: String, modifier: Modifier = Modifier) {
    RiotCard(modifier = modifier) {
        RiotEyebrow(title)
        RiotText(text = body, style = RiotType.body(15), color = RiotTheme.colors.inkSoft)
    }
}

/** A person's initials disc, coloured deterministically from their key. */
@Composable
fun RiotAvatar(initials: String, keyHex: String, modifier: Modifier = Modifier) {
    Box(
        modifier = modifier.size(34.dp).clip(RoundedCornerShape(RiotTheme.spacing.pillCorner)).background(riotAvatarColor(keyHex)),
        contentAlignment = Alignment.Center,
    ) {
        RiotText(
            text = initials.take(2).uppercase(),
            style = RiotType.mono(12, bold = true),
            color = RiotTheme.colors.onAccent,
        )
    }
}

/**
 * The bottom tab bar. Android's convention is bottom navigation, and iOS's shell
 * is also a tab bar, so the two apps agree here without either feeling foreign.
 */
@Composable
fun RiotTabBar(
    tabs: List<String>,
    selected: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    val theme = RiotTheme.colors
    val spacing = RiotTheme.spacing
    // SCROLLING, not squeezed. Eight surfaces do not fit a phone's width: fixed
    // shares truncate every label to nonsense ("APP DIF", "NEWSWIF"), which was
    // visible on a real device. A label that cannot be read is worse than one
    // that needs a swipe, so the row scrolls and each tab keeps its full name.
    // When the information architecture catches up with the iOS shell (four
    // tabs) this can go back to equal shares.
    Row(
        modifier =
            modifier
                .fillMaxWidth()
                .background(theme.paper2)
                .horizontalScroll(rememberScrollState())
                .padding(vertical = spacing.snug, horizontal = spacing.snug),
        horizontalArrangement = Arrangement.spacedBy(spacing.tight),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        tabs.forEachIndexed { index, label ->
            val isSelected = index == selected
            Box(
                modifier =
                    Modifier
                        .defaultMinSize(minHeight = spacing.touchTarget)
                        .clip(RoundedCornerShape(spacing.pillCorner))
                        .clickable(role = Role.Tab) { onSelect(index) }
                        .padding(horizontal = spacing.cozy, vertical = spacing.snug),
                contentAlignment = Alignment.Center,
            ) {
                RiotText(
                    text = label.uppercase(),
                    // ONE LINE, ALWAYS. Without this a long label ("NEWSWIRE")
                    // wraps mid-word and the bar grows a second row — seen on a
                    // real device before this was pinned.
                    style = RiotType.mono(10, bold = isSelected),
                    color = if (isSelected) theme.pink else theme.inkSoft,
                    maxLines = 1,
                    softWrap = false,
                    textAlign = TextAlign.Center,
                )
            }
        }
    }
}
