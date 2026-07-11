import Foundation
import SwiftUI

@MainActor
public final class NearbyTransportController: ObservableObject {
    @Published public private(set) var state: NearbyConnectionState = .idle
    @Published public private(set) var phones: [DiscoveredPhone] = []
    @Published public private(set) var activeRoute: NearbyRoute?

    private var service: CoreBluetoothNearbyService?
    private var selected: DiscoveredPhone?
    private var isInboundRequest = false
    private var listener: LocalNetworkListener?
    private var acceptedLocalChannel: LocalTCPFrameChannel?
    private var remoteEndpoint: LocalEndpoint?
    private var nearbyConnection: NearbyConnection?
    private var coordinator: SyncCoordinator?
    private var syncBoundaryProvider: (() throws -> MobileSyncSessionBoundary)?

    public init() {}

    public func findNearby(syncBoundaryProvider: @escaping () throws -> MobileSyncSessionBoundary) {
        self.syncBoundaryProvider = syncBoundaryProvider
        let service = service ?? makeService()
        self.service = service
        state = .looking
        let listener = LocalNetworkListener()
        listener.onAccepted = { [weak self] channel in
            Task { @MainActor in self?.acceptedLocalChannel = channel }
        }
        self.listener = listener
        listener.start { [weak service] endpoint in service?.setLocalEndpoint(endpoint) }
        service.startLooking()
    }

    public func requestConnection(to phone: DiscoveredPhone) {
        selected = phone
        isInboundRequest = false
        state = .confirm(name: phone.friendlyName)
        service?.requestPairing(with: phone)
    }

    public func confirmConnection() {
        guard selected != nil else { return }
        state = .connecting
        if isInboundRequest {
            do { try service?.confirmInboundPairing() } catch { state = .failed }
        } else {
            service?.confirmPairing()
        }
    }

    public func cancelConnection() {
        selected = nil
        if isInboundRequest { service?.cancelInboundPairing() } else { service?.cancelPairing() }
        isInboundRequest = false
        state = .looking
    }

    public func stop() {
        service?.stop()
        listener?.stop()
        service = nil
        selected = nil
        phones = []
        activeRoute = nil
        nearbyConnection = nil
        coordinator = nil
        state = .idle
    }

    public func addPreviewedContent() {
        coordinator?.addPreviewedContent()
    }

    public func rejectPreviewedContent() {
        coordinator?.rejectPreviewedContent()
    }

    private func makeService() -> CoreBluetoothNearbyService {
        let service = CoreBluetoothNearbyService()
        service.onPhonesChanged = { [weak self] phones in
            Task { @MainActor in self?.phones = phones }
        }
        service.onConnected = { [weak self] in
            Task { @MainActor in
                guard let self, let selected = self.selected else { return }
                self.state = .gettingLatest(name: selected.friendlyName)
                self.chooseSessionRoute(remoteName: selected.friendlyName)
            }
        }
        service.onRemoteEndpoint = { [weak self] endpoint in
            Task { @MainActor in self?.remoteEndpoint = endpoint }
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
        guard let service else { return }
        let bluetooth = CoreBluetoothFrameChannel(service: service)
        if service.friendlyName < remoteName, let remoteEndpoint {
            LocalTCPFrameChannel.attempt(endpoint: remoteEndpoint) { [weak self] channel in
                Task { @MainActor in self?.finishRouteSelection(bluetooth: bluetooth, local: channel) }
            }
        } else if service.friendlyName > remoteName {
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
            state = .failed
        }
    }
}
