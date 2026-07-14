package org.riot.evidence

import java.io.File
import java.security.SecureRandom
import uniffi.riot_ffi.AlertCertainty
import uniffi.riot_ffi.AlertDraftInput
import uniffi.riot_ffi.AppRuntimeSession
import uniffi.riot_ffi.AlertSeverity
import uniffi.riot_ffi.AlertUrgency
import uniffi.riot_ffi.CurrentEntry
import uniffi.riot_ffi.MobileImportPreview
import uniffi.riot_ffi.MobileProfile
import uniffi.riot_ffi.ProfileSession
import uniffi.riot_ffi.PublicSpace
import uniffi.riot_ffi.PublicIdentity
import uniffi.riot_ffi.openLocalProfile
import uniffi.riot_ffi.openLocalProfileWithDatabase
import uniffi.riot_ffi.openProfileFromSealedIdentity
import uniffi.riot_ffi.openProfileFromSealedIdentityWithDatabase
import org.riot.evidence.transport.GeneratedMobileSyncBridge
import org.riot.evidence.transport.MobileSyncSessionBridge

class RiotController(filesDir: File) : AutoCloseable {
    private val databasePath = File(filesDir, "riot.db").absolutePath
    private val store = AndroidKeystoreProfileStore(
        "riot-conference-profile",
        File(filesDir, "conference-profile.bin"),
    )
    private val profile: MobileProfile
    // Mutated only through `mutatePersisted`/`mutatePersistedIfPresent`, which
    // serialize the read-modify-persist on `persistLock` so writes from the
    // WebView bridge thread (app-data puts) can't drop a concurrent UI/sync
    // write. Volatile so a reader on another thread sees the latest reference.
    @Volatile private var persisted: PersistedProfile? = null
    private val persistLock = Any()
    private var pendingPreview: MobileImportPreview? = null
    private var pendingImportBytes: ByteArray? = null

    var currentSpace: PublicSpace? = null
        private set

    init {
        val snapshot = store.load()
        profile = openProfile(snapshot)
        restore(snapshot)
    }

    fun createSpace(title: String): PublicSpace {
        val space = profile.createPublicSpace(title.trim())
        currentSpace = space
        persisted = PersistedProfile(PersistedSpace(space.namespaceId, space.title), emptyList())
        persist(persisted!!)
        return space
    }

    fun joinSpace(space: PublicSpace): PublicSpace {
        val joined = profile.joinPublicSpace(space)
        currentSpace = joined
        persisted = PersistedProfile(PersistedSpace(joined.namespaceId, joined.title), emptyList())
        persist(persisted!!)
        return joined
    }

    fun identity(): PublicIdentity = profile.identity()

    fun openAppRuntime(): AppRuntimeSession = profile.appRuntime()

    /** The persisted apps to re-admit into a fresh [RiotAppsController] on open. */
    fun installedAppsSnapshot(): List<PersistedApp> = persisted?.installedApps ?: emptyList()

    /**
     * Records a live app install so it survives a relaunch: the manifest and
     * bundle bytes are what `restore()` re-`install_app`s. No-op before a space
     * exists (apps require one). Called on the UI thread from the storefront.
     */
    fun onAppInstalled(appId: String, manifestBytes: ByteArray, bundleBytes: ByteArray) =
        mutatePersistedIfPresent { recordInstalledApp(it, appId, manifestBytes, bundleBytes) }

    /** Records a trust decision so `restore()` can re-apply it via `trust_app`. */
    fun onAppTrusted(appId: String) =
        mutatePersistedIfPresent { recordAppTrust(it, appId) }

    /**
     * Records the committed app-data bundle bytes so `restore()` can re-admit
     * them via `replay_app_data_bundle`. Runs on the WebView bridge thread, so
     * the read-modify-persist is serialized against UI/sync writers.
     */
    fun onAppDataCommitted(appId: String, key: String, bundleBytes: ByteArray) =
        mutatePersistedIfPresent { recordAppData(it, appId, key, bundleBytes) }

    /**
     * The display-name surface, for `riot.whoami()` / `riot.profile(id)`.
     *
     * Replaces the old `displayName()` placeholder (`"member-<hex>"`), which was
     * a label with nowhere for a real name to go. Real names now live in the
     * profile FFI, and an app stores the **id** and re-resolves the name at
     * render time — so a rename repairs every row that person ever touched
     * instead of leaving a snapshot behind forever.
     */
    fun profileSession(): ProfileSession = profile.profile()

    fun entries(): List<CurrentEntry> = profile.listCurrentEntries()

