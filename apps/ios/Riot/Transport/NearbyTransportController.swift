import Foundation
import SwiftUI

enum LocalNetworkRole: Equatable {
    case attempt
    case wait

    static func select(localName: String, localToken: String, remoteName: String, remoteToken: String) -> Self {
        if localName != remoteName { return localName < remoteName ? .attempt : .wait }
        return localToken < remoteToken ? .attempt : .wait
    }
}

enum LocalChannelAdmission {
    static func accepts(callbackGeneration: UUID, currentGeneration: UUID?, routeChosen: Bool) -> Bool {
        callbackGeneration == currentGeneration && !routeChosen
    }
}

@MainActor
public final class NearbyTransportController: ObservableObject {
    @Published public private(set) var state: NearbyConnectionState = .idle
    @Published public private(set) var phones: [DiscoveredPhone] = []
    @Published public private(set) var activeRoute: NearbyRoute?

    private var service: CoreBluetoothNearbyService?
    /// Runs alongside Bluetooth, not instead of it. A radio cannot find a peer on
    /// the same machine (one BLE controller never hears its own advertisement),
    /// so peers that Bluetooth structurally cannot see arrive over Bonjour.
    private var localService: LocalNetworkNearbyService?
    private var bluetoothPhones: [DiscoveredPhone] = []
    private var localPhones: [DiscoveredPhone] = []
    private var selected: DiscoveredPhone?
    private var isInboundRequest = false
    /// Set when the pairing in flight came from the local network, so confirm and
    /// cancel are routed to the service that actually owns it.
    private var selectedIsLocal = false
    private var listener: LocalNetworkListener?
    private var acceptedLocalChannel: LocalTCPFrameChannel?
    private var remoteEndpoint: LocalEndpoint?
    private var remoteTieBreaker: String?
    private var nearbyConnection: NearbyConnection?
    private var coordinator: SyncCoordinator?
    private var syncBoundaryProvider: (() throws -> MobileSyncSessionBoundary)?
    private var listenerGeneration: UUID?

    public init() {}

    public func findNearby(syncBoundaryProvider: @escaping () throws -> MobileSyncSessionBoundary) {
        resetSession()
        self.syncBoundaryProvider = syncBoundaryProvider
        let service = makeService()
        self.service = service
        let generation = UUID()
        listenerGeneration = generation
        state = .looking
        let listener = LocalNetworkListener()
        listener.onAccepted = { [weak self, weak service] channel in
            Task { @MainActor in
                guard let self, let service,
                      self.service === service,
                      LocalChannelAdmission.accepts(
                        callbackGeneration: generation,
                        currentGeneration: self.listenerGeneration,
                        routeChosen: self.nearbyConnection != nil
                      ) else {
                    channel.disconnect()
                    return
                }
                self.acceptedLocalChannel = channel
            }
        }
        self.listener = listener
        listener.start { [weak service] endpoint in
            Task { @MainActor in service?.setLocalEndpoint(endpoint) }
        }
        service.startLooking()

        let localService = makeLocalService()
        self.localService = localService
        localService.startLooking()
    }

    public func requestConnection(to phone: DiscoveredPhone) {
        selected = phone
        isInboundRequest = false
        selectedIsLocal = localService?.canPair(with: phone) ?? false
        state = .confirm(name: phone.friendlyName)
        // A local-network peer is NOT dialled here: opening the connection is
        // what makes their device ask them to accept, and it must not happen
        // until the person on THIS side has said yes. Bluetooth keeps its own
        // two-step (request over the radio, then confirm) below.
        if !selectedIsLocal {
            service?.requestPairing(with: phone)
        }
    }

    public func confirmConnection() {
        guard selected != nil else { return }
        state = .connecting
        if selectedIsLocal {
            if isInboundRequest {
                localService?.confirmInboundPairing()
            } else if let selected {
                // Now that this side has consented, dial them — which is what
                // raises the prompt on theirs. Both people say yes, or nothing
                // happens.
                localService?.requestPairing(with: selected)
            }
            return
        }
        if isInboundRequest {
            do { try service?.confirmInboundPairing() } catch { state = .failed }
        } else {
            service?.confirmPairing()
        }
    }

    public func cancelConnection() {
        selected = nil
        if selectedIsLocal {
            if isInboundRequest { localService?.cancelInboundPairing() } else { localService?.cancelPairing() }
        } else if isInboundRequest {
            service?.cancelInboundPairing()
        } else {
            service?.cancelPairing()
        }
        isInboundRequest = false
        selectedIsLocal = false
        state = .looking
    }

    public func stop() {
        resetSession()
        syncBoundaryProvider = nil
        state = .idle
    }

    private func resetSession() {
        coordinator?.stop()
        nearbyConnection?.disconnect()
        service?.stop()
        localService?.stop()
        listener?.stop()
        acceptedLocalChannel?.disconnect()
        service = nil
        localService = nil
        listener = nil
        listenerGeneration = nil
        selected = nil
        selectedIsLocal = false
        bluetoothPhones = []
        localPhones = []
        phones = []
        activeRoute = nil
        nearbyConnection = nil
        coordinator = nil
        acceptedLocalChannel = nil
        remoteEndpoint = nil
        remoteTieBreaker = nil
        isInboundRequest = false
    }

    public func addPreviewedContent() {
        coordinator?.addPreviewedContent()
    }

    public func rejectPreviewedContent() {
        coordinator?.rejectPreviewedContent()
    }

