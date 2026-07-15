package org.riot.evidence.apps

import uniffi.riot_ffi.AppRuntimeSessionInterface
import uniffi.riot_ffi.InstalledAppRecord

/**
 * Install/trust/launch decisions for signed JS apps in this profile.
 *
 * Takes the session as its INTERFACE, not the concrete UniFFI object, so the
 * trust decisions below can be driven in JVM tests with no FFI behind them —
 * the same shape as [DirectoryController]'s ports.
 */
class RiotAppsController(
    private val session: AppRuntimeSessionInterface,
    // Persistence hooks: fired only on live user actions so RiotController can
    // record the manifest/bundle bytes and trust decision. Left as no-ops in
    // JVM tests and during restore (see [restore], which must not re-persist).
    private val onInstalled: (appId: String, manifestBytes: ByteArray, bundleBytes: ByteArray) -> Unit = { _, _, _ -> },
    private val onTrusted: (appId: String) -> Unit = {},
    private val onUntrusted: (appId: String) -> Unit = {},
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

    /**
     * Takes up an app that arrived from a neighbour. A carried app has no file
     * on this side to install from — the store holds the only copy — so Rust
     * admits it from its own verified bytes and hands both halves back in one
     * read: the bundle to serve its pages from, the manifest to re-admit it on
     * the next launch. That is why this fires the same persistence hook a
     * hand-picked install does; without it the app would be gone by morning.
     *
     * Getting an app trusts nothing. It lands untrusted like any other and
     * still has to pass review before it can open.
     */
    override fun getFromDirectory(appIdBytes: ByteArray): InstalledApp {
        val record = session.installFromDirectory(appIdBytes)
        val pair = session.appPairBytes(appIdBytes)
        val app = register(record, pair.bundleBytes)
        onInstalled(app.record.appId, pair.manifestBytes, pair.bundleBytes)
        return app
    }

    fun apps(): List<InstalledApp> = store.all()

    /** The installed tool for a content-derived app id (hex), or null. */
    override fun find(appIdHex: String): InstalledApp? = store.find(appIdHex)

    fun isTrusted(app: InstalledApp): Boolean = session.isAppTrusted(app.record.appId)

    /** Whether this profile is the organizer of the current space (the trust gate). */
    fun isOrganizer(): Boolean = session.isOrganizer()

    fun trust(app: InstalledApp) {
        session.trustApp(app.record.appId)
        onTrusted(app.record.appId)
    }

    /**
     * Revokes trust in Rust and records the decision so `restore()` does not
     * re-trust the app. Mirrors [trust]: Rust first, persistence second.
     */
    fun untrust(app: InstalledApp) {
        session.untrustApp(app.record.appId)
        onUntrusted(app.record.appId)
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

    private fun admit(manifestBytes: ByteArray, bundleBytes: ByteArray): InstalledApp =
        register(session.installApp(manifestBytes, bundleBytes), bundleBytes)

    /**
     * The serving-decode, shared by every way an app gets here: Rust has
     * already accepted the bytes as an app (it is the integrity oracle), so
     * all that is left is to decode the bundle this side will serve from and
     * check it agrees with Rust about the entry point.
     */
    private fun register(record: InstalledAppRecord, bundleBytes: ByteArray): InstalledApp {
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
