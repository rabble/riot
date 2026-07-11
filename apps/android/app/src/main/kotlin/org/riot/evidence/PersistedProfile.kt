package org.riot.evidence

import java.io.ByteArrayInputStream
import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.EOFException

data class PersistedSpace(
    val namespaceId: String,
    val title: String,
)

data class PersistedAlert(
    val entryId: String,
    val namespaceId: String,
    val signerId: String,
    val headline: String,
    val createdAt: Long,
    val validFrom: Long?,
    val expiresAt: Long,
    val aiAssisted: Boolean,
    val bundleBytes: ByteArray,
)

data class PersistedProfile(
    val space: PersistedSpace,
    val alerts: List<PersistedAlert>,
    val identityState: PersistedIdentityState? = null,
)

data class PersistedIdentityState(
    val wrappingKey: ByteArray,
    val sealedIdentity: ByteArray,
)

object PersistedProfileCodec {
    const val MAX_ENCODED_BYTES = 4 * 1024 * 1024 - 64
    private const val MAGIC = 0x52494f54
    private const val VERSION = 2
    private const val LEGACY_VERSION = 1
    const val WRAPPING_KEY_BYTES = 32
    const val SEALED_IDENTITY_BYTES = 112
    private const val MAX_ALERTS = 256
    private const val MAX_STRING_BYTES = 16 * 1024
    private const val MAX_BUNDLE_BYTES = 2 * 1024 * 1024

    fun encode(profile: PersistedProfile): ByteArray = encodeInternal(profile, {}, {})

    internal fun encodeWithHooksForTest(
        profile: PersistedProfile,
        onStreamAllocated: () -> Unit = {},
        afterCopy: (ByteArray) -> Unit = {},
    ): ByteArray = encodeInternal(profile, onStreamAllocated, afterCopy)

    internal fun encodedSizeForTest(profile: PersistedProfile): Int = encodedSize(profile)

    private fun encodeInternal(
        profile: PersistedProfile,
        onStreamAllocated: () -> Unit,
        afterCopy: (ByteArray) -> Unit,
    ): ByteArray {
        val expectedSize = encodedSize(profile)
        var encoded: ByteArray? = null
        try {
            onStreamAllocated()
            encoded = WipingByteArrayOutputStream(expectedSize).use { bytes ->
                val output = DataOutputStream(bytes)
                output.writeInt(MAGIC)
                output.writeInt(VERSION)
                output.writeString(profile.space.namespaceId)
                output.writeString(profile.space.title)
                output.writeInt(profile.alerts.size)
                profile.alerts.forEach { alert ->
                    output.writeString(alert.entryId)
                    output.writeString(alert.namespaceId)
                    output.writeString(alert.signerId)
                    output.writeString(alert.headline)
                    output.writeLong(alert.createdAt)
                    output.writeBoolean(alert.validFrom != null)
                    alert.validFrom?.let(output::writeLong)
                    output.writeLong(alert.expiresAt)
                    output.writeBoolean(alert.aiAssisted)
                    output.writeInt(alert.bundleBytes.size)
                    output.write(alert.bundleBytes)
                }
                output.writeBoolean(profile.identityState != null)
                profile.identityState?.let { identity ->
                    output.write(identity.wrappingKey)
                    output.write(identity.sealedIdentity)
                }
                output.flush()
                bytes.toByteArray()
            }
            check(encoded.size == expectedSize) { "persisted profile size changed during encoding" }
            afterCopy(encoded)
            return encoded
        } catch (error: Throwable) {
            encoded?.fill(0)
            throw error
        }
    }

    fun decode(bytes: ByteArray): PersistedProfile = decodeInternal(bytes, null)

    internal fun decodeWithIdentityKeyObserverForTest(
        bytes: ByteArray,
        onRejectedIdentityKey: (ByteArray) -> Unit,
    ): PersistedProfile = decodeInternal(bytes, onRejectedIdentityKey)

