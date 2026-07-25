package net.protest.riot.design

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.TextFieldValue
import net.protest.riot.NewswireCommentRow
import net.protest.riot.NewswirePostDisplay
import net.protest.riot.NewswirePostRow
import net.protest.riot.NewswireSurface
import net.protest.riot.NewswireTreatmentCopy
import net.protest.riot.NewswireWireCopy
import net.protest.riot.NewswireWireState
import uniffi.riot_ffi.CurrentEntry

/**
 * The surfaces ported onto the design system.
 *
 * Each takes plain data and lambdas — no controller, no Activity — so the
 * rendering is independent of how the app is wired and can be previewed and
 * reasoned about on its own. The Activity keeps owning state and side effects.
 */

/** The community's space: what this device holds, and how to start one. */
@Composable
fun SpacesScreen(
    spaceTitle: String?,
    namespaceId: String?,
    onCreateSpace: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    var draft by remember { mutableStateOf(TextFieldValue(spaceTitle ?: "Berlin Mutual Aid")) }
    RiotSurfaceColumn(modifier) {
        Spacer(Modifier.height(RiotTheme.spacing.tight))
        if (spaceTitle == null) {
            RiotEmptyState(
                title = "No space yet",
                body = "Create a public space and everything you publish stays on this " +
                    "device until you hand it to someone.",
            )
        } else {
            RiotCard {
                RiotEyebrow("This community")
                RiotText(spaceTitle, RiotType.serif(22))
                RiotText("Public namespace", RiotType.body(13), color = RiotTheme.colors.inkSoft)
                // The namespace is identity-bearing, so it is shown in mono and
                // truncated: a person can check it, without it dominating the card.
                namespaceId?.let {
                    RiotText(
                        it.take(16) + "…",
                        RiotType.mono(11),
                        color = RiotTheme.colors.inkSoft,
                    )
                }
            }
        }
        RiotCard {
            RiotEyebrow("Start a space")
            RiotTextField(value = draft, onValueChange = { draft = it }, placeholder = "Space title")
            RiotPrimaryButton("Create public space", onClick = { onCreateSpace(draft.text) })
        }
        Spacer(Modifier.height(RiotTheme.spacing.comfortable))
    }
}

/** The alert board: signed, offline-available alerts. */
@Composable
fun AlertsScreen(entries: List<CurrentEntry>, modifier: Modifier = Modifier) {
    RiotSurfaceColumn(modifier) {
        Spacer(Modifier.height(RiotTheme.spacing.tight))
        if (entries.isEmpty()) {
            RiotEmptyState(
                title = "No alerts yet",
                body = "Everything shown here is available offline.",
            )
        }
        entries.forEach { entry ->
            RiotCard {
                RiotEyebrow("Alert")
                RiotText(entry.headline, RiotType.serif(20))
                Row(
                    horizontalArrangement = Arrangement.spacedBy(RiotTheme.spacing.snug),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    RiotAvatar(initials = entry.signerId.take(2), keyHex = entry.signerId)
                    Column {
                        RiotText("Signer", RiotType.body(13))
                        RiotText(
                            entry.signerId.take(12),
                            RiotType.mono(11),
                            color = RiotTheme.colors.inkSoft,
                        )
                    }
                }
                // The provenance line is the point of the whole app: a reader must
                // be able to see whether a human wrote it and whether a human signed it.
                RiotBadge(
                    if (entry.aiAssisted) "AI-assisted · human signed" else "Human drafted and signed"
                )
            }
        }
        Spacer(Modifier.height(RiotTheme.spacing.comfortable))
    }
}

/**
 * The community newswire, rendered from core's already-split signed projection.
 *
 * Editorial treatment is honoured exactly as the projection presents it: a
 * hidden or tombstoned post is redacted to its interstitial rather than styled
 * differently, because the treatment IS the content at that point.
 */
