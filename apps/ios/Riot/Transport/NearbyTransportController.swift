import Foundation
import SwiftUI
#if canImport(UIKit)
import UIKit
#endif

/// Whether a previewed nearby import may commit into the currently selected
/// community.
///
/// The rule, in one place, so the wrong-community race is provable without a
/// radio: an import may land only in the community whose session produced it. If
/// the selected community has changed (or gone) since the session opened, the
/// import is refused — never repointed at whatever community is selected now.
public enum NearbyImportAdmission {
    public static func permits(owned: String?, current: String?) -> Bool {
        guard let owned, let current else { return false }
        return owned.lowercased() == current.lowercased()
    }
}

/// The §4.7 recovery when Bluetooth or local-network access is denied: a
/// plain-language explanation of what still works offline and a deep link into
/// Settings — never a raw permission error.
public enum NearbyPermissionRecovery {
    public static var settingsURL: URL? {
        #if canImport(UIKit)
        return URL(string: UIApplication.openSettingsURLString)
        #else
        return URL(string: "x-apple.systempreferences:")
        #endif
    }

    public static let message =
        "Riot needs Bluetooth or local-network access to find nearby devices. "
        + "You can still read and post updates offline. Open Settings to turn access on."
}

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

    /// The device this one is in a session with RIGHT NOW, or nil if it is only
    /// looking.
    ///
    /// The state enum cannot answer this: its terminal cases (`caughtUp`,
    /// `alreadyCurrent`) carry no name, so the screen could say "All caught up"
    /// without ever saying caught up WITH WHOM — which was the whole complaint.
    /// Set once a connection is live and held through sync, so the answer survives
    /// the states that forget it. Cleared the moment the session ends.
    @Published public private(set) var connectedPeer: String?

    /// True once Bluetooth or local-network access has been denied. The Nearby
    /// route reads it to render the §4.7 Settings recovery in place of the device
    /// list. Never surfaces a raw permission error.
    @Published public private(set) var permissionDenied = false

    /// How many things came over in this session's last accepted import — the
    /// concrete number behind "Synced". Nil until something has actually arrived,
    /// which is not the same as zero: zero would claim an empty sync happened.
    @Published public private(set) var itemsBroughtOver: UInt32?

    /// The count the peer offered, remembered while the person looks at it, so it
    /// can be reported once the import is accepted (the accepted state carries no
    /// count of its own).
    private var offeredCount: UInt32?

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
    private var host: NearbySpaceHost?
    private var pairing: SpacePairing?
    /// The peer's space, once they have announced it and this phone has none —
    /// held while the person decides whether to join. Nothing is joined until
    /// they say so.
    private var pendingAdoption: RiotSpace?
    private var listenerGeneration: UUID?
    /// The namespace of the community this session's coordinator belongs to,
    /// captured when the coordinator is adopted. A previewed import is admitted
    /// only while the host is still in this community — see `NearbyImportAdmission`.
    private var ownedNamespace: String?

    /// Whether to run Bluetooth discovery alongside the local network.
    ///
    /// Off in tests, and only there. Two peers in ONE process can never meet over
    /// the radio anyway — a single BLE controller does not hear its own
    /// advertisement — and merely constructing `CBCentralManager` inside an xctest
    /// host, which carries no Bluetooth usage string, aborts the process on a TCC
    /// privacy violation. That crash, not any network problem, is why the two-peer
    /// test has never once run to completion.
    private let usesBluetooth: Bool

    /// Fired after this phone has joined a peer's space, so the app can re-read a
    /// profile that now has a space it did not have a moment ago.
    public var onSpaceJoined: (() -> Void)?

    /// The Bonjour type discovery runs on. The app never sets it; a test passes a
    /// unique one so its two peers meet each other and not every other Riot on the
    /// machine. See `LocalNetworkNearbyService.serviceType`.
    private let serviceType: String

    public init(
        usesBluetooth: Bool = true,
        serviceType: String = LocalNetworkNearbyService.serviceType
    ) {
        self.usesBluetooth = usesBluetooth
        self.serviceType = serviceType
    }

    public func findNearby(host: NearbySpaceHost?) {
        resetSession()
        self.host = host
        state = .looking
        if usesBluetooth { startBluetooth() }

        let localService = makeLocalService()
        self.localService = localService
        localService.startLooking()
    }

    /// Bluetooth discovery, plus the TCP listener a Bluetooth pairing upgrades onto.
    /// Both belong to the radio path: a peer found over the local network arrives on
    /// a channel that is already the session's, with nothing to upgrade to.
    private func startBluetooth() {
        let service = makeService()
        self.service = service
        let generation = UUID()
        listenerGeneration = generation
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
    }

    /// A human tapped this device and chose to connect. This is the outbound
    /// half of "a human confirms every connection": discovery never dials on its
    /// own, so reaching here is always a deliberate human action, and it is that
    /// human's consent that `SpacePairing` carries as the confirmation token. The
    /// peer's own human must accept before either community is disclosed.
    public func requestConnection(to phone: DiscoveredPhone) {
        selected = phone
        isInboundRequest = false
        selectedIsLocal = localService?.canPair(with: phone) ?? false
        confirmConnection()
    }

    /// A peer is asking to pair with this device. Discovery never auto-accepts
    /// (nav design §"Nearby security and lifecycle"): this records the request and
    /// moves to a confirmation the human must accept. Nothing is dialled, and no
    /// community is disclosed, until they call `confirmConnection`.
    func receiveInboundPairingRequest(name: String, isLocal: Bool) {
        isInboundRequest = true
        selectedIsLocal = isLocal
        selected = DiscoveredPhone(id: UUID(), friendlyName: name)
        state = .confirm(name: name)
    }

    /// Records that Bluetooth or local-network access was denied, so the Nearby
    /// route can offer the §4.7 Settings recovery instead of an empty list.
    func notePermissionDenied() {
        permissionDenied = true
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
        host = nil
        state = .idle
    }

    private func resetSession() {
        pairing?.cancel()
        pairing = nil
        pendingAdoption = nil
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
        connectedPeer = nil
        itemsBroughtOver = nil
        offeredCount = nil
        ownedNamespace = nil
    }

    public func addPreviewedContent() {
        // Fail closed on the wrong-community race: a previewed import may commit
        // only while the host is still in the community whose session produced it.
        // If the selected community changed (or went) since the coordinator was
        // adopted, refuse — never repaint another community with this import.
        guard NearbyImportAdmission.permits(owned: ownedNamespace, current: host?.currentSpace?.namespaceID) else {
            coordinator?.rejectPreviewedContent()
            return
        }
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
        let localService = LocalNetworkNearbyService(serviceType: serviceType)
        localService.onPhonesChanged = { [weak self] phones in
            Task { @MainActor in
                guard let self else { return }
                self.localPhones = phones
                self.republishPhones()
            }
        }
        localService.onInboundPairingRequested = { [weak self] name in
            Task { @MainActor in
                self?.receiveInboundPairingRequest(name: name, isLocal: true)
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
                // They are gone: stop saying this device is connected to them.
                self.connectedPeer = nil
                if let selected = self.selected, self.selectedIsLocal {
                    self.state = .outOfRange(name: selected.friendlyName)
                }
            }
        }
        return localService
    }

    /// A peer found over the local link is already connected on the channel that
    /// carried the pairing, so that channel is the session's base route.
    private func startLocalSession(channel: FrameChannel, peer: NearbyPeerIdentity) {
        guard nearbyConnection == nil else { return }
        let connection = NearbyConnection(base: channel, baseRoute: .localNetwork, localAttempt: { nil })
        connection.confirmPairing()
        do {
            try connection.activate()
            nearbyConnection = connection
            activeRoute = connection.route
            beginSpaceHandshake(on: connection, peerName: peer.friendlyName)
        } catch {
            connection.disconnect()
            coordinator?.stop()
            state = .failed
        }
    }

    /// The UI sees one list; the two discovery paths never see the same peer, so
    /// a plain concatenation is correct.
    ///
    /// Discovery NEVER auto-connects (nav design §"Nearby security and lifecycle":
    /// "Discovery never auto-connects or auto-accepts"). Seeing a peer only lists
    /// it — a human taps a device and confirms before any connection is dialled,
    /// and the space announce that would disclose this community stays withheld
    /// until BOTH devices have confirmed (`SpacePairing`'s bilateral gate).
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
                self?.receiveInboundPairingRequest(name: name, isLocal: false)
            }
        }
        service.onAuthorizationDenied = { [weak self] in
            Task { @MainActor in self?.notePermissionDenied() }
        }
        service.onDisconnected = { [weak self] in
            Task { @MainActor in
                guard let self else { return }
                self.coordinator?.stop()
                // They are gone: stop saying this device is connected to them.
                self.connectedPeer = nil
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
            if let selected {
                beginSpaceHandshake(on: connection, peerName: selected.friendlyName)
            }
        } catch {
            connection.disconnect()
            coordinator?.stop()
            state = .failed
        }
    }

    /// Before anything is synced, the two phones say which space they are in.
    ///
    /// A phone with no space cannot even open a sync session (Rust refuses one
    /// without a space), so the space has to travel first — and the only thing
    /// that knows it is the phone standing next to this one. What happens next is
    /// `SpaceAdoption.decide`: sync, offer to join, refuse, or say there is
    /// nothing to share.
    private func beginSpaceHandshake(on connection: NearbyConnection, peerName: String) {
        guard let host else {
            state = .failed
            return
        }
        state = .connecting
        // The wire is up and this peer is the one on the other end of it. From
        // here the screen can name them, whatever the sync state does next.
        connectedPeer = peerName
        let pairing = SpacePairing(connection: connection, host: host, friendlyName: peerName)
        self.pairing = pairing
        pairing.begin(
            onDecision: { [weak self] decision in
                Task { @MainActor in self?.settle(decision, peerName: peerName) }
            },
            onFailure: { [weak self] in
                Task { @MainActor in self?.failSession() }
            }
        )
    }

    private func settle(_ decision: SpaceDecision, peerName: String) {
        guard pairing != nil else { return }
        switch decision {
        case .proceed:
            startSync(joining: nil, peerName: peerName)
        case let .adopt(space):
            // Their space is not taken silently. The person is told whose it is
            // and what it is called, and they choose.
            pendingAdoption = space
            state = .joinSpace(title: space.title, name: peerName)
        case .differentSpace:
            state = .differentSpace(name: peerName)
            endSession()
        case .nothingToShare:
            state = .nothingToShare
            endSession()
        }
    }

    /// The person said yes to joining the peer's space.
    public func confirmJoinSpace() {
        guard let space = pendingAdoption, let selected else { return }
        pendingAdoption = nil
        startSync(joining: space, peerName: selected.friendlyName)
    }

    /// The person said no. Nothing is joined, and the connection ends.
    public func declineJoinSpace() {
        pendingAdoption = nil
        state = .looking
        endSession()
    }

    private func startSync(joining space: RiotSpace?, peerName: String) {
        guard let pairing else { return }
        do {
            // `resume` joins first when adopting: Rust refuses a join while a sync
            // session is open, and refuses to open one without a space, so the
            // order is forced.
            let coordinator = try pairing.resume(joining: space)
            if space != nil { onSpaceJoined?() }
            adopt(coordinator)
            // Only now does the wire belong to the session — and whatever the peer
            // sent while this phone was deciding is replayed into it, in order.
            pairing.handOff(to: coordinator)
        } catch {
            failSession()
        }
    }

    private func failSession() {
        state = .failed
        endSession()
    }

    /// Tears down the connection without touching discovery: the person can still
    /// pick another phone from the list.
    private func endSession() {
        pairing?.cancel()
        pairing = nil
        pendingAdoption = nil
        coordinator?.stop()
        coordinator = nil
        nearbyConnection?.disconnect()
        nearbyConnection = nil
        activeRoute = nil
        // Every path here is a session ending WITHOUT a completed sync (a refused
        // join, a different space, nothing to share, a failure). A finished sync
        // leaves the connection up and never comes through here, which is why
        // "Synced — 6 things arrived" survives on screen and a dead session's
        // claims do not.
        connectedPeer = nil
        itemsBroughtOver = nil
        offeredCount = nil
        ownedNamespace = nil
    }

    /// Takes ownership of the session's coordinator and opens it from EXACTLY
    /// ONE side of the pairing.
    ///
    /// The core's `ReconcileSession` accepts a `Hello` only from an idle
    /// session, so two initiators fail each other and nothing replicates.
    /// The role (initiator vs responder) is decided deterministically from
    /// the two namespace IDs both peers know after the space handshake: the
    /// peer with the lexicographically smaller local namespace ID starts.
    /// This does not depend on discovery timing (`isInboundRequest`), which
    /// can race when both peers auto-connect simultaneously.
    private func adopt(_ coordinator: SyncCoordinator) {
        // Bind this session's imports to the community it opened against. By the
        // time we adopt, any join has completed, so the host's current namespace
        // IS the session's community — captured here and enforced on accept.
        ownedNamespace = host?.currentSpace?.namespaceID
        coordinator.onStateChanged = { [weak self] state in
            guard let self else { return }
            // Carry the offered count across to the accepted state, which does not
            // carry one — so "Synced" can say how many things actually arrived
            // instead of just asserting that something did.
            switch state {
            case let .preview(count, _): self.offeredCount = count
            case .caughtUp: self.itemsBroughtOver = self.offeredCount
            default: break
            }
            self.state = state
        }
        // A synced change must reach an app the person already has open. The
        // store is updated by the time this fires (it fires on accept, not on
        // receipt), so a live app re-reading now sees the imported items.
        coordinator.onImportAccepted = { AppRuntimeView.postDataChanged() }
        self.coordinator = coordinator
        if shouldStartSync { coordinator.start() } else { coordinator.answer() }
    }

    /// Deterministic sync-role tiebreaker: the peer whose local namespace ID
    /// is lexicographically smaller is the initiator. Both peers compute the
    /// same answer because both know both namespace IDs after the handshake.
    /// Falls back to `isInboundRequest` (the dialer starts) when the remote
    /// namespace is unknown (e.g. a spaceless peer adopting).
    private var shouldStartSync: Bool {
        let local = host?.currentSpace?.namespaceID ?? ""
        let remote = pairing?.remoteSpace?.namespaceID ?? ""
        if local.isEmpty && remote.isEmpty {
            return !isInboundRequest
        }
        return local < remote
    }
}
