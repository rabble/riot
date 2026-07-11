package org.riot.evidence.transport

import java.io.IOException
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class BleOutboundQueueTest {
    @Test
    fun queuedFramesEmitChunksInFrameOrderWithoutPreallocatingEveryChunk() {
        val queue = BleOutboundQueue(maxQueuedFrames = 2, maxQueuedBytes = 32)
        val first = ByteArray(17) { it.toByte() }
        val second = byteArrayOf(99)
        queue.add(first)
        queue.add(second)

        val decoder = BleFrameCodec.Decoder()
        val decoded = generateSequence(queue::pollChunk).mapNotNull(decoder::receive).toList()

        assertEquals(2, decoded.size)
        assertArrayEquals(first, decoded[0])
        assertArrayEquals(second, decoded[1])
    }

    @Test
    fun pendingCountOrBytesOverflowFailsClosed() {
        val countBound = BleOutboundQueue(maxQueuedFrames = 1, maxQueuedBytes = 8)
        countBound.add(byteArrayOf(1))
        assertThrows(IOException::class.java) { countBound.add(byteArrayOf(2)) }
        assertThrows(IOException::class.java) { countBound.pollChunk() }

        val byteBound = BleOutboundQueue(maxQueuedFrames = 2, maxQueuedBytes = 1)
        assertThrows(IOException::class.java) { byteBound.add(byteArrayOf(1, 2)) }
    }

    @Test
    fun maximumProtocolFrameUsesOnePendingFrameAndStreamsOneChunkAtATime() {
        val queue = BleOutboundQueue()
        queue.add(ByteArray(MAX_SYNC_FRAME_BYTES))

        assertEquals(1, queue.pendingFrameCountForTest)
        assertEquals(MAX_SYNC_FRAME_BYTES, queue.pendingBytesForTest)
        assertEquals(20, queue.pollChunk()!!.size)
        assertEquals(1, queue.pendingFrameCountForTest)
    }
}
