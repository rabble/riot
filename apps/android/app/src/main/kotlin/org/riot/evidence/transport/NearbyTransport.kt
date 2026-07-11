package org.riot.evidence.transport

import java.io.IOException
import java.security.SecureRandom

const val MAX_SYNC_FRAME_BYTES = 8_388_608 + 128

@JvmInline
value class PeerHandle internal constructor(internal val value: String)

data class DiscoveredPhone(val friendlyName: String, val handle: PeerHandle)
data class LocalEndpoint(val host: String, val port: Int) {
    init {
        require(host.isNotBlank()) { "local endpoint host is empty" }
        require(port in 1..65_535) { "local endpoint port is invalid" }
    }
}

enum class TransportKind { LOCAL_IP, BLE }

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
    private val adjectives = listOf("Blue", "Quiet", "Bright", "Gentle", "Swift", "Warm")
    private val nouns = listOf("Kite", "River", "Fox", "Cedar", "Robin", "Cloud")

    fun generate(picker: WordPicker = SecureWordPicker()): String =
        "${adjectives[picker.pick(adjectives.size)]} ${nouns[picker.pick(nouns.size)]}"

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
