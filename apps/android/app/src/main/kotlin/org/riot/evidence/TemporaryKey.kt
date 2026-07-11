package org.riot.evidence

import java.io.ByteArrayOutputStream

object TemporaryKey {
    inline fun <T> useCopy(stored: ByteArray, block: (ByteArray) -> T): T {
        val temporary = stored.copyOf()
        return try {
            block(temporary)
        } finally {
            temporary.fill(0)
        }
    }

    inline fun <T> useOwned(bytes: ByteArray, block: (ByteArray) -> T): T = try {
        block(bytes)
    } finally {
        bytes.fill(0)
    }
}

internal class WipingByteArrayOutputStream(initialSize: Int = 32) : ByteArrayOutputStream(initialSize) {
    override fun close() {
        buf.fill(0)
        super.close()
    }

    internal fun isInternalBufferWipedForTest(): Boolean = buf.all { it == 0.toByte() }
}
