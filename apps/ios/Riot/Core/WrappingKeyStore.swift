import Foundation
import OSLog
import Security

public protocol WrappingKeyStore {
    func loadOrCreateWrappingKey() throws -> Data
}

public final class KeychainWrappingKeyStore: WrappingKeyStore {
    private static let logger = Logger(subsystem: "net.protest.riot", category: "identity-keychain")
    private let service: String
    private let account: String

    public init(
        service: String = "net.protest.riot.identity-wrapping.v2",
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
            lookupQuery() as CFDictionary,
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

        var item = identityAttributes()
        item[kSecValueData as String] = key
        item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly
        var status = SecItemAdd(item as CFDictionary, nil)
        var usedFallback = false

        // Simulators and devices without a configured passcode cannot use the
        // strongest passcode-gated class. Keep the key device-only and unlocked
        // in that environment; never fall back to a synchronizable class.
        if status == errSecParam || status == errSecNotAvailable || status == errSecAuthFailed {
            item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
            status = SecItemAdd(item as CFDictionary, nil)
            usedFallback = true
        }
        if status == errSecDuplicateItem {
            let (readStatus, existing) = read()
            guard readStatus == errSecSuccess, let existing else {
                throw KeychainWrappingKeyError.status(readStatus)
            }
            return existing
        }
        guard status == errSecSuccess else { throw KeychainWrappingKeyError.status(status) }
        if usedFallback {
            Self.logger.warning("Wrapping key stored with when-unlocked-this-device-only simulator fallback")
        } else {
            Self.logger.notice("Wrapping key stored with when-passcode-set-this-device-only accessibility")
        }
        return key
    }

    private func identityAttributes() -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecAttrSynchronizable as String: false,
        ]
    }

    private func lookupQuery() -> [String: Any] {
        var query = identityAttributes()
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        return query
    }
}

public enum KeychainWrappingKeyError: Error {
    case randomGeneration
    case status(OSStatus)
}
