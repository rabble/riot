package org.riot.evidence.apps

import android.content.Context
import org.json.JSONObject

/**
 * Packs the committed `fixtures/apps/checklist/` sources (frozen
 * content-hash inputs — never edit) into canonical manifest/bundle bytes.
 * `install_app`'s strict Rust decoder is the oracle proving this Kotlin
 * encoding stays byte-exact with `manifest.rs`/`bundle.rs`; packed `.cbor`
 * artifacts replace this packer when the directory plan's CLI lands
 * (gated Task 7).
 */
object ChecklistFixture {
    // Fixed public placeholder author (never key material; install_app
    // verifies content, not authorship) — conference-fixture precedent.
    private val NAMESPACE_ID = ByteArray(32) { 0x11 }
    private val SUBSPACE_ID = ByteArray(32) { 0x22 }
    private val SIGNING_KEY_ID = ByteArray(32) { 0x33 }

    private val CONTENT_TYPES = mapOf(
        "index.html" to "text/html",
        "app.js" to "text/javascript",
        "style.css" to "text/css",
    )

    fun manifestBytes(context: Context): ByteArray {
        val source = JSONObject(readAsset(context, "checklist/riot-app.json").decodeToString())
        // map(9), integer keys 0..8 in order — exactly
        // crates/riot-core/src/apps/manifest.rs::encode_manifest.
        val writer = CborWriter()
        writer.map(9)
        writer.uint(0).text(source.getString("name"))
        writer.uint(1).text(source.getString("description"))
        writer.uint(2).text(source.getString("version"))
        writer.uint(3).bytes(NAMESPACE_ID)
        writer.uint(4).bytes(SUBSPACE_ID)
        writer.uint(5).uint(0) // NamespaceKind::Communal
        writer.uint(6).bytes(SIGNING_KEY_ID)
        val permissions = source.getJSONArray("permissions")
        writer.uint(7).array(permissions.length())
        (0 until permissions.length()).forEach { writer.text(permissions.getString(it)) }
        writer.uint(8).text(source.getString("entry_point"))
        return writer.toByteArray()
    }

    fun bundleBytes(context: Context): ByteArray {
        val resources = CONTENT_TYPES.entries
            .map { (name, contentType) ->
                AppResource(name, contentType, readAsset(context, "checklist/$name"))
            }
            .sortedBy { it.path }
        return AppBundleCodec.encode(DecodedAppBundle("index.html", resources))
    }

    private fun readAsset(context: Context, path: String): ByteArray =
        context.assets.open(path).use { it.readBytes() }
}
