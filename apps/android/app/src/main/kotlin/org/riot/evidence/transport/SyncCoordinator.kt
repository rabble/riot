package org.riot.evidence.transport

sealed interface SyncBridgeOutcome {
    data object SendMore : SyncBridgeOutcome
    data class ReadyToPreview(val count: Int) : SyncBridgeOutcome
    data object Done : SyncBridgeOutcome
    data object Failed : SyncBridgeOutcome
}

/** Narrow boundary implemented by the generated UniFFI SyncSession adapter. */
interface MobileSyncSessionBridge {
    fun begin(): SyncBridgeOutcome
    fun nextOutbound(): ByteArray?
    fun receive(frame: ByteArray): SyncBridgeOutcome
    fun acceptImport(): SyncBridgeOutcome
    fun rejectImport(): SyncBridgeOutcome
    fun close()
}

class SyncCoordinator(
    private val connection: NearbyConnection,
    private val bridge: MobileSyncSessionBridge,
    private val friendlyName: String,
    private val onStateChange: (NearbyUiState) -> Unit = {},
) : AutoCloseable {
    var state: NearbyUiState = NearbyUiState.Connecting
        private set

    init {
        connection.onReceive(::receive)
    }

    fun start() {
        update(NearbyUiState.GettingLatest(friendlyName))
        safely { handle(bridge.begin()) }
    }

    fun acceptImport() {
        safely {
            handle(bridge.acceptImport(), accepted = true)
        }
    }

    fun rejectImport() {
        safely {
            handle(bridge.rejectImport())
        }
    }

    override fun close() {
        runCatching(bridge::close)
        connection.disconnect()
    }

    private fun receive(frame: ByteArray) {
        safely {
            handle(bridge.receive(frame))
        }
    }

    private fun handle(outcome: SyncBridgeOutcome, accepted: Boolean = false) {
        when (outcome) {
            SyncBridgeOutcome.SendMore -> drainOutbound()
            is SyncBridgeOutcome.ReadyToPreview -> {
                require(outcome.count >= 0) { "negative update count" }
                update(NearbyUiState.UpdatesReady(outcome.count, friendlyName))
            }
            SyncBridgeOutcome.Done -> {
                update(if (accepted) NearbyUiState.CaughtUp else NearbyUiState.AlreadyCurrent)
                runCatching(bridge::close)
                connection.disconnect()
            }
            SyncBridgeOutcome.Failed -> fail()
        }
    }

    private fun drainOutbound() {
        repeat(MAX_OUTBOUND_FRAMES_PER_TURN) {
            val frame = bridge.nextOutbound() ?: return
            connection.send(frame)
        }
        check(bridge.nextOutbound() == null) { "too many queued outbound frames" }
    }

    private fun safely(action: () -> Unit) {
        try {
            action()
        } catch (_: Exception) {
            fail()
        }
    }

    private fun fail() {
        runCatching(bridge::close)
        connection.disconnect()
        update(NearbyUiState.Failed)
    }

    private fun update(next: NearbyUiState) {
        state = next
        onStateChange(next)
    }

    private companion object {
        const val MAX_OUTBOUND_FRAMES_PER_TURN = 256
    }
}
