package org.riot.evidence.transport

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class BleFrameCodecTest {
    @Test
    fun boundedChunksReassembleOneLogicalFrameInOrder() {
        val frame = ByteArray(2_000) { (it % 251).toByte() }
        val chunks = BleFrameCodec.chunk(frame)
        val decoder = BleFrameCodec.Decoder()

        val decoded = chunks.mapNotNull(decoder::receive)

        assertEquals(1, decoded.size)
        assertArrayEquals(frame, decoded.single())
        chunks.forEach { assertTrue(it.size <= 20) }
    }

    @Test
    fun defaultChunksFitTheUnnegotiatedGattPayloadLimit() {
        val chunks = BleFrameCodec.chunk(ByteArray(17))

        assertEquals(listOf(20, 20, 13), chunks.map(ByteArray::size))
        assertThrows(IllegalArgumentException::class.java) {
            BleFrameCodec.chunk(byteArrayOf(1), maxChunkBytes = 21)
        }
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

    @Test
    fun decoderRejectsChunkCountThatCannotMatchDeclaredFrame() {
        val impossible = byteArrayOf(
            0, 0, 0, 1,
            0, 0, 0, 0,
            0x7f, 0xff.toByte(), 0xff.toByte(), 0xff.toByte(),
            1,
        )

        assertThrows(IllegalArgumentException::class.java) {
            BleFrameCodec.Decoder().receive(impossible)
        }
    }
}
