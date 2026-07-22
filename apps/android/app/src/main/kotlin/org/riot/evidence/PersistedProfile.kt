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

/**
 * A signed JS app held by this profile: the exact manifest and bundle bytes
 * Rust admitted, so `restore()` can re-`install_app` them into a fresh
 * session, plus the profile-local trust decision (re-applied via `trust_app`,
 * which is in-memory in Rust and mints no signed entry — see the iOS
 * `trustedAppIDs` precedent).
 */
data class PersistedApp(
    val appId: String,
    val manifestBytes: ByteArray,
    val bundleBytes: ByteArray,
    val trusted: Boolean,
)

/**
 * A committed app-data write kept as the canonical signed bundle bytes that
 * `app_data_put_with_receipt` returned. `restore()` re-admits these via
 * `replay_app_data_bundle` — never by re-`put`-ing, which would mint fresh
 * signed entries and diverge across synced devices. Latest bundle per
 * `(appId, key)` supersedes, bounding growth to the number of live keys.
 */
data class PersistedAppData(
    val appId: String,
    val key: String,
    val bundleBytes: ByteArray,
)

data class PersistedProfile(
    val space: PersistedSpace,
    val alerts: List<PersistedAlert>,
    val identityState: PersistedIdentityState? = null,
    val installedApps: List<PersistedApp> = emptyList(),
    val appData: List<PersistedAppData> = emptyList(),
    // The starter-catalog generation this profile was created under. `null` is
    // the zero-byte durable encoding of generation 1 (a legacy profile predating
    // this marker, kept as wire v3); a fresh profile records `2` (wire v4).
    // Trailing + defaulted so positional legacy constructors stay source-compatible.
    val starterCatalogGeneration: Int? = null,
)

data class PersistedIdentityState(
    val wrappingKey: ByteArray,
    val sealedIdentity: ByteArray,
)

object PersistedProfileCodec {
    const val MAX_ENCODED_BYTES = 4 * 1024 * 1024 - 64
    private const val MAGIC = 0x52494f54
    // The highest wire version this codec understands. A profile is written at the
    // LOWEST version that can represent it: a null-marker profile stays v3
    // (byte-identical to before this field existed), a generation-bearing profile
    // is v4. Decode accepts 1..VERSION.
    private const val VERSION = 4
    private const val VERSION_WITH_IDENTITY = 2
    private const val VERSION_WITH_APPS = 3
    private const val VERSION_WITH_STARTER_CATALOG_GENERATION = 4
    // The version a null-marker (generation-1) profile is written at, so a
    // grandfathered profile never grows a marker or a version bump.
    private const val LEGACY_WRITABLE_VERSION = 3
    private const val MIN_VERSION = 1
    const val WRAPPING_KEY_BYTES = 32
    const val SEALED_IDENTITY_BYTES = 112
    private const val MAX_ALERTS = 256
    private const val MAX_STRING_BYTES = 16 * 1024
    private const val MAX_BUNDLE_BYTES = 2 * 1024 * 1024
    const val MAX_INSTALLED_APPS = 32
    const val MAX_APP_DATA_ENTRIES = 512
    private const val MAX_APP_MANIFEST_BYTES = 4 * 1024
    private const val MAX_APP_BUNDLE_BYTES = 1 * 1024 * 1024
    private const val MAX_APP_DATA_BUNDLE_BYTES = MAX_BUNDLE_BYTES

    fun encode(profile: PersistedProfile): ByteArray = encodeInternal(profile, {}, {})

    internal fun encodeWithHooksForTest(
        profile: PersistedProfile,
        onStreamAllocated: () -> Unit = {},
        afterCopy: (ByteArray) -> Unit = {},
    ): ByteArray = encodeInternal(profile, onStreamAllocated, afterCopy)

