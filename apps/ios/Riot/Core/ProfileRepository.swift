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

/// One installed space app as shown in the Tools list: the person-facing
/// manifest fields plus this profile's trust decision. `appIDHex` is the
/// content-derived id (lowercased hex) used to address the app's resources
/// and data.
public struct RiotSpaceApp: Equatable, Sendable, Identifiable {
    public let appIDHex: String
    public let name: String
    public let description: String
    public let version: String
    public let permissions: [String]
    public let trusted: Bool

    public var id: String { appIDHex }
}

/// One served in-bundle resource: its declared content type and raw bytes.
public struct RiotAppResource: Equatable, Sendable {
    public let contentType: String
    public let bytes: Data
}

private struct PersistedAlert: Codable {
    let bundle: Data
}

/// An app this profile took up from a neighbour, kept as the exact pair of bytes
/// Rust verified. The store it arrived in is in-memory and does not survive a
/// process restart, so without this copy a carried app would vanish on relaunch;
/// with it, `open` re-installs the app through the same `installApp` the starter
/// catalog uses, and Rust re-verifies the pair before it is held again.
private struct PersistedAppPack: Codable {
    let appIDHex: String
    let manifest: Data
    let bundle: Data
}

private struct PersistedProfile: Codable {
    var space: RiotSpace?
    var alerts: [PersistedAlert]
    var sealedIdentity: Data?
    var trustedAppIDs: [String]
    // Committed signed app-data bundles (the receipts returned by
    // `appDataPutWithReceipt`), replayed in order on open so app data survives a
    // process restart (Rust's app-data store is in-memory per session).
    var appDataBundles: [Data]
    // Apps carried here by other people, kept as bytes so they survive a restart.
    var carriedApps: [PersistedAppPack]

    static let empty = PersistedProfile(
        space: nil,
        alerts: [],
        sealedIdentity: nil,
        trustedAppIDs: [],
        appDataBundles: [],
        carriedApps: []
    )

    init(
        space: RiotSpace?,
        alerts: [PersistedAlert],
        sealedIdentity: Data?,
        trustedAppIDs: [String],
        appDataBundles: [Data],
        carriedApps: [PersistedAppPack]
    ) {
        self.space = space
        self.alerts = alerts
        self.sealedIdentity = sealedIdentity
        self.trustedAppIDs = trustedAppIDs
        self.appDataBundles = appDataBundles
        self.carriedApps = carriedApps
    }

    // Custom decode so snapshots written before `trustedAppIDs`/`appDataBundles`/
    // `carriedApps` existed decode to empty lists rather than failing
    // (synthesized Codable would throw on the missing key). Encoding stays
    // synthesized.
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        space = try container.decodeIfPresent(RiotSpace.self, forKey: .space)
        alerts = try container.decodeIfPresent([PersistedAlert].self, forKey: .alerts) ?? []
        sealedIdentity = try container.decodeIfPresent(Data.self, forKey: .sealedIdentity)
        trustedAppIDs = try container.decodeIfPresent([String].self, forKey: .trustedAppIDs) ?? []
        appDataBundles = try container.decodeIfPresent([Data].self, forKey: .appDataBundles) ?? []
        carriedApps = try container.decodeIfPresent([PersistedAppPack].self, forKey: .carriedApps) ?? []
    }
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

/// An installed app: Rust's verified display record plus the client-side
/// resolver that serves its decoded bundle. Mirrors Android's `InstalledApp`
/// — retained in memory because serving comes from the decoded bundle, while
/// Rust remains the integrity oracle (it accepts the bytes before we decode).
private struct InstalledApp {
    let record: InstalledAppRecord
    let resolver: AppResourceResolver
}

