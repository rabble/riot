package org.riot.evidence

import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
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
)

object PersistedProfileCodec {
    const val MAX_ENCODED_BYTES = 4 * 1024 * 1024 - 64
    private const val MAGIC = 0x52494f54
    private const val VERSION = 1
    private const val MAX_ALERTS = 256
    private const val MAX_STRING_BYTES = 16 * 1024
    private const val MAX_BUNDLE_BYTES = 2 * 1024 * 1024

    fun encode(profile: PersistedProfile): ByteArray {
        require(profile.alerts.size <= MAX_ALERTS) { "too many persisted alerts" }
        val encoded = ByteArrayOutputStream().use { bytes ->
            DataOutputStream(bytes).use { output ->
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
                    require(alert.bundleBytes.size <= MAX_BUNDLE_BYTES) { "bundle is too large" }
                    output.writeInt(alert.bundleBytes.size)
                    output.write(alert.bundleBytes)
                }
            }
            bytes.toByteArray()
        }
        require(encoded.size <= MAX_ENCODED_BYTES) { "persisted profile is too large" }
        return encoded
    }

    fun decode(bytes: ByteArray): PersistedProfile = try {
        DataInputStream(ByteArrayInputStream(bytes)).use { input ->
            require(input.readInt() == MAGIC) { "invalid profile header" }
            require(input.readInt() == VERSION) { "unsupported profile version" }
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
            require(input.available() == 0) { "trailing persisted profile bytes" }
            PersistedProfile(space, alerts)
        }
    } catch (error: EOFException) {
        throw IllegalArgumentException("truncated persisted profile", error)
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
