import Foundation
@preconcurrency import CoreBluetooth

public struct DiscoveredPhone: Identifiable, Equatable, Sendable {
    public let id: UUID
    public let friendlyName: String
}

struct BLEWriteCursor {
    let data: Data
    private(set) var offset = 0
    var remainingCount: Int { data.count - offset }
    var retainedCount: Int { data.count }
    var isComplete: Bool { offset == data.count }

    mutating func nextChunk(limit: Int) -> Data {
        let chunk = peekChunk(limit: limit)
        advance(by: chunk.count)
        return chunk
    }

    func peekChunk(limit: Int) -> Data {
        data.subdata(in: offset..<min(offset + limit, data.count))
    }

    mutating func advance(by count: Int) {
        offset = min(offset + count, data.count)
    }
}

enum BLEEnvelope {
    static func content(_ frame: Data) throws -> Data {
        guard frame.count <= NearbyLimits.maxFrameBytes else { throw NearbyTransportError.disconnected }
        var envelope = Data([3])
        envelope.append(frame)
        return envelope
    }
}

struct BLEPeripheralPeerRegistry {
    private var decoders: [UUID: FrameDecoder] = [:]
    private(set) var pendingPeer: UUID?
    private(set) var confirmedPeer: UUID?

    mutating func beginPairing(with peer: UUID) -> Bool {
        guard confirmedPeer == nil || confirmedPeer == peer,
              pendingPeer == nil || pendingPeer == peer else { return false }
        pendingPeer = peer
        return true
    }

    mutating func validatedPairingRequest(_ payload: Data, from peer: UUID) -> (name: String, endpoint: LocalEndpoint?, tieBreaker: String)? {
        guard let handoff = PairingHandoff.decode(payload), beginPairing(with: peer) else { return nil }
        return handoff
    }

    mutating func confirmPending() -> UUID? {
        guard let pendingPeer else { return nil }
        confirmedPeer = pendingPeer
        self.pendingPeer = nil
        return pendingPeer
    }

    mutating func cancelPending() { pendingPeer = nil }
    func acceptsContent(from peer: UUID) -> Bool { confirmedPeer == peer }

    mutating func decode(_ bytes: Data, from peer: UUID) throws -> [Data] {
        var decoder = decoders[peer] ?? FrameDecoder(maxFrameBytes: NearbyLimits.maxBLEEnvelopeBytes)
        do {
            let frames = try decoder.append(bytes)
            decoders[peer] = decoder
            return frames
        } catch {
            decoders.removeValue(forKey: peer)
            throw error
        }
    }

    mutating func remove(_ peer: UUID) {
        decoders.removeValue(forKey: peer)
        if pendingPeer == peer { pendingPeer = nil }
        if confirmedPeer == peer { confirmedPeer = nil }
    }
}

public final class CoreBluetoothNearbyService: NSObject, @unchecked Sendable {
    public var onPhonesChanged: (([DiscoveredPhone]) -> Void)?
    public var onPairingRequested: ((DiscoveredPhone) -> Void)?
    public var onInboundPairingRequested: ((String) -> Void)?
    public var onFrame: ((Data) -> Void)? {
        get { frameInbox.onReceive }
        set { frameInbox.onReceive = newValue }
    }
    public var onConnected: ((LocalEndpoint?, String) -> Void)?
    public var onDisconnected: (() -> Void)?

    public let friendlyName: String
    public let tieBreaker: String
    private let serviceID = CBUUID(string: "91F1B3A0-37A8-4EF5-8AB5-A9826D00A001")
    private let frameID = CBUUID(string: "91F1B3A0-37A8-4EF5-8AB5-A9826D00A002")
    private lazy var central = CBCentralManager(delegate: self, queue: nil)
    private lazy var peripheralManager = CBPeripheralManager(delegate: self, queue: nil)
    private var phones: [UUID: DiscoveredPhone] = [:]
    private var peripherals: [UUID: CBPeripheral] = [:]
    private var pending: CBPeripheral?
    private var connected: CBPeripheral?
    private var remoteFrameCharacteristic: CBCharacteristic?
    private var localFrameCharacteristic: CBMutableCharacteristic?
    private var subscribers: [UUID: CBCentral] = [:]
    private var outboundConfirmed = false
    private var localEndpoint: LocalEndpoint?
    private var pendingRemoteEndpoint: LocalEndpoint?
    private var pendingRemoteTieBreaker: String?
    private var centralWrites: [BLEWriteCursor] = []
    private var peripheralWrites: [BLEWriteCursor] = []
    private var centralDecoder = FrameDecoder(maxFrameBytes: NearbyLimits.maxBLEEnvelopeBytes)
    private var peripheralPeers = BLEPeripheralPeerRegistry()
    private let frameInbox = BoundedFrameInbox()
    private var isStopping = false

