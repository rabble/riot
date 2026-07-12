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

    /// The device this one is in a session with RIGHT NOW, or nil if it is only
    /// looking.
    ///
    /// The state enum cannot answer this: its terminal cases (`caughtUp`,
    /// `alreadyCurrent`) carry no name, so the screen could say "All caught up"
    /// without ever saying caught up WITH WHOM — which was the whole complaint.
    /// Set once a connection is live and held through sync, so the answer survives
    /// the states that forget it. Cleared the moment the session ends.
    @Published public private(set) var connectedPeer: String?

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

    /// Fired after this phone has joined a peer's space, so the app can re-read a
    /// profile that now has a space it did not have a moment ago.
    public var onSpaceJoined: (() -> Void)?

    public init() {}

    public func findNearby(host: NearbySpaceHost?) {
        resetSession()
        self.host = host
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
        // Auto-connect: connecting to a peer is not a decision worth a tap —
        // it moves no data by itself, and every byte still passes the review
        // gate before it lands. (Joining a SPACE is a real decision and keeps
        // its confirmation.) Go straight to what the confirm tap used to do.
        confirmConnection()
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
                // Auto-accept: a peer reaching us is not a decision worth a tap.
                self.confirmConnection()
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
    private func republishPhones() {
        phones = bluetoothPhones + localPhones
        autoConnectToFirstPeer()
    }

    /// Auto-connect: a peer we can see is a peer we connect to. Nobody should
    /// have to tap "connect" to a phone standing next to them — and connecting
    /// moves no data on its own; the review gate still stands between a peer
    /// and this store. Only dials while idle, so an in-flight session is never
    /// interrupted, and never re-dials a peer already selected.
    private func autoConnectToFirstPeer() {
        guard case .idle = state, selected == nil else { return }
        guard let peer = phones.first else { return }
        requestConnection(to: peer)
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
                self.selectedIsLocal = false
                // Auto-accept: a peer reaching us is not a decision worth a tap.
                self.confirmConnection()
            }
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
    }

    /// Takes ownership of the session's coordinator and opens it from EXACTLY
    /// ONE side of the pairing.
    ///
    /// Both `startLocalSession` and `finishRouteSelection` run on both peers, so
    /// whichever of them built the coordinator, only one device may `start()`
    /// it: the core accepts a `Hello` only from an idle session, so two
    /// initiators fail each other and nothing replicates. `isInboundRequest` is
    /// the asymmetry already on hand — the person who tapped the other phone's
    /// name dialled, and the person who answered the prompt did not — and it is
    /// true on exactly one side of every pairing, over either transport.
    private func adopt(_ coordinator: SyncCoordinator) {
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
        if isInboundRequest { coordinator.answer() } else { coordinator.start() }
    }
}
