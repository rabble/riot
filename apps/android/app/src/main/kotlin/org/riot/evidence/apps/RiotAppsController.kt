package org.riot.evidence.apps

import uniffi.riot_ffi.AppRuntimeSession

/** Install/trust/launch decisions for signed JS apps in this profile. */
class RiotAppsController(private val session: AppRuntimeSession) {
    private val store = InstalledAppsStore()

    /**
     * Rust is the integrity oracle: `installApp` must accept the bytes
     * before the Kotlin serving-decode ever runs. In-memory retention is
     * the documented stopgap until `app_resource` lands (spec, gated).
     */
    fun install(manifestBytes: ByteArray, bundleBytes: ByteArray): InstalledApp {
        val record = session.installApp(manifestBytes, bundleBytes)
        val bundle = AppBundleCodec.decode(bundleBytes)
        check(bundle.entryPoint == record.entryPoint) { "That file isn't a Riot tool" }
        return store.register(record, bundle)
    }

    fun apps(): List<InstalledApp> = store.all()

    fun isTrusted(app: InstalledApp): Boolean = session.isAppTrusted(app.record.appId)

    fun trust(app: InstalledApp) = session.trustApp(app.record.appId)

    /**
     * The launch gate. Rust does not gate data calls — this is the
     * enforcement point (platform handoff, deferred item 3).
     */
    fun requireTrusted(app: InstalledApp): InstalledApp {
        check(isTrusted(app)) { "Ask an organizer to turn this on" }
        return app
    }
}