    public override init() {
        var nonce = UInt64.random(in: UInt64.min...UInt64.max)
        friendlyName = FriendlyNameGenerator.name(sessionNonce: nonce)
        tieBreaker = UUID().uuidString
        nonce = 0
        super.init()
        _ = central
        _ = peripheralManager
    }

    public func startLooking() {
        isStopping = false
        if central.state == .poweredOn {
            central.scanForPeripherals(withServices: [serviceID], options: [CBCentralManagerScanOptionAllowDuplicatesKey: false])
        }
        if peripheralManager.state == .poweredOn { startAdvertising() }
    }

    public func setLocalEndpoint(_ endpoint: LocalEndpoint?) {
        localEndpoint = endpoint
    }

    public func stop() {
        isStopping = true
        central.stopScan()
        peripheralManager.stopAdvertising()
        if let connected { central.cancelPeripheralConnection(connected) }
        centralWrites.removeAll()
        peripheralWrites.removeAll()
        subscribers.removeAll()
        outboundConfirmed = false
        pendingRemoteEndpoint = nil
        pendingRemoteTieBreaker = nil
        peripheralPeers = BLEPeripheralPeerRegistry()
        centralDecoder = FrameDecoder(maxFrameBytes: NearbyLimits.maxBLEEnvelopeBytes)
    }

    public func requestPairing(with phone: DiscoveredPhone) {
        guard let peripheral = peripherals[phone.id] else { return }
        pending = peripheral
        onPairingRequested?(phone)
    }

    public func confirmPairing() {
        guard let pending else { return }
        central.connect(pending)
        self.pending = nil
    }

    public func cancelPairing() {
        pending = nil
    }

    public func confirmInboundPairing() throws {
        guard let pendingPeer = peripheralPeers.pendingPeer, let peer = subscribers[pendingPeer],
              let remoteTieBreaker = pendingRemoteTieBreaker else {
            throw NearbyTransportError.notConnected
        }
        guard peripheralPeers.confirmPending() == pendingPeer else { throw NearbyTransportError.notConnected }
        var acknowledgement = Data([2])
        acknowledgement.append(PairingHandoff.encode(name: friendlyName, endpoint: localEndpoint, tieBreaker: tieBreaker))
        do {
            try notifyEnvelope(acknowledgement, to: peer)
        } catch {
            peripheralPeers.remove(pendingPeer)
            pendingRemoteEndpoint = nil
            pendingRemoteTieBreaker = nil
            throw error
        }
        onConnected?(pendingRemoteEndpoint, remoteTieBreaker)
    }

    public func cancelInboundPairing() {
        peripheralPeers.cancelPending()
        pendingRemoteEndpoint = nil
        pendingRemoteTieBreaker = nil
    }

    public func sendFrame(_ frame: Data) throws {
        let envelope = try BLEEnvelope.content(frame)
        if let connected, let characteristic = remoteFrameCharacteristic, outboundConfirmed {
            try writeEnvelope(envelope, to: connected, characteristic: characteristic)
        } else if let peerID = peripheralPeers.confirmedPeer, let peer = subscribers[peerID] {
            try notifyEnvelope(envelope, to: peer)
        } else {
            throw NearbyTransportError.pairingNotConfirmed
        }
    }

    private func writeEnvelope(_ envelope: Data, to connected: CBPeripheral, characteristic: CBCharacteristic) throws {
        let encoded = try FrameDecoder.encode(envelope, maxFrameBytes: NearbyLimits.maxBLEEnvelopeBytes)
        guard centralWrites.count < NearbyLimits.maxPendingFrames,
              centralWrites.reduce(0, { $0 + $1.retainedCount }) + encoded.count <= NearbyLimits.maxBLEEnvelopeBytes + 4 else {
            stop()
            throw NearbyTransportError.disconnected
        }
        centralWrites.append(BLEWriteCursor(data: encoded))
        drainCentralChunks()
    }

    private func notifyEnvelope(_ envelope: Data, to peer: CBCentral) throws {
        guard let characteristic = localFrameCharacteristic else {
            throw NearbyTransportError.notConnected
        }
        let encoded = try FrameDecoder.encode(envelope, maxFrameBytes: NearbyLimits.maxBLEEnvelopeBytes)
        guard peripheralWrites.count < NearbyLimits.maxPendingFrames,
              peripheralWrites.reduce(0, { $0 + $1.retainedCount }) + encoded.count <= NearbyLimits.maxBLEEnvelopeBytes + 4 else {
            stop()
            throw NearbyTransportError.disconnected
        }
        peripheralWrites.append(BLEWriteCursor(data: encoded))
        drainPeripheralChunks(characteristic: characteristic, peer: peer)
    }

