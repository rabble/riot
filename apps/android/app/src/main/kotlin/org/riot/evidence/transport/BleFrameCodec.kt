package org.riot.evidence.transport

import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

object BleFrameCodec {
    internal const val HEADER_BYTES = 12
    /** Fits the 20-byte characteristic payload available before any GATT MTU negotiation. */
    const val MAX_BLE_CHUNK_BYTES = 20

    fun chunk(frame: ByteArray, maxChunkBytes: Int = MAX_BLE_CHUNK_BYTES): List<ByteArray> {
        require(frame.size <= MAX_SYNC_FRAME_BYTES) { "frame is too large" }
        require(maxChunkBytes in (HEADER_BYTES + 1)..MAX_BLE_CHUNK_BYTES) { "invalid BLE chunk size" }
        return List(chunkCount(frame.size, maxChunkBytes)) { index -> chunkAt(frame, index, maxChunkBytes) }
    }

    internal fun chunkCount(frameBytes: Int, maxChunkBytes: Int = MAX_BLE_CHUNK_BYTES): Int {
        require(frameBytes in 0..MAX_SYNC_FRAME_BYTES) { "frame is too large" }
        require(maxChunkBytes in (HEADER_BYTES + 1)..MAX_BLE_CHUNK_BYTES) { "invalid BLE chunk size" }
        val payloadBytes = maxChunkBytes - HEADER_BYTES
        return maxOf(1, (frameBytes + payloadBytes - 1) / payloadBytes)
    }

    internal fun chunkAt(
        frame: ByteArray,
        index: Int,
        maxChunkBytes: Int = MAX_BLE_CHUNK_BYTES,
    ): ByteArray {
        val count = chunkCount(frame.size, maxChunkBytes)
        require(index in 0 until count) { "invalid BLE chunk index" }
        val payloadBytes = maxChunkBytes - HEADER_BYTES
        val start = index * payloadBytes
        val end = minOf(frame.size, start + payloadBytes)
        return ByteBuffer.allocate(HEADER_BYTES + end - start)
            .order(ByteOrder.BIG_ENDIAN)
            .putInt(frame.size)
            .putInt(index)
            .putInt(count)
            .put(frame, start, end - start)
            .array()
    }

    class Decoder {
        private var expectedLength = -1
        private var expectedCount = -1
        private var nextIndex = 0
        private var bytes: ByteArrayOutputStream? = null

        fun receive(chunk: ByteArray): ByteArray? {
            require(chunk.size in HEADER_BYTES..MAX_BLE_CHUNK_BYTES) { "invalid BLE chunk length" }
            val buffer = ByteBuffer.wrap(chunk).order(ByteOrder.BIG_ENDIAN)
            val totalLength = buffer.int
            val index = buffer.int
            val count = buffer.int
            require(totalLength in 0..MAX_SYNC_FRAME_BYTES) { "declared frame is too large" }
            require(count > 0 && index in 0 until count) { "invalid BLE chunk sequence" }
            require(count == chunkCount(totalLength)) { "BLE chunk count does not match frame length" }
            val payloadBytes = MAX_BLE_CHUNK_BYTES - HEADER_BYTES
            val expectedPayload = minOf(payloadBytes, totalLength - index * payloadBytes)
            require(buffer.remaining() == expectedPayload) { "BLE chunk payload does not match declaration" }
            if (index == 0) {
                reset()
                expectedLength = totalLength
                expectedCount = count
                bytes = ByteArrayOutputStream(totalLength)
            }
            require(index == nextIndex && count == expectedCount && totalLength == expectedLength) {
                "out-of-order BLE chunk"
            }
            val payload = ByteArray(buffer.remaining())
            buffer.get(payload)
            val output = checkNotNull(bytes) { "missing first BLE chunk" }
            require(output.size() + payload.size <= expectedLength) { "BLE frame exceeds declaration" }
            output.write(payload)
            nextIndex += 1
            if (nextIndex != expectedCount) return null
            require(output.size() == expectedLength) { "incomplete BLE frame" }
            val frame = output.toByteArray()
            reset()
            return frame
        }

        private fun reset() {
            expectedLength = -1
            expectedCount = -1
            nextIndex = 0
            bytes = null
        }
    }
}
