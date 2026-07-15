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

/// One app this profile recommended, kept as the CLAIM rather than the signed
/// marker, for the same reason `displayName` is: `endorse_app` hands back no
/// bundle to replay, and the entry it writes lives in Rust's in-memory store.
/// Re-endorsing on open rewrites the same last-write-wins coordinate.
private struct PersistedEndorsement: Codable {
    let appIDHex: String
    let note: String
}

private struct PersistedProfile: Codable {
    var space: RiotSpace?
    var alerts: [PersistedAlert]
    var sealedIdentity: Data?
    var trustedAppIDs: [String]
    // Apps this person recommended to the people they sync with. Rust's marker
    // store is in-memory like the rest, so without this the endorsement is gone
    // on the next launch — the app stays in the directory, but this person's own
    // "I vouch for this" is silently withdrawn from everyone they sync with.
    var endorsements: [PersistedEndorsement]
    // Committed signed app-data bundles (the receipts returned by
    // `appDataPutWithReceipt`), replayed in order on open so app data survives a
    // process restart (Rust's app-data store is in-memory per session).
    var appDataBundles: [Data]
    // Apps carried here by other people, kept as bytes so they survive a restart.
    var carriedApps: [PersistedAppPack]
    // The seeded demo bundle, when demo mode is on. Kept as the bundle's own
    // bytes, not as a flag: Rust's store is in-memory, so the ONLY way the demo
    // space is still there after a relaunch is to hand the same signed bytes back
    // to the same `load_demo_space` import. The presenter loads it backstage and
    // the phone may then sit, sleep, or be restarted before they walk on.
    var demoBundle: Data?
    // The name this person claimed for themselves, as they typed it.
    //
    // Kept as the CLAIM, not as the signed entry: `set_display_name` hands back
    // no bundle to replay, and the entry it writes lives in Rust's in-memory
    // store, so a relaunch would come back as `member · <tag>` with nothing to
    // restore from. Re-claiming the same string on open rewrites the same
    // last-write-wins slot, and the name survives.
    //
    // It also survives the identity CHURNING, which is the subtler reason to
    // hold the claim rather than the entry: joining a space regenerates the
    // author (see `joinSpace`), so a card written under the old subspace is
    // orphaned. Re-claiming afterwards writes it under whoever this person now
    // is. Rust remains the only sanitizer — this string is never rendered, only
    // handed back to `set_display_name`.
    var displayName: String?

    static let empty = PersistedProfile(
        space: nil,
        alerts: [],
        sealedIdentity: nil,
        trustedAppIDs: [],
        endorsements: [],
        appDataBundles: [],
        carriedApps: [],
        demoBundle: nil,
        displayName: nil
    )

    init(
        space: RiotSpace?,
        alerts: [PersistedAlert],
        sealedIdentity: Data?,
        trustedAppIDs: [String],
        endorsements: [PersistedEndorsement],
        appDataBundles: [Data],
        carriedApps: [PersistedAppPack],
        demoBundle: Data?,
        displayName: String?
    ) {
        self.space = space
        self.alerts = alerts
        self.sealedIdentity = sealedIdentity
        self.trustedAppIDs = trustedAppIDs
        self.endorsements = endorsements
        self.appDataBundles = appDataBundles
        self.carriedApps = carriedApps
        self.demoBundle = demoBundle
        self.displayName = displayName
    }

