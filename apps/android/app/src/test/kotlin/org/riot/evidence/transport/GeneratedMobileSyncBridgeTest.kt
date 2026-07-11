package org.riot.evidence.transport

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.riot_ffi.SyncOutcome
import uniffi.riot_ffi.SyncOutcomeKind

class GeneratedMobileSyncBridgeTest {
    @Test
    fun acceptedPreviewPersistsTheExactReviewedBundleBeforeCoreAcceptance() {
        val bundle = byteArrayOf(7, 8, 9)
        val events = mutableListOf<String>()
        val handle = RecordingGeneratedSyncHandle(
            receiveOutcome = outcome(SyncOutcomeKind.REVIEW_IMPORT, bundle),
            events = events,
        )
        var persisted: ByteArray? = null
        val bridge = GeneratedMobileSyncBridge(handle) { bytes, _ ->
            events += "persist"
            persisted = bytes.copyOf()
        }

        assertEquals(SyncBridgeOutcome.ReadyToPreview(0), bridge.receive(byteArrayOf(1)))
        assertFalse(handle.accepted)
        assertEquals(SyncBridgeOutcome.SendMore(terminal = false), bridge.acceptImport())

        assertTrue(handle.accepted)
        assertArrayEquals(bundle, persisted)
        assertArrayEquals(byteArrayOf(10), bridge.nextOutbound())
        assertTrue(handle.lifecycle.isEmpty())

        assertEquals(SyncBridgeOutcome.Done, bridge.receive(byteArrayOf(11)))
        assertEquals(listOf("persist", "accept", "take", "close"), events)
    }

    @Test
    fun persistenceFailureNeverAcceptsCoreImport() {
        val handle = RecordingGeneratedSyncHandle(
            receiveOutcome = outcome(SyncOutcomeKind.REVIEW_IMPORT, byteArrayOf(3)),
        )
        val bridge = GeneratedMobileSyncBridge(handle) { _, _ -> error("disk full") }

        bridge.receive(byteArrayOf(1))

        org.junit.Assert.assertThrows(IllegalStateException::class.java) { bridge.acceptImport() }
        assertFalse(handle.accepted)
    }

    @Test
    fun rejectedPreviewIsNotPersisted() {
        val handle = RecordingGeneratedSyncHandle(
            receiveOutcome = outcome(SyncOutcomeKind.REVIEW_IMPORT, byteArrayOf(3)),
        )
        var persisted = false
        val bridge = GeneratedMobileSyncBridge(handle) { _, _ -> persisted = true }

        bridge.receive(byteArrayOf(1))
        assertEquals(SyncBridgeOutcome.SendMore(terminal = true), bridge.rejectImport())
        assertArrayEquals(byteArrayOf(12), bridge.nextOutbound())

        assertTrue(handle.rejected)
        assertFalse(persisted)
        assertEquals(listOf("close"), handle.lifecycle)
    }

    @Test
    fun closeCancelsProtocolBeforeDisposingGeneratedHandle() {
        val handle = RecordingGeneratedSyncHandle()
        val bridge = GeneratedMobileSyncBridge(handle) { _, _ -> }

        bridge.close()

        assertEquals(listOf("cancel", "close"), handle.lifecycle)
    }

    @Test
    fun terminalCompletionDisposesWithoutCancellingCompletedProtocol() {
        val handle = RecordingGeneratedSyncHandle(
            beginOutcome = outcome(SyncOutcomeKind.COMPLETE),
        )
        val bridge = GeneratedMobileSyncBridge(handle) { _, _ -> }

        assertEquals(SyncBridgeOutcome.Done, bridge.begin())
        bridge.close()

        assertEquals(listOf("close"), handle.lifecycle)
    }
}

private class RecordingGeneratedSyncHandle(
    private val beginOutcome: SyncOutcome = outcome(SyncOutcomeKind.FRAME_READY),
    private val receiveOutcome: SyncOutcome = outcome(SyncOutcomeKind.FRAME_READY),
    private val events: MutableList<String> = mutableListOf(),
) : GeneratedSyncHandle {
    var accepted = false
    var rejected = false
    val lifecycle = mutableListOf<String>()

    override fun begin(): SyncOutcome = beginOutcome
    private val outbound = ArrayDeque<ByteArray>()
    private var acceptedAwaitingComplete = false
    private var terminalFrameQueued = false
    private var terminalFrameTaken = false
    override fun takeOutboundFrame(): ByteArray? {
        if (terminalFrameTaken) error("terminal session is already closed")
        events += "take"
        return outbound.removeFirstOrNull()?.also {
            if (terminalFrameQueued) terminalFrameTaken = true
        }
    }
    override fun receiveFrame(frame: ByteArray): SyncOutcome =
        if (acceptedAwaitingComplete) outcome(SyncOutcomeKind.COMPLETE) else receiveOutcome
    override fun acceptImport(): SyncOutcome {
        events += "accept"
        accepted = true
        acceptedAwaitingComplete = true
        outbound += byteArrayOf(10)
        return outcome(SyncOutcomeKind.FRAME_READY, terminal = false)
    }
    override fun rejectImport(code: UByte): SyncOutcome {
        rejected = true
        outbound += byteArrayOf(12)
        terminalFrameQueued = true
        return outcome(SyncOutcomeKind.FRAME_READY, terminal = true)
    }
    override fun cancel() { lifecycle += "cancel" }
    override fun close() { lifecycle += "close"; events += "close" }
}

private fun outcome(
    kind: SyncOutcomeKind,
    bundle: ByteArray? = null,
    terminal: Boolean = kind == SyncOutcomeKind.COMPLETE || kind == SyncOutcomeKind.REJECTED,
) = SyncOutcome(
    kind = kind,
    entries = emptyList(),
    rejectionCode = null,
    terminal = terminal,
    importBundleBytes = bundle,
)
