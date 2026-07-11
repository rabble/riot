package org.riot.evidence.apps

import uniffi.riot_ffi.AppRuntimeSession

/** Install/trust/launch decisions for signed JS apps in this profile. */
class RiotAppsController(
    private val session: AppRuntimeSession,
    // Persistence hooks: fired only on live user actions so RiotController can
    // record the manifest/bundle bytes and trust decision. Left as no-ops in
    // JVM tests and during restore (see [restore], which must not re-persist).
    private val onInstalled: (appId: String, manifestBytes: ByteArray, bundleBytes: ByteArray) -> Unit = { _, _, _ -> },
    private val onTrusted: (appId: String) -> Unit = {},
) : InstalledAppsAccess {
    private val store = InstalledAppsStore()

    /**
     * Rust is the integrity oracle: `installApp` must accept the bytes
     * before the Kotlin serving-decode ever runs. In-memory retention is
     * the documented stopgap until `app_resource` lands (spec, gated).
     */
    override fun install(manifestBytes: ByteArray, bundleBytes: ByteArray): InstalledApp {
        val app = admit(manifestBytes, bundleBytes)
        onInstalled(app.record.appId, manifestBytes, bundleBytes)
        return app
    }

    fun apps(): List<InstalledApp> = store.all()

    /** The installed tool for a content-derived app id (hex), or null. */
    override fun find(appIdHex: String): InstalledApp? = store.find(appIdHex)

    fun isTrusted(app: InstalledApp): Boolean = session.isAppTrusted(app.record.appId)

    fun trust(app: InstalledApp) {
        session.trustApp(app.record.appId)
        onTrusted(app.record.appId)
    }

    /**
     * Re-admits apps persisted across a relaunch: the same `install_app` +
     * serving-decode as a live install, plus re-applying the persisted trust
     * decision (`trust_app` is in-memory in Rust and mints no entry). This
     * path fires no persistence hooks — the state it rebuilds is already on
     * disk, and re-persisting would be wasteful and could reorder entries.
     */
    fun restore(apps: List<org.riot.evidence.PersistedApp>) {
        apps.forEach { app ->
            admit(app.manifestBytes, app.bundleBytes)
            if (app.trusted) session.trustApp(app.appId)
        }
    }

    private fun admit(manifestBytes: ByteArray, bundleBytes: ByteArray): InstalledApp {
        val record = session.installApp(manifestBytes, bundleBytes)
        val bundle = AppBundleCodec.decode(bundleBytes)
        check(bundle.entryPoint == record.entryPoint) { "That file isn't a Riot tool" }
        return store.register(record, bundle)
    }

    /**
     * The launch gate. Rust does not gate data calls — this is the
     * enforcement point (platform handoff, deferred item 3).
     */
    fun requireTrusted(app: InstalledApp): InstalledApp {
        check(isTrusted(app)) { "Ask an organizer to turn this on" }
        return app
    }
}