    private fun decodeInternal(
        bytes: ByteArray,
        onRejectedIdentityKey: ((ByteArray) -> Unit)?,
    ): PersistedProfile {
        require(bytes.size <= MAX_ENCODED_BYTES) { "persisted profile is too large" }
        var pendingIdentityKey: ByteArray? = null
        try {
            return DataInputStream(ByteArrayInputStream(bytes)).use { input ->
                require(input.readInt() == MAGIC) { "invalid profile header" }
                val version = input.readInt()
                require(version == LEGACY_VERSION || version == VERSION) { "unsupported profile version" }
                val space = PersistedSpace(input.readString(), input.readString())
                val count = input.readInt()
                require(count in 0..MAX_ALERTS) { "invalid persisted alert count" }
                val alerts = List(count) {
                    val entryId = input.readString()
                    val namespaceId = input.readString()
                    val signerId = input.readString()
                    val headline = input.readString()
                    val createdAt = input.readLong()
                    val validFrom = if (input.readBoolean()) input.readLong() else null
                    val expiresAt = input.readLong()
                    val aiAssisted = input.readBoolean()
                    val bundleLength = input.readInt()
                    require(bundleLength in 0..MAX_BUNDLE_BYTES) { "invalid bundle length" }
                    val bundle = ByteArray(bundleLength)
                    input.readFully(bundle)
                    PersistedAlert(
                        entryId,
                        namespaceId,
                        signerId,
                        headline,
                        createdAt,
                        validFrom,
                        expiresAt,
                        aiAssisted,
                        bundle,
                    )
                }
                val identityState = if (version == VERSION && input.readBoolean()) {
                    val wrappingKey = ByteArray(WRAPPING_KEY_BYTES)
                    pendingIdentityKey = wrappingKey
                    input.readFully(wrappingKey)
                    val sealedIdentity = ByteArray(SEALED_IDENTITY_BYTES).also(input::readFully)
                    PersistedIdentityState(wrappingKey, sealedIdentity)
                } else {
                    null
                }
                require(input.available() == 0) { "trailing persisted profile bytes" }
                val result = PersistedProfile(space, alerts, identityState)
                pendingIdentityKey = null
                result
            }
        } catch (error: EOFException) {
            throw IllegalArgumentException("truncated persisted profile", error)
        } finally {
            pendingIdentityKey?.let { key ->
                key.fill(0)
                onRejectedIdentityKey?.invoke(key)
            }
        }
    }

    private fun encodedSize(profile: PersistedProfile): Int {
        require(profile.alerts.size <= MAX_ALERTS) { "too many persisted alerts" }
        profile.identityState?.let { identity ->
            require(identity.wrappingKey.size == WRAPPING_KEY_BYTES) {
                "invalid identity wrapping key length"
            }
            require(identity.sealedIdentity.size == SEALED_IDENTITY_BYTES) {
                "invalid sealed identity length"
            }
        }
        var total = 0L
        fun add(bytes: Int) {
            total += bytes.toLong()
            require(total <= MAX_ENCODED_BYTES) { "persisted profile is too large" }
        }
        fun addString(value: String) {
            val size = value.toByteArray(Charsets.UTF_8).size
            require(size <= MAX_STRING_BYTES) { "persisted string is too large" }
            add(Int.SIZE_BYTES)
            add(size)
        }

        add(Int.SIZE_BYTES * 2)
        addString(profile.space.namespaceId)
        addString(profile.space.title)
        add(Int.SIZE_BYTES)
        profile.alerts.forEach { alert ->
            addString(alert.entryId)
            addString(alert.namespaceId)
            addString(alert.signerId)
            addString(alert.headline)
            add(Long.SIZE_BYTES)
            add(1)
            if (alert.validFrom != null) add(Long.SIZE_BYTES)
            add(Long.SIZE_BYTES)
            add(1)
            require(alert.bundleBytes.size <= MAX_BUNDLE_BYTES) { "bundle is too large" }
            add(Int.SIZE_BYTES)
            add(alert.bundleBytes.size)
        }
        add(1)
        if (profile.identityState != null) add(WRAPPING_KEY_BYTES + SEALED_IDENTITY_BYTES)
        return total.toInt()
    }

    private fun DataOutputStream.writeString(value: String) {
        val encoded = value.toByteArray(Charsets.UTF_8)
        require(encoded.size <= MAX_STRING_BYTES) { "persisted string is too large" }
        writeInt(encoded.size)
        write(encoded)
    }

    private fun DataInputStream.readString(): String {
        val length = readInt()
        require(length in 0..MAX_STRING_BYTES) { "invalid persisted string length" }
        val encoded = ByteArray(length)
        readFully(encoded)
        return encoded.toString(Charsets.UTF_8)
    }
}
