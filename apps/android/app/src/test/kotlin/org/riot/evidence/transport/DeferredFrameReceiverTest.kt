package org.riot.evidence.transport

import java.io.IOException
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class DeferredFrameReceiverTest {
    @Test
    fun framesCompletedBeforeRegistrationAreDeliveredExactlyOnceInOrder() {
        val deferred = DeferredFrameReceiver(maxQueuedFrames = 3, maxQueuedBytes = 8)
        deferred.deliver(byteArrayOf(1, 2))
        deferred.deliver(byteArrayOf(3))
        val received = mutableListOf<ByteArray>()

        deferred.register { received += it }
        deferred.deliver(byteArrayOf(4, 5))

        assertEquals(3, received.size)
        assertArrayEquals(byteArrayOf(1, 2), received[0])
        assertArrayEquals(byteArrayOf(3), received[1])
        assertArrayEquals(byteArrayOf(4, 5), received[2])
    }

    @Test
    fun queuedFrameCountOverflowFailsClosed() {
        val deferred = DeferredFrameReceiver(maxQueuedFrames = 1, maxQueuedBytes = 8)
        deferred.deliver(byteArrayOf(1))

        assertThrows(IOException::class.java) { deferred.deliver(byteArrayOf(2)) }
        assertThrows(IOException::class.java) { deferred.register {} }
    }

    @Test
    fun queuedByteOverflowFailsClosed() {
        val deferred = DeferredFrameReceiver(maxQueuedFrames = 3, maxQueuedBytes = 2)

        assertThrows(IOException::class.java) { deferred.deliver(byteArrayOf(1, 2, 3)) }
        assertThrows(IOException::class.java) { deferred.deliver(byteArrayOf(4)) }
    }
}
