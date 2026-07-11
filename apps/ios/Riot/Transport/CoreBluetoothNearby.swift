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

public final class CoreBluetoothNearbyService: NSObject, @unchecked Sendable {
    public var onPhonesChanged: (([DiscoveredPhone]) -> Void)?
    public var onPairingRequested: ((DiscoveredPhone) -> Void)?
    public var onInboundPairingRequested: ((String) -> Void)?
    public var onFrame: ((Data) -> Void)? {
        get { frameInbox.onReceive }
        set { frameInbox.onReceive = newValue }
    }
    public var onConnected: (() -> Void)?
    public var onRemoteEndpoint: ((LocalEndpoint?) -> Void)?
    public var onDisconnected: (() -> Void)?

    public let friendlyName: String
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
    private var subscribers: [CBCentral] = []
    private var outboundConfirmed = false
    private var inboundConfirmed = false
    private var localEndpoint: LocalEndpoint?
    private var centralWrites: [BLEWriteCursor] = []
    private var peripheralWrites: [BLEWriteCursor] = []
    private var centralDecoder = FrameDecoder()
    private var peripheralDecoder = FrameDecoder()
    private let frameInbox = BoundedFrameInbox()

    public override init() {
        var nonce = UInt64.random(in: UInt64.min...UInt64.max)
        friendlyName = FriendlyNameGenerator.name(sessionNonce: nonce)
        nonce = 0
        super.init()
        _ = central
        _ = peripheralManager
    }

    public func startLooking() {
        if central.state == .poweredOn {
            central.scanForPeripherals(withServices: [serviceID], options: [CBCentralManagerScanOptionAllowDuplicatesKey: false])
        }
        if peripheralManager.state == .poweredOn { startAdvertising() }
    }

    public func setLocalEndpoint(_ endpoint: LocalEndpoint?) {
        localEndpoint = endpoint
    }

    public func stop() {
        central.stopScan()
        peripheralManager.stopAdvertising()
        if let connected { central.cancelPeripheralConnection(connected) }
        centralWrites.removeAll()
        peripheralWrites.removeAll()
        subscribers.removeAll()
        outboundConfirmed = false
        inboundConfirmed = false
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
        inboundConfirmed = true
        var acknowledgement = Data([2])
        acknowledgement.append(PairingHandoff.encode(name: friendlyName, endpoint: localEndpoint))
        try notifyEnvelope(acknowledgement)
        onConnected?()
    }

    public func cancelInboundPairing() {
        inboundConfirmed = false
    }

    public func sendFrame(_ frame: Data) throws {
        guard frame.count <= NearbyLimits.maxFrameBytes else { throw NearbyTransportError.disconnected }
        var envelope = Data([3])
        envelope.append(frame)
        if let connected, let characteristic = remoteFrameCharacteristic, outboundConfirmed {
            try writeEnvelope(envelope, to: connected, characteristic: characteristic)
        } else if inboundConfirmed, !subscribers.isEmpty {
            try notifyEnvelope(envelope)
        } else {
            throw NearbyTransportError.pairingNotConfirmed
        }
    }

    private func writeEnvelope(_ envelope: Data, to connected: CBPeripheral, characteristic: CBCharacteristic) throws {
        let encoded = try FrameDecoder.encode(envelope)
        guard centralWrites.count < NearbyLimits.maxPendingFrames,
              centralWrites.reduce(0, { $0 + $1.remainingCount }) + encoded.count <= NearbyLimits.maxFrameBytes + 5 else {
            stop()
            throw NearbyTransportError.disconnected
        }
        centralWrites.append(BLEWriteCursor(data: encoded))
        drainCentralChunks()
    }

    private func notifyEnvelope(_ envelope: Data) throws {
        guard let characteristic = localFrameCharacteristic, !subscribers.isEmpty else {
            throw NearbyTransportError.notConnected
        }
        let encoded = try FrameDecoder.encode(envelope)
        guard peripheralWrites.count < NearbyLimits.maxPendingFrames,
              peripheralWrites.reduce(0, { $0 + $1.remainingCount }) + encoded.count <= NearbyLimits.maxFrameBytes + 5 else {
            stop()
            throw NearbyTransportError.disconnected
        }
        peripheralWrites.append(BLEWriteCursor(data: encoded))
        drainPeripheralChunks(characteristic: characteristic)
    }