    private fun encodeInternal(
        profile: PersistedProfile,
        onStreamAllocated: () -> Unit,
        afterCopy: (ByteArray) -> Unit,
    ): ByteArray {
        val expectedSize = encodedSize(profile)
        // A null-marker profile is written at the legacy v3 representation so it
        // stays byte-identical to a pre-marker snapshot; only a generation-bearing
        // profile advances to v4 and carries the trailing marker.
        val wireVersion = if (profile.starterCatalogGeneration == null) {
            LEGACY_WRITABLE_VERSION
        } else {
            VERSION_WITH_STARTER_CATALOG_GENERATION
        }
        var encoded: ByteArray? = null
        try {
            onStreamAllocated()
            encoded = WipingByteArrayOutputStream(expectedSize).use { bytes ->
                val output = DataOutputStream(bytes)
                output.writeInt(MAGIC)
                output.writeInt(wireVersion)
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
                output.writeInt(profile.installedApps.size)
                profile.installedApps.forEach { app ->
                    output.writeString(app.appId)
                    output.writeBytes(app.manifestBytes)
                    output.writeBytes(app.bundleBytes)
                    output.writeBoolean(app.trusted)
                }
                output.writeInt(profile.appData.size)
                profile.appData.forEach { data ->
                    output.writeString(data.appId)
                    output.writeString(data.key)
                    output.writeBytes(data.bundleBytes)
                }
                // v4 only: the trailing 32-bit starter-catalog generation.
                profile.starterCatalogGeneration?.let(output::writeInt)
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
                require(version in MIN_VERSION..VERSION) { "unsupported profile version" }
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
                val identityState = if (version >= VERSION_WITH_IDENTITY && input.readBoolean()) {
                    val wrappingKey = ByteArray(WRAPPING_KEY_BYTES)
                    pendingIdentityKey = wrappingKey
                    input.readFully(wrappingKey)
                    val sealedIdentity = ByteArray(SEALED_IDENTITY_BYTES).also(input::readFully)
                    PersistedIdentityState(wrappingKey, sealedIdentity)
                } else {
                    null
                }
                val installedApps = if (version >= VERSION_WITH_APPS) {
                    val appCount = input.readInt()
                    require(appCount in 0..MAX_INSTALLED_APPS) { "invalid persisted app count" }
                    List(appCount) {
                        val appId = input.readString()
                        val manifest = input.readBytes(MAX_APP_MANIFEST_BYTES)
                        val bundle = input.readBytes(MAX_APP_BUNDLE_BYTES)
                        val trusted = input.readBoolean()
                        PersistedApp(appId, manifest, bundle, trusted)
                    }
                } else {
                    emptyList()
                }
                val appData = if (version >= VERSION_WITH_APPS) {
                    val dataCount = input.readInt()
                    require(dataCount in 0..MAX_APP_DATA_ENTRIES) { "invalid persisted app-data count" }
                    List(dataCount) {
                        val appId = input.readString()
                        val key = input.readString()
                        val bundle = input.readBytes(MAX_APP_DATA_BUNDLE_BYTES)
                        PersistedAppData(appId, key, bundle)
                    }
                } else {
                    emptyList()
                }
                val starterCatalogGeneration =
                    if (version >= VERSION_WITH_STARTER_CATALOG_GENERATION) {
                        val generation = input.readInt()
                        require(generation == 1 || generation == 2) {
                            "invalid starter-catalog generation"
                        }
                        generation
                    } else {
                        null
                    }
                require(input.available() == 0) { "trailing persisted profile bytes" }
                val result = PersistedProfile(
                    space,
                    alerts,
                    identityState,
                    installedApps,
                    appData,
                    starterCatalogGeneration,
                )
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

    /**
     * The exact number of bytes [encode] will produce for [profile], computed
     * without allocating the stream. This is the production preflight WU-002c
     * runs (while holding the authority/persistence lock) before a durable
     * mutation: it validates every field against its ceiling — including the
     * starter-catalog marker's membership — and enforces [MAX_ENCODED_BYTES], so
     * a prospective over-limit profile is rejected before any allocation. It must
     * stay the SAME function [encodeInternal] calls, so a prospective size can
     * never disagree with the actual encoding.
     */
    internal fun encodedSize(profile: PersistedProfile): Int {
        require(profile.alerts.size <= MAX_ALERTS) { "too many persisted alerts" }
        require(profile.installedApps.size <= MAX_INSTALLED_APPS) { "too many persisted apps" }
        require(profile.appData.size <= MAX_APP_DATA_ENTRIES) { "too many persisted app-data entries" }
        profile.starterCatalogGeneration?.let { generation ->
            require(generation == 1 || generation == 2) { "invalid starter-catalog generation" }
        }
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
        fun addBytes(value: ByteArray, max: Int) {
            require(value.size <= max) { "persisted byte field is too large" }
            add(Int.SIZE_BYTES)
            add(value.size)
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
        add(Int.SIZE_BYTES)
        profile.installedApps.forEach { app ->
            addString(app.appId)
            addBytes(app.manifestBytes, MAX_APP_MANIFEST_BYTES)
            addBytes(app.bundleBytes, MAX_APP_BUNDLE_BYTES)
            add(1)
        }
        add(Int.SIZE_BYTES)
        profile.appData.forEach { data ->
            addString(data.appId)
            addString(data.key)
            addBytes(data.bundleBytes, MAX_APP_DATA_BUNDLE_BYTES)
        }
        // v4 only: the trailing 32-bit generation marker.
        if (profile.starterCatalogGeneration != null) add(Int.SIZE_BYTES)
        return total.toInt()
    }

    private fun DataOutputStream.writeString(value: String) {
        val encoded = value.toByteArray(Charsets.UTF_8)
        require(encoded.size <= MAX_STRING_BYTES) { "persisted string is too large" }
        writeInt(encoded.size)
        write(encoded)
    }

    private fun DataOutputStream.writeBytes(value: ByteArray) {
        writeInt(value.size)
        write(value)
    }

    private fun DataInputStream.readString(): String {
        val length = readInt()
        require(length in 0..MAX_STRING_BYTES) { "invalid persisted string length" }
        val encoded = ByteArray(length)
        readFully(encoded)
        return encoded.toString(Charsets.UTF_8)
    }

    private fun DataInputStream.readBytes(max: Int): ByteArray {
        val length = readInt()
        require(length in 0..max) { "invalid persisted byte-field length" }
        val bytes = ByteArray(length)
        readFully(bytes)
        return bytes
    }
}
