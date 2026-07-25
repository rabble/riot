package net.protest.riot.design

import android.view.View
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.viewinterop.AndroidView

/**
 * The shell every Android screen sits in: masthead, the surface itself, the tab
 * bar, and a status line.
 *
 * MIGRATION SHAPE. The old app was one Activity that mutated a LinearLayout per
 * surface. Rewriting all eight surfaces at once would have been a single
 * unreviewable change with real risk of dropping a capability, so this shell
 * hosts BOTH: surfaces already ported render as composables, and the rest render
 * through [AndroidView] into the same view tree they always used. The chrome —
 * which is most of what makes the app look like Riot — is Compose from this
 * commit, and the remaining surfaces move one at a time behind it.
 */
@Composable
fun RiotAppShell(
    title: String,
    subtitle: String,
    tabs: List<String>,
    selectedTab: Int,
    onSelectTab: (Int) -> Unit,
    status: String,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    val spacing = RiotTheme.spacing
    Column(
        modifier = modifier.fillMaxSize().background(RiotTheme.colors.paper).systemBarsPadding(),
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(
                start = spacing.comfortable,
                end = spacing.comfortable,
                top = spacing.comfortable,
                bottom = spacing.snug,
            ),
            verticalArrangement = Arrangement.spacedBy(spacing.tight),
        ) {
            RiotEyebrow(title)
            RiotText(subtitle, RiotType.serif(28))
        }
        Box(modifier = Modifier.weight(1f)) { content() }
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = spacing.comfortable, vertical = spacing.snug),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            RiotText(status, RiotType.mono(11), color = RiotTheme.colors.inkSoft)
        }
        RiotTabBar(tabs = tabs, selected = selectedTab, onSelect = onSelectTab)
    }
}

/**
 * A surface that has not been ported yet, drawn by the original view code.
 *
 * The legacy container is built once by the Activity and handed here; this only
 * parents it. It sits on the themed paper ground so a half-migrated app still
 * reads as one app rather than two.
 */
@Composable
fun LegacySurface(view: View, modifier: Modifier = Modifier) {
    AndroidView(
        factory = { view },
        modifier = modifier
            .fillMaxSize()
            .padding(horizontal = RiotTheme.spacing.comfortable),
    )
}

/** A scrolling column of cards — the shape of nearly every Riot surface. */
@Composable
fun RiotSurfaceColumn(
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    Column(
        modifier = modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = RiotTheme.spacing.comfortable),
        verticalArrangement = Arrangement.spacedBy(RiotTheme.spacing.comfortable),
    ) {
        content()
    }
}

/** A themed single-line field. Foundation ships only BasicTextField, so this dresses it. */
@Composable
fun RiotTextField(
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    placeholder: String,
    modifier: Modifier = Modifier,
) {
    val theme = RiotTheme.colors
    val spacing = RiotTheme.spacing
    Box(
        modifier = modifier
            .fillMaxWidth()
            .background(theme.paper2)
            .padding(horizontal = spacing.cozy, vertical = spacing.cozy),
    ) {
        if (value.text.isEmpty()) {
            RiotText(placeholder, RiotType.body(15), color = theme.inkSoft)
        }
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            textStyle = RiotType.body(15).copy(color = theme.ink),
            modifier = Modifier.fillMaxWidth(),
        )
    }
}
