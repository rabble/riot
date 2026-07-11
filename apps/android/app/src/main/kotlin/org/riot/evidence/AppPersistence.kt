package org.riot.evidence

/**
 * Pure state transitions for the app-persistence fields of [PersistedProfile]
 * and the restore that replays them. These have no Android or FFI types so
 * they run entirely in JVM tests, the same shape as [mergeAcceptedSync].
 *
 * The invariant the restore path enforces is the one the spec is strict
 * about: on reopen, app data is re-admitted from its committed signed bundle
 * bytes via `replay_app_data_bundle` — never re-`put`, which would mint fresh
 * signed entries and diverge across synced devices. [AppRestorePort]
 * deliberately exposes no `put`, so the type system makes a re-put
 * unexpressible.
 */

/** Records a freshly installed app, replacing any prior copy for the same id. */
internal fun recordInstalledApp(
    profile: PersistedProfile,
    appId: String,
    manifestBytes: ByteArray,
    bundleBytes: ByteArray,
): PersistedProfile {
    val existing = profile.installedApps.firstOrNull { it.appId == appId }
    val app = PersistedApp(
        appId = appId,
        manifestBytes = manifestBytes.copyOf(),
        bundleBytes = bundleBytes.copyOf(),
        // Re-installing an already-held app keeps its trust decision.
        trusted = existing?.trusted ?: false,
    )
    return profile.copy(
        installedApps = profile.installedApps.filterNot { it.appId == appId } + app,
    )
}

/** Marks a held app trusted; a no-op if the app is not persisted. */
internal fun recordAppTrust(profile: PersistedProfile, appId: String): PersistedProfile =
    profile.copy(
        installedApps = profile.installedApps.map { app ->
            if (app.appId == appId && !app.trusted) app.copy(trusted = true) else app
        },
    )

/**
 * Records a committed app-data write, keeping only the latest bundle per
 * `(appId, key)` so growth is bounded by the number of live keys.
 */
internal fun recordAppData(
    profile: PersistedProfile,
    appId: String,
    key: String,
    bundleBytes: ByteArray,
): PersistedProfile {
    val entry = PersistedAppData(appId, key, bundleBytes.copyOf())
    return profile.copy(
        appData = profile.appData.filterNot { it.appId == appId && it.key == key } + entry,
    )
}

/**
 * The restore surface. `install`/`trust` re-establish the in-memory and Rust
 * session state; `replayAppData` re-admits a committed bundle. There is no
 * `put`: replay is the only way app data comes back.
 */
interface AppRestorePort {
    fun install(appId: String, manifestBytes: ByteArray, bundleBytes: ByteArray)
    fun trust(appId: String)
    fun replayAppData(bundleBytes: ByteArray)
}

/** Replays persisted apps and app data through [port] in a fixed order. */
internal fun restoreApps(profile: PersistedProfile, port: AppRestorePort) {
    profile.installedApps.forEach { app ->
        port.install(app.appId, app.manifestBytes, app.bundleBytes)
        if (app.trusted) port.trust(app.appId)
    }
    profile.appData.forEach { data -> port.replayAppData(data.bundleBytes) }
}
