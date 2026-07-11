package org.riot.evidence.transport

import android.annotation.SuppressLint
import android.app.Activity
import android.content.pm.PackageManager
import kotlin.concurrent.thread

class AndroidNearbyController(
    private val activity: Activity,
    private val onChanged: () -> Unit,
    private val onConnected: (DiscoveredPhone, NearbyConnection) -> Unit,
) : AutoCloseable {
    private val discovery by lazy { AndroidBleDiscovery(activity) }
    private val pairing by lazy { PairingController(AndroidGattBleConnector(activity)) }
    private val selector = SessionTransportSelector(SocketLocalIpConnector())
    private val incomingGate = IncomingPairingGate()
    private var connection: NearbyConnection? = null
    private var listener: LocalIpListener? = null
    private var server: AndroidGattServer? = null
    private var outgoingPhone: DiscoveredPhone? = null
    private var incomingPhone: DiscoveredPhone? = null

    val phones = mutableListOf<DiscoveredPhone>()
    var state: NearbyUiState = NearbyUiState.Idle
        private set

    fun findNearby() {
        val missing = NearbyPermissions.runtimePermissions().filter {
            activity.checkSelfPermission(it) != PackageManager.PERMISSION_GRANTED
        }
        if (missing.isNotEmpty()) {
            activity.requestPermissions(missing.toTypedArray(), PERMISSION_REQUEST)
            return
        }
        startDiscovery()
    }

    @SuppressLint("MissingPermission")
    fun permissionResult(granted: Boolean) {
        if (granted) startDiscovery() else update(NearbyUiState.Failed)
    }

    fun select(phone: DiscoveredPhone) {
        outgoingPhone = phone
        incomingPhone = null
        pairing.select(phone)
        update(pairing.state)
    }

    fun cancelPairing() {
        incomingPhone?.let { server?.reject(it.handle) }
        incomingGate.reject()
        pairing.cancel()
        outgoingPhone = null
        incomingPhone = null
        update(NearbyUiState.Idle)
    }

    fun confirmPairing() {
        update(NearbyUiState.Connecting)
        thread(name = "riot-nearby-connect", isDaemon = true) {
            try {
                val phone = incomingPhone ?: outgoingPhone
                    ?: throw IllegalStateException("Nearby phone disappeared")
                val selected = if (incomingGate.hasPendingRequest) {
                    incomingGate.confirm()
                    val ble = checkNotNull(server).confirm(phone.handle)
                    val acceptedLocal = listener?.awaitAccepted(INCOMING_LOCAL_WAIT_MILLIS)
                    if (acceptedLocal != null) {
                        NearbyConnection(acceptedLocal, TransportKind.LOCAL_IP)
                    } else {
                        NearbyConnection(ble.channel, TransportKind.BLE)
                    }
                } else {
                    selector.select(pairing.confirm())
                }
                connection = selected
                activity.runOnUiThread {
                    update(NearbyUiState.Connecting)
                    onConnected(phone, selected)
                }
            } catch (_: Exception) {
                activity.runOnUiThread { update(NearbyUiState.Failed) }
            }
        }
    }

    override fun close() {
        runCatching(discovery::close)
        runCatching { server?.close() }
        runCatching { listener?.close() }
        connection?.disconnect()
        connection = null
    }

    @SuppressLint("MissingPermission")
    private fun startDiscovery() {
        phones.clear()
        update(NearbyUiState.Looking)
        try {
            listener = runCatching(LocalIpListener::forDevice).getOrNull()
            server = AndroidGattServer(activity, listener?.endpoint) { phone, link ->
                activity.runOnUiThread {
                    outgoingPhone = null
                    incomingPhone = phone
                    incomingGate.request(phone, link)
                    update(incomingGate.state)
                }
            }
            discovery.start(
                onPhoneFound = { phone ->
                    activity.runOnUiThread {
                        phones += phone
                        server?.phoneDiscovered(phone)
                        onChanged()
                    }
                },
                onFailure = { activity.runOnUiThread { update(NearbyUiState.Failed) } },
            )
        } catch (_: Exception) {
            update(NearbyUiState.Failed)
        }
    }

    private fun update(next: NearbyUiState) {
        state = next
        onChanged()
    }

    companion object {
        const val PERMISSION_REQUEST = 2105
        const val INCOMING_LOCAL_WAIT_MILLIS = 2_000L
    }
}
