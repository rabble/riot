package org.riot.evidence.transport

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class BleFrameCodecTest {
    @Test
    fun boundedChunksReassembleOneLogicalFrameInOrder() {
        val frame = ByteArray(2_000) { (it % 251).toByte() }
        val chunks = BleFrameCodec.chunk(frame, maxChunkBytes = 128)
        val decoder = BleFrameCodec.Decoder()

        val decoded = chunks.mapNotNull(decoder::receive)

        assertEquals(1, decoded.size)
        assertArrayEquals(frame, decoded.single())
        chunks.forEach { assert(it.size <= 128) }
    }

    @Test
    fun decoderRejectsDeclaredFrameBeyondProtocolCeiling() {
        val invalidHeader = byteArrayOf(
            0x00, 0x80.toByte(), 0x00, 0x81.toByte(),
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01,
        )

        assertThrows(IllegalArgumentException::class.java) {
            BleFrameCodec.Decoder().receive(invalidHeader)
        }
    }
}
