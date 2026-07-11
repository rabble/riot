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

/**
 * Thin adapter; prefix scoping, value caps, and key validation live in Rust.
 *
 * `put` uses the receipt-returning FFI variant so the host can persist the
 * committed signed bundle bytes and replay them on the next open — `onCommitted`
 * hands those bytes to the persistence layer. A persistence failure surfaces to
 * the caller (the bridge reports "couldn't save"), matching how the alert and
 * trust paths propagate a failed durable write.
 */
class UniffiAppDataPort(
    private val session: AppRuntimeSession,
    private val appIdHex: String,
    private val onCommitted: (key: String, bundleBytes: ByteArray) -> Unit = { _, _ -> },
) : AppDataPort {
    override fun put(key: String, value: ByteArray) {
        val bundle = session.appDataPutWithReceipt(appIdHex, key, value)
        onCommitted(key, bundle)
    }

    override fun get(key: String): ByteArray? = session.appDataGet(appIdHex, key)
    override fun list(prefix: String): List<Pair<String, ByteArray>> =
        session.appDataList(appIdHex, prefix).map { it.key to it.value }
}
