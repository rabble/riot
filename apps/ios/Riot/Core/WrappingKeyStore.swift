import Foundation
import OSLog
import Security

public protocol WrappingKeyStore {
    func loadOrCreateWrappingKey() throws -> Data
}

public final class KeychainWrappingKeyStore: DestructibleSecretStore {
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
            // The ONLY outcome that preserves an existing identity. Everything
            // else below mints a new key, and a new key cannot unseal the
            // identity already on disk — the profile is then quarantined at
            // open and the person silently becomes a new author with no
            // authority in their own community. That failure was invisible
            // because this path said nothing; it says something now.
            Self.logger.notice("wrapping key loaded from keychain (existing identity preserved)")
            return key
        case (errSecItemNotFound, _):
            Self.logger.error(
                """
                wrapping key NOT FOUND in keychain (service=\(self.service, privacy: .public), \
                account=\(self.account, privacy: .public)) — minting a NEW key. Any identity \
                sealed with the previous key can no longer be opened and its profile will be \
                quarantined at next open.
                """
            )
            return try create()
        default:
            Self.logger.error(
                """
                wrapping key UNREADABLE: OSStatus=\(status, privacy: .public) \
                (service=\(self.service, privacy: .public)). Failing closed rather than \
                minting a replacement key.
                """
            )
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

    /// Removes the key from the Keychain and hands back what was removed.
    ///
    /// This is the emergency wipe's crypto-erase: the sealed identity is
    /// encrypted under this key, so deleting it renders that identity
    /// unrecoverable immediately — no need to overwrite the database first.
    /// The returned copy exists ONLY so a brief undo can put it back; it is
    /// never written anywhere.
    @discardableResult
    public func destroyWrappingKey() throws -> Data? {
        let (status, existing) = read()
        let deleteStatus = SecItemDelete(identityAttributes() as CFDictionary)
        guard deleteStatus == errSecSuccess || deleteStatus == errSecItemNotFound else {
            throw KeychainWrappingKeyError.status(deleteStatus)
        }
        Self.logger.notice("identity wrapping key destroyed")
        return status == errSecSuccess ? existing : nil
    }

    /// Puts a previously destroyed key back, for undo. Restores the SAME
    /// accessibility policy the key was created under, so an undone wipe cannot
    /// quietly downgrade the key's protection.
    public func restoreWrappingKey(_ key: Data) throws {
        guard key.count == 32 else { throw KeychainWrappingKeyError.status(errSecParam) }
        var item = identityAttributes()
        item[kSecValueData as String] = key
        item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly
        var status = SecItemAdd(item as CFDictionary, nil)
        if status == errSecParam || status == errSecNotAvailable || status == errSecAuthFailed {
            item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
            status = SecItemAdd(item as CFDictionary, nil)
        }
        guard status == errSecSuccess else { throw KeychainWrappingKeyError.status(status) }
        Self.logger.notice("identity wrapping key restored after an undone wipe")
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
