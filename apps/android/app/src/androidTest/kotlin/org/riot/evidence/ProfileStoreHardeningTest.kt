package org.riot.evidence

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.io.RandomAccessFile
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ProfileStoreHardeningTest {
    @Test
    fun oversizedFileFailsClosedWithoutChangingValidSnapshot() {
        assertRejectedCandidateDoesNotChangeValidSnapshot { candidate ->
            RandomAccessFile(candidate, "rw").use { it.setLength(4L * 1024 * 1024 + 65) }
        }
    }

    @Test
    fun corruptEnvelopeFailsClosedWithoutChangingValidSnapshot() {
        assertRejectedCandidateDoesNotChangeValidSnapshot { candidate ->
            candidate.writeBytes(ByteArray(36))
        }
    }

    @Test
    fun truncatedCiphertextFailsClosedWithoutChangingValidSnapshot() {
        assertRejectedCandidateDoesNotChangeValidSnapshot { candidate ->
            val truncated = java.io.ByteArrayOutputStream().use { bytes ->
                java.io.DataOutputStream(bytes).use { output ->
                    output.writeInt(12)
                    output.write(ByteArray(12))
                    output.writeInt(17)
                    output.write(ByteArray(16))
                }
                bytes.toByteArray()
            }
            candidate.writeBytes(truncated)
        }
    }

    private fun assertRejectedCandidateDoesNotChangeValidSnapshot(writeCandidate: (File) -> Unit) {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val directory = File(context.cacheDir, "store-hardening-${System.nanoTime()}").apply { mkdirs() }
        val validFile = File(directory, "valid.bin")
        val validStore = AndroidKeystoreProfileStore("riot-store-valid-${System.nanoTime()}", validFile)
        val original = PersistedProfile(PersistedSpace("02".repeat(32), "Preserved"), emptyList())
        validStore.save(original)
        val originalEnvelope = validFile.readBytes()

        val candidate = File(directory, "candidate.bin")
        writeCandidate(candidate)
        val candidateStore = AndroidKeystoreProfileStore("riot-store-candidate-${System.nanoTime()}", candidate)
        assertThrows(Exception::class.java) { candidateStore.load() }

        assertArrayEquals(originalEnvelope, validFile.readBytes())
        assertEquals(original, validStore.load())
        directory.deleteRecursively()
    }
}
