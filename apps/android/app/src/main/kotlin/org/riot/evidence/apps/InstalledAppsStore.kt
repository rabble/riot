package org.riot.evidence.apps

import uniffi.riot_ffi.InstalledAppRecord

/** An installed app: Rust's display record plus the serving bundle. */
data class InstalledApp(val record: InstalledAppRecord, val bundle: DecodedAppBundle)

/**
 * In-memory registry of installed apps, keyed by content-derived app id.
 * Rust records the ids; retaining the decoded bundle here is the documented
 * stopgap until `app_resource` lands (spec, gated task).
 */
class InstalledAppsStore {
    private val apps = LinkedHashMap<String, InstalledApp>()

    fun register(record: InstalledAppRecord, bundle: DecodedAppBundle): InstalledApp =
        InstalledApp(record, bundle).also { apps[record.appId] = it }

    fun all(): List<InstalledApp> = apps.values.toList()

    fun find(appIdHex: String): InstalledApp? = apps[appIdHex]
}
