import Foundation
import SwiftUI

@MainActor
public final class NearbyTransportController: ObservableObject {
    @Published public private(set) var state: NearbyConnectionState = .idle
    @Published public private(set) var phones: [DiscoveredPhone] = []

    private var service: CoreBluetoothNearbyService?
    private var selected: DiscoveredPhone?
    private var isInboundRequest = false

    public init() {}

    public func findNearby() {
        let service = service ?? makeService()
        self.service = service
        state = .looking
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
        service = nil
        selected = nil
        phones = []
        state = .idle
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
                if let selected = self.selected {
                    self.state = .outOfRange(name: selected.friendlyName)
                } else {
                    self.state = .failed
                }
            }
        }
        return service
    }
}
