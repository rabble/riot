package org.riot.evidence

import java.io.ByteArrayInputStream
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class BoundedInputTest {
    @Test
    fun rejectsInputBeyondTheLimit() {
        assertArrayEquals(
            byteArrayOf(1, 2, 3),
            BoundedInput.read(ByteArrayInputStream(byteArrayOf(1, 2, 3)), maxBytes = 3),
        )
        assertThrows(IllegalArgumentException::class.java) {
            BoundedInput.read(ByteArrayInputStream(ByteArray(4)), maxBytes = 3)
        }
    }
}