    private func drainCentralChunks() {
        guard let connected, let characteristic = remoteFrameCharacteristic else { return }
        while connected.canSendWriteWithoutResponse, !centralWrites.isEmpty {
            let next = centralWrites[0].nextChunk(limit: max(20, connected.maximumWriteValueLength(for: .withoutResponse)))
            connected.writeValue(next, for: characteristic, type: .withoutResponse)
            if centralWrites[0].isComplete { centralWrites.removeFirst() }
        }
    }

    private func drainPeripheralChunks(characteristic: CBMutableCharacteristic? = nil, peer: CBCentral? = nil) {
        guard let characteristic = characteristic ?? localFrameCharacteristic,
              let target = peer ?? peripheralPeers.confirmedPeer.flatMap({ subscribers[$0] }) else { return }
        while !peripheralWrites.isEmpty {
            let limit = max(20, target.maximumUpdateValueLength)
            let next = peripheralWrites[0].peekChunk(limit: limit)
            guard peripheralManager.updateValue(next, for: characteristic, onSubscribedCentrals: [target]) else { return }
            peripheralWrites[0].advance(by: next.count)
            if peripheralWrites[0].isComplete { peripheralWrites.removeFirst() }
        }
    }

    private func startAdvertising() {
        guard localFrameCharacteristic == nil else {
            peripheralManager.startAdvertising([CBAdvertisementDataServiceUUIDsKey: [serviceID], CBAdvertisementDataLocalNameKey: friendlyName])
            return
        }
        let characteristic = CBMutableCharacteristic(
            type: frameID,
            properties: [.write, .writeWithoutResponse, .notify],
            value: nil,
            permissions: [.writeable]
        )
        localFrameCharacteristic = characteristic
        let service = CBMutableService(type: serviceID, primary: true)
        service.characteristics = [characteristic]
        peripheralManager.add(service)
    }

    private func acceptFromConnectedPeripheral(_ bytes: Data) {
        do {
            try acceptEnvelopes(centralDecoder.append(bytes), peerID: nil)
        } catch {
            stop()
            onDisconnected?()
        }
    }

    private func acceptFromSubscribedCentral(_ bytes: Data, peerID: UUID) {
        do {
            try acceptEnvelopes(peripheralPeers.decode(bytes, from: peerID), peerID: peerID)
        } catch {
            let wasConfirmed = peripheralPeers.acceptsContent(from: peerID)
            peripheralPeers.remove(peerID)
            if wasConfirmed {
                stop()
                onDisconnected?()
            }
        }
    }

    private func acceptEnvelopes(_ frames: [Data], peerID: UUID?) throws {
        for envelope in frames {
            guard let kind = envelope.first else { continue }
            let payload = envelope.dropFirst()
            switch kind {
            case 1:
                guard let peerID,
                      let handoff = peripheralPeers.validatedPairingRequest(Data(payload), from: peerID) else { continue }
                pendingRemoteEndpoint = handoff.endpoint
                pendingRemoteTieBreaker = handoff.tieBreaker
                onInboundPairingRequested?(handoff.name)
            case 2:
                guard peerID == nil else { continue }
                guard let handoff = PairingHandoff.decode(Data(payload)) else { continue }
                outboundConfirmed = true
                onConnected?(handoff.endpoint, handoff.tieBreaker)
            case 3 where (peerID == nil && outboundConfirmed)
                || (peerID.map { peripheralPeers.acceptsContent(from: $0) } ?? false):
                if !frameInbox.receive(Data(payload)) {
                    stop()
                    onDisconnected?()
                }
            default:
                continue
            }
        }
    }
}

extension CoreBluetoothNearbyService: CBCentralManagerDelegate, CBPeripheralDelegate {
    public func centralManagerDidUpdateState(_ central: CBCentralManager) {
        if central.state == .poweredOn { startLooking() }
    }

    public func centralManager(_ central: CBCentralManager, didDiscover peripheral: CBPeripheral, advertisementData: [String: Any], rssi RSSI: NSNumber) {
        guard let name = advertisementData[CBAdvertisementDataLocalNameKey] as? String else { return }
        let phone = DiscoveredPhone(id: peripheral.identifier, friendlyName: name)
        phones[phone.id] = phone
        peripherals[phone.id] = peripheral
        onPhonesChanged?(phones.values.sorted { $0.friendlyName < $1.friendlyName })
    }

