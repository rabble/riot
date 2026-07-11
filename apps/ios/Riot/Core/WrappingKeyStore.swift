import Foundation
import Security

public protocol WrappingKeyStore {
    func loadOrCreateWrappingKey() throws -> Data
}

public final class KeychainWrappingKeyStore: WrappingKeyStore {
    private let service: String
    private let account: String

    public init(
        service: String = "net.protest.riot.identity-wrapping",
        account: String = "local-profile"
    ) {
        self.service = service
        self.account = account
    }

    public func loadOrCreateWrappingKey() throws -> Data {
        let (status, existing) = read()
        switch (status, existing) {
        case (errSecSuccess, let key?):
            return key
        case (errSecItemNotFound, _):
            return try create()
        default:
            throw KeychainWrappingKeyError.status(status)
        }
    }

    private func read() -> (OSStatus, Data?) {
        var result: CFTypeRef?
        let status = SecItemCopyMatching(
            baseQuery(returnData: true) as CFDictionary,
            &result
        )
        guard status == errSecSuccess else { return (status, nil) }
        guard let key = result as? Data, key.count == 32 else { return (errSecDecode, nil) }
        return (errSecSuccess, key)
    }

    private func create() throws -> Data {
        var bytes = [UInt8](repeating: 0, count: 32)
        defer { _ = bytes.withUnsafeMutableBytes { $0.initializeMemory(as: UInt8.self, repeating: 0) } }
        let randomStatus = bytes.withUnsafeMutableBytes { buffer in
            SecRandomCopyBytes(kSecRandomDefault, buffer.count, buffer.baseAddress!)
        }
        guard randomStatus == errSecSuccess else {
            throw KeychainWrappingKeyError.randomGeneration
        }
        var key = Data(bytes)
        defer { key.resetBytes(in: key.startIndex..<key.endIndex) }

        var item = baseQuery(returnData: false)
        item[kSecValueData as String] = key
        item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly
        var status = SecItemAdd(item as CFDictionary, nil)

        // Simulators and devices without a configured passcode cannot use the
        // strongest passcode-gated class. Keep the key device-only and unlocked
        // in that environment; never fall back to a synchronizable class.
        if status == errSecParam || status == errSecNotAvailable || status == errSecAuthFailed {
            item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
            status = SecItemAdd(item as CFDictionary, nil)
        }
        if status == errSecDuplicateItem {
            let (readStatus, existing) = read()
            guard readStatus == errSecSuccess, let existing else {
                throw KeychainWrappingKeyError.status(readStatus)
            }
            return existing
        }
        guard status == errSecSuccess else { throw KeychainWrappingKeyError.status(status) }
        return Data(bytes)
    }

    private func baseQuery(returnData: Bool) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecAttrSynchronizable as String: false,
            kSecReturnData as String: returnData,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
    }
}

public enum KeychainWrappingKeyError: Error {
    case randomGeneration
    case status(OSStatus)
}
