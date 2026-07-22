package org.riot.evidence

import java.io.File
import java.security.SecureRandom
import uniffi.riot_ffi.AlertCertainty
import uniffi.riot_ffi.AlertDraftInput
import uniffi.riot_ffi.AppExecutionSession
import uniffi.riot_ffi.AppRuntimeSession
import uniffi.riot_ffi.AlertSeverity
import uniffi.riot_ffi.AlertUrgency
import uniffi.riot_ffi.CommunityRow
import uniffi.riot_ffi.CreatedSite
import uniffi.riot_ffi.CurrentEntry
import uniffi.riot_ffi.FollowedSiteRow
import uniffi.riot_ffi.ImportSummary
import uniffi.riot_ffi.MobileImportPreview
import uniffi.riot_ffi.MobileProfile
import uniffi.riot_ffi.NewswireEditorialActionInput
import uniffi.riot_ffi.NewswireEditorialActionKind
import uniffi.riot_ffi.NewswireOperationalProfile
import uniffi.riot_ffi.NewswirePostInput
import uniffi.riot_ffi.NewswireProjectionView
import uniffi.riot_ffi.NewswireShareReference
import uniffi.riot_ffi.NewswireSignedRecord
import uniffi.riot_ffi.NewswireSpaceInput
import uniffi.riot_ffi.ProfileSession
import uniffi.riot_ffi.PublicSpace
import uniffi.riot_ffi.PublicIdentity
import uniffi.riot_ffi.ResolvedCompositeSite
import uniffi.riot_ffi.openLocalProfile
import uniffi.riot_ffi.openLocalProfileForStarterCatalogGeneration
import uniffi.riot_ffi.openLocalProfileWithDatabase
import uniffi.riot_ffi.openLocalProfileWithDatabaseForStarterCatalogGeneration
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

    /**
     * The local "store changed" signal — the Android twin of iOS's
     * `dataChangedNotification`. Fired after an accepted nearby sync is persisted.
     * The UI layer (which knows the foreground phase and the seen cursor) sets
     * this and drives `LocalNotifier.evaluate`; it is null until the
     * notifications + unread go-live PR (with `SeenCursorStore` and the newswire
     * screen) connects it. Kept as a plain callback so this controller stays free
     * of notification/UI concerns.
     */
    var onDataChanged: (() -> Unit)? = null

    init {
        val snapshot = store.load()
        profile = openProfile(snapshot)
        restore(snapshot)
    }

    fun createSpace(title: String): PublicSpace {
        val space = profile.createPublicSpace(title.trim())
        currentSpace = space
        // A truly fresh base profile records the current starter-catalog
        // generation (2 / wire v4) before its first persist.
        persisted = PersistedProfile(
            PersistedSpace(space.namespaceId, space.title),
            emptyList(),
            starterCatalogGeneration = 2,
        )
        persist(persisted!!)
        // Seal the new community's author now, minimizing the unsealed-in-RAM
        // window (Risk 13) rather than leaving it until app background.
        persistCommunities()
        return space
    }

    fun joinSpace(space: PublicSpace): PublicSpace {
        // `withWrappingKey` ALWAYS supplies the 32-byte Keystore-protected key
        // (never empty), so the join seals any displaced author INLINE and a real
        // user never reaches core's keyless unsealed-parking fallback (Risk 13).
        // The empty-key path exists only for ephemeral in-memory test profiles.
        val joined = withWrappingKey { key -> profile.joinPublicSpace(space, key) }
        currentSpace = joined
        // First-time join is a fresh base profile: record generation 2 (wire v4).
        persisted = PersistedProfile(
            PersistedSpace(joined.namespaceId, joined.title),
            emptyList(),
            starterCatalogGeneration = 2,
        )
        persist(persisted!!)
        persistCommunities()
        return joined
    }

    /**
     * Follows a SECOND community from a pasted share reference (Unit 3D — manual
     * multi-community join). Routes through the multi-community core join (parks
     * the current author, mints a fresh UNLINKABLE one) and reprojects onto the
     * joined community. Kept SEPARATE from any nearby-adopt path so that flow's
     * ownership/confirmation contract is untouched. The reference carries only
     * coordinates, so the community is "pending first sync" until its descriptor
     * and content arrive over sync. Seals immediately (Risk 13).
     */
    fun joinAdditionalCommunity(space: PublicSpace, descriptorEntryId: String): CommunityRow {
        // Adopting a SECOND community displaces the current author; the join seals
        // it INLINE under the wrapping key rather than parking it unsealed (Risk
        // 13). It joins through `joinNewswireCommunity` so the registry row CARRIES
        // the descriptor handle from the share reference (Risk 15) — otherwise it
        // is a dead follow whose Home can never reproject. Still "pending first
        // sync" until content arrives over sync.
        val joined =
            withWrappingKey { key -> profile.joinNewswireCommunity(space, descriptorEntryId, key) }
        currentSpace = joined
        // An existing-profile mutation, NOT a fresh base: read-modify-write the
        // loaded snapshot so alerts, identity, installed apps, app data, AND the
        // starter-catalog generation are preserved. Never materialize a marker on
        // a grandfathered null profile — only the space slot changes.
        mutatePersisted { snapshot ->
            snapshot.copy(space = PersistedSpace(joined.namespaceId, joined.title))
        }
        persistCommunities()
        return activeCommunity() ?: throw IllegalStateException("no active community after join")
    }

    /** Decodes a pasted `riot://newswire/join/v1/...` share reference to its
     *  namespace + descriptor + digest coordinates. Refuses a non-canonical one. */
    fun decodeShareReference(encoded: String): NewswireShareReference =
        uniffi.riot_ffi.newswireDecodeShareReference(encoded)

    fun identity(): PublicIdentity = profile.identity()

    fun openAppRuntime(): AppRuntimeSession = profile.appRuntime()

    /**
     * Open a gated execution session for a trusted app (Unit 0C). This IS the
     * launch gate — Rust refuses an untrusted app — and it captures the approval
     * generation + namespace so a later revoke / re-approval / namespace swap
     * fails the running app's next read or commit before it touches data.
     */
    fun openAppExecution(appIdHex: String): AppExecutionSession = profile.openAppExecution(appIdHex)

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

    /** Records a revoke so `restore()` does not re-trust the app. */
    fun onAppUntrusted(appId: String) =
        mutatePersistedIfPresent { recordAppUntrust(it, appId) }

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

    /**
     * The open newswire: a signed community-publishing space. These go straight
     * to [MobileProfile] (the newswire functions live there, not on the app
     * runtime session). The space descriptor's entry id is the handle every
     * later call threads through, so the UI keeps it after creating a space.
     */
    fun createNewswireSpace(
        name: String,
        summary: String,
        languages: List<String> = emptyList(),
        geographicTags: List<String> = emptyList(),
        topicTags: List<String> = emptyList(),
        editorialRoster: List<String> = emptyList(),
    ): NewswireSignedRecord {
        val record = profile.createNewswireSpace(
            NewswireSpaceInput(name, summary, languages, geographicTags, topicTags, editorialRoster),
        )
        // Seal the new community's author now (Risk 13: minimize the RAM window).
        persistCommunities()
        return record
    }

    fun createNewswirePost(
        spaceDescriptorEntryId: String,
        headline: String,
        body: String,
        language: String = "en",
        eventTimeUnixSeconds: ULong? = null,
        expiresAtUnixSeconds: ULong? = null,
        coarseLocation: String? = null,
        sourceClaims: List<String> = emptyList(),
        operationalProfile: NewswireOperationalProfile? = null,
        aiAssisted: Boolean = false,
    ): NewswireSignedRecord = profile.createNewswirePost(
        NewswirePostInput(
            spaceDescriptorEntryId,
            headline,
            body,
            language,
            eventTimeUnixSeconds,
            expiresAtUnixSeconds,
            coarseLocation,
            sourceClaims,
            operationalProfile,
            aiAssisted,
        ),
    )

    /**
     * Signs an editorial action (feature, verify, correct, hide, tombstone,
     * retract) on an existing post. Core is the authorization boundary: it REFUSES
     * to sign an action whose signer is not in the descriptor's editorial roster,
     * so this THROWS for a non-editor — UI visibility is never the gate. The
     * reason/replacement text must obey the closed field table (see
     * [EditorialActionValidator]); core validates it again.
     */
    fun createNewswireEditorialAction(
        spaceDescriptorEntryId: String,
        targetEntryId: String,
        kind: NewswireEditorialActionKind,
        reason: String?,
        correctionText: String?,
    ): NewswireSignedRecord = profile.createNewswireEditorialAction(
        NewswireEditorialActionInput(spaceDescriptorEntryId, targetEntryId, kind, reason, correctionText),
    )

    /**
     * Signs a communal reply to a post (or another reply) on the open wire. Like
     * every newswire write this goes straight to [MobileProfile]; core is the
     * authorization boundary and refuses a reply whose parent it does not hold, so
     * this THROWS for a dangling parent — the surface never re-decides admission.
     */
    fun createNewswireComment(
        spaceDescriptorEntryId: String,
        parentEntryId: String,
        body: String,
        language: String = "en",
    ): NewswireSignedRecord =
        profile.createNewswireComment(spaceDescriptorEntryId, parentEntryId, body, language)

    fun projectNewswire(spaceDescriptorEntryId: String): NewswireProjectionView =
        profile.projectNewswireSpace(spaceDescriptorEntryId)

    /**
     * Resolve a composite site rooted at [root] from this profile's synced
     * store, for rendering by [CompositeSiteReadModel.from]. A thin
     * pass-through — no wrapping key is needed, and a validation/resolution
     * problem is a STATE carried in the returned record's `degradation`,
     * never a thrown error (WU-006 Tasks 1-3, Android parity with iOS
     * `resolveCompositeSite`).
     */
    fun resolveCompositeSite(
        entryBytes: ByteArray,
        capabilityBytes: ByteArray,
        signature: ByteArray,
        payloadBytes: ByteArray,
        root: ByteArray,
        nowUnixSeconds: ULong,
    ): ResolvedCompositeSite = profile.resolveCompositeSite(
        entryBytes, capabilityBytes, signature, payloadBytes, root, nowUnixSeconds,
    )

    /**
     * Mints a fresh owned masthead namespace and seals it under this
     * profile's wrapping key, exactly as [persistCommunities] does. No
     * business logic beyond the call: the caller (the creation flow) is
     * responsible for showing the §9.3 seizure disclosure
     * ([SiteSeizureDisclosure]) and gating this call on
     * [OwnedSiteCreationGate.canMint] BEFORE invoking it — mirrors iOS
     * `ProfileRepository.createOwnedSite()` (WU-006 Task 8a, Android parity).
     * NOT wired to a reachable entry point yet (the owned masthead write path
     * is deferred to Rung 5) — a minted site is currently inert.
     */
    fun createOwnedSite(): CreatedSite = withWrappingKey { key -> uniffi.riot_ffi.createOwnedSite(key) }

    // Followed composite sites (Option C HTTP-pull). Thin passthroughs to the
    // merged core FFI — the ticket's signature/expiry and every pulled record are
    // verified in core, never here. followSite persists a Following record;
    // importFollowedSiteBundle re-verifies UNTRUSTED pulled bytes (owner cap +
    // Following-gate + family-gate) before anything lands.
    fun followSite(ticket: String): FollowedSiteRow = profile.followSite(ticket)

    fun listFollowedSites(): List<FollowedSiteRow> = profile.listFollowedSites()

    fun importFollowedSiteBundle(bytes: ByteArray, followedSiteRoot: ByteArray): ImportSummary =
        profile.importFollowedSiteBundle(bytes, followedSiteRoot)

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
        currentSpace =
            withWrappingKey { key -> profile.joinPublicSpace(snapshot.space.toPublicSpace(), key) }
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

    private fun persistAcceptedSync(bundle: ByteArray, entries: List<CurrentEntry>) {
        mutatePersisted { snapshot ->
            val prospective = mergeAcceptedSync(snapshot, bundle, entries)
            TemporaryKey.useOwned(PersistedProfileCodec.encode(prospective)) { Unit }
            prospective
        }
        // New content just landed and was persisted — surface the local signal the
        // notifier hooks. Best-effort and side-effect-only; never blocks the sync.
        onDataChanged?.invoke()
    }

    private fun openProfile(snapshot: PersistedProfile?): MobileProfile {
        // No persisted state at all: a truly fresh profile, generation 2.
        if (snapshot == null) return openLocalProfileWithDatabase(databasePath)
        val generation = snapshot.starterCatalogGeneration?.toUByte()
        // Persisted but identityless (a legacy snapshot, or one that never sealed):
        // NOT fresh — retain the exact marker via the generation-aware restore
        // rather than the fresh no-argument API, so a grandfathered null/v3 profile
        // is never silently upgraded to generation 2.
        val state = snapshot.identityState
            ?: return openLocalProfileWithDatabaseForStarterCatalogGeneration(databasePath, generation)
        return try {
            TemporaryKey.useCopy(state.wrappingKey) { temporary ->
                openProfileFromSealedIdentityWithDatabase(
                    databasePath,
                    temporary,
                    state.sealedIdentity,
                    generation,
                )
            }
        } finally {
            state.wrappingKey.fill(0)
        }
    }

    // --- Multiple communities (Unit 3) ---------------------------------------

    /** Every held community for the chooser. Reads metadata only — no unseal. */
    fun listCommunities(): List<CommunityRow> = profile.listCommunities()

    /** The active community, or null before one is chosen (returning-last-available). */
    fun activeCommunity(): CommunityRow? = profile.activeCommunity()

    /**
     * Switches the active community. Seals/unseals per-community authors, so it
     * routes through the SAME Keystore-protected wrapping key as the primary
     * sealed identity — the key is loaded transiently and zeroized after use. No
     * raw secret is exposed and no new key or store is introduced.
     */
    fun switchToCommunity(namespaceId: String): CommunityRow =
        withWrappingKey { key -> profile.switchCommunity(namespaceId, key) }

    fun archiveCommunity(namespaceId: String) = profile.archiveCommunity(namespaceId)

    fun restoreCommunity(namespaceId: String): CommunityRow =
        profile.restoreCommunity(namespaceId)

    /** Seals every held community's author so they survive a reopen. */
    fun persistCommunities() = withWrappingKey { key -> profile.persistCommunities(key) }

    fun communityRegistryQuarantined(): Boolean = profile.communityRegistryQuarantined()

    /**
     * Runs [operation] with the profile wrapping key from the Keystore-protected
     * store (minted on first use, as for identity sealing), zeroized after use.
     */
    private inline fun <T> withWrappingKey(operation: (ByteArray) -> T): T =
        synchronized(persistLock) {
            val state = store.load()?.identityState ?: createIdentityState()
            try {
                TemporaryKey.useCopy(state.wrappingKey) { key -> operation(key) }
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
