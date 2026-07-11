package org.riot.evidence.transport

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class RemoteConfirmationTest {
    @Test
    fun delayedRemoteConfirmationNeverMarksConnectionReadyEarly() {
        val wait = RemoteConfirmationWait(maxAttempts = 3)

        assertTrue(wait.beginAttempt())
        assertEquals(ConfirmationDecision.RETRY, wait.completeAttempt(confirmed = false))
        assertFalse(wait.isConfirmed)
        assertTrue(wait.beginAttempt())
        assertEquals(ConfirmationDecision.READY, wait.completeAttempt(confirmed = true))
        assertTrue(wait.isConfirmed)
    }

    @Test
    fun exhaustedConfirmationFailsInsteadOfStartingBleSync() {
        val wait = RemoteConfirmationWait(maxAttempts = 1)

        assertTrue(wait.beginAttempt())
        assertEquals(ConfirmationDecision.FAILED, wait.completeAttempt(confirmed = false))
        assertFalse(wait.isConfirmed)
        assertFalse(wait.beginAttempt())
    }
}
