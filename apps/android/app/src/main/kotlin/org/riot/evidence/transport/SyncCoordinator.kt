package org.riot.evidence.transport

sealed interface SyncBridgeOutcome {
    data class SendMore(val terminal: Boolean) : SyncBridgeOutcome
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
    private var acceptedImport = false

    init {
        connection.onReceive(::receive)
    }

    fun start() {
        update(NearbyUiState.GettingLatest(friendlyName))
        safely { handle(bridge.begin()) }
    }

    /// The answering half: ready to receive, but does NOT open the protocol.
    /// The initiator's Hello lands in the one phase that accepts it. See start().
    fun answer() {
        update(NearbyUiState.GettingLatest(friendlyName))
    }

    fun acceptImport() {
        safely {
            acceptedImport = true
            update(NearbyUiState.GettingLatest(friendlyName))
            handle(bridge.acceptImport(), terminalState = NearbyUiState.CaughtUp)
        }
    }

    fun rejectImport() {
        safely {
            handle(bridge.rejectImport(), terminalState = NearbyUiState.Idle)
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

    private fun handle(outcome: SyncBridgeOutcome, terminalState: NearbyUiState? = null) {
        when (outcome) {
            is SyncBridgeOutcome.SendMore -> {
                sendOutboundFrame()
                if (outcome.terminal) {
                    runCatching(bridge::close)
                    update(terminalState ?: if (acceptedImport) {
                        NearbyUiState.CaughtUp
                    } else {
                        NearbyUiState.AlreadyCurrent
                    })
                }
            }
            is SyncBridgeOutcome.ReadyToPreview -> {
                require(outcome.count >= 0) { "negative update count" }
                update(NearbyUiState.UpdatesReady(outcome.count, friendlyName))
            }
            SyncBridgeOutcome.Done -> {
                update(if (acceptedImport) NearbyUiState.CaughtUp else NearbyUiState.AlreadyCurrent)
                runCatching(bridge::close)
                connection.disconnect()
            }
            SyncBridgeOutcome.Failed -> fail()
        }
    }

    private fun sendOutboundFrame() = connection.send(
        checkNotNull(bridge.nextOutbound()) { "missing queued outbound frame" },
    )

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
}
