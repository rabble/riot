package org.riot.evidence.apps

/** One served resource: an in-bundle path, its content type, and its bytes. */
data class AppResource(val path: String, val contentType: String, val bytes: ByteArray) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is AppResource) return false
        return path == other.path &&
            contentType == other.contentType &&
            bytes.contentEquals(other.bytes)
    }

    override fun hashCode(): Int {
        var result = path.hashCode()
        result = 31 * result + contentType.hashCode()
        result = 31 * result + bytes.contentHashCode()
        return result
    }
}

/** A decoded app bundle: a primary entry point plus its resource list. */
data class DecodedAppBundle(val entryPoint: String, val resources: List<AppResource>)

class AppBundleCodecException(message: String) : Exception(message)

/**
 * Strict Kotlin mirror of `crates/riot-core/src/apps/bundle.rs`: the same
 * map/key layout (`map(2){0: entry_point, 1: [map(3){0: path, 1:
 * content_type, 2: bytes}]}`), the same bounds, and the same canonicality
 * proof (decode re-encodes and compares byte-for-byte).
 *
 * This is a *serving* mirror, not a security boundary: production only ever
 * decodes bytes that Rust's `install_app` already accepted. Any drift from
 * Rust surfaces as a loud install failure, never as a silently divergent
 * decode.
 */
object AppBundleCodec {
    const val MAX_BUNDLE_RESOURCES = 32
    const val MAX_RESOURCE_PATH_BYTES = 256
    const val MAX_RESOURCE_CONTENT_TYPE_BYTES = 64
    const val MAX_BUNDLE_TOTAL_BYTES = 1_048_576

    fun encode(bundle: DecodedAppBundle): ByteArray {
        validate(bundle)

        val writer = CborWriter()
        writer.map(2)
        writer.uint(0).text(bundle.entryPoint)
        writer.uint(1).array(bundle.resources.size)
        for (resource in bundle.resources) {
            writer.map(3)
            writer.uint(0).text(resource.path)
            writer.uint(1).text(resource.contentType)
            writer.uint(2).bytes(resource.bytes)
        }

        val encoded = writer.toByteArray()
        if (encoded.size > MAX_BUNDLE_TOTAL_BYTES) {
            throw AppBundleCodecException("encoded bundle exceeds size limit")
        }
        return encoded
    }

    fun decode(input: ByteArray): DecodedAppBundle {
        if (input.size > MAX_BUNDLE_TOTAL_BYTES) {
            throw AppBundleCodecException("bundle exceeds size limit")
        }

        val reader = CborReader(input)
        reader.expectMap(2)

        reader.expectUInt(0)
        val entryPoint = reader.readText(MAX_RESOURCE_PATH_BYTES)

        reader.expectUInt(1)
        val resourceCount = reader.readArrayHeader()
        if (resourceCount == 0L || resourceCount > MAX_BUNDLE_RESOURCES.toLong()) {
            throw AppBundleCodecException("resource count out of bounds")
        }

        val resources = ArrayList<AppResource>(resourceCount.toInt())
        repeat(resourceCount.toInt()) {
            reader.expectMap(3)

            reader.expectUInt(0)
            val path = reader.readText(MAX_RESOURCE_PATH_BYTES)

            reader.expectUInt(1)
            val contentType = reader.readText(MAX_RESOURCE_CONTENT_TYPE_BYTES)

            reader.expectUInt(2)
            val bytes = reader.readBytes(MAX_BUNDLE_TOTAL_BYTES)

            resources.add(AppResource(path, contentType, bytes))
        }

        if (reader.position != reader.size) {
            throw AppBundleCodecException("trailing bytes after bundle")
        }

        val bundle = DecodedAppBundle(entryPoint, resources)
        validate(bundle)

        // Canonicality proof: only the exact encoder output is acceptable.
        if (!encode(bundle).contentEquals(input)) {
            throw AppBundleCodecException("non-canonical bundle encoding")
        }
        return bundle
    }

    private fun validate(bundle: DecodedAppBundle) {
        if (bundle.resources.isEmpty() || bundle.resources.size > MAX_BUNDLE_RESOURCES) {
            throw AppBundleCodecException("resource count out of bounds")
        }

        var totalBytes = 0L
        var entryPointFound = false
        for (resource in bundle.resources) {
            if (resource.path.isEmpty() ||
                resource.path.toByteArray(Charsets.UTF_8).size > MAX_RESOURCE_PATH_BYTES
            ) {
                throw AppBundleCodecException("resource path out of bounds")
            }
            if (resource.contentType.isEmpty() ||
                resource.contentType.toByteArray(Charsets.UTF_8).size > MAX_RESOURCE_CONTENT_TYPE_BYTES
            ) {
                throw AppBundleCodecException("resource content type out of bounds")
            }
            if (resource.path == bundle.entryPoint) {
                entryPointFound = true
            }
            totalBytes += resource.bytes.size.toLong()
        }

        if (!entryPointFound) {
            throw AppBundleCodecException("entry point not present among resources")
        }
        if (totalBytes > MAX_BUNDLE_TOTAL_BYTES) {
            throw AppBundleCodecException("bundle exceeds size limit")
        }
    }
}
