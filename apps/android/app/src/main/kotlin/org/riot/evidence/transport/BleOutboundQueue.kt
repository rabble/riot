package org.riot.evidence.transport

import java.io.IOException
import java.util.ArrayDeque

internal class BleOutboundQueue(
    private val maxQueuedFrames: Int = 4,
    private val maxQueuedBytes: Int = MAX_SYNC_FRAME_BYTES,
    private val maxQueuedChunks: Int = BleFrameCodec.chunkCount(MAX_SYNC_FRAME_BYTES),
) {
    private val frames = ArrayDeque<PendingFrame>()
    private var queuedBytes = 0
    private var queuedChunks = 0
    private var closed = false
    internal val pendingFrameCountForTest: Int get() = frames.size
    internal val pendingBytesForTest: Int get() = queuedBytes

    fun add(frame: ByteArray) {
        if (closed) throw IOException("nearby write queue closed")
        val chunks = BleFrameCodec.chunkCount(frame.size)
        if (frames.size >= maxQueuedFrames || queuedBytes + frame.size > maxQueuedBytes ||
            queuedChunks + chunks > maxQueuedChunks
        ) {
            close()
            throw IOException("nearby write queue is full")
        }
        frames += PendingFrame(frame.copyOf(), chunks)
        queuedBytes += frame.size
        queuedChunks += chunks
    }

    fun pollChunk(): ByteArray? {
        if (closed) throw IOException("nearby write queue closed")
        val pending = frames.firstOrNull() ?: return null
        val chunk = BleFrameCodec.chunkAt(pending.frame, pending.nextIndex++)
        queuedChunks -= 1
        if (pending.nextIndex == pending.chunkCount) {
            frames.removeFirst()
            queuedBytes -= pending.frame.size
        }
        return chunk
    }

    fun close() {
        closed = true
        frames.clear()
        queuedBytes = 0
        queuedChunks = 0
    }

    private data class PendingFrame(
        val frame: ByteArray,
        val chunkCount: Int,
        var nextIndex: Int = 0,
    )
}