    // Custom decode so snapshots written before `trustedAppIDs`/`appDataBundles`/
    // `carriedApps`/`demoBundle`/`displayName` existed decode to empty rather than
    // failing (synthesized Codable would throw on the missing key). Encoding stays
    // synthesized.
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        space = try container.decodeIfPresent(RiotSpace.self, forKey: .space)
        alerts = try container.decodeIfPresent([PersistedAlert].self, forKey: .alerts) ?? []
        sealedIdentity = try container.decodeIfPresent(Data.self, forKey: .sealedIdentity)
        trustedAppIDs = try container.decodeIfPresent([String].self, forKey: .trustedAppIDs) ?? []
        endorsements = try container
            .decodeIfPresent([PersistedEndorsement].self, forKey: .endorsements) ?? []
        appDataBundles = try container.decodeIfPresent([Data].self, forKey: .appDataBundles) ?? []
        carriedApps = try container.decodeIfPresent([PersistedAppPack].self, forKey: .carriedApps) ?? []
        demoBundle = try container.decodeIfPresent(Data.self, forKey: .demoBundle)
        displayName = try container.decodeIfPresent(String.self, forKey: .displayName)
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
        starterPacks: [(manifest: Data, bundle: Data)] = [],
        databasePath: String? = nil
    ) throws -> RiotProfileRepository {
        var persisted = try storage.load()
        let profile: MobileProfile
        if let sealedIdentity = persisted.sealedIdentity {
            guard sealedIdentity.count == 112 else { throw RepositoryError.invalidSealedIdentity }
            profile = try withWrappingKey(from: keyStore) { wrappingKey in
                if let databasePath {
                    try openProfileFromSealedIdentityWithDatabase(
                        dbPath: databasePath,
                        wrappingKey: wrappingKey,
                        sealedIdentity: sealedIdentity
                    )
                } else {
                    try openProfileFromSealedIdentity(
                        wrappingKey: wrappingKey,
                        sealedIdentity: sealedIdentity
                    )
                }
            }
        } else {
            if let databasePath {
                profile = try openLocalProfileWithDatabase(dbPath: databasePath)
            } else {
                profile = try openLocalProfile()
            }
        }
        if let space = persisted.space {
            if let demoBundle = persisted.demoBundle {
                // Demo mode survives a relaunch by REPLAYING THE BUNDLE, not by
                // re-joining the namespace. `join_public_space` would list an
                // empty space — the seeded alerts live in Rust's in-memory store
                // and are gone — and it would never set the demo-mode state that
                // `hide_demo_space` needs to put the person's own identity back.
                // Handing the same signed bytes to the same import restores all
                // three (the listing, the entries, the borrowed author), and it
                // is idempotent because the entries are content-addressed.
                _ = try profile.loadDemoSpace(bytes: demoBundle)
            } else {
                _ = try profile.joinPublicSpace(
                    space: PublicSpace(namespaceId: space.namespaceID, title: space.title, isPublic: true)
                )
            }
            for alert in persisted.alerts {
                let preview = try profile.inspectBytes(bytes: alert.bundle, route: "protected-local-reload")
                let entryIDs = try preview.eligibleEntries().map(\.entryId)
                // No `guard !entryIDs.isEmpty`: eligibleEntries lists reviewable
                // ALERT rows only, so a bundle that arrived over sync carrying app
                // data or a trust marker has none — and skipping it silently threw
                // that bundle away on every relaunch. A member who synced the
                // organizer's checklist came back to it empty AND locked (the
                // approval is a synced entry too). Planning an empty selection
                // commits the bundle's hidden entries, exactly as the FFI contract
                // test `portable_app_only_review_can_plan_hidden_entries_without_
                // fake_rows` pins. Alert bundles are unaffected — theirs is non-empty.
                _ = try preview.createPlan(selectedEntryIds: entryIDs).accept()
            }
        }

        // Put this person's name back on. Rust's profile store is in-memory like
        // the rest, so without this the person who named themselves "Ana" comes
        // back after a relaunch as `member · a3f91122` — to themselves, and to
        // everyone they sync with.
        //
        // AFTER the space restore, and only outside demo mode, because both move
        // the author this writes under: re-joining pins the joined author (a
        // claim written before it would be orphaned under the pre-join subspace),
        // and `load_demo_space` BORROWS someone else's author entirely — claiming
        // there would print this person's name on the demo persona. Demo mode
        // repairs itself on `hideDemoSpace`, which re-claims once the real author
        // is back.
        if let claimed = persisted.displayName, persisted.demoBundle == nil {
            try? profile.profile().setDisplayName(name: claimed)
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

        // Endorsement markers live in that same in-memory store. Re-assert them
        // under the CURRENT author, which is why this runs after the space is
        // restored: a join regenerates the author, and a marker written under the
        // old subspace would be signed by someone who no longer exists. Endorsing
        // an app whose bytes are not held here is allowed by design, so this does
        // not depend on the install loop above having succeeded.
        for endorsement in persisted.endorsements {
            guard let appID = RiotDirectoryRow.bytes(hex: endorsement.appIDHex) else { continue }
            try? appRuntime.endorseApp(appId: appID, note: endorsement.note, retract: false)
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

    /// Joins a space someone else is already in — how a phone with nothing on it
    /// becomes part of a community: by standing next to someone who is in one.
    ///
    /// RE-SEALS THE IDENTITY, and must. `join_public_space` REGENERATES the
    /// author whenever the namespace differs from the one it currently holds
    /// (`generate_communal_author_for_namespace` mints a fresh random subspace),
    /// while `open` seals the identity at FIRST open — before any space exists.
    /// Join without re-sealing and the next launch restores the PRE-JOIN
    /// identity, re-joins, and mints a DIFFERENT subspace again: the person's
    /// signing identity churns on every launch and everything they wrote last
    /// time is orphaned from everything they write next. Re-sealing pins the
    /// joined author, and `open`'s re-join then finds a namespace that already
    /// matches and leaves the author alone.
    ///
    /// Joining the space this profile is already in is a no-op. Joining a
    /// DIFFERENT space is refused — a phone is in one space.
    public func joinSpace(_ space: RiotSpace) throws {
        if let existing = persisted.space {
            guard existing.namespaceID.lowercased() == space.namespaceID.lowercased() else {
                throw RepositoryError.spaceMismatch
            }
            return
        }
        let joined = try profile.joinPublicSpace(
            space: PublicSpace(namespaceId: space.namespaceID, title: space.title, isPublic: true)
        )
        persisted.space = RiotSpace(namespaceID: joined.namespaceId, title: joined.title)
        persisted.sealedIdentity = try sealCurrentIdentity()
        try storage.save(persisted)
        // The join just REGENERATED the author (see above), so the profile card
        // written under the old subspace is orphaned: this person would appear to
        // the space they just joined as `member · <new tag>`, nameless, on every
        // row they sign from here on. Re-claim under who they now are.
        reclaimDisplayName()
    }

    /// Re-asserts the persisted claim under the CURRENT author.
    ///
    /// Called after the two operations that move the author out from under a
    /// profile card: joining a space (which regenerates it) and leaving demo mode
    /// (which restores the real one after `load_demo_space` borrowed another).
    /// Best-effort on purpose — a name that will not re-claim must not fail the
    /// join or strand the person in the demo space, and the claim stays on disk
    /// for the next open to retry.
    private func reclaimDisplayName() {
        guard let claimed = persisted.displayName else { return }
        try? profile.profile().setDisplayName(name: claimed)
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

    /// Whether this profile may approve apps for its space. Ask BEFORE offering
    /// "Let everyone here use this": a button that cannot succeed should not be
    /// drawn, which is better than any error message it could show.
    public func isOrganizer() throws -> Bool {
        try appRuntime.isOrganizer()
    }

    /// False only for a profile made before spaces had organizers — it can never
    /// approve an app for ANY space, and the only remedy is a new profile. This is
    /// what separates that from the ordinary "you are a member here" case.
    public func canOrganize() throws -> Bool {
        try appRuntime.canOrganize()
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

    /// Revokes trust for an app in Rust and drops the persisted decision so the
    /// revoke survives a relaunch (Rust's trust state is in-memory, so without
    /// this the `open` path would re-trust it). Mirrors `trustApp`: Rust first,
    /// disk second — a revoke Rust refuses never reaches disk.
    public func untrustApp(appID: String) throws {
        try appRuntime.untrustApp(appId: appID)
        let lowered = appID.lowercased()
        if persisted.trustedAppIDs.contains(where: { $0.lowercased() == lowered }) {
            persisted.trustedAppIDs.removeAll { $0.lowercased() == lowered }
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
        // Opening the gated execution session IS the trust gate — Rust refuses an
        // untrusted app (Unit 0C) — and it captures the approval generation +
        // namespace, so a later revoke / re-approval / namespace swap fails the
        // running app's next read or commit BEFORE it touches data. The bridge
        // performs its write through this session and hands back the committed
        // bytes to persist for replay; reads/list go straight through it too.
        guard let execution = try? profile.openAppExecution(appId: appID) else { return nil }
        return AppRuntimeDataBridge(
            execution: execution,
            profiles: profile.profile()
        ) { [weak self] _, bundleBytes in
            try self?.persistAppDataBundle(bundleBytes)
        }
    }

    /// Persists a committed app-data bundle (the receipt from the gated write) so
    /// the value survives a process restart (replayed on `open`). A durable-write
    /// failure propagates so the page hears "couldn't save".
    public func persistAppDataBundle(_ bundleBytes: Data) throws {
        persisted.appDataBundles.append(bundleBytes)
        try storage.save(persisted)
    }

    /// Host-side convenience write (not the page path): commits with a receipt
    /// and persists it. The security-critical page path goes through the gated
    /// `AppExecutionSession` in `appDataBridge`; this direct method is used by the
    /// host and tests to seed/inspect an app's data.
    public func appDataPut(appID: String, key: String, valueJSON: String) throws {
        let receipt = try appRuntime.appDataPutWithReceipt(
            appId: appID, key: key, value: Data(valueJSON.utf8)
        )
        try persistAppDataBundle(receipt)
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
    /// The id handed in was not a subspace id at all (wrong length, not hex).
    /// An id this device has simply never met is NOT this error — core answers
    /// that with the `member` fallback, on purpose.
    case malformedPersonID
}

// MARK: - People

/// The seam where a key id becomes something a screen may draw.
///
/// Core sanitizes the claimed name and derives the tag from the key itself, so
/// `RiotPerson.rendered` — `"Ana · a3f91122"` — is the sanctioned rendering and
/// the only one. Nothing in here ever hands a caller a bare claimed name (which
/// would be the impersonation the tag exists to blunt) or a raw id (which is not
/// a name at all).
public extension RiotProfileRepository {
    /// Who this device is, ready to draw.
    func me() throws -> RiotPerson {
        RiotPerson(try profile.profile().whoami())
    }

    /// The name this person last claimed, exactly as they typed it — nil if they
    /// never have.
    ///
    /// The ONE place a raw claim is handed back, and only for putting it back in
    /// the field they typed it into. It is not a rendering and must never be drawn
    /// as one: showing a bare claimed name is the impersonation the tag exists to
    /// blunt. Anything on screen goes through ``me()``.
    var claimedName: String? { persisted.displayName }

    /// Claims a name for this person and keeps the claim, so it is still theirs
    /// after a relaunch.
    ///
    /// The name is NOT validated here. Core is the single enforcement point — it
    /// sanitizes the string and bounds its length, and an empty or oversized name
    /// comes back from there as `InvalidInput`. Re-implementing those rules on
    /// this side would only let the two disagree.
    ///
    /// Rust first, disk second: a name it refuses is never written, so a claim on
    /// disk is always one core accepted. It throws while a sync is in flight (the
    /// commit would clobber the preview an in-flight review is holding) — the
    /// caller is expected to say so in plain language and let the person retry.
    func setDisplayName(_ name: String) throws {
        try profile.profile().setDisplayName(name: name)
        persisted.displayName = name
        try storage.save(persisted)
    }

    /// Resolves one subspace id (lowercase hex) to a drawable person.
    ///
    /// An id this device has never seen a profile for is NOT an error: core
    /// answers with the `member` fallback, which is load-bearing — a board row
    /// signed by someone whose profile has not synced yet still has to be drawn,
    /// and a peer who never claimed a name is a normal peer, not a failure.
    func person(idHex: String) throws -> RiotPerson {
        guard let id = RiotDirectoryRow.bytes(hex: idHex) else {
            throw RepositoryError.malformedPersonID
        }
        return RiotPerson(try profile.profile().profileFor(id: id))
    }

    /// Every display name this device knows, keyed by lowercase hex subspace id,
    /// already rendered by core. The map a board, an endorsement line, or an
    /// attribution row needs in one call instead of one call per row.
    func displayNames() throws -> [String: String] {
        var names: [String: String] = [:]
        for record in try profile.profile().displayNames() {
            names[RiotDirectoryRow.hex(record.subspaceId)] = record.rendered
        }
        return names
    }
}

private extension RiotPerson {
    init(_ who: WhoAmI) {
        self.init(
            id: RiotDirectoryRow.hex(who.id),
            displayName: who.displayName,
            tag: who.tag
        )
    }
}

// MARK: - Demo mode

/// Demo mode, persisted.
///
/// `DemoProfileLoader` (in `DemoMode.swift`) speaks to Rust and forgets; this
/// conformance is the one the app actually installs, because the app has to
/// survive being put down. The bundle's own bytes go into the snapshot beside
/// the listed space — see `PersistedProfile.demoBundle` for why the bytes and
/// not a flag.
extension RiotProfileRepository: DemoSpaceLoading {
    public func loadDemoSpace(bytes: Data) throws -> RiotSpace {
        // Rust first. A bundle it refuses — corrupt, unsigned, or landing on a
        // phone that is already in a real space — is never written to disk.
        let listed = try profile.loadDemoSpace(bytes: bytes)
        let space = RiotSpace(namespaceID: listed.namespaceId, title: listed.title)
        persisted.space = space
        persisted.demoBundle = bytes
        try storage.save(persisted)
        return space
    }

    public func hideDemoSpace() throws {
        try profile.hideDemoSpace()
        persisted.space = nil
        persisted.demoBundle = nil
        try storage.save(persisted)
        // Demo mode borrowed someone else's author; hiding it hands this person
        // their own back. Their name went with the author that just left, so put
        // it back on the one that returned.
        reclaimDisplayName()
    }

    /// Whether this profile is showing the seeded demo space. The shell asks so
    /// the finale banner appears on a demo phone and nowhere else.
    public var isDemoSpaceLoaded: Bool { persisted.demoBundle != nil }
}

// MARK: - Nearby pairing

/// The profile a nearby pairing acts on. Every member is already implemented
/// above — this states that the repository is the thing `SpacePairing` talks to,
/// so the transport never reaches into storage.
extension RiotProfileRepository: NearbySpaceHost {}

// MARK: - App directory

public extension Notification.Name {
    /// The set of apps this profile HOLDS changed — someone took up a carried app.
    ///
    /// Two models read that set and neither owns the other: the directory (which
    /// performs the get) and the app model (whose `apps` drives the Spaces → Tools
    /// card). The directory used to refresh only ITSELF, so an app just taken up
    /// was held, listed in the directory, and yet Tools still said "No tools yet" —
    /// leaving the app reachable from nowhere, because Tools is the only route to
    /// Open.
    ///
    /// Posted from the repository rather than from either model because the
    /// repository is where the set actually changes, and it is the one thing both
    /// models already share. Whoever shows held apps hears about it — including
    /// surfaces that do not exist yet.
    static let riotHeldAppsDidChange = Notification.Name("riot.heldAppsDidChange")
}

/// The repository is the storefront's port onto Rust. It only forwards: the
/// directory is computed in the core on every call and never stored here, so a
/// row that appears is a row Rust has verified.
extension RiotProfileRepository: DirectoryPorting {
    /// The computed directory: the starter catalog plus every verified app in
    /// the live app-index, with trust and endorsement summaries.
    public func directoryListings() throws -> [DirectoryListing] {
        try appRuntime.directoryListings()
    }

    /// The lowercase-hex app ids this profile has endorsed — the source of the
    /// "Take back recommendation" affordance on each directory row.
    public var endorsedAppIDs: Set<String> {
        Set(persisted.endorsements.map { $0.appIDHex.lowercased() })
    }

    /// Writes (or withdraws) this profile's recommendation of an app. Endorsing
    /// an app whose bytes have not arrived yet is allowed by design — the marker
    /// composes with the app's later arrival.
    public func endorseApp(appID: Data, note: String, retract: Bool) throws {
        // Rust first: a marker it refuses is never written to disk.
        try appRuntime.endorseApp(appId: appID, note: note, retract: retract)
        let appIDHex = RiotDirectoryRow.hex(appID).lowercased()
        persisted.endorsements.removeAll { $0.appIDHex == appIDHex }
        if !retract {
            persisted.endorsements.append(
                PersistedEndorsement(appIDHex: appIDHex, note: note)
            )
        }
        try storage.save(persisted)
    }

    /// Withdraws this profile's recommendation of an app. A named view of
    /// `endorseApp(...retract: true)` for call-site clarity; drops the persisted
    /// endorsement so the take-back survives a relaunch.
    public func retractEndorsement(appID: Data) throws {
        try endorseApp(appID: appID, note: "", retract: true)
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

        // The app is now held and saved. Anything showing held apps is stale as of
        // this line — most visibly the Tools card, the only route to Open. Posted
        // after the save so no observer can read a set a later throw would undo.
        NotificationCenter.default.post(name: .riotHeldAppsDidChange, object: self)

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

// MARK: - Newswire hosting

/// The open newswire — a separate signed space for community publishing. Unlike
/// the private `PublicSpace`, a newswire space is its own signed descriptor in
/// the same namespace, and these calls go straight to `MobileProfile` (the
/// newswire functions live on that object, not on `AppRuntimeSession`). The
/// space descriptor's entry id is the handle the rest of the surface threads
/// through every later call, so `createNewswireSpace` returns it for the model
/// to keep.
public extension RiotProfileRepository {
    /// Creates and signs a newswire space descriptor, importing it into the
    /// store. The signer becomes the founding editor in the roster, so only this
    /// profile can act editorially on its posts until others are added.
    @discardableResult
    func createNewswireSpace(
        name: String,
        summary: String,
        languages: [String] = [],
        geographicTags: [String] = [],
        topicTags: [String] = [],
        editorialRoster: [String] = []
    ) throws -> NewswireSignedRecord {
        try profile.createNewswireSpace(input: NewswireSpaceInput(
            name: name,
            summary: summary,
            languages: languages,
            geographicTags: geographicTags,
            topicTags: topicTags,
            editorialRoster: editorialRoster
        ))
    }

    /// Publishes a freeform news post under an existing newswire space. The
    /// space descriptor must already be in the store (its entry id is the
    /// parent). Returns the signed record carrying the post's own entry id.
    @discardableResult
    func createNewswirePost(
        spaceDescriptorEntryID: String,
        headline: String,
        body: String,
        language: String = "en",
        eventTimeUnixSeconds: UInt64? = nil,
        expiresAtUnixSeconds: UInt64? = nil,
        coarseLocation: String? = nil,
        sourceClaims: [String] = [],
        operationalProfile: NewswireOperationalProfile? = nil,
        aiAssisted: Bool = false
    ) throws -> NewswireSignedRecord {
        try profile.createNewswirePost(input: NewswirePostInput(
            spaceDescriptorEntryId: spaceDescriptorEntryID,
            headline: headline,
            body: body,
            language: language,
            eventTimeUnixSeconds: eventTimeUnixSeconds,
            expiresAtUnixSeconds: expiresAtUnixSeconds,
            coarseLocation: coarseLocation,
            sourceClaims: sourceClaims,
            operationalProfile: operationalProfile,
            aiAssisted: aiAssisted
        ))
    }

    /// The collective view of a newswire space: the open wire (all non-expired
    /// posts, newest-first) and the front page (ordinary posts with an active
    /// Feature action). `Hidden`/`Tombstoned` posts arrive with `body == nil`.
    func projectNewswire(spaceDescriptorEntryID: String) throws -> NewswireProjectionView {
        try profile.projectNewswireSpace(spaceDescriptorEntryId: spaceDescriptorEntryID)
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
