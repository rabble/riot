package org.riot.evidence

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Host-JVM twin of iOS `RiotTests/LocalNotifierTests.swift`. Exercises the pure
 * `NotificationDecision.decide` over its four inputs and the `LocalNotifier`
 * effect layer (de-spam + phase routing) through a spy scheduler — no real
 * notification manager, no native library.
 */
class LocalNotifierTest {
    /** Controllable [NotificationScheduling] double: reports a set authorization
     * and records every scheduled request. */
    private class SpyScheduler(var authorization: NotifierAuthorization) : NotificationScheduling {
        var requestCount = 0
        val scheduled = mutableListOf<Scheduled>()

        data class Scheduled(
            val identifier: String,
            val title: String,
            val body: String,
            val communityId: String,
        )

        override fun currentAuthorization(): NotifierAuthorization = authorization

        override fun requestAuthorization(): NotifierAuthorization {
            requestCount += 1
            // A grant flips the reported status, mirroring the real prompt.
            if (authorization == NotifierAuthorization.NOT_DETERMINED) {
                authorization = NotifierAuthorization.AUTHORIZED
            }
            return authorization
        }

        override fun schedule(identifier: String, title: String, body: String, communityId: String) {
            scheduled += Scheduled(identifier, title, body, communityId)
        }
    }

    private fun ref(id: String, tai: ULong) = SeenPostRef(entryId = id, taiJ2000Micros = tai)

    private fun unread(posts: List<SeenPostRef>, cursor: ULong?) =
        NewswireUnread(posts = posts, cursor = cursor)

    // MARK: - NewswireUnread derivation

    @Test
    fun unreadCountsPostsNewerThanCursor() {
        val state = unread(listOf(ref("a", 30u), ref("b", 20u), ref("c", 10u)), cursor = 15u)
        assertEquals(2, state.count)
        assertTrue(state.hasUnread)
        assertEquals(30u.toULong(), state.latestTimestamp)
        assertTrue(state.isNew("a"))
        assertFalse(state.isNew("c"))
    }

    @Test
    fun unreadWithNullCursorTreatsAllAsNew() {
        val state = unread(listOf(ref("a", 30u), ref("b", 20u)), cursor = null)
        assertEquals(2, state.count)
        assertEquals(30u.toULong(), state.latestTimestamp)
    }

    @Test
    fun unreadWithNothingShownHasNoLatest() {
        val state = unread(emptyList(), cursor = null)
        assertEquals(0, state.count)
        assertFalse(state.hasUnread)
        assertNull(state.latestTimestamp)
    }

    // MARK: - Pure decision logic

    @Test
    fun backgroundedNewContentAuthorizedSystemNotifies() {
        val state = unread(listOf(ref("a", 30u), ref("b", 20u)), cursor = 10u)
        val decision = NotificationDecision.decide(
            unread = state, lastNotifiedUpTo = null,
            phase = NotifierPhase.BACKGROUND, authorization = NotifierAuthorization.AUTHORIZED,
        )
        assertEquals(NotificationDecision.SystemNotify(count = 2, upTo = 30u), decision)
    }

    @Test
    fun foregroundedNewContentBannersNotSystemNotify() {
        val state = unread(listOf(ref("a", 30u)), cursor = null)
        val decision = NotificationDecision.decide(
            unread = state, lastNotifiedUpTo = null,
            phase = NotifierPhase.ACTIVE, authorization = NotifierAuthorization.AUTHORIZED,
        )
        assertEquals(NotificationDecision.InAppBanner(count = 1, upTo = 30u), decision)
    }

    @Test
    fun deniedAuthorizationWhileBackgroundedDoesNothing() {
        val state = unread(listOf(ref("a", 30u)), cursor = null)
        val decision = NotificationDecision.decide(
            unread = state, lastNotifiedUpTo = null,
            phase = NotifierPhase.BACKGROUND, authorization = NotifierAuthorization.DENIED,
        )
        assertEquals(NotificationDecision.Nothing, decision)
    }

    @Test
    fun notDeterminedAuthorizationWhileBackgroundedDoesNothing() {
        val state = unread(listOf(ref("a", 30u)), cursor = null)
        val decision = NotificationDecision.decide(
            unread = state, lastNotifiedUpTo = null,
            phase = NotifierPhase.BACKGROUND, authorization = NotifierAuthorization.NOT_DETERMINED,
        )
        assertEquals(NotificationDecision.Nothing, decision)
    }

    @Test
    fun noUnreadDoesNothing() {
        val state = unread(listOf(ref("a", 30u)), cursor = 30u)
        val decision = NotificationDecision.decide(
            unread = state, lastNotifiedUpTo = null,
            phase = NotifierPhase.BACKGROUND, authorization = NotifierAuthorization.AUTHORIZED,
        )
        assertEquals(NotificationDecision.Nothing, decision)
    }