@Composable
fun NewswireScreenUi(
    communityTitle: String?,
    surface: NewswireSurface?,
    onGoToSpaces: () -> Unit,
    onCompose: () -> Unit,
    onOpenAlerts: () -> Unit,
    onReply: (parentEntryId: String, body: String) -> Unit,
    modifier: Modifier = Modifier,
) {
    RiotSurfaceColumn(modifier) {
        Spacer(Modifier.height(RiotTheme.spacing.tight))
        if (communityTitle == null || surface == null) {
            RiotEmptyState(
                title = "No community yet",
                body = "Join or create a community to see its newswire. Everything you " +
                    "already hold stays available offline.",
            )
            RiotSecondaryButton("Go to spaces", onClick = onGoToSpaces)
            return@RiotSurfaceColumn
        }
        // Writing is the point of a wire, so it is the one filled action on
        // Home; the alert board sits beside it as the quieter route.
        RiotCard {
            RiotEyebrow("This wire")
            RiotPrimaryButton("Post an update", onClick = onCompose)
            RiotSecondaryButton("Alerts", onClick = onOpenAlerts)
        }
        if (surface.unread.hasUnread) {
            RiotBadge("${surface.unread.count} new since you last looked", tint = RiotTheme.colors.pink)
        }
        when (val wire = surface.wire) {
            NewswireWireState.OfflineStale ->
                RiotEmptyState(
                    title = NewswireWireCopy.OFFLINE_TITLE,
                    body = NewswireWireCopy.OFFLINE_MESSAGE,
                )
            NewswireWireState.EmptyWire ->
                RiotEmptyState(
                    title = NewswireWireCopy.EMPTY_TITLE,
                    body = NewswireWireCopy.EMPTY_MESSAGE,
                )
            is NewswireWireState.PostsButNoFeature -> {
                RiotEmptyState(
                    title = NewswireWireCopy.NO_FEATURE_TITLE,
                    body = NewswireWireCopy.NO_FEATURE_MESSAGE,
                )
                wire.openWire.forEach { post ->
                    PostCard("Open wire", post, surface.comments(post.id), onReply)
                }
            }
            is NewswireWireState.Featured -> {
                // Core re-lists every featured post on the open wire, so the
                // highlight is headline-only and the thread is drawn once, on the
                // canonical open-wire row — `featuredOnlyIds` is what decides that.
                val featuredOnly = wire.featuredOnlyIds
                wire.frontPage.forEach { post ->
                    PostCard(
                        "Featured",
                        post,
                        if (post.id in featuredOnly) surface.comments(post.id) else emptyList(),
                        onReply,
                        allowReply = post.id in featuredOnly,
                    )
                }
                wire.openWire.forEach { post ->
                    PostCard("Open wire", post, surface.comments(post.id), onReply)
                }
            }
        }
        Spacer(Modifier.height(RiotTheme.spacing.comfortable))
    }
}

@Composable
private fun PostCard(
    eyebrow: String,
    post: NewswirePostRow,
    comments: List<NewswireCommentRow>,
    onReply: (String, String) -> Unit,
    allowReply: Boolean = true,
) {
    var replyDraft by remember(post.id) { mutableStateOf(TextFieldValue("")) }
    var replying by remember(post.id) { mutableStateOf(false) }
    RiotCard {
        RiotEyebrow(eyebrow)
        when (post.display) {
            NewswirePostDisplay.HIDDEN_INTERSTITIAL -> {
                RiotText(NewswireTreatmentCopy.HIDDEN_TITLE, RiotType.serif(18))
                RiotText(
                    NewswireTreatmentCopy.HIDDEN_BODY,
                    RiotType.body(15),
                    color = RiotTheme.colors.inkSoft,
                )
            }
            NewswirePostDisplay.TOMBSTONED -> {
                RiotText(NewswireTreatmentCopy.TOMBSTONE_TITLE, RiotType.serif(18))
                RiotText(
                    NewswireTreatmentCopy.TOMBSTONE_BODY,
                    RiotType.body(15),
                    color = RiotTheme.colors.inkSoft,
                )
            }
            NewswirePostDisplay.ORDINARY -> {
                post.headline?.let { RiotText(it, RiotType.serif(22)) }
                Row(
                    horizontalArrangement = Arrangement.spacedBy(RiotTheme.spacing.snug),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    RiotAvatar(initials = post.author.take(2), keyHex = post.authorKeyHex)
                    Column {
                        RiotText(post.author, RiotType.body(15))
                        RiotText(
                            post.authorKeyHex.take(8),
                            RiotType.mono(11),
                            color = RiotTheme.colors.inkSoft,
                        )
                    }
                }
                // Provenance, the reason this app exists: whether a human wrote
                // it, whether it was corrected, and how many people verified it.
                Row(horizontalArrangement = Arrangement.spacedBy(RiotTheme.spacing.snug)) {
                    if (post.aiAssisted) RiotBadge("AI-assisted · human signed")
                    if (post.hasCorrection) RiotBadge("Corrected", tint = RiotTheme.colors.pink)
                    if (post.verificationCount > 0) RiotBadge("Verified ${post.verificationCount}")
                }
            }
        }
        comments.forEach { comment ->
            // A hidden or tombstoned reply arrives with body == null and draws its
            // interstitial instead of the words — the same redaction contract a
            // post gets, so treatment is never silently dropped.
            RiotText(
                comment.body ?: NewswireTreatmentCopy.HIDDEN_TITLE,
                RiotType.body(14),
                color = RiotTheme.colors.inkSoft,
                modifier = Modifier.padding(start = RiotTheme.spacing.cozy),
            )
        }
        if (post.display == NewswirePostDisplay.ORDINARY && allowReply) {
            if (replying) {
                RiotTextField(
                    value = replyDraft,
                    onValueChange = { replyDraft = it },
                    placeholder = "Reply to the wire",
                )
                RiotPrimaryButton("Post reply", onClick = {
                    onReply(post.id, replyDraft.text)
                    replyDraft = TextFieldValue("")
                    replying = false
                })
            } else {
                RiotSecondaryButton("Reply", onClick = { replying = true })
            }
        }
    }
}
