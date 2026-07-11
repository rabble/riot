package org.riot.evidence

import java.io.File
import uniffi.riot_ffi.AlertCertainty
import uniffi.riot_ffi.AlertDraftInput
import uniffi.riot_ffi.AlertSeverity
import uniffi.riot_ffi.AlertUrgency
import uniffi.riot_ffi.CurrentEntry
import uniffi.riot_ffi.MobileImportPreview
import uniffi.riot_ffi.MobileProfile
import uniffi.riot_ffi.PublicSpace
import uniffi.riot_ffi.openLocalProfile

class RiotController(filesDir: File) : AutoCloseable {
    private val profile: MobileProfile = openLocalProfile()
    private val store = AndroidKeystoreProfileStore(
        "riot-conference-profile",
        File(filesDir, "conference-profile.bin"),
    )
    private var persisted: PersistedProfile? = null
    private var pendingPreview: MobileImportPreview? = null
    private var pendingImportBytes: ByteArray? = null

    var currentSpace: PublicSpace? = null
        private set

    init {
        restore()
    }

    fun createSpace(title: String): PublicSpace {
        val space = profile.createPublicSpace(title.trim())
        currentSpace = space
        persisted = PersistedProfile(PersistedSpace(space.namespaceId, space.title), emptyList())
        store.save(persisted!!)
        return space
    }

    fun joinSpace(space: PublicSpace): PublicSpace {
        val joined = profile.joinPublicSpace(space)
        currentSpace = joined
        persisted = PersistedProfile(PersistedSpace(joined.namespaceId, joined.title), emptyList())
        store.save(persisted!!)
        return joined
    }

    fun entries(): List<CurrentEntry> = profile.listCurrentEntries()

    fun createAndSignAlert(headline: String, description: String, aiAssisted: Boolean): CurrentEntry {
        check(currentSpace != null) { "Create or join a public space first" }
        val now = System.currentTimeMillis().toULong() / 1_000UL
        val draft = profile.createDraftAlert(
            AlertDraftInput(
                validFrom = now,
                expiresAt = now + 86_400UL,
                language = "en",
                urgency = AlertUrgency.EXPECTED,
                severity = AlertSeverity.MODERATE,
                certainty = AlertCertainty.LIKELY,
                headline = headline.trim(),
                description = description.trim(),
                affectedAreaClaim = null,
                sourceClaims = listOf("local author"),
                aiAssisted = aiAssisted,
            ),
        )
        val signed = profile.signDraft(draft.draftId)
        val snapshot = checkNotNull(persisted)
        persisted = snapshot.copy(alerts = snapshot.alerts + signed.entry.toPersisted(signed.bundleBytes))
        store.save(persisted!!)
        return signed.entry
    }

    fun previewImport(bytes: ByteArray): List<CurrentEntry> {
        require(bytes.size <= PersistedProfileCodec.MAX_ENCODED_BYTES) { "selected bundle is too large" }
        pendingPreview?.close()
        pendingImportBytes = null
        pendingPreview = profile.inspectBytes(bytes, "android://document-picker")
        pendingImportBytes = bytes.copyOf()
        return pendingPreview!!.eligibleEntries()
    }

    fun acceptPreview(): List<CurrentEntry> {
        val preview = checkNotNull(pendingPreview) { "Select and preview a signed bundle first" }
        val bundle = checkNotNull(pendingImportBytes) { "Selected bundle is no longer available" }
        val entries = preview.eligibleEntries()
        val snapshot = checkNotNull(persisted) { "Create or join a public space first" }
        val existingIds = snapshot.alerts.mapTo(mutableSetOf()) { it.entryId }
        val prospective = snapshot.copy(
            alerts = snapshot.alerts + entries
                .filterNot { it.entryId in existingIds }
                .map { it.toPersisted(bundle) },
        )
        PersistedProfileCodec.encode(prospective)
        preview.createPlan(entries.map { it.entryId }).use { it.accept() }
        preview.close()
        pendingPreview = null
        pendingImportBytes = null
        persisted = prospective
        store.save(prospective)
        return entries
    }

    override fun close() {
        pendingPreview?.close()
        pendingImportBytes = null
        profile.close()
    }

    private fun restore() {
        val snapshot = store.load() ?: return
        currentSpace = profile.joinPublicSpace(snapshot.space.toPublicSpace())
        val restoredBundles = mutableListOf<ByteArray>()
        snapshot.alerts.forEach { alert ->
            if (restoredBundles.any { it.contentEquals(alert.bundleBytes) }) return@forEach
            profile.inspectBytes(alert.bundleBytes, "android://encrypted-profile").use { preview ->
                val ids = preview.eligibleEntries().map { it.entryId }
                preview.createPlan(ids).use { it.accept() }
            }
            restoredBundles += alert.bundleBytes
        }
        persisted = snapshot
    }
}