    @Test
    fun sameBatchAlreadyNotifiedDoesNothing() {
        // Latest shown is 30 and we already announced up to 30 → a refresh that
        // re-shows the same posts stays silent (de-spam).
        val state = unread(listOf(ref("a", 30u), ref("b", 20u)), cursor = 10u)
        val decision = NotificationDecision.decide(
            unread = state, lastNotifiedUpTo = 30u,
            phase = NotifierPhase.BACKGROUND, authorization = NotifierAuthorization.AUTHORIZED,
        )
        assertEquals(NotificationDecision.Nothing, decision)
    }

    @Test
    fun strictlyNewerBatchThanLastNotifiedNotifies() {
        val state = unread(listOf(ref("c", 40u), ref("a", 30u)), cursor = 10u)
        val decision = NotificationDecision.decide(
            unread = state, lastNotifiedUpTo = 30u,
            phase = NotifierPhase.BACKGROUND, authorization = NotifierAuthorization.AUTHORIZED,
        )
        assertEquals(NotificationDecision.SystemNotify(count = 2, upTo = 40u), decision)
    }

    // MARK: - Effect layer (de-spam + phase routing through the scheduler)

    @Test
    fun evaluateBackgroundedSchedulesOnceThenDeSpams() {
        val scheduler = SpyScheduler(NotifierAuthorization.AUTHORIZED)
        val notifier = LocalNotifier(scheduler)
        val state = unread(listOf(ref("a", 30u), ref("b", 20u)), cursor = 10u)

        notifier.evaluate("ns1", "Springfield", state, NotifierPhase.BACKGROUND)
        assertEquals(1, scheduler.scheduled.size)
        assertEquals("Springfield", scheduler.scheduled.first().title)
        assertEquals("ns1", scheduler.scheduled.first().communityId)

        // The same projection arriving again must not re-notify.
        notifier.evaluate("ns1", "Springfield", state, NotifierPhase.BACKGROUND)
        assertEquals(1, scheduler.scheduled.size)
    }

    @Test
    fun evaluateNewerBatchNotifiesAgain() {
        val scheduler = SpyScheduler(NotifierAuthorization.AUTHORIZED)
        val notifier = LocalNotifier(scheduler)

        notifier.evaluate("ns1", "Springfield", unread(listOf(ref("a", 30u)), cursor = 10u), NotifierPhase.BACKGROUND)
        notifier.evaluate(
            "ns1", "Springfield",
            unread(listOf(ref("b", 55u), ref("a", 30u)), cursor = 10u), NotifierPhase.BACKGROUND,
        )
        assertEquals(2, scheduler.scheduled.size)
    }

    @Test
    fun evaluateForegroundedBannersNotSchedules() {
        val scheduler = SpyScheduler(NotifierAuthorization.AUTHORIZED)
        val notifier = LocalNotifier(scheduler)

        notifier.evaluate("ns1", "Springfield", unread(listOf(ref("a", 30u)), cursor = 10u), NotifierPhase.ACTIVE)

        assertTrue(scheduler.scheduled.isEmpty())
        assertEquals("Springfield", notifier.banner?.communityName)
        assertEquals(1, notifier.banner?.count)
        assertEquals("1 new in Springfield", notifier.banner?.text)
    }

    @Test
    fun evaluateEmptyCommunityIdIsInert() {
        val scheduler = SpyScheduler(NotifierAuthorization.AUTHORIZED)
        val notifier = LocalNotifier(scheduler)

        notifier.evaluate("", "Springfield", unread(listOf(ref("a", 30u)), cursor = 10u), NotifierPhase.BACKGROUND)

        assertTrue(scheduler.scheduled.isEmpty())
        assertNull(notifier.banner)
    }

    @Test
    fun evaluateDeniedWhileBackgroundedSchedulesNothing() {
        val scheduler = SpyScheduler(NotifierAuthorization.DENIED)
        val notifier = LocalNotifier(scheduler)

        notifier.evaluate("ns1", "Springfield", unread(listOf(ref("a", 30u)), cursor = 10u), NotifierPhase.BACKGROUND)

        assertTrue(scheduler.scheduled.isEmpty())
    }

    @Test
    fun requestAuthorizationOnlyWhenNotDetermined() {
        val undetermined = SpyScheduler(NotifierAuthorization.NOT_DETERMINED)
        LocalNotifier(undetermined).requestAuthorizationIfNeeded()
        assertEquals(1, undetermined.requestCount)

        // Already decided → never prompt again.
        val denied = SpyScheduler(NotifierAuthorization.DENIED)
        LocalNotifier(denied).requestAuthorizationIfNeeded()
        assertEquals(0, denied.requestCount)
    }

    @Test
    fun bannerDismissClears() {
        val scheduler = SpyScheduler(NotifierAuthorization.AUTHORIZED)
        val notifier = LocalNotifier(scheduler)
        notifier.evaluate("ns1", "Springfield", unread(listOf(ref("a", 30u)), cursor = 10u), NotifierPhase.ACTIVE)
        assertTrue(notifier.banner != null)
        notifier.dismissBanner()
        assertNull(notifier.banner)
    }
}
