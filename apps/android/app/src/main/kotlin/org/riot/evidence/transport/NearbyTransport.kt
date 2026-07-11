package org.riot.evidence.transport

import java.io.IOException
import java.security.SecureRandom

const val MAX_SYNC_FRAME_BYTES = 8_388_608 + 128

@JvmInline
value class PeerHandle internal constructor(internal val value: String)

data class DiscoveredPhone(
    val friendlyName: String,
    val handle: PeerHandle,
    val pairingToken: Int = 0,
)
data class LocalEndpoint(val host: String, val port: Int) {
    init {
        require(host.isNotBlank()) { "local endpoint host is empty" }
        require(port in 1..65_535) { "local endpoint port is invalid" }
    }
}

enum class TransportKind { LOCAL_IP, BLE }

enum class PairingRole { INITIATE, ACCEPT_INCOMING }

object PairingRoleSelector {
    fun matchesConfirmedPhone(confirmed: DiscoveredPhone, incoming: DiscoveredPhone): Boolean =
        confirmed.handle == incoming.handle && confirmed.pairingToken == incoming.pairingToken

    fun choose(concurrentIncoming: Boolean, localToken: Int, remoteToken: Int): PairingRole {
        require(localToken in 0..65_535 && remoteToken in 0..65_535)
        if (!concurrentIncoming) return PairingRole.INITIATE
        require(localToken != remoteToken) { "Nearby pairing tokens collided; find nearby again" }
        return if (localToken < remoteToken) PairingRole.INITIATE else PairingRole.ACCEPT_INCOMING
    }
}

class PairingAttemptArbiter {
    private val active = mutableSetOf<Long>()
    private var nextId = 0L
    private var settled = false

    @Synchronized
    fun begin(): Long = (++nextId).also(active::add)

    @Synchronized
    fun succeeded(attempt: Long) {
        if (active.remove(attempt)) settled = true
    }

    @Synchronized
    fun failed(attempt: Long): Boolean {
        if (!active.remove(attempt)) return false
        return active.isEmpty() && !settled
    }

    @Synchronized
    fun reset() {
        active.clear()
        settled = false
    }
}

enum class ConfirmationDecision { RETRY, READY, FAILED }

class RemoteConfirmationWait(private val maxAttempts: Int) {
    private var attempts = 0
    var isConfirmed = false
        private set

    init {
        require(maxAttempts > 0)
    }

    fun beginAttempt(): Boolean {
        if (isConfirmed || attempts >= maxAttempts) return false
        attempts += 1
        return true
    }

    fun completeAttempt(confirmed: Boolean): ConfirmationDecision {
        if (confirmed) {
            isConfirmed = true
            return ConfirmationDecision.READY
        }
        return if (attempts >= maxAttempts) ConfirmationDecision.FAILED else ConfirmationDecision.RETRY
    }
}

interface FrameChannel : AutoCloseable {
    @Throws(IOException::class)
    fun send(frame: ByteArray)
    fun onReceive(receiver: (ByteArray) -> Unit)
    override fun close()
}

interface LocalIpConnector {
    fun connect(endpoint: LocalEndpoint): FrameChannel?
}

interface BleConnector {
    fun connect(handle: PeerHandle): PairedBleLink
}

data class PairedBleLink(val channel: FrameChannel, val localEndpoint: LocalEndpoint?)

class NearbyConnection internal constructor(
    private val channel: FrameChannel,
    val kind: TransportKind,
) : AutoCloseable {
    @Volatile
    private var open = true

    fun send(frame: ByteArray) {
        require(frame.size <= MAX_SYNC_FRAME_BYTES) { "frame is too large" }
        if (!open) throw IOException("disconnected")
        channel.send(frame.copyOf())
    }

    fun onReceive(receiver: (ByteArray) -> Unit) {
        channel.onReceive { frame ->
            if (frame.size > MAX_SYNC_FRAME_BYTES) {
                disconnect()
                return@onReceive
            }
            receiver(frame.copyOf())
        }
    }

    fun disconnect() {
        if (open) {
            open = false
            channel.close()
        }
    }

    override fun close() = disconnect()
}

class NearbyConnectionWinner : AutoCloseable {
    private var winner: NearbyConnection? = null
    val hasWinner: Boolean @Synchronized get() = winner != null

    @Synchronized
    fun claim(candidate: NearbyConnection): Boolean {
        if (winner != null) {
            candidate.disconnect()
            return false
        }
        winner = candidate
        return true
    }

    @Synchronized
    override fun close() {
        winner?.disconnect()
        winner = null
    }
}

class SessionTransportSelector(private val localIpConnector: LocalIpConnector) {
    fun select(ble: PairedBleLink): NearbyConnection {
        val local = ble.localEndpoint?.let { endpoint ->
            try {
                localIpConnector.connect(endpoint)
            } catch (_: IOException) {
                null
            }
        }
        return if (local != null) {
            ble.channel.close()
            NearbyConnection(local, TransportKind.LOCAL_IP)
        } else {
            NearbyConnection(ble.channel, TransportKind.BLE)
        }
    }
}

