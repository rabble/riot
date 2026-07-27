package net.protest.riot.design

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

/**
 * Every component in the design system on one screen, in both schemes.
 *
 * This is the reference the Android port is checked against — open the Compose
 * preview beside a screenshot of the iOS app and the two should read as the same
 * product. It is also what makes the components reviewable BEFORE any screen is
 * migrated onto them, so a mistake in the system is caught once here rather than
 * in every screen that copied it.
 */
@Composable
fun RiotShowcase(modifier: Modifier = Modifier) {
    var tab by remember { mutableIntStateOf(0) }
    val spacing = RiotTheme.spacing
    Column(
        modifier = modifier.fillMaxSize().background(RiotTheme.colors.paper),
    ) {
        Column(
            modifier =
                Modifier
                    .weight(1f)
                    .verticalScroll(rememberScrollState())
                    .padding(spacing.comfortable),
            verticalArrangement = Arrangement.spacedBy(spacing.comfortable),
        ) {
            RiotHeader(eyebrow = "Community", title = "What's happening")

            RiotCard {
                RiotEyebrow("Open wire")
                RiotText("Free breakfast at the corner church", RiotType.serif(22))
                RiotText(
                    "Full report inside.",
                    RiotType.body(15),
                    color = RiotTheme.colors.inkSoft,
                )
                Row(
                    horizontalArrangement = Arrangement.spacedBy(spacing.snug),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    RiotAvatar(initials = "EB", keyHex = "eba7a869")
                    Column {
                        RiotText("Member", RiotType.body(15))
                        RiotText(
                            "eba7a869",
                            RiotType.mono(11),
                            color = RiotTheme.colors.inkSoft,
                        )
                    }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(spacing.snug)) {
                    RiotBadge("🤝")
                    RiotBadge("✊ 1")
                    RiotBadge("❗️ 1")
                    RiotBadge("🕯️")
                }
            }

            RiotCard {
                RiotEyebrow("Actions")
                RiotPrimaryButton("Post an update", onClick = {})
                RiotSecondaryButton("Share this community", onClick = {})
                RiotSecondaryButton("Emergency wipe", onClick = {}, destructive = true)
            }

            RiotEmptyState(
                title = "Nothing featured yet",
                body = "The collective has not selected a feature. See the open wire for every report.",
            )
        }
        RiotTabBar(
            tabs = listOf("Home", "Tools", "People", "Nearby"),
            selected = tab,
            onSelect = { tab = it },
        )
    }
}

@Preview(name = "Riot design system — light", showBackground = true, heightDp = 900)
@Composable
private fun RiotShowcaseLightPreview() {
    RiotTheme(darkTheme = false) { RiotShowcase() }
}

@Preview(name = "Riot design system — dark", showBackground = true, heightDp = 900)
@Composable
private fun RiotShowcaseDarkPreview() {
    RiotTheme(darkTheme = true) { RiotShowcase() }
}
