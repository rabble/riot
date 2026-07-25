package net.protest.riot

/**
 * A place to keep one secret, encrypted independently of the profile blob.
 *
 * The seam exists so the split logic below is testable on the host JVM, which
 * has no AndroidKeyStore. Production uses [KeystoreSecretStore].
 */
interface SecretStore {
    fun read(): ByteArray?

    fun write(secret: ByteArray)

    fun clear()
}

/** The sealed identity cannot be opened because its wrapping key is gone. */
class MissingWrappingKeyException(message: String) : IllegalStateException(message)

/**
 * Keeps the identity's wrapping key OUT of the persisted profile blob.
 *
 * WHY: `PersistedProfile` used to carry `wrappingKey` and `sealedIdentity`
 * adjacently, and the whole blob was encrypted under a single Keystore key. The
 * XChaCha20Poly1305 seal therefore bought no defence in depth on Android —
 * whoever could decrypt the blob got the key AND the thing it unseals in one
 * read. iOS never had this shape: there the key lives in the Keychain and the
 * sealed identity in a file, so an attacker needs both stores.
 *
 * The key now lives in its own file under its own Keystore alias, and the blob
 * carries [MOVED_SENTINEL] in the key's place.
 *
 * WHY A SENTINEL RATHER THAN A WIRE-FORMAT CHANGE: the sentinel is exactly
 * `WRAPPING_KEY_BYTES` long, so the encoding is byte-for-byte the shape every
 * released version already reads and writes. No version bump, no size
 * accounting change, no new decode branch — and therefore no way for this
 * change to make an existing profile undecodable. The migration is data, not
 * format.
 */
object IdentitySecretSplit {
    /**
     * What the blob carries where the key used to be. All zeroes: never a real
     * key in practice, and if one were ever generated it would be rejected as
     * unusable rather than silently accepted.
     */
    val MOVED_SENTINEL = ByteArray(PersistedProfileCodec.WRAPPING_KEY_BYTES)

    /** True when this profile still carries a real key inline (a pre-split install). */
    fun needsRewrite(profile: PersistedProfile): Boolean {
        val identity = profile.identityState ?: return false
        return !identity.wrappingKey.contentEquals(MOVED_SENTINEL)
    }

    /**
     * Writes the wrapping key to [keys] and returns the profile to encode — the
     * one whose identity block holds the sentinel instead of the key.
     */
    fun forStorage(profile: PersistedProfile, keys: SecretStore): PersistedProfile {
        val identity = profile.identityState ?: return profile
        if (!identity.wrappingKey.contentEquals(MOVED_SENTINEL)) {
            keys.write(identity.wrappingKey)
        }
        return profile.copy(
            identityState =
                PersistedIdentityState(MOVED_SENTINEL.copyOf(), identity.sealedIdentity))
    }

    /**
     * Returns the profile the app should use, with the real key put back from
     * [keys].
     *
     * A pre-split profile still carries its key inline: that key is relocated
     * into [keys] here so the very first load after upgrading is already
     * migrated, and [needsRewrite] tells the caller to re-save so the blob stops
     * carrying it.
     */
    fun fromStorage(profile: PersistedProfile, keys: SecretStore): PersistedProfile {
        val identity = profile.identityState ?: return profile
        if (!identity.wrappingKey.contentEquals(MOVED_SENTINEL)) {
            // Legacy inline key — migrate it out, but keep serving it so this
            // load succeeds.
            keys.write(identity.wrappingKey)
            return profile
        }
        val key =
            keys.read()
                ?: throw MissingWrappingKeyException(
                    "the identity wrapping key is absent from the secure store; the sealed " +
                        "identity cannot be opened")
        require(key.size == PersistedProfileCodec.WRAPPING_KEY_BYTES) {
            "stored wrapping key has the wrong length"
        }
        return profile.copy(
            identityState = PersistedIdentityState(key, identity.sealedIdentity))
    }
}