sealed interface NearbyUiState {
    val message: String

    data object Idle : NearbyUiState { override val message = "Ready to find nearby phones" }
    data object Looking : NearbyUiState { override val message = "Looking for nearby phones..." }
    data class ConfirmPairing(val name: String) : NearbyUiState {
        override val message = "Connect with $name?"
    }
    data object Connecting : NearbyUiState { override val message = "Connecting..." }
    data class GettingLatest(val name: String) : NearbyUiState {
        override val message = "Getting the latest from $name..."
    }
    data class UpdatesReady(val count: Int, val name: String) : NearbyUiState {
        override val message = "$count new ${if (count == 1) "update" else "updates"} from $name"
    }
    data object CaughtUp : NearbyUiState { override val message = "All caught up" }
    data object AlreadyCurrent : NearbyUiState { override val message = "You're already up to date" }
    data object Failed : NearbyUiState { override val message = "Couldn't connect — try again" }
    data class OutOfRange(val name: String) : NearbyUiState {
        override val message = "$name went out of range"
    }
}

object NearbyUiActions {
    fun canFindAgain(state: NearbyUiState): Boolean = state is NearbyUiState.Idle ||
        state is NearbyUiState.Failed || state is NearbyUiState.CaughtUp ||
        state is NearbyUiState.AlreadyCurrent
}

class PairingController(private val bleConnector: BleConnector) {
    var state: NearbyUiState = NearbyUiState.Idle
        private set
    private var selected: DiscoveredPhone? = null

    fun select(phone: DiscoveredPhone) {
        selected = phone
        state = NearbyUiState.ConfirmPairing(phone.friendlyName)
    }

    fun confirm(): PairedBleLink {
        val phone = checkNotNull(selected) { "Select a nearby phone first" }
        state = NearbyUiState.Connecting
        return try {
            bleConnector.connect(phone.handle)
        } catch (error: Exception) {
            state = NearbyUiState.Failed
            throw error
        }
    }

    fun cancel() {
        selected = null
        state = NearbyUiState.Idle
    }
}

class IncomingPairingGate {
    val hasPendingRequest: Boolean get() = requestedLink != null
    var state: NearbyUiState = NearbyUiState.Idle
        private set
    var activeLink: PairedBleLink? = null
        private set
    private var requestedLink: PairedBleLink? = null

    fun request(phone: DiscoveredPhone, link: PairedBleLink) {
        requestedLink?.channel?.close()
        activeLink?.channel?.close()
        activeLink = null
        requestedLink = link
        state = NearbyUiState.ConfirmPairing(phone.friendlyName)
    }

    fun confirm(): PairedBleLink {
        val link = checkNotNull(requestedLink) { "No nearby connection to confirm" }
        requestedLink = null
        activeLink = link
        state = NearbyUiState.Connecting
        return link
    }

    fun reject() {
        requestedLink?.channel?.close()
        requestedLink = null
        activeLink = null
        state = NearbyUiState.Idle
    }
}

interface WordPicker {
    fun pick(bound: Int): Int
}

object FriendlyNameGenerator {
    private val adjectives = listOf("Blue", "Quiet", "Bold", "Kind", "Swift", "Warm")
    private val nouns = listOf("Kite", "River", "Fox", "Cedar", "Robin", "Cloud")

    fun generate(picker: WordPicker = SecureWordPicker()): String =
        "${adjectives[picker.pick(adjectives.size)]} ${nouns[picker.pick(nouns.size)]}"

    internal fun candidates(): List<String> = adjectives.flatMap { adjective ->
        nouns.map { noun -> "$adjective $noun" }
    }

    private class SecureWordPicker : WordPicker {
        private val random = SecureRandom()
        override fun pick(bound: Int): Int = random.nextInt(bound)
    }
}

class LoopbackFrameChannel private constructor() : FrameChannel {
    private var peer: LoopbackFrameChannel? = null
    private var receiver: (ByteArray) -> Unit = {}
    private var open = true

    @Synchronized
    override fun send(frame: ByteArray) {
        if (!open) throw IOException("disconnected")
        val target = peer ?: throw IOException("loopback unavailable")
        target.deliver(frame.copyOf())
    }

    @Synchronized
    override fun onReceive(receiver: (ByteArray) -> Unit) {
        this.receiver = receiver
    }

    @Synchronized
    private fun deliver(frame: ByteArray) {
        if (!open) throw IOException("disconnected")
        receiver(frame)
    }

    @Synchronized
    override fun close() {
        open = false
    }

    companion object {
        fun pair(): Pair<LoopbackFrameChannel, LoopbackFrameChannel> {
            val left = LoopbackFrameChannel()
            val right = LoopbackFrameChannel()
            left.peer = right
            right.peer = left
            return left to right
        }
    }
}