    public func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        connected = peripheral
        peripheral.delegate = self
        peripheral.discoverServices([serviceID])
    }

    public func centralManager(_ central: CBCentralManager, didDisconnectPeripheral peripheral: CBPeripheral, error: Error?) {
        connected = nil
        remoteFrameCharacteristic = nil
        centralWrites.removeAll()
        if !isStopping { onDisconnected?() }
    }

    public func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        peripheral.services?.forEach { peripheral.discoverCharacteristics([frameID], for: $0) }
    }

    public func peripheral(_ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService, error: Error?) {
        guard let characteristic = service.characteristics?.first(where: { $0.uuid == frameID }) else { return }
        remoteFrameCharacteristic = characteristic
        peripheral.setNotifyValue(true, for: characteristic)
    }

    public func peripheral(_ peripheral: CBPeripheral, didUpdateNotificationStateFor characteristic: CBCharacteristic, error: Error?) {
        guard error == nil, characteristic.isNotifying else { return }
        var request = Data([1])
        request.append(PairingHandoff.encode(name: friendlyName, endpoint: localEndpoint, tieBreaker: tieBreaker))
        do {
            try writeEnvelope(request, to: peripheral, characteristic: characteristic)
        } catch {
            isStopping = true
            central.cancelPeripheralConnection(peripheral)
            onDisconnected?()
        }
    }

    public func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, error: Error?) {
        if let value = characteristic.value { acceptFromConnectedPeripheral(value) }
    }

    public func peripheralIsReady(toSendWriteWithoutResponse peripheral: CBPeripheral) {
        drainCentralChunks()
    }
}

enum PairingHandoff {
    static func encode(name: String, endpoint: LocalEndpoint?, tieBreaker: String) -> Data {
        let host = endpoint?.host ?? ""
        let port = endpoint.map { String($0.port) } ?? ""
        return Data("\(name)\n\(host)\n\(port)\n\(tieBreaker)".utf8)
    }

    static func decode(_ data: Data) -> (name: String, endpoint: LocalEndpoint?, tieBreaker: String)? {
        guard let value = String(data: data, encoding: .utf8), value.utf8.count <= 256 else { return nil }
        let fields = value.split(separator: "\n", omittingEmptySubsequences: false)
        guard fields.count == 4, !fields[0].isEmpty, fields[0].count <= 64,
              !fields[3].isEmpty, fields[3].count <= 64 else { return nil }
        let endpoint: LocalEndpoint?
        if fields[1].isEmpty && fields[2].isEmpty {
            endpoint = nil
        } else if let port = UInt16(fields[2]), let valid = LocalEndpoint(host: String(fields[1]), port: port) {
            endpoint = valid
        } else {
            return nil
        }
        return (String(fields[0]), endpoint, String(fields[3]))
    }
}

extension CoreBluetoothNearbyService: CBPeripheralManagerDelegate {
    public func peripheralManagerDidUpdateState(_ peripheral: CBPeripheralManager) {
        if peripheral.state == .poweredOn { startAdvertising() }
    }

    public func peripheralManager(_ peripheral: CBPeripheralManager, didAdd service: CBService, error: Error?) {
        if error == nil { startAdvertising() }
    }

    public func peripheralManager(_ peripheral: CBPeripheralManager, didReceiveWrite requests: [CBATTRequest]) {
        for request in requests {
            if let value = request.value { acceptFromSubscribedCentral(value, peerID: request.central.identifier) }
            peripheral.respond(to: request, withResult: .success)
        }
    }

    public func peripheralManager(_ peripheral: CBPeripheralManager, central: CBCentral, didSubscribeTo characteristic: CBCharacteristic) {
        subscribers[central.identifier] = central
    }

    public func peripheralManager(_ peripheral: CBPeripheralManager, central: CBCentral, didUnsubscribeFrom characteristic: CBCharacteristic) {
        subscribers.removeValue(forKey: central.identifier)
        let wasConfirmed = peripheralPeers.confirmedPeer == central.identifier
        peripheralPeers.remove(central.identifier)
        if wasConfirmed && !isStopping { onDisconnected?() }
    }

    public func peripheralManagerIsReady(toUpdateSubscribers peripheral: CBPeripheralManager) {
        drainPeripheralChunks()
    }
}

public final class CoreBluetoothFrameChannel: FrameChannel, @unchecked Sendable {
    public var onReceive: ((Data) -> Void)?
    public var onFailure: (() -> Void)?
    private let service: CoreBluetoothNearbyService

    public init(service: CoreBluetoothNearbyService) {
        self.service = service
        service.onFrame = { [weak self] in self?.onReceive?($0) }
    }

    public func send(_ frame: Data) throws { try service.sendFrame(frame) }
    public func disconnect() { service.stop() }
}
