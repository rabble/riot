import Foundation

public struct RiotSpace: Codable, Equatable, Sendable {
    public let namespaceID: String
    public let title: String
}

public struct AlertDraft: Equatable, Sendable {
    public let expiresAt: UInt64
    public let headline: String
    public let description: String
    public let sourceClaims: [String]
    public let aiAssisted: Bool

    public init(
        expiresAt: UInt64,
        headline: String,
        description: String,
        sourceClaims: [String],
        aiAssisted: Bool
    ) {
        self.expiresAt = expiresAt
        self.headline = headline
        self.description = description
        self.sourceClaims = sourceClaims
        self.aiAssisted = aiAssisted
    }
}

public struct RiotEntry: Codable, Equatable, Identifiable, Sendable {
    public var id: String { entryID }
    public let entryID: String
    public let namespaceID: String
    public let signerID: String
    public let headline: String
    public let createdAt: UInt64
    public let validFrom: UInt64?
    public let expiresAt: UInt64
    public let aiAssisted: Bool
}

private struct PersistedAlert: Codable {
    let bundle: Data
}

private struct PersistedProfile: Codable {
    var space: RiotSpace?
    var alerts: [PersistedAlert]
    var sealedIdentity: Data?

    static let empty = PersistedProfile(space: nil, alerts: [], sealedIdentity: nil)
}

public final class ProtectedProfileStorage {
    private let fileURL: URL

    public init(fileURL: URL) throws {
        self.fileURL = fileURL
        try FileManager.default.createDirectory(
            at: fileURL.deletingLastPathComponent(),
            withIntermediateDirectories: true,
            attributes: [.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication]
        )
    }

    fileprivate func load() throws -> PersistedProfile {
        guard FileManager.default.fileExists(atPath: fileURL.path) else { return .empty }
        return try JSONDecoder().decode(PersistedProfile.self, from: Data(contentsOf: fileURL))
    }

    fileprivate func save(_ profile: PersistedProfile) throws {
        let data = try JSONEncoder().encode(profile)
        try data.write(to: fileURL, options: .atomic)
        try FileManager.default.setAttributes(
            [.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication],
            ofItemAtPath: fileURL.path
        )
    }
}

public final class RiotProfileRepository {
    private let profile: MobileProfile
    private let storage: ProtectedProfileStorage
    private let keyStore: WrappingKeyStore
    private var persisted: PersistedProfile

    public var currentSpace: RiotSpace? { persisted.space }

    private init(
        profile: MobileProfile,
        storage: ProtectedProfileStorage,
        keyStore: WrappingKeyStore,
        persisted: PersistedProfile
    ) {
        self.profile = profile
        self.storage = storage
        self.keyStore = keyStore
        self.persisted = persisted
    }

    public static func open(
        storage: ProtectedProfileStorage,
        keyStore: WrappingKeyStore = KeychainWrappingKeyStore()
    ) throws -> RiotProfileRepository {
        var persisted = try storage.load()
        let profile: MobileProfile
        if let sealedIdentity = persisted.sealedIdentity {
            guard sealedIdentity.count == 112 else { throw RepositoryError.invalidSealedIdentity }
            profile = try withWrappingKey(from: keyStore) { wrappingKey in
                try openProfileFromSealedIdentity(
                    wrappingKey: wrappingKey,
                    sealedIdentity: sealedIdentity
                )
            }
        } else {
            profile = try openLocalProfile()
        }
        if let space = persisted.space {
            _ = try profile.joinPublicSpace(
                space: PublicSpace(namespaceId: space.namespaceID, title: space.title, isPublic: true)
            )
            for alert in persisted.alerts {
                let preview = try profile.inspectBytes(bytes: alert.bundle, route: "protected-local-reload")
                let entryIDs = try preview.eligibleEntries().map(\.entryId)
                guard !entryIDs.isEmpty else { continue }
                _ = try preview.createPlan(selectedEntryIds: entryIDs).accept()
            }
        }
        let repository = RiotProfileRepository(
            profile: profile,
            storage: storage,
            keyStore: keyStore,
            persisted: persisted
        )
        if persisted.sealedIdentity == nil {
            persisted.sealedIdentity = try repository.sealCurrentIdentity()
            repository.persisted = persisted
            try storage.save(persisted)
        }
        return repository
    }

    public func createPublicSpace(title: String) throws -> RiotSpace {
        let created = try profile.createPublicSpace(title: title)
        let space = RiotSpace(namespaceID: created.namespaceId, title: created.title)
        persisted.space = space
        try storage.save(persisted)
        return space
    }

    public func signAlert(in space: RiotSpace, draft: AlertDraft) throws -> RiotEntry {
        guard persisted.space == space else { throw RepositoryError.spaceMismatch }
        let record = try profile.createDraftAlert(
            input: AlertDraftInput(
                validFrom: nil,
                expiresAt: draft.expiresAt,
                language: "en",
                urgency: .immediate,
                severity: .severe,
                certainty: .observed,
                headline: draft.headline,
                description: draft.description,
                affectedAreaClaim: nil,
                sourceClaims: draft.sourceClaims,
                aiAssisted: draft.aiAssisted
            )
        )
        let signed = try profile.signDraft(draftId: record.draftId)
        persisted.alerts.append(PersistedAlert(bundle: signed.bundleBytes))
        try storage.save(persisted)
        return RiotEntry(signed.entry)
    }

    public func currentEntries() throws -> [RiotEntry] {
        guard currentSpace != nil else { return [] }
        return try profile.listCurrentEntries().map(RiotEntry.init)
    }

    private func sealCurrentIdentity() throws -> Data {
        let sealed = try Self.withWrappingKey(from: keyStore) { wrappingKey in
            try profile.sealIdentity(wrappingKey: wrappingKey)
        }
        guard sealed.count == 112 else { throw RepositoryError.invalidSealedIdentity }
        return sealed
    }

    private static func withWrappingKey<T>(
        from keyStore: WrappingKeyStore,
        operation: (Data) throws -> T
    ) throws -> T {
        var key = try keyStore.loadOrCreateWrappingKey()
        defer { key.resetBytes(in: key.startIndex..<key.endIndex) }
        guard key.count == 32 else { throw RepositoryError.invalidWrappingKey }
        return try operation(key)
    }
}

public enum RepositoryError: Error {
    case spaceMismatch
    case invalidSealedIdentity
    case invalidWrappingKey
}

private extension RiotEntry {
    init(_ entry: CurrentEntry) {
        self.init(
            entryID: entry.entryId,
            namespaceID: entry.namespaceId,
            signerID: entry.signerId,
            headline: entry.headline,
            createdAt: entry.freshness.createdAt,
            validFrom: entry.freshness.validFrom,
            expiresAt: entry.freshness.expiresAt,
            aiAssisted: entry.aiAssisted
        )
    }
}
