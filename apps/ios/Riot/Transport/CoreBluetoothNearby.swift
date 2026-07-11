import Foundation
@preconcurrency import CoreBluetooth

public struct DiscoveredPhone: Identifiable, Equatable, Sendable {
    public let id: UUID
    public let friendlyName: String
}

public final class CoreBluetoothNearbyService: NSObject, @unchecked Sendable {
    public var onPhonesChanged: (([DiscoveredPhone]) -> Void)?
    public var onPairingRequested: ((DiscoveredPhone) -> Void)?
    public var onInboundPairingRequested: ((String) -> Void)?
    public var onFrame: ((Data) -> Void)?
    public var onConnected: (() -> Void)?
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
    private var decoder = FrameDecoder()

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

    public func stop() {
        central.stopScan()
        peripheralManager.stopAdvertising()
        if let connected { central.cancelPeripheralConnection(connected) }
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
        try notifyEnvelope(Data([2]))
        onConnected?()
    }

    public func cancelInboundPairing() {
        inboundConfirmed = false
    }

    public func sendFrame(_ frame: Data) throws {
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
        let encoded = FrameDecoder.encode(envelope)
        let limit = max(20, connected.maximumWriteValueLength(for: .withoutResponse))
        for offset in stride(from: 0, to: encoded.count, by: limit) {
            connected.writeValue(encoded.subdata(in: offset..<min(offset + limit, encoded.count)), for: characteristic, type: .withoutResponse)
        }
    }

    private func notifyEnvelope(_ envelope: Data) throws {
        guard let characteristic = localFrameCharacteristic, !subscribers.isEmpty else {
            throw NearbyTransportError.notConnected
        }
        let encoded = FrameDecoder.encode(envelope)
        let limit = max(20, peripheralManager.maximumUpdateValueLength)
        for offset in stride(from: 0, to: encoded.count, by: limit) {
            let chunk = encoded.subdata(in: offset..<min(offset + limit, encoded.count))
            guard peripheralManager.updateValue(chunk, for: characteristic, onSubscribedCentrals: subscribers) else {
                throw NearbyTransportError.disconnected
            }
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

    private func accept(_ bytes: Data) {
        guard let frames = try? decoder.append(bytes) else { return }
        for envelope in frames {
            guard let kind = envelope.first else { continue }
            let payload = envelope.dropFirst()
            switch kind {
            case 1:
                guard let name = String(data: payload, encoding: .utf8), !name.isEmpty, name.count <= 64 else { continue }
                onInboundPairingRequested?(name)
            case 2:
                outboundConfirmed = true
                onConnected?()
            case 3 where outboundConfirmed || inboundConfirmed:
                onFrame?(Data(payload))
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
        request.append(Data(friendlyName.utf8))
        try? writeEnvelope(request, to: peripheral, characteristic: characteristic)
    }

    public func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, error: Error?) {
        if let value = characteristic.value { accept(value) }
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
            if let value = request.value { accept(value) }
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
}

public final class CoreBluetoothFrameChannel: FrameChannel {
    public var onReceive: ((Data) -> Void)?
    private let service: CoreBluetoothNearbyService

    public init(service: CoreBluetoothNearbyService) {
        self.service = service
        service.onFrame = { [weak self] in self?.onReceive?($0) }
    }

    public func send(_ frame: Data) throws { try service.sendFrame(frame) }
    public func disconnect() { service.stop() }
}
