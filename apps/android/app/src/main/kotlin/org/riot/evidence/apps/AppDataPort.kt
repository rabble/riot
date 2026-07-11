package org.riot.evidence.apps

import uniffi.riot_ffi.AppRuntimeSession

/**
 * The bridge's only I/O surface. Kept as a plain interface so JVM tests
 * drive the bridge without Android or the FFI. The real security boundary
 * — prefix scoping, value caps, key validation — lives in Rust's
 * `AppDataBridge`, not here.
 */
interface AppDataPort {
    fun put(key: String, value: ByteArray)
    fun get(key: String): ByteArray?
    fun list(prefix: String): List<Pair<String, ByteArray>>
}

/** Thin adapter; prefix scoping, value caps, and key validation live in Rust. */
class UniffiAppDataPort(
    private val session: AppRuntimeSession,
    private val appIdHex: String,
) : AppDataPort {
    override fun put(key: String, value: ByteArray) = session.appDataPut(appIdHex, key, value)
    override fun get(key: String): ByteArray? = session.appDataGet(appIdHex, key)
    override fun list(prefix: String): List<Pair<String, ByteArray>> =
        session.appDataList(appIdHex, prefix).map { it.key to it.value }
}
