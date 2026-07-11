package org.riot.evidence.transport

import uniffi.riot_ffi.CurrentEntry
import uniffi.riot_ffi.MobileSyncSession
import uniffi.riot_ffi.SyncOutcome
import uniffi.riot_ffi.SyncOutcomeKind

internal interface GeneratedSyncHandle : AutoCloseable {
    fun begin(): SyncOutcome
    fun takeOutboundFrame(): ByteArray?
    fun receiveFrame(frame: ByteArray): SyncOutcome
    fun acceptImport(): SyncOutcome
    fun rejectImport(code: UByte): SyncOutcome
    fun cancel()
}

private class UniFfiSyncHandle(private val session: MobileSyncSession) : GeneratedSyncHandle {
    override fun begin() = session.begin()
    override fun takeOutboundFrame() = session.takeOutboundFrame()
    override fun receiveFrame(frame: ByteArray) = session.receiveFrame(frame)
    override fun acceptImport() = session.acceptImport()
    override fun rejectImport(code: UByte) = session.rejectImport(code)
    override fun cancel() = session.cancel()
    override fun close() = session.close()
}

class GeneratedMobileSyncBridge internal constructor(
    private val handle: GeneratedSyncHandle,
    private val persistAccepted: (ByteArray, List<CurrentEntry>) -> Unit,
) : MobileSyncSessionBridge {
    constructor(
        session: MobileSyncSession,
        persistAccepted: (ByteArray, List<CurrentEntry>) -> Unit,
    ) : this(UniFfiSyncHandle(session), persistAccepted)

    private var pending: PendingImport? = null
    private var closed = false
    private var terminalAfterDrain = false

    override fun begin(): SyncBridgeOutcome = map(handle.begin())

    override fun nextOutbound(): ByteArray? {
        val frame = handle.takeOutboundFrame()?.copyOf()
        if (frame == null && terminalAfterDrain) disposeTerminal()
        return frame
    }

    override fun receive(frame: ByteArray): SyncBridgeOutcome = map(handle.receiveFrame(frame.copyOf()))

    override fun acceptImport(): SyncBridgeOutcome {
        val reviewed = checkNotNull(pending) { "No nearby updates to add" }
        persistAccepted(reviewed.bundle.copyOf(), reviewed.entries)
        val outcome = map(handle.acceptImport())
        pending = null
        return outcome
    }

    override fun rejectImport(): SyncBridgeOutcome {
        checkNotNull(pending) { "No nearby updates to dismiss" }
        val outcome = map(handle.rejectImport(USER_DECLINED))
        pending = null
        return outcome
    }

    override fun close() {
        if (closed) return
        closed = true
        pending = null
        terminalAfterDrain = false
        try {
            handle.cancel()
        } finally {
            handle.close()
        }
    }

    private fun map(outcome: SyncOutcome): SyncBridgeOutcome = when (outcome.kind) {
        SyncOutcomeKind.FRAME_READY -> {
            terminalAfterDrain = outcome.terminal
            SyncBridgeOutcome.SendMore(terminal = outcome.terminal)
        }
        SyncOutcomeKind.REVIEW_IMPORT -> {
            val bundle = checkNotNull(outcome.importBundleBytes) { "Reviewed update bundle is missing" }
            pending = PendingImport(bundle.copyOf(), outcome.entries.toList())
            SyncBridgeOutcome.ReadyToPreview(outcome.entries.size)
        }
        SyncOutcomeKind.COMPLETE -> {
            disposeTerminal()
            SyncBridgeOutcome.Done
        }
        SyncOutcomeKind.REJECTED -> {
            disposeTerminal()
            SyncBridgeOutcome.Failed
        }
    }

    private fun disposeTerminal() {
        if (closed) return
        closed = true
        terminalAfterDrain = false
        pending = null
        runCatching(handle::close)
    }

    private data class PendingImport(val bundle: ByteArray, val entries: List<CurrentEntry>)

    private companion object {
        val USER_DECLINED: UByte = 1u
    }
}
