package net.protest.riot.design

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import net.protest.riot.NewswirePostDisplay
import net.protest.riot.NewswirePostRow
import net.protest.riot.NewswireSurface
import net.protest.riot.NewswireWireState

/**
 * One person on this wire, folded from the posts they signed.
 *
 * Android has no membership list to read — there is no roster record — so the
 * only honest source of "who is here" is who has actually published. That is
 * also the truthful thing to show: a wire's people are the ones who have said
 * something on it, not a directory someone typed.
 */
data class WirePerson(
    val rendered: String,
    val keyHex: String,
    val posts: Int,
)

/**
 * Distinct authors of ordinary posts, most prolific first, then by name so the
 * order is stable between renders.
 *
 * Redacted posts are excluded deliberately: a hidden or tombstoned post is not
 * evidence its author is a participant to be listed, and surfacing them here
 * would route around the editorial treatment.
 */
fun NewswireSurface.people(): List<WirePerson> {
    val posts: List<NewswirePostRow> =
        when (val wire = this.wire) {
            is NewswireWireState.Featured -> wire.openWire
            is NewswireWireState.PostsButNoFeature -> wire.openWire
            else -> emptyList()
        }
    return posts
        .filter { it.display == NewswirePostDisplay.ORDINARY }
        .groupBy { it.authorKeyHex }
        .map { (key, rows) ->
            WirePerson(rendered = rows.first().author, keyHex = key, posts = rows.size)
        }
        .sortedWith(compareByDescending<WirePerson> { it.posts }.thenBy { it.rendered })
}

/** The People tab: who has published on this wire. */
@Composable
fun PeopleScreen(
    people: List<WirePerson>,
    modifier: Modifier = Modifier,
) {
    RiotSurfaceColumn(modifier) {
        Spacer(Modifier.height(RiotTheme.spacing.tight))
        if (people.isEmpty()) {
            RiotEmptyState(
                title = "Nobody yet",
                body = "The people here are the ones who have published on this wire. " +
                    "When a report arrives, its author appears.",
            )
        }
        people.forEach { person ->
            RiotCard {
                Row(
                    horizontalArrangement = Arrangement.spacedBy(RiotTheme.spacing.cozy),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    RiotAvatar(initials = person.rendered.take(2), keyHex = person.keyHex)
                    Column(
                        modifier = Modifier.weight(1f),
                        verticalArrangement = Arrangement.spacedBy(RiotTheme.spacing.tight),
                    ) {
                        RiotText(person.rendered, RiotType.body(17))
                        // The key tag, because a display name is claimed and a
                        // key is not — the same honesty the iOS byline keeps.
                        RiotText(
                            person.keyHex.take(8),
                            RiotType.mono(11),
                            color = RiotTheme.colors.inkSoft,
                        )
                    }
                    RiotBadge(if (person.posts == 1) "1 report" else "${person.posts} reports")
                }
            }
        }
        Spacer(Modifier.height(RiotTheme.spacing.comfortable))
    }
}

/**
 * The Nearby tab's own content: the actions that belong to radios, above
 * whichever legacy screen is pushed.
 */
@Composable
fun NearbyScreen(
    onOpenConnection: () -> Unit,
    onOpenImport: () -> Unit,
    modifier: Modifier = Modifier,
) {
    RiotSurfaceColumn(modifier) {
        Spacer(Modifier.height(RiotTheme.spacing.tight))
        RiotCard {
            RiotEyebrow("Nearby")
            RiotText(
                "Pair with a phone in the room and trade what each of you holds. " +
                    "No internet, no server in the middle.",
                RiotType.body(15),
                color = RiotTheme.colors.inkSoft,
            )
            RiotPrimaryButton("Connect to a phone nearby", onClick = onOpenConnection)
        }
        RiotCard {
            RiotEyebrow("Carried in by hand")
            RiotText(
                "A bundle from a file or a card is previewed before anything lands.",
                RiotType.body(15),
                color = RiotTheme.colors.inkSoft,
            )
            RiotSecondaryButton("Import a bundle", onClick = onOpenImport)
        }
        Spacer(Modifier.height(RiotTheme.spacing.comfortable))
    }
}

/** The Tools tab's own content: the routes to the directory and to followed sites. */
@Composable
fun ToolsScreen(
    onOpenDirectory: () -> Unit,
    onOpenFollowSites: () -> Unit,
    modifier: Modifier = Modifier,
) {
    RiotSurfaceColumn(modifier) {
        Spacer(Modifier.height(RiotTheme.spacing.tight))
        RiotCard {
            RiotEyebrow("Tools")
            RiotText(
                "Every tool your communities carry. Nothing runs until an organizer " +
                    "turns it on for a space.",
                RiotType.body(15),
                color = RiotTheme.colors.inkSoft,
            )
            RiotPrimaryButton("Open the directory", onClick = onOpenDirectory)
        }
        RiotCard {
            RiotEyebrow("Followed sites")
            RiotText(
                "Publications this community carries, verified before anything lands.",
                RiotType.body(15),
                color = RiotTheme.colors.inkSoft,
            )
            RiotSecondaryButton("Follow a site", onClick = onOpenFollowSites)
        }
        Spacer(Modifier.height(RiotTheme.spacing.comfortable))
    }
}
