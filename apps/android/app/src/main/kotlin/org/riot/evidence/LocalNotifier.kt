package org.riot.evidence

/**
 * Local, P2P-native new-content notifications — the pure-Kotlin twin of iOS
 * `Riot/LocalNotifier.swift` (+ the `NewswireUnread` value from `WhatsNew.swift`).
 *
 * Riot has no server and no push channel — nothing upstream tells a device "you
 * have new mail". The only truth about new content is what LOCAL events surface:
 * an accepted nearby sync, or the app coming to the foreground. This turns that
 * signal into a notification: it recomputes the per-community unread count from
 * the SAME seen cursor the what's-new surface uses, and decides — purely —
 * whether to raise a system notification (app backgrounded), a subtle in-app
 * banner (app foregrounded), or nothing. The decision is a pure function so it is
 * host-JVM testable without a real notification manager; the
 * NotificationManagerCompat effect is a thin seam around it (see
 * [NotificationScheduling] / `AndroidNotificationScheduler`).
 *
 * PLATFORM NOTE vs iOS: iOS's scheduler methods are `async`; Android's
 * authorization check (`NotificationManagerCompat.areNotificationsEnabled`) is
 * synchronous, so [NotificationScheduling] is synchronous and the notifier needs
 * no coroutines. The runtime POST_NOTIFICATIONS grant on API 33+ is an
 * Activity-scoped concern handled by the effect layer, not this pure core.
 */

/** The minimum a post contributes to the unread computation: its stable entry id
 * and the Willow order key (`tai_j2000_micros`, newest-first) the wire sorts by.
 * Narrower than a projected post so the math is testable without an FFI view. */
data class SeenPostRef(val entryId: String, val taiJ2000Micros: ULong)

/** The unread state for one community's wire: how many shown posts are newer than
 * the seen cursor, which posts those are, and the newest order key shown (what a
 * mark-all-seen advances the cursor to). Pure over the shown posts and the stored
 * cursor — no side effects, no persistence. Twin of iOS `NewswireUnread`. */
data class NewswireUnread(val posts: List<SeenPostRef>, val cursor: ULong?) {
    private val unreadPosts: List<SeenPostRef> =
        posts.filter { cursor == null || it.taiJ2000Micros > cursor }

    /** How many shown posts are newer than the cursor. */
    val count: Int = unreadPosts.size

    /** The entry ids of exactly those unread posts (drives per-row "new" dots). */
    val newEntryIds: Set<String> = unreadPosts.mapTo(mutableSetOf()) { it.entryId }

    /** The newest order key among the shown posts, or null when none are shown.
     * Marking all seen advances the cursor to this — never past an unloaded post. */
    val latestTimestamp: ULong? = posts.maxOfOrNull { it.taiJ2000Micros }

    /** Whether there is anything new to announce (drives the badge + banner). */
    val hasUnread: Boolean get() = count > 0

    /** Whether the given post is one of the unread ones. */
    fun isNew(entryId: String): Boolean = entryId in newEntryIds

    companion object {
        /** The empty unread state — nothing shown, nothing new. */
        val NONE = NewswireUnread(posts = emptyList(), cursor = null)
    }
}

/** The scene state the notifier cares about, reduced to the only distinction the
 * decision needs: is the app in front of the reader ([ACTIVE]) or not
 * ([BACKGROUND]). A system alert is only raised when the app is not on screen. */
enum class NotifierPhase { ACTIVE, BACKGROUND }

/** Whether this device may raise a system notification. A small mirror of the
 * platform state so the pure decision never touches the notification manager. */
enum class NotifierAuthorization {
    AUTHORIZED,
    DENIED,
    NOT_DETERMINED,
    ;

    /** Only an explicit grant lets a system alert through; NOT_DETERMINED is
     * "not yet, ask first", never permission. */
    val canPostSystemNotifications: Boolean get() = this == AUTHORIZED
}

/** What the notifier should do about a community's current unread state. A pure
 * value the effect layer turns into a posted notification, an in-app banner, or
 * nothing. `upTo` is the newest order key announced, so the caller can advance its
 * per-community de-spam cursor to exactly what it just told the reader about. */
sealed interface NotificationDecision {
    /** Raise a system notification (app backgrounded and notifications allowed). */
    data class SystemNotify(val count: Int, val upTo: ULong) : NotificationDecision

    /** Surface a subtle in-app banner (app foregrounded — never a system alert). */
    data class InAppBanner(val count: Int, val upTo: ULong) : NotificationDecision

    /** Nothing new to announce, or the app cannot notify. */
    data object Nothing : NotificationDecision

