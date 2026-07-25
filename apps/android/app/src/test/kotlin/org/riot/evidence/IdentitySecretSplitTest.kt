package org.riot.evidence

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/** In-memory stand-in for the Keystore-backed secret file (host JVM has no AndroidKeyStore). */
private class FakeSecretStore(private var secret: ByteArray? = null) : SecretStore {
    var writes = 0
        private set

    override fun read(): ByteArray? = secret?.copyOf()

    override fun write(secret: ByteArray) {
        writes += 1
        this.secret = secret.copyOf()
    }

    override fun clear() {
        secret = null
    }
}

private fun profileWithIdentity(key: ByteArray, sealed: ByteArray) =
    PersistedProfile(
        space = PersistedSpace("ns", "Title"),
        alerts = emptyList(),
        identityState = PersistedIdentityState(key, sealed),
    )

class IdentitySecretSplitTest {
    private val realKey = ByteArray(PersistedProfileCodec.WRAPPING_KEY_BYTES) { (it + 1).toByte() }
    private val sealed = ByteArray(PersistedProfileCodec.SEALED_IDENTITY_BYTES) { 9 }

    /**
     * The point of the whole change: what gets encoded into the profile blob must
     * NOT contain the wrapping key. Storing the key beside the thing it unseals
     * means one compromised file yields both halves.
     */
    @Test
    fun `forStorage moves the key out of the blob and into the secret store`() {
        val keys = FakeSecretStore()
        val stored = IdentitySecretSplit.forStorage(profileWithIdentity(realKey, sealed), keys)

        assertArrayEquals("the key belongs in its own store", realKey, keys.read())
        val identity = stored.identityState
        assertNotNull(identity)
        assertArrayEquals(
            "the blob must carry the sentinel, never the real key",
            IdentitySecretSplit.MOVED_SENTINEL,
            identity!!.wrappingKey,
        )
        assertFalse(
            "the real key must not reach the blob",
            identity.wrappingKey.contentEquals(realKey),
        )
        assertArrayEquals(
            "the sealed identity still travels", sealed, identity.sealedIdentity)
    }

    /** Round trip: what the store wrote is what the app gets back. */
    @Test
    fun `fromStorage restores the key from the secret store`() {
        val keys = FakeSecretStore()
        val stored = IdentitySecretSplit.forStorage(profileWithIdentity(realKey, sealed), keys)
        val loaded = IdentitySecretSplit.fromStorage(stored, keys)

        val identity = loaded.identityState
        assertNotNull(identity)
        assertArrayEquals(realKey, identity!!.wrappingKey)
        assertArrayEquals(sealed, identity.sealedIdentity)
    }

    /**
     * MIGRATION — the brick-risk case. An install from before this change has the
     * real key inline and nothing in the secret store. It must keep working AND
     * relocate the key, not lose it.
     */
    @Test
    fun `fromStorage migrates a legacy inline key into the secret store`() {
        val keys = FakeSecretStore()
        val legacy = profileWithIdentity(realKey, sealed)

        val loaded = IdentitySecretSplit.fromStorage(legacy, keys)

        assertArrayEquals(
            "the identity still opens", realKey, loaded.identityState!!.wrappingKey)
        assertArrayEquals("and the key has been relocated", realKey, keys.read())
        assertEquals(1, keys.writes)
        assertTrue(
            "the caller is told to re-save so the blob stops carrying the key",
            IdentitySecretSplit.needsRewrite(legacy),
        )
    }

    /** An already-migrated profile must not be pointlessly rewritten on every load. */
    @Test
    fun `an already migrated profile needs no rewrite`() {
        val keys = FakeSecretStore()
        val stored = IdentitySecretSplit.forStorage(profileWithIdentity(realKey, sealed), keys)
        assertFalse(IdentitySecretSplit.needsRewrite(stored))
    }

    /**
     * The unrecoverable case must be loud. A sentinel blob with an empty secret
     * store means the key is gone (wiped, or a restored backup without the
     * Keystore). Returning the sentinel as if it were a key would hand
     * XChaCha20Poly1305 an all-zero key and silently produce garbage.
     */
    @Test
    fun `fromStorage refuses a sentinel blob when the secret store is empty`() {
        val keys = FakeSecretStore()
        val stored = IdentitySecretSplit.forStorage(profileWithIdentity(realKey, sealed), keys)
        keys.clear()

        assertThrows(MissingWrappingKeyException::class.java) {
            IdentitySecretSplit.fromStorage(stored, keys)
        }
    }

    /** A profile with no identity at all touches the secret store not at all. */
    @Test
    fun `a profile without an identity is passed through untouched`() {
        val keys = FakeSecretStore()
        val bare = PersistedProfile(space = PersistedSpace("ns", "Title"), alerts = emptyList())

        assertNull(IdentitySecretSplit.forStorage(bare, keys).identityState)
        assertNull(IdentitySecretSplit.fromStorage(bare, keys).identityState)
        assertEquals(0, keys.writes)
        assertFalse(IdentitySecretSplit.needsRewrite(bare))
    }

    /**
     * The sentinel is what distinguishes "migrated" from "legacy", so its shape is
     * part of the contract: 32 zero bytes, the same length as a real key so the
     * wire format is unchanged and no version bump is needed.
     */
    @Test
    fun `the sentinel is a full length run of zeroes`() {
        assertEquals(
            PersistedProfileCodec.WRAPPING_KEY_BYTES, IdentitySecretSplit.MOVED_SENTINEL.size)
        assertTrue(IdentitySecretSplit.MOVED_SENTINEL.all { it == 0.toByte() })
    }
}
