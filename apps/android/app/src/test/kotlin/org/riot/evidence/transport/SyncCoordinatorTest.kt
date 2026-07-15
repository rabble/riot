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
        channel.deliver(byteArrayOf(4))

        assertTrue(bridge.begun)
        assertEquals(NearbyUiState.GettingLatest("Blue Kite"), coordinator.state)
        assertArrayEquals(byteArrayOf(1), channel.sent[0])
        assertArrayEquals(byteArrayOf(2, 3), channel.sent[1])
    }

    @Test
    fun inboundFrameCanProducePreviewThenExplicitAccept() {
        val channel = CoordinatorFrameChannel()
        val bridge = RecordingSyncBridge(
            receiveResults = ArrayDeque(
                listOf(SyncBridgeOutcome.ReadyToPreview(2), SyncBridgeOutcome.Done),
            ),
        )
        val coordinator = SyncCoordinator(NearbyConnection(channel, TransportKind.BLE), bridge, "Quiet River")
        coordinator.start()
        channel.sent.clear()

        channel.deliver(byteArrayOf(9))

        assertEquals(NearbyUiState.UpdatesReady(2, "Quiet River"), coordinator.state)
        coordinator.acceptImport()
        assertTrue(bridge.accepted)
        assertEquals(NearbyUiState.GettingLatest("Quiet River"), coordinator.state)
        assertArrayEquals(byteArrayOf(10), channel.sent.single())
        assertTrue(!channel.closed)

        channel.deliver(byteArrayOf(11))

        assertEquals(NearbyUiState.CaughtUp, coordinator.state)
        assertTrue(channel.closed)
    }

    @Test
    fun terminalRejectFrameIsSentOnceThenReturnsToIdleWithoutEarlyDisconnect() {
        val channel = CoordinatorFrameChannel()
        val bridge = RecordingSyncBridge(
            receiveResults = ArrayDeque(listOf(SyncBridgeOutcome.ReadyToPreview(1))),
        )
        val coordinator = SyncCoordinator(NearbyConnection(channel, TransportKind.BLE), bridge, "Blue Kite")
        coordinator.start()
        channel.sent.clear()
        channel.deliver(byteArrayOf(9))
        bridge.outboundTakes = 0

        coordinator.rejectImport()

        assertArrayEquals(byteArrayOf(12), channel.sent.single())
        assertEquals(NearbyUiState.Idle, coordinator.state)
        assertTrue(bridge.closed)
        assertTrue(!channel.closed)
        assertEquals(1, bridge.outboundTakes)
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
    fun terminalResponderFrameDrainsBeforeAlreadyCurrentWithoutEarlyDisconnect() {
        val channel = CoordinatorFrameChannel()
        val bridge = RecordingSyncBridge(
            outbound = ArrayDeque(listOf(byteArrayOf(20))),
            beginResult = SyncBridgeOutcome.SendMore(terminal = true),
        )
        val coordinator = SyncCoordinator(NearbyConnection(channel, TransportKind.BLE), bridge, "Blue Kite")

        coordinator.start()

        assertArrayEquals(byteArrayOf(20), channel.sent.single())
        assertEquals(NearbyUiState.AlreadyCurrent, coordinator.state)
        assertTrue(bridge.closed)
        assertTrue(!channel.closed)
        assertEquals(1, bridge.outboundTakes)
    }

    @Test
    fun stoppedCoordinatorRefusesToCommitAPendingImport() {
        val channel = CoordinatorFrameChannel()
        val bridge = RecordingSyncBridge(
            receiveResults = ArrayDeque(listOf(SyncBridgeOutcome.ReadyToPreview(1))),
        )
        val coordinator = SyncCoordinator(NearbyConnection(channel, TransportKind.BLE), bridge, "Blue Kite")
        coordinator.start()
        channel.deliver(byteArrayOf(9))
        assertEquals(NearbyUiState.UpdatesReady(1, "Blue Kite"), coordinator.state)

        // A community switch stops the coordinator; a racing "Add them" lands after.
        // A stopped session must never commit a pending import — fail closed.
        coordinator.close()
        coordinator.acceptImport()

        assertTrue("a stopped session committed a pending import", !bridge.accepted)
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
    private val outbound: ArrayDeque<ByteArray> = ArrayDeque(listOf(byteArrayOf(1))),
    private val beginResult: SyncBridgeOutcome = SyncBridgeOutcome.SendMore(terminal = false),
    private val receiveResults: ArrayDeque<SyncBridgeOutcome> = ArrayDeque(),
    private val receiveFailure: Exception? = null,
) : MobileSyncSessionBridge {
    var accepted = false
    var begun = false
    var closed = false
    var outboundTakes = 0
    private var terminalFrameExpected = (beginResult as? SyncBridgeOutcome.SendMore)?.terminal == true
    private var terminalFrameTaken = false
    override fun begin(): SyncBridgeOutcome { begun = true; return beginResult }
    override fun nextOutbound(): ByteArray? {
        if (terminalFrameTaken) error("terminal session is already closed")
        outboundTakes += 1
        return outbound.removeFirstOrNull()?.also {
            if (terminalFrameExpected) terminalFrameTaken = true
        }
    }
    override fun receive(frame: ByteArray): SyncBridgeOutcome {
        receiveFailure?.let { throw it }
        return receiveResults.removeFirstOrNull() ?: SyncBridgeOutcome.SendMore(terminal = false)
    }
    override fun acceptImport(): SyncBridgeOutcome {
        accepted = true
        outbound += byteArrayOf(10)
        return SyncBridgeOutcome.SendMore(terminal = false)
    }
    override fun rejectImport(): SyncBridgeOutcome {
        outbound += byteArrayOf(12)
        terminalFrameExpected = true
        return SyncBridgeOutcome.SendMore(terminal = true)
    }
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
