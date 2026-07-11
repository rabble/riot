package org.riot.evidence.transport

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SyncCoordinatorTest {
    @Test
    fun coordinatorDrainsGeneratedFfiBoundaryFramesInOrder() {
        val channel = CoordinatorFrameChannel()
        val bridge = RecordingSyncBridge(
            outbound = ArrayDeque(listOf(byteArrayOf(1), byteArrayOf(2, 3))),
        )
        val coordinator = SyncCoordinator(
            NearbyConnection(channel, TransportKind.BLE),
            bridge,
            "Blue Kite",
        )

        coordinator.start()

        assertTrue(bridge.begun)
        assertEquals(NearbyUiState.GettingLatest("Blue Kite"), coordinator.state)
        assertArrayEquals(byteArrayOf(1), channel.sent[0])
        assertArrayEquals(byteArrayOf(2, 3), channel.sent[1])
    }

    @Test
    fun inboundFrameCanProducePreviewThenExplicitAccept() {
        val channel = CoordinatorFrameChannel()
        val bridge = RecordingSyncBridge(receiveResult = SyncBridgeOutcome.ReadyToPreview(2))
        val coordinator = SyncCoordinator(NearbyConnection(channel, TransportKind.BLE), bridge, "Quiet River")
        coordinator.start()

        channel.deliver(byteArrayOf(9))

        assertEquals(NearbyUiState.UpdatesReady(2, "Quiet River"), coordinator.state)
        coordinator.acceptImport()
        assertTrue(bridge.accepted)
        assertEquals(NearbyUiState.CaughtUp, coordinator.state)
        assertTrue(channel.closed)
    }

    @Test
    fun alreadyCurrentSessionReleasesNearbyConnection() {
        val channel = CoordinatorFrameChannel()
        val bridge = RecordingSyncBridge(beginResult = SyncBridgeOutcome.Done)
        val coordinator = SyncCoordinator(NearbyConnection(channel, TransportKind.BLE), bridge, "Blue Kite")

        coordinator.start()

        assertEquals(NearbyUiState.AlreadyCurrent, coordinator.state)
        assertTrue(channel.closed)
    }

    @Test
    fun protocolFailureCollapsesToPlainLanguageAndDisconnects() {
        val channel = CoordinatorFrameChannel()
        val bridge = RecordingSyncBridge(receiveFailure = IllegalArgumentException("raw protocol detail"))
        val coordinator = SyncCoordinator(NearbyConnection(channel, TransportKind.BLE), bridge, "Blue Kite")
        coordinator.start()

        channel.deliver(byteArrayOf(7))

        assertEquals("Couldn't connect — try again", coordinator.state.message)
        assertTrue(channel.closed)
        assertTrue(bridge.closed)
    }
}

private class RecordingSyncBridge(
    private val outbound: ArrayDeque<ByteArray> = ArrayDeque(),
    private val beginResult: SyncBridgeOutcome = SyncBridgeOutcome.SendMore,
    private val receiveResult: SyncBridgeOutcome = SyncBridgeOutcome.SendMore,
    private val receiveFailure: Exception? = null,
) : MobileSyncSessionBridge {
    var accepted = false
    var begun = false
    var closed = false
    override fun begin(): SyncBridgeOutcome { begun = true; return beginResult }
    override fun nextOutbound(): ByteArray? = outbound.removeFirstOrNull()
    override fun receive(frame: ByteArray): SyncBridgeOutcome {
        receiveFailure?.let { throw it }
        return receiveResult
    }
    override fun acceptImport(): SyncBridgeOutcome { accepted = true; return SyncBridgeOutcome.Done }
    override fun rejectImport(): SyncBridgeOutcome = SyncBridgeOutcome.Done
    override fun close() { closed = true }
}

private class CoordinatorFrameChannel : FrameChannel {
    val sent = mutableListOf<ByteArray>()
    var closed = false
    private var receiver: (ByteArray) -> Unit = {}
    override fun send(frame: ByteArray) { sent += frame.copyOf() }
    override fun onReceive(receiver: (ByteArray) -> Unit) { this.receiver = receiver }
    override fun close() { closed = true }
    fun deliver(frame: ByteArray) = receiver(frame)
}