    private func drainCentralChunks() {
        guard let connected, let characteristic = remoteFrameCharacteristic else { return }
        while connected.canSendWriteWithoutResponse, !centralWrites.isEmpty {
            let next = centralWrites[0].nextChunk(limit: max(20, connected.maximumWriteValueLength(for: .withoutResponse)))
            connected.writeValue(next, for: characteristic, type: .withoutResponse)
            if centralWrites[0].isComplete { centralWrites.removeFirst() }
        }
    }

    private func drainPeripheralChunks(characteristic: CBMutableCharacteristic? = nil) {
        guard let characteristic = characteristic ?? localFrameCharacteristic else { return }
        while !peripheralWrites.isEmpty {
            let limit = max(20, subscribers.map(\.maximumUpdateValueLength).min() ?? 20)
            let next = peripheralWrites[0].peekChunk(limit: limit)
            guard peripheralManager.updateValue(next, for: characteristic, onSubscribedCentrals: subscribers) else { return }
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

    private func accept(_ bytes: Data, fromCentralRole: Bool) {
        let frames: [Data]?
        if fromCentralRole {
            frames = try? centralDecoder.append(bytes)
        } else {
            frames = try? peripheralDecoder.append(bytes)
        }
        guard let frames else { return }
        for envelope in frames {
            guard let kind = envelope.first else { continue }
            let payload = envelope.dropFirst()
            switch kind {
            case 1:
                guard let handoff = PairingHandoff.decode(Data(payload)) else { continue }
                onRemoteEndpoint?(handoff.endpoint)
                onInboundPairingRequested?(handoff.name)
            case 2:
                guard let handoff = PairingHandoff.decode(Data(payload)) else { continue }
                onRemoteEndpoint?(handoff.endpoint)
                outboundConfirmed = true
                onConnected?()
            case 3 where outboundConfirmed || inboundConfirmed:
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
        onDisconnected?()
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
        request.append(PairingHandoff.encode(name: friendlyName, endpoint: localEndpoint))
        try? writeEnvelope(request, to: peripheral, characteristic: characteristic)
    }

    public func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, error: Error?) {
        if let value = characteristic.value { accept(value, fromCentralRole: true) }
    }

    public func peripheralIsReady(toSendWriteWithoutResponse peripheral: CBPeripheral) {
        drainCentralChunks()
    }
}

private enum PairingHandoff {
    static func encode(name: String, endpoint: LocalEndpoint?) -> Data {
        let host = endpoint?.host ?? ""
        let port = endpoint.map { String($0.port) } ?? ""
        return Data("\(name)\n\(host)\n\(port)".utf8)
    }

    static func decode(_ data: Data) -> (name: String, endpoint: LocalEndpoint?)? {
        guard let value = String(data: data, encoding: .utf8), value.utf8.count <= 256 else { return nil }
        let fields = value.split(separator: "\n", omittingEmptySubsequences: false)
        guard fields.count == 3, !fields[0].isEmpty, fields[0].count <= 64 else { return nil }
        let endpoint: LocalEndpoint?
        if fields[1].isEmpty && fields[2].isEmpty {
            endpoint = nil
        } else if let port = UInt16(fields[2]), let valid = LocalEndpoint(host: String(fields[1]), port: port) {
            endpoint = valid
        } else {
            return nil
        }
        return (String(fields[0]), endpoint)
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
            if let value = request.value { accept(value, fromCentralRole: false) }
            peripheral.respond(to: request, withResult: .success)
        }
    }

    public func peripheralManager(_ peripheral: CBPeripheralManager, central: CBCentral, didSubscribeTo characteristic: CBCharacteristic) {
        if !subscribers.contains(where: { $0.identifier == central.identifier }) { subscribers.append(central) }
    }

    public func peripheralManager(_ peripheral: CBPeripheralManager, central: CBCentral, didUnsubscribeFrom characteristic: CBCharacteristic) {
        subscribers.removeAll { $0.identifier == central.identifier }
        if subscribers.isEmpty { onDisconnected?() }
    }

    public func peripheralManagerIsReady(toUpdateSubscribers peripheral: CBPeripheralManager) {
        drainPeripheralChunks()
    }
}

public final class CoreBluetoothFrameChannel: FrameChannel, @unchecked Sendable {
    public var onReceive: ((Data) -> Void)?
    private let service: CoreBluetoothNearbyService

    public init(service: CoreBluetoothNearbyService) {
        self.service = service
        service.onFrame = { [weak self] in self?.onReceive?($0) }
    }

    public func send(_ frame: Data) throws { try service.sendFrame(frame) }
    public func disconnect() { service.stop() }
}
