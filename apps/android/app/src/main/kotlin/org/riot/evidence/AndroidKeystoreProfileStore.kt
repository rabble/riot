package org.riot.evidence

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.AtomicFile
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

class AndroidKeystoreProfileStore(
    private val keyAlias: String,
    file: File,
) {
    private val atomicFile = AtomicFile(file)

    fun save(profile: PersistedProfile) {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, getOrCreateKey())
        val plaintext = PersistedProfileCodec.encode(profile)
        val ciphertext = TemporaryKey.useOwned(plaintext) { cipher.doFinal(it) }
        val envelope = ByteArrayOutputStream().use { bytes ->
            DataOutputStream(bytes).use { output ->
                output.writeInt(cipher.iv.size)
                output.write(cipher.iv)
                output.writeInt(ciphertext.size)
                output.write(ciphertext)
            }
            bytes.toByteArray()
        }

        atomicFile.baseFile.parentFile?.mkdirs()
        val stream = atomicFile.startWrite()
        try {
            stream.write(envelope)
            atomicFile.finishWrite(stream)
        } catch (error: Throwable) {
            atomicFile.failWrite(stream)
            throw error
        }
    }

    fun load(): PersistedProfile? {
        if (!atomicFile.baseFile.exists()) return null
        requireBoundedFileLength(atomicFile.baseFile.length())
        val envelope = atomicFile.readFully()
        val (iv, ciphertext) = DataInputStream(ByteArrayInputStream(envelope)).use { input ->
            val ivLength = input.readInt()
            require(ivLength in 12..32) { "invalid encrypted profile IV" }
            val iv = ByteArray(ivLength).also(input::readFully)
            val ciphertextLength = input.readInt()
            require(ciphertextLength in 16..MAX_ENCRYPTED_PROFILE_BYTES) {
                "invalid encrypted profile length"
            }
            val ciphertext = ByteArray(ciphertextLength).also(input::readFully)
            require(input.available() == 0) { "trailing encrypted profile bytes" }
            iv to ciphertext
        }
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, getOrCreateKey(), GCMParameterSpec(128, iv))
        val plaintext = cipher.doFinal(ciphertext)
        return TemporaryKey.useOwned(plaintext, PersistedProfileCodec::decode)
    }

    fun clear() {
        atomicFile.delete()
    }

    private fun getOrCreateKey(): SecretKey {
        val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        (keyStore.getKey(keyAlias, null) as? SecretKey)?.let { return it }
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE).run {
            init(
                KeyGenParameterSpec.Builder(
                    keyAlias,
                    KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
                )
                    .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                    .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                    .build(),
            )
            generateKey()
        }
    }

    companion object {
        private const val ANDROID_KEYSTORE = "AndroidKeyStore"
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
        private const val MAX_ENCRYPTED_PROFILE_BYTES = 4 * 1024 * 1024
        private const val MIN_ENCRYPTED_FILE_BYTES = 4 + 12 + 4 + 16
        private const val MAX_ENCRYPTED_FILE_BYTES = MAX_ENCRYPTED_PROFILE_BYTES + 4 + 32 + 4

        private fun requireBoundedFileLength(length: Long) {
            require(length in MIN_ENCRYPTED_FILE_BYTES.toLong()..MAX_ENCRYPTED_FILE_BYTES.toLong()) {
                "invalid encrypted profile file length"
            }
        }

        internal fun requireBoundedFileLengthForTest(length: Long) = requireBoundedFileLength(length)
    }
}
