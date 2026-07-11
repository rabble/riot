package org.riot.evidence.apps

import android.content.res.AssetManager

/**
 * Reads the built-in checklist's packed manifest+bundle from app assets — the
 * same `.cbor` files the core embeds, shipped via the main sourceSet's assets
 * dir (see build.gradle.kts). Missing/unreadable assets return null rather
 * than throwing: the built-in still lists (it's compiled into the core), it
 * just can't be opened until its bytes are present.
 */
class AssetStarterCatalog(private val assets: AssetManager) : StarterCatalog {
    override fun read(): Pair<ByteArray, ByteArray>? = try {
        val manifest = assets.open(MANIFEST_ASSET).use { it.readBytes() }
        val bundle = assets.open(BUNDLE_ASSET).use { it.readBytes() }
        manifest to bundle
    } catch (error: Exception) {
        null
    }

    private companion object {
        const val MANIFEST_ASSET = "checklist.manifest.cbor"
        const val BUNDLE_ASSET = "checklist.bundle.cbor"
    }
}
