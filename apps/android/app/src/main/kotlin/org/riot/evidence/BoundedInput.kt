package org.riot.evidence

import java.io.ByteArrayOutputStream
import java.io.InputStream

object BoundedInput {
    fun read(input: InputStream, maxBytes: Int): ByteArray {
        require(maxBytes >= 0) { "negative input limit" }
        val output = ByteArrayOutputStream(minOf(maxBytes, 8 * 1024))
        val buffer = ByteArray(8 * 1024)
        var total = 0
        while (true) {
            val count = input.read(buffer)
            if (count < 0) return output.toByteArray()
            total += count
            require(total <= maxBytes) { "selected bundle is too large" }
            output.write(buffer, 0, count)
        }
    }
}