    companion object {
        /**
         * The single decision, pure over four inputs:
         * - [unread]: the current per-device unread state.
         * - [lastNotifiedUpTo]: the newest order key already announced for this
         *   community, or null if never — the de-spam cursor.
         * - [phase]: foreground vs background.
         * - [authorization]: whether system alerts are permitted.
         *
         * De-spam is monotonic on the newest shown order key: a refresh that
         * re-shows the same posts (or older ones) is silent, so the reader gets
         * one notification per genuinely-new batch, not one per refresh.
         */
        fun decide(
            unread: NewswireUnread,
            lastNotifiedUpTo: ULong?,
            phase: NotifierPhase,
            authorization: NotifierAuthorization,
        ): NotificationDecision {
            // Nothing newer than the reader's own seen cursor → nothing to announce.
            val latest = unread.latestTimestamp
            if (!unread.hasUnread || latest == null) return Nothing
            // Already announced up to (or past) this batch → stay silent.
            if (lastNotifiedUpTo != null && latest <= lastNotifiedUpTo) return Nothing
            return when (phase) {
                // On screen: never hijack with a system alert — a subtle banner.
                NotifierPhase.ACTIVE -> InAppBanner(count = unread.count, upTo = latest)
                // Off screen: a system notification, but only if the reader allowed it.
                NotifierPhase.BACKGROUND ->
                    if (authorization.canPostSystemNotifications) {
                        SystemNotify(count = unread.count, upTo = latest)
                    } else {
                        Nothing
                    }
            }
        }
    }
}

/** The in-app banner shown when new content lands while the app is foregrounded —
 * the foreground counterpart to a system notification. A plain value the shell
 * renders as a subtle, auto-dismissing toast. */
data class NewContentBanner(val communityId: String, val communityName: String, val count: Int) {
    /** The one line both the banner and the system-notification body read from,
     * so the phrasing can never drift between the two surfaces. */
    val text: String get() = summary(count, communityName)

    companion object {
        /** "3 new in Springfield" / "1 new in Springfield". */
        fun summary(count: Int, community: String): String = "$count new in $community"
    }
}

/** Schedules (or suppresses) real system notifications. Abstracted so the
 * notifier's effect layer is testable without the real notification manager and
 * so the platform-specific code lives in exactly one place. Synchronous because
 * Android's authorization check is synchronous. */
interface NotificationScheduling {
    /** The current authorization, read fresh each evaluation (the reader may have
     * changed it in Settings between syncs). */
    fun currentAuthorization(): NotifierAuthorization

    /** Prompt for authorization. Called at most once, only when undetermined. */
    fun requestAuthorization(): NotifierAuthorization

    /** Post a local notification. Reusing [identifier] per community means a newer
     * batch REPLACES the older pending one, so the count always reflects the
     * latest — one entry per community, not a stack that grows per post. */
    fun schedule(identifier: String, title: String, body: String, communityId: String)
}

/** Turns the local "store changed" signal into new-content notifications. Owns the
 * per-community de-spam cursor and the current foreground banner; delegates the
 * pure choice to [NotificationDecision.decide] and the actual scheduling to an
 * injected [NotificationScheduling]. Twin of iOS `LocalNotifier`. */
class LocalNotifier(private val scheduler: NotificationScheduling) {
    /** The current foreground banner, if any. The shell reads this and shows a
     * subtle toast; null means nothing to show. */
    var banner: NewContentBanner? = null
        private set

    /** Per community (keyed by its stable namespace id), the newest order key
     * already announced. In-memory: de-spam within a run of the app, the window in
     * which a projection can refresh and re-show the same posts. */
    private val lastNotifiedUpTo = mutableMapOf<String, ULong>()

    /** Ask for notification permission once, and only if the reader has not yet
     * decided. Called at a relevant first moment (a community shell appearing),
     * never aggressively on cold launch. A denied reader is left alone. */
    fun requestAuthorizationIfNeeded() {
        if (scheduler.currentAuthorization() != NotifierAuthorization.NOT_DETERMINED) return
        scheduler.requestAuthorization()
    }

    /** Evaluate one community's fresh unread state and act on the decision. Safe to
     * call on every data-changed signal: de-spam keeps repeat calls for the same
     * batch silent. An empty community id (a community with no identity yet) is
     * inert. */
    fun evaluate(
        communityId: String,
        communityName: String,
        unread: NewswireUnread,
        phase: NotifierPhase,
    ) {
        if (communityId.isEmpty()) return
        val decision = NotificationDecision.decide(
            unread = unread,
            lastNotifiedUpTo = lastNotifiedUpTo[communityId],
            phase = phase,
            authorization = scheduler.currentAuthorization(),
        )
        when (decision) {
            is NotificationDecision.Nothing -> Unit
            is NotificationDecision.SystemNotify -> {
                lastNotifiedUpTo[communityId] = decision.upTo
                scheduler.schedule(
                    identifier = "riot.newcontent.$communityId",
                    title = communityName,
                    body = NewContentBanner.summary(decision.count, communityName),
                    communityId = communityId,
                )
            }
            is NotificationDecision.InAppBanner -> {
                lastNotifiedUpTo[communityId] = decision.upTo
                banner = NewContentBanner(communityId, communityName, decision.count)
            }
        }
    }

    /** Dismiss the foreground banner (the toast auto-dismisses; this is also the
     * hook a tap or a route change uses). */
    fun dismissBanner() {
        banner = null
    }
}