    fun openSyncBridge(): MobileSyncSessionBridge = GeneratedMobileSyncBridge(
        profile.openSyncSession(),
        ::persistAcceptedSync,
    )

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
        mutatePersisted { it.copy(alerts = it.alerts + signed.entry.toPersisted(signed.bundleBytes)) }
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
        // Hold the lock across the store commit and the persist so no other
        // writer can interleave between admitting the entries and saving them.
        mutatePersisted { snapshot ->
            val existingIds = snapshot.alerts.mapTo(mutableSetOf()) { it.entryId }
            val prospective = snapshot.copy(
                alerts = snapshot.alerts + entries
                    .filterNot { it.entryId in existingIds }
                    .map { it.toPersisted(bundle) },
            )
            TemporaryKey.useOwned(PersistedProfileCodec.encode(prospective)) { Unit }
            preview.createPlan(entries.map { it.entryId }).use { it.accept() }
            prospective
        }
        preview.close()
        pendingPreview = null
        pendingImportBytes = null
        return entries
    }

    override fun close() {
        pendingPreview?.close()
        pendingImportBytes = null
        profile.close()
    }

    private fun restore(snapshot: PersistedProfile?) {
        if (snapshot == null) return
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
        // Re-admit persisted app data from its committed signed bundle bytes —
        // never re-`put`, which would mint fresh entries. Installed apps and
        // trust are rebuilt separately by RiotAppsController.restore(), which
        // owns the in-memory serving store; app data lives in the willow store
        // and is independent of whether the app is installed yet.
        val runtime = profile.appRuntime()
        // Best-effort: a single unreplayable bundle (corruption, a rejected
        // path) must not crash-loop launch — skip it and keep the rest.
        snapshot.appData.forEach { data ->
            runCatching { runtime.replayAppDataBundle(data.bundleBytes) }
        }
        persisted = snapshot.copy(identityState = null)
        if (snapshot.identityState == null) {
            persist(persisted!!)
        }
    }

    private fun persistAcceptedSync(bundle: ByteArray, entries: List<CurrentEntry>) =
        mutatePersisted { snapshot ->
            val prospective = mergeAcceptedSync(snapshot, bundle, entries)
            TemporaryKey.useOwned(PersistedProfileCodec.encode(prospective)) { Unit }
            prospective
        }

    private fun openProfile(snapshot: PersistedProfile?): MobileProfile {
        val state = snapshot?.identityState ?: return openLocalProfileWithDatabase(databasePath)
        return try {
            TemporaryKey.useCopy(state.wrappingKey) { temporary ->
                openProfileFromSealedIdentityWithDatabase(databasePath, temporary, state.sealedIdentity)
            }
        } finally {
            state.wrappingKey.fill(0)
        }
    }

    /**
     * Serializes a persisted-profile read-modify-write against every other
     * writer — UI, sync, and the WebView bridge thread. [persist] re-enters the
     * same lock (reentrant). Throws if no space exists yet.
     */
    private inline fun mutatePersisted(transform: (PersistedProfile) -> PersistedProfile) {
        synchronized(persistLock) {
            val snapshot = checkNotNull(persisted) { "Create or join a public space first" }
            persist(transform(snapshot))
        }
    }

    /** As [mutatePersisted], but a no-op before a space exists (the app hooks). */
    private inline fun mutatePersistedIfPresent(transform: (PersistedProfile) -> PersistedProfile) {
        synchronized(persistLock) {
            val snapshot = persisted ?: return
            persist(transform(snapshot))
        }
    }

    private fun persist(content: PersistedProfile) {
        // Serializes concurrent saves — app-data puts arrive on the WebView
        // bridge thread while alerts/sync/trust persist from UI/sync threads.
        synchronized(persistLock) {
            val storedState = store.load()?.identityState
            val state = storedState ?: createIdentityState()
            try {
                store.save(content.copy(identityState = state))
                persisted = content.copy(identityState = null)
            } finally {
                state.wrappingKey.fill(0)
            }
        }
    }

    private fun createIdentityState(): PersistedIdentityState {
        val wrappingKey = ByteArray(PersistedProfileCodec.WRAPPING_KEY_BYTES)
        SecureRandom().nextBytes(wrappingKey)
        return try {
            val sealed = TemporaryKey.useCopy(wrappingKey) { temporary ->
                profile.sealIdentity(temporary)
            }
            check(sealed.size == PersistedProfileCodec.SEALED_IDENTITY_BYTES) {
                "Core returned an invalid sealed identity"
            }
            PersistedIdentityState(wrappingKey, sealed)
        } catch (error: Throwable) {
            wrappingKey.fill(0)
            throw error
        }
    }
}
