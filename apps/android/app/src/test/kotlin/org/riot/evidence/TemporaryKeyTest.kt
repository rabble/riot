package org.riot.evidence

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.assertThrows
import org.junit.Test

class TemporaryKeyTest {
    @Test
    fun temporaryCopyIsClearedAfterFfiStyleCall() {
        val stored = ByteArray(32) { 0x5a }
        lateinit var observedCopy: ByteArray

        TemporaryKey.useCopy(stored) { temporary ->
            observedCopy = temporary
            assertArrayEquals(stored, temporary)
        }

        assertArrayEquals(ByteArray(32), observedCopy)
        assertArrayEquals(ByteArray(32) { 0x5a }, stored)
    }

    @Test
    fun ownedPlaintextIsClearedEvenWhenConsumerThrows() {
        val plaintext = ByteArray(48) { 0x33 }

        assertThrows(IllegalStateException::class.java) {
            TemporaryKey.useOwned(plaintext) { throw IllegalStateException("test") }
        }

        assertArrayEquals(ByteArray(48), plaintext)
    }

    @Test
    fun byteArrayOutputStreamWipesItsInternalBufferOnClose() {
        val stream = WipingByteArrayOutputStream()
        stream.write(ByteArray(32) { 0x44 })

        stream.close()

        assertTrue(stream.isInternalBufferWipedForTest())
    }
}
