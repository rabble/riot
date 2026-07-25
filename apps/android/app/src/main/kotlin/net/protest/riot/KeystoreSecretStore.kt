package net.protest.riot

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

/**
 * One small secret, in its own file, encrypted under its OWN AndroidKeyStore
 * alias.
 *
 * The separate alias is the entire point: if the identity wrapping key were
 * encrypted under the same Keystore key as the profile blob, splitting the two
 * files would buy nothing. Two aliases means an attacker who can use one
 * Keystore key still cannot read the other file.
 *
 * Deliberately not a general-purpose store: it holds a single fixed-size
 * secret, so there is no length field to get wrong and no framing to attack.
 */
class KeystoreSecretStore(
    private val keyAlias: String,
    file: File,
    private val expectedBytes: Int = PersistedProfileCodec.WRAPPING_KEY_BYTES,
) : SecretStore {
    private val atomicFile = AtomicFile(file)

    override fun read(): ByteArray? {
        if (!atomicFile.baseFile.exists()) return null
        val envelope = atomicFile.readFully()
        val (iv, ciphertext) =
            DataInputStream(ByteArrayInputStream(envelope)).use { input ->
                val ivLength = input.readInt()
                require(ivLength in 12..32) { "invalid secret IV" }
                val iv = ByteArray(ivLength).also(input::readFully)
                val ciphertextLength = input.readInt()
                require(ciphertextLength in 16..MAX_SECRET_ENVELOPE_BYTES) {
                    "invalid secret length"
                }
                val ciphertext = ByteArray(ciphertextLength).also(input::readFully)
                require(input.available() == 0) { "trailing secret bytes" }
                iv to ciphertext
            }
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, getOrCreateKey(), GCMParameterSpec(128, iv))
        val plaintext = cipher.doFinal(ciphertext)
        require(plaintext.size == expectedBytes) { "stored secret has the wrong length" }
        return plaintext
    }

    override fun write(secret: ByteArray) {
        require(secret.size == expectedBytes) { "refusing to store a wrong-length secret" }
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, getOrCreateKey())
        val ciphertext = cipher.doFinal(secret)
        val envelope =
            ByteArrayOutputStream().use { bytes ->
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

    /**
     * Deletes the file AND the Keystore key. Dropping the key is the part that
     * matters: it makes any copy of the file that escaped deletion (a backup, an
     * un-TRIMmed block) permanently undecryptable.
     */
    override fun clear() {
        atomicFile.delete()
        runCatching {
            KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }.deleteEntry(keyAlias)
        }
    }

    private fun getOrCreateKey(): SecretKey {
        val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        (keyStore.getKey(keyAlias, null) as? SecretKey)?.let { return it }
        // StrongBox puts the key in dedicated tamper-resistant hardware where
        // available. It is requested first and retried without on the many
        // devices that lack it — StrongBoxUnavailableException is thrown at
        // generate() time, not at build() time, so the retry has to wrap the
        // generation itself.
        return runCatching { generateKey(strongBox = true) }
            .getOrElse { generateKey(strongBox = false) }
    }

    private fun generateKey(strongBox: Boolean): SecretKey =
        KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE).run {
            val spec =
                KeyGenParameterSpec.Builder(
                        keyAlias,
                        KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
                    )
                    .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                    .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.P) {
                // The key is unusable while the device is locked. This matches
                // iOS, whose Keychain item uses
                // kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly — also
                // unlocked-only. It is safe HERE, and only here, because the
                // wrapping key is needed to seal or unseal the IDENTITY, not to
                // read or write entries: background sync keeps working on a
                // locked phone. The same flag on the profile-blob key would
                // break exactly that, which is why this class holds one small
                // secret rather than the whole profile.
                spec.setUnlockedDeviceRequired(true)
                if (strongBox) spec.setIsStrongBoxBacked(true)
            }
            init(spec.build())
            generateKey()
        }

    companion object {
        private const val ANDROID_KEYSTORE = "AndroidKeyStore"
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
        private const val MAX_SECRET_ENVELOPE_BYTES = 1024
    }
}
