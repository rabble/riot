package org.riot.evidence.transport

import java.io.IOException
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class NearbyTransportContractTest {
    @Test
    fun discoveryUsesFriendlyEphemeralNamesNotTechnicalIdentifiers() {
        val first = FriendlyNameGenerator.generate(SequenceWordPicker(0, 0))
        val second = FriendlyNameGenerator.generate(SequenceWordPicker(1, 1))

        assertEquals("Blue Kite", first)
        assertEquals("Quiet River", second)
        assertFalse(first.contains(":"))
        assertFalse(first.any(Char::isDigit))
    }

    @Test
    fun selectingAPhoneNeverConnectsBeforeExplicitConfirmation() {
        val connector = RecordingBleConnector()
        val pairing = PairingController(connector)
        val phone = DiscoveredPhone("Blue Kite", PeerHandle("internal-handle"))

        pairing.select(phone)

        assertEquals(NearbyUiState.ConfirmPairing("Blue Kite"), pairing.state)
        assertEquals(0, connector.attempts)

        pairing.confirm()

        assertEquals(1, connector.attempts)
    }

    @Test
    fun incomingConnectionCannotBecomeActiveBeforeLocalConfirmation() {
        val gate = IncomingPairingGate()
        val phone = DiscoveredPhone("Quiet River", PeerHandle("remote"))
        val link = PairedBleLink(RecordingFrameChannel(), null)

        gate.request(phone, link)

        assertEquals(NearbyUiState.ConfirmPairing("Quiet River"), gate.state)
        assertEquals(null, gate.activeLink)

        gate.confirm()

        assertEquals(link, gate.activeLink)
    }

    @Test
    fun loopbackPreservesEveryFrameByteAndOrder() {
        val (leftChannel, rightChannel) = LoopbackFrameChannel.pair()
        val left = NearbyConnection(leftChannel, TransportKind.BLE)
        val right = NearbyConnection(rightChannel, TransportKind.BLE)
        val received = mutableListOf<ByteArray>()
        right.onReceive { received += it }

        val frames = listOf(byteArrayOf(0, 1, 2), byteArrayOf(9, 8), ByteArray(128) { it.toByte() })
        frames.forEach(left::send)

        assertEquals(frames.size, received.size)
        frames.indices.forEach { assertArrayEquals(frames[it], received[it]) }
    }

    @Test
    fun oversizedFrameRejectsBeforeChannelMutation() {
        val channel = RecordingFrameChannel()
        val connection = NearbyConnection(channel, TransportKind.BLE)

        assertThrows(IllegalArgumentException::class.java) {
            connection.send(ByteArray(MAX_SYNC_FRAME_BYTES + 1))
        }
        assertTrue(channel.sent.isEmpty())
    }

    @Test
    fun disconnectDoesNotCorruptRetrySession() {
        val first = RecordingFrameChannel()
        val firstConnection = NearbyConnection(first, TransportKind.BLE)
        firstConnection.send(byteArrayOf(1))
        firstConnection.disconnect()
        assertThrows(IOException::class.java) { firstConnection.send(byteArrayOf(2)) }

        val retry = RecordingFrameChannel()
        val retryConnection = NearbyConnection(retry, TransportKind.BLE)
        retryConnection.send(byteArrayOf(2))

        assertEquals(1, first.sent.size)
        assertArrayEquals(byteArrayOf(2), retry.sent.single())
    }

    @Test
    fun localIpIsAttemptedOnceThenBleFallbackIsFixedForSession() {
        val ble = RecordingFrameChannel()
        val local = RecordingLocalIpConnector(result = null)
        val selector = SessionTransportSelector(local)

        val connection = selector.select(PairedBleLink(ble, LocalEndpoint("192.168.1.5", 4567)))
        connection.send(byteArrayOf(1))
        connection.send(byteArrayOf(2))

        assertEquals(1, local.attempts)
        assertEquals(TransportKind.BLE, connection.kind)
        assertEquals(2, ble.sent.size)
    }

    @Test
    fun failedChosenLocalSessionNeverSwitchesPerMessageOrToInternet() {
        val chosenLocal = RecordingFrameChannel()
        val ble = RecordingFrameChannel()
        val local = RecordingLocalIpConnector(chosenLocal)
        val connection = SessionTransportSelector(local).select(
            PairedBleLink(ble, LocalEndpoint("192.168.1.5", 4567)),
        )
        chosenLocal.close()

        assertThrows(IOException::class.java) { connection.send(byteArrayOf(3)) }
        assertTrue(ble.sent.isEmpty())
        assertEquals(1, local.attempts)
        assertEquals(TransportKind.LOCAL_IP, connection.kind)
        assertTrue(ble.closed)
    }

    @Test
    fun productionSocketConnectorRejectsPublicInternetAddressesBeforeDialing() {
        val connector = SocketLocalIpConnector()
        listOf("8.8.8.8", "example.com", "2001:4860:4860::8888").forEach { host ->
            assertThrows("must reject $host", IOException::class.java) {
                connector.connect(LocalEndpoint(host, 443))
            }
        }
    }
}

private class SequenceWordPicker(private var first: Int, private var second: Int) : WordPicker {
    private var call = 0
    override fun pick(bound: Int): Int = if (call++ == 0) first % bound else second % bound
}

private class RecordingBleConnector : BleConnector {
    var attempts = 0
    override fun connect(handle: PeerHandle): PairedBleLink {
        attempts += 1
        return PairedBleLink(RecordingFrameChannel(), null)
    }
}

private class RecordingLocalIpConnector(private val result: FrameChannel?) : LocalIpConnector {
    var attempts = 0
    override fun connect(endpoint: LocalEndpoint): FrameChannel? {
        attempts += 1
        return result
    }
}

private class RecordingFrameChannel : FrameChannel {
    val sent = mutableListOf<ByteArray>()
    private var receiver: (ByteArray) -> Unit = {}
    private var open = true
    val closed: Boolean get() = !open

    override fun send(frame: ByteArray) {
        if (!open) throw IOException("disconnected")
        sent += frame.copyOf()
        receiver(frame.copyOf())
    }

    override fun onReceive(receiver: (ByteArray) -> Unit) {
        this.receiver = receiver
    }

    override fun close() {
        open = false
    }
}