public final class RiotProfileRepository {
    private let profile: MobileProfile
    private let storage: ProtectedProfileStorage
    private let keyStore: WrappingKeyStore
    private let appRuntime: AppRuntimeSession
    /// Insertion-ordered registry of installed apps (like Android's
    /// LinkedHashMap-backed `InstalledAppsStore`), keyed for lookup by
    /// lowercased hex app id. It grows after open: an app carried in by a
    /// neighbour joins it the moment the person gets it.
    private var installed: [InstalledApp]
    private var persisted: PersistedProfile

    public var currentSpace: RiotSpace? { persisted.space }

    private init(
        profile: MobileProfile,
        storage: ProtectedProfileStorage,
        keyStore: WrappingKeyStore,
        appRuntime: AppRuntimeSession,
        installed: [InstalledApp],
        persisted: PersistedProfile
    ) {
        self.profile = profile
        self.storage = storage
        self.keyStore = keyStore
        self.appRuntime = appRuntime
        self.installed = installed
        self.persisted = persisted
    }

    public static func open(
        storage: ProtectedProfileStorage,
        keyStore: WrappingKeyStore = KeychainWrappingKeyStore(),
        starterPacks: [(manifest: Data, bundle: Data)] = []
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

        // Install the starter catalog. Rust's `installApp` is the integrity
        // oracle; a pair that fails to install, decode, or match its declared
        // entry point is silently excluded (spec's silent-exclusion rule).
        let appRuntime = profile.appRuntime()
        var installedApps: [InstalledApp] = []
        for pack in starterPacks {
            guard let installed = try? installPack(
                appRuntime: appRuntime,
                manifest: pack.manifest,
                bundle: pack.bundle
            ) else { continue }
            installedApps.append(installed)
        }

        // Then the apps other people carried here. They go back in through the
        // same install as the starter catalog — Rust re-verifies the pair, so a
        // snapshot that was tampered with on disk is refused, not trusted.
        for pack in persisted.carriedApps {
            guard let installed = try? installPack(
                appRuntime: appRuntime,
                manifest: pack.manifest,
                bundle: pack.bundle
            ) else { continue }
            installedApps.append(installed)
        }

        // Trust is profile-local in-memory in Rust and does not survive process
        // restart, so re-apply the persisted trust decisions. Individual
        // failures (e.g. an app that no longer installs) are ignored.
        for appID in persisted.trustedAppIDs {
            try? appRuntime.trustApp(appId: appID)
        }

        // Rust's app-data store is in-memory per session, so re-commit the
        // persisted signed bundles in the order they were written. A single
        // corrupt bundle is skipped (`try?`) rather than aborting the open.
        for bundle in persisted.appDataBundles {
            try? appRuntime.replayAppDataBundle(bytes: bundle)
        }

        let repository = RiotProfileRepository(
            profile: profile,
            storage: storage,
            keyStore: keyStore,
            appRuntime: appRuntime,
            installed: installedApps,
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

    /// The installed space apps and this profile's trust decision for each.
    /// Empty until a space is joined, matching `currentEntries()`.
    public func spaceApps() throws -> [RiotSpaceApp] {
        guard currentSpace != nil else { return [] }
        return try installedApps()
    }

    /// Every app whose bytes this profile actually holds, with its trust
    /// decision — the directory's answer to "can I open this one?".
    ///
    /// Unlike `spaceApps()` this is deliberately not gated on a space existing:
    /// the directory lists the built-ins before anyone has created one, and a
    /// row must still be able to offer Review there.
    public func installedApps() throws -> [RiotSpaceApp] {
        try installed.map(spaceApp)
    }

    private func spaceApp(_ app: InstalledApp) throws -> RiotSpaceApp {
        RiotSpaceApp(
            appIDHex: app.record.appId,
            name: app.record.name,
            description: app.record.description,
            version: app.record.version,
            permissions: app.record.permissions,
            trusted: try appRuntime.isAppTrusted(appId: app.record.appId)
        )
    }

    /// Marks an app trusted in Rust and persists the decision so it survives a
    /// process restart (Rust's trust state is in-memory and re-applied on
    /// `open`).
    public func trustApp(appID: String) throws {
        try appRuntime.trustApp(appId: appID)
        if !persisted.trustedAppIDs.contains(appID) {
            persisted.trustedAppIDs.append(appID)
            try storage.save(persisted)
        }
    }

    /// Serves one of an installed app's resources by exact path. Unknown app id
    /// or path throws — the resolver does no path interpretation, so "../x"
    /// simply matches nothing.
    public func appResource(appID: String, path: String) throws -> RiotAppResource {
        guard let installed = installedApp(appID: appID) else {
            throw RepositoryError.unknownApp
        }
        guard let resource = installed.resolver.resolve(path: path) else {
            throw RepositoryError.unknownAppResource
        }
        return RiotAppResource(contentType: resource.contentType, bytes: resource.bytes)
    }

    /// The app-data bridge for a TRUSTED installed app, or nil otherwise.
    ///
    /// This is the host-side trust gate the platform depends on: Rust
    /// deliberately does NOT trust-gate `app_data_put/get/list` (see the HARD
    /// CONTRACT on `AppBridgeController`), so a bridge is only ever handed out
    /// for an app that is trusted in the current profile.
    public func appDataBridge(appID: String) -> AppDataBridging? {
        guard installedApp(appID: appID) != nil else { return nil }
        guard (try? appRuntime.isAppTrusted(appId: appID)) == true else { return nil }
        // Route the bridge's writes through `appDataPut` so every put from the
        // page is committed with a receipt and persisted for replay on the next
        // open; reads/list/name still go straight to the session.
        return AppRuntimeDataBridge(session: appRuntime, appIDHex: appID) { [weak self] key, valueJSON in
            try self?.appDataPut(appID: appID, key: key, valueJSON: valueJSON)
        }
    }

    /// Commits an app-data write with a receipt and persists the returned signed
    /// bundle so the value survives a process restart (replayed on `open`). This
    /// is the single write path; the WebView bridge delegates here.
    public func appDataPut(appID: String, key: String, valueJSON: String) throws {
        let receipt = try appRuntime.appDataPutWithReceipt(
            appId: appID, key: key, value: Data(valueJSON.utf8)
        )
        persisted.appDataBundles.append(receipt)
        try storage.save(persisted)
    }

    /// Reads an app-data value as the JSON text the page stored, or nil if the
    /// key is unset.
    public func appDataGet(appID: String, key: String) throws -> String? {
        guard let data = try appRuntime.appDataGet(appId: appID, key: key) else { return nil }
        return String(decoding: data, as: UTF8.self)
    }

    /// The resource resolver for a TRUSTED installed app, or nil otherwise.
    ///
    /// Mirrors `appDataBridge(appID:)`'s host-side trust gate: the scheme
    /// handler serving an app's bytes is only ever handed out for an app that is
    /// trusted in the current profile, so the runtime host cannot even mount an
    /// untrusted app's WebView. The returned resolver also carries the verified
    /// `entryPoint` the host loads first.
    public func appResolver(appID: String) -> AppResourceResolver? {
        guard let installed = installedApp(appID: appID) else { return nil }
        guard (try? appRuntime.isAppTrusted(appId: appID)) == true else { return nil }
        return installed.resolver
    }

    private func installedApp(appID: String) -> InstalledApp? {
        let target = appID.lowercased()
        return installed.first { $0.record.appId.lowercased() == target }
    }

    /// Installs one starter pair: Rust verifies the bytes, then we decode the
    /// bundle for serving and confirm its entry point matches Rust's record
    /// before retaining a resolver. Any failure throws so the caller can skip
    /// the pair.
    private static func installPack(
        appRuntime: AppRuntimeSession,
        manifest: Data,
        bundle: Data
    ) throws -> InstalledApp {
        let record = try appRuntime.installApp(manifestBytes: manifest, bundleBytes: bundle)
        return try retain(record: record, bundle: bundle)
    }

    /// Decodes the bytes Rust just accepted and keeps a resolver for them. The
    /// entry-point check is the one thing the host verifies for itself: Rust's
    /// record names the page to load first, and a bundle whose own entry point
    /// disagrees is never served.
    private static func retain(record: InstalledAppRecord, bundle: Data) throws -> InstalledApp {
        let decoded = try AppBundleCodec.decode(bundle)
        guard decoded.entryPoint == record.entryPoint else {
            throw RepositoryError.appBundleMismatch
        }
        return InstalledApp(
            record: record,
            resolver: AppResourceResolver(appIDHex: record.appId, bundle: decoded)
        )
    }

    public func openSyncBoundary() throws -> MobileSyncSessionBoundary {
        let backend = try profile.openSyncSession()
        return GeneratedSyncSessionAdapter(backend: backend) { [weak self] bundle in
            guard let self else { throw RepositoryError.profileClosed }
            if !self.persisted.alerts.contains(where: { $0.bundle == bundle }) {
                self.persisted.alerts.append(PersistedAlert(bundle: bundle))
                try self.storage.save(self.persisted)
            }
        }
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
    case profileClosed
    case unknownApp
    case unknownAppResource
    case appBundleMismatch
    case noCurrentSpace
}

// MARK: - App directory

/// The repository is the storefront's port onto Rust. It only forwards: the
/// directory is computed in the core on every call and never stored here, so a
/// row that appears is a row Rust has verified.
extension RiotProfileRepository: DirectoryPorting {
    /// The computed directory: the starter catalog plus every verified app in
    /// the live app-index, with trust and endorsement summaries.
    public func directoryListings() throws -> [DirectoryListing] {
        try appRuntime.directoryListings()
    }

    /// Writes (or withdraws) this profile's recommendation of an app. Endorsing
    /// an app whose bytes have not arrived yet is allowed by design — the marker
    /// composes with the app's later arrival.
    public func endorseApp(appID: Data, note: String, retract: Bool) throws {
        try appRuntime.endorseApp(appId: appID, note: note, retract: retract)
    }

    /// Takes up an app this profile carries but has not run: Rust admits it from
    /// the store's own copy (the same pair invariant as any other install), and
    /// the host then serves its pages from those same stored bytes — a carried
    /// app has no file on this device, so the store holds the only copy. That
    /// copy is written to the profile snapshot as well, because the store itself
    /// is in-memory: without it the app would be gone on the next launch.
    ///
    /// Getting an app turns nothing on. It joins the held apps as UNTRUSTED, so
    /// the review sheet still stands between a neighbour's app and a WebView.
    public func getCarriedApp(appID: Data) throws -> RiotSpaceApp {
        // Admission first: an app Rust refuses is never written to disk.
        let record = try appRuntime.installFromDirectory(appId: appID)
        let pair = try appRuntime.appPairBytes(appId: appID)
        let app = try Self.retain(record: record, bundle: pair.bundleBytes)

        if let existing = installed.firstIndex(where: {
            $0.record.appId.lowercased() == record.appId.lowercased()
        }) {
            installed[existing] = app
        } else {
            installed.append(app)
        }

        let pack = PersistedAppPack(
            appIDHex: record.appId.lowercased(),
            manifest: pair.manifestBytes,
            bundle: pair.bundleBytes
        )
        persisted.carriedApps.removeAll { $0.appIDHex == pack.appIDHex }
        persisted.carriedApps.append(pack)
        try storage.save(persisted)

        return try spaceApp(app)
    }

    /// Passes an app on to the current space with this profile as carrier.
    /// Sharing never turns the app on for anyone: the organizer on the other
    /// side still makes their own decision.
    public func shareApp(appID: Data) throws {
        guard let space = persisted.space else { throw RepositoryError.noCurrentSpace }
        try appRuntime.shareApp(
            appId: appID,
            space: PublicSpace(
                namespaceId: space.namespaceID,
                title: space.title,
                isPublic: true
            )
        )
    }
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