    /// Discovery and pairing for peers Bluetooth structurally cannot reach —
    /// notably another instance on this same machine. The channel it hands back
    /// is already the session channel, so there is no route negotiation to do:
    /// it IS the local network.
    private func makeLocalService() -> LocalNetworkNearbyService {
        let localService = LocalNetworkNearbyService()
        localService.onPhonesChanged = { [weak self] phones in
            Task { @MainActor in
                guard let self else { return }
                self.localPhones = phones
                self.republishPhones()
            }
        }
        localService.onInboundPairingRequested = { [weak self] name in
            Task { @MainActor in
                guard let self else { return }
                self.isInboundRequest = true
                self.selectedIsLocal = true
                self.selected = DiscoveredPhone(id: UUID(), friendlyName: name)
                self.state = .confirm(name: name)
            }
        }
        localService.onPaired = { [weak self] channel, peer in
            Task { @MainActor in
                guard let self else { return }
                self.startLocalSession(channel: channel, peer: peer)
            }
        }
        localService.onDisconnected = { [weak self] in
            Task { @MainActor in
                guard let self else { return }
                self.coordinator?.stop()
                if let selected = self.selected, self.selectedIsLocal {
                    self.state = .outOfRange(name: selected.friendlyName)
                }
            }
        }
        return localService
    }

    /// A peer found over the local link is already connected on the channel that
    /// carried the handshake, so that channel is the session's base route.
    private func startLocalSession(channel: FrameChannel, peer: NearbyPeerIdentity) {
        guard nearbyConnection == nil else { return }
        let connection = NearbyConnection(base: channel, baseRoute: .localNetwork, localAttempt: { nil })
        connection.confirmPairing()
        do {
            try connection.activate()
            nearbyConnection = connection
            activeRoute = connection.route
            if let provider = syncBoundaryProvider {
                let coordinator = try SyncCoordinator(
                    session: provider(),
                    connection: connection,
                    friendlyName: peer.friendlyName
                )
                coordinator.onStateChanged = { [weak self] state in self?.state = state }
                self.coordinator = coordinator
                coordinator.start()
            }
        } catch {
            connection.disconnect()
            coordinator?.stop()
            state = .failed
        }
    }

    /// The UI sees one list; the two discovery paths never see the same peer, so
    /// a plain concatenation is correct.
    private func republishPhones() {
        phones = bluetoothPhones + localPhones
    }

    private func makeService() -> CoreBluetoothNearbyService {
        let service = CoreBluetoothNearbyService()
        service.onPhonesChanged = { [weak self] phones in
            Task { @MainActor in
                guard let self else { return }
                self.bluetoothPhones = phones
                self.republishPhones()
            }
        }
        service.onConnected = { [weak self] endpoint, tieBreaker in
            Task { @MainActor in
                guard let self, let selected = self.selected else { return }
                self.remoteEndpoint = endpoint
                self.remoteTieBreaker = tieBreaker
                self.state = .gettingLatest(name: selected.friendlyName)
                self.chooseSessionRoute(remoteName: selected.friendlyName)
            }
        }
        service.onInboundPairingRequested = { [weak self] name in
            Task { @MainActor in
                guard let self else { return }
                self.isInboundRequest = true
                let phone = DiscoveredPhone(id: UUID(), friendlyName: name)
                self.selected = phone
                self.state = .confirm(name: name)
            }
        }
        service.onDisconnected = { [weak self] in
            Task { @MainActor in
                guard let self else { return }
                self.coordinator?.stop()
                if let selected = self.selected {
                    self.state = .outOfRange(name: selected.friendlyName)
                } else {
                    self.state = .failed
                }
            }
        }
        return service
    }

    private func chooseSessionRoute(remoteName: String) {
        guard let service, let remoteTieBreaker else { return }
        let bluetooth = CoreBluetoothFrameChannel(service: service)
        let role = LocalNetworkRole.select(
            localName: service.friendlyName,
            localToken: service.tieBreaker,
            remoteName: remoteName,
            remoteToken: remoteTieBreaker
        )
        if role == .attempt, let remoteEndpoint {
            LocalTCPFrameChannel.attempt(endpoint: remoteEndpoint) { [weak self] channel in
                Task { @MainActor in self?.finishRouteSelection(bluetooth: bluetooth, local: channel) }
            }
        } else if role == .wait {
            Task { @MainActor [weak self] in
                try? await Task.sleep(for: .seconds(2))
                self?.finishRouteSelection(bluetooth: bluetooth, local: self?.acceptedLocalChannel)
            }
        } else {
            finishRouteSelection(bluetooth: bluetooth, local: nil)
        }
    }

    private func finishRouteSelection(bluetooth: FrameChannel, local: FrameChannel?) {
        guard nearbyConnection == nil else { return }
        listener?.stop()
        listenerGeneration = nil
        if let acceptedLocalChannel, acceptedLocalChannel !== local { acceptedLocalChannel.disconnect() }
        self.acceptedLocalChannel = nil
        let connection = NearbyConnection(bluetooth: bluetooth) { local }
        connection.confirmPairing()
        do {
            try connection.activate()
            nearbyConnection = connection
            activeRoute = connection.route
            if let selected, let provider = syncBoundaryProvider {
                let coordinator = try SyncCoordinator(
                    session: provider(),
                    connection: connection,
                    friendlyName: selected.friendlyName
                )
                coordinator.onStateChanged = { [weak self] state in self?.state = state }
                self.coordinator = coordinator
                coordinator.start()
            }
        } catch {
            connection.disconnect()
            coordinator?.stop()
            state = .failed
        }
    }
}
