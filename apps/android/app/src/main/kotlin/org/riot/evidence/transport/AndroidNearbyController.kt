package org.riot.evidence.transport

import android.annotation.SuppressLint
import android.app.Activity
import android.content.pm.PackageManager
import kotlin.concurrent.thread
import java.util.concurrent.atomic.AtomicBoolean

class AndroidNearbyController(
    private val activity: Activity,
    private val onChanged: () -> Unit,
    private val onConnected: (DiscoveredPhone, NearbyConnection, Boolean) -> Unit,
) : AutoCloseable {
    private val discovery by lazy { AndroidBleDiscovery(activity) }
    private val pairing by lazy { PairingController(AndroidGattBleConnector(activity)) }
    private val selector = SessionTransportSelector(SocketLocalIpConnector())
    private val incomingGate = IncomingPairingGate()
    private val winner = NearbyConnectionWinner()
    private val pairingInProgress = AtomicBoolean(false)
    private val attempts = PairingAttemptArbiter()
    private var listener: LocalIpListener? = null
    private var server: AndroidGattServer? = null
    private var outgoingPhone: DiscoveredPhone? = null
    private var incomingPhone: DiscoveredPhone? = null

    val phones = mutableListOf<DiscoveredPhone>()
    var state: NearbyUiState = NearbyUiState.Idle
        private set

    fun findNearby() {
        winner.close()
        attempts.reset()
        pairingInProgress.set(false)
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
        pairingInProgress.set(false)
        outgoingPhone = null
        incomingPhone = null
        update(NearbyUiState.Idle)
    }

    fun confirmPairing() {
        if (!pairingInProgress.compareAndSet(false, true)) return
        val incoming = incomingPhone
        val outgoing = outgoingPhone
        val phone = incoming ?: outgoing ?: run {
            pairingInProgress.set(false)
            update(NearbyUiState.Failed)
            return
        }
        connectConfirmedPairing(phone, incoming != null)
    }

    private fun connectConfirmedPairing(phone: DiscoveredPhone, incoming: Boolean) {
        val attempt = attempts.begin()
        update(NearbyUiState.Connecting)
        thread(name = "riot-nearby-connect", isDaemon = true) {
            try {
                val selected = if (incoming) {
                    incomingGate.confirm()
                    val ble = checkNotNull(server).confirm(phone.handle)
                    val acceptedLocal = listener?.awaitAccepted(INCOMING_LOCAL_WAIT_MILLIS)
                    listener?.close()
                    listener = null
                    if (acceptedLocal != null) {
                        ble.channel.close()
                        NearbyConnection(acceptedLocal, TransportKind.LOCAL_IP)
                    } else {
                        NearbyConnection(ble.channel, TransportKind.BLE)
                    }
                } else {
                    selector.select(pairing.confirm()).also {
                        listener?.close()
                        listener = null
                    }
                }
                if (!winner.claim(selected)) {
                    attempts.failed(attempt)
                    return@thread
                }
                attempts.succeeded(attempt)
                runCatching(discovery::close)
                if (!incoming || selected.kind == TransportKind.LOCAL_IP) {
                    runCatching { server?.close() }
                    server = null
                }
                activity.runOnUiThread {
                    update(NearbyUiState.Connecting)
                    onConnected(phone, selected, incoming)
                }
            } catch (_: Exception) {
                if (attempts.failed(attempt) && !winner.hasWinner) {
                    pairingInProgress.set(false)
                    activity.runOnUiThread { update(NearbyUiState.Failed) }
                }
            }
        }
    }

    override fun close() {
        runCatching(discovery::close)
        runCatching { server?.close() }
        runCatching { listener?.close() }
        winner.close()
        attempts.reset()
    }

    @SuppressLint("MissingPermission")
    private fun startDiscovery() {
        runCatching { server?.close() }
        runCatching { listener?.close() }
        server = null
        listener = null
        phones.clear()
        update(NearbyUiState.Looking)
        try {
            listener = runCatching(LocalIpListener::forDevice).getOrNull()
            server = AndroidGattServer(activity, listener?.endpoint) { phone, link ->
                activity.runOnUiThread {
                    val outgoing = outgoingPhone
                    if (pairingInProgress.get() && outgoing != null &&
                        PairingRoleSelector.matchesConfirmedPhone(outgoing, phone)
                    ) {
                        val role = runCatching {
                            PairingRoleSelector.choose(
                                concurrentIncoming = true,
                                localToken = discovery.pairingToken,
                                remoteToken = phone.pairingToken,
                            )
                        }.getOrNull()
                        if (role != PairingRole.ACCEPT_INCOMING) {
                            server?.reject(phone.handle)
                            return@runOnUiThread
                        }
                        incomingPhone = phone
                        incomingGate.request(phone, link)
                        connectConfirmedPairing(phone, incoming = true)
                    } else if (pairingInProgress.get()) {
                        server?.reject(phone.handle)
                    } else {
                        outgoingPhone = null
                        incomingPhone = phone
                        incomingGate.request(phone, link)
                        update(incomingGate.state)
                    }
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
