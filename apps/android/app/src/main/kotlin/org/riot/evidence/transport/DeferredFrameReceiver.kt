package org.riot.evidence.transport

import java.io.IOException
import java.util.ArrayDeque

internal class DeferredFrameReceiver(
    private val maxQueuedFrames: Int = 4,
    private val maxQueuedBytes: Int = MAX_SYNC_FRAME_BYTES,
) {
    private val queued = ArrayDeque<ByteArray>()
    private var queuedBytes = 0
    private var receiver: ((ByteArray) -> Unit)? = null
    private var closed = false

    init {
        require(maxQueuedFrames > 0)
        require(maxQueuedBytes > 0)
    }

    @Synchronized
    fun deliver(frame: ByteArray) {
        if (closed) throw IOException("nearby connection closed")
        receiver?.let {
            it(frame.copyOf())
            return
        }
        if (queued.size >= maxQueuedFrames || queuedBytes + frame.size > maxQueuedBytes) {
            close()
            throw IOException("too many frames arrived before connection was ready")
        }
        queued += frame.copyOf()
        queuedBytes += frame.size
    }

    @Synchronized
    fun register(receiver: (ByteArray) -> Unit) {
        if (closed) throw IOException("nearby connection closed")
        check(this.receiver == null) { "nearby receiver is already registered" }
        this.receiver = receiver
        while (queued.isNotEmpty()) receiver(queued.removeFirst())
        queuedBytes = 0
    }

    @Synchronized
    fun close() {
        closed = true
        queued.clear()
        queuedBytes = 0
        receiver = null
    }
}
