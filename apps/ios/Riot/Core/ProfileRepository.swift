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
public struct RiotSpaceApp: Equatable, Hashable, Sendable, Identifiable {
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
    /// Provenance: a bundle that arrived over SYNC from another peer, versus one
    /// this device authored locally (`signAlert`). Both live here because they
    /// replay through the same reload loop, but they degrade under DIFFERENT
    /// recovery reasons — a poison SYNCED bundle is quarantined as `.syncImport`,
    /// a poison local alert as `.alertReplay` — so the recovery record is honest
    /// about where the bad bytes came from (Phase 3).
    let fromSync: Bool

    init(bundle: Data, fromSync: Bool = false) {
        self.bundle = bundle
        self.fromSync = fromSync
    }

    // Custom decode so snapshots written before `fromSync` existed decode to
    // `false` (a local alert) rather than failing — the same backward-compatible
    // discipline `PersistedProfile` uses. A benign field addition must never
    // brick decoding (which Phase 5 would then quarantine). Encoding stays
    // synthesized.
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        bundle = try container.decode(Data.self, forKey: .bundle)
        fromSync = try container.decodeIfPresent(Bool.self, forKey: .fromSync) ?? false
    }
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

    /// The primary snapshot file, so the recovery core can MOVE it aside on the
    /// deepest failure (leaving the primary path clean for a fresh open).
    var snapshotURL: URL { fileURL }

    /// The app-storage folder this snapshot lives in — the root under which
    /// `RecoveryQuarantine` writes `quarantine/`.
    public var directory: URL { fileURL.deletingLastPathComponent() }

    /// The raw persisted bytes as they are on disk right now (the pre-degrade
    /// state), for preserving as a quarantine BLOB when the live file will be
    /// rewritten. `nil` if nothing has been written yet.
    func rawSnapshotBytes() -> Data? {
        guard FileManager.default.fileExists(atPath: fileURL.path) else { return nil }
        return try? Data(contentsOf: fileURL)
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

    /// The recovery record for this repository. Built at `open` and carried on the
    /// instance so post-open boundaries (a per-community switch that finds an
    /// unopenable community, Phase 2) can record into the SAME report and set
    /// aside through the SAME quarantine — recovery is one system, not per-call.
    private let recoveryReport: RecoveryReport
    private let quarantine: RecoveryQuarantine

    public var currentSpace: RiotSpace? { persisted.space }

    /// What this repository had to recover to reach a usable state, or `nil` if
    /// everything is clean. The shell reads this to surface an honest, non-fatal
    /// notice instead of a dead RETRY, and a recovery view lists its quarantined
    /// items. Computed so a post-open recovery (a per-community switch) is
    /// reflected the moment it is recorded. See ``RecoveryReport``.
    public var recovery: RecoveryReport? { recoveryReport.isClean ? nil : recoveryReport }

    private init(
        profile: MobileProfile,
        storage: ProtectedProfileStorage,
        keyStore: WrappingKeyStore,
        appRuntime: AppRuntimeSession,
        installed: [InstalledApp],
        persisted: PersistedProfile,
        recoveryReport: RecoveryReport,
        quarantine: RecoveryQuarantine
    ) {
        self.profile = profile
        self.storage = storage
        self.keyStore = keyStore
        self.appRuntime = appRuntime
        self.installed = installed
        self.persisted = persisted
        self.recoveryReport = recoveryReport
        self.quarantine = quarantine
    }

    public static func open(
        storage: ProtectedProfileStorage,
        keyStore: WrappingKeyStore = KeychainWrappingKeyStore(),
        starterPacks: [(manifest: Data, bundle: Data)] = [],
        databasePath: String? = nil
    ) throws -> RiotProfileRepository {
        // The self-healing open, built on the reusable recovery core
        // (`RecoveryQuarantine` + `recovering` + `RecoveryReport`). Each restore
        // step is isolated: it either succeeds or is quarantined-and-degraded so
        // the launch always reaches a usable state. Phases 2–5 add their own
        // `RecoveryStep`s and reuse this same core.
        let report = RecoveryReport()
        let quarantine = RecoveryQuarantine(storageDirectory: storage.directory)

        // STEP 0 (Phase 5) — the outermost safety net: LOAD the persisted blob.
        // This runs before open even begins. If the bytes will not DECODE at all
        // (a truncated write, a foreign file, a format the JSON can't survive),
        // set the raw blob aside — never deleting it — and start from an EMPTY
        // profile. That is a strictly earlier, distinct failure from a decodable
        // snapshot the CORE later rejects (`.profileOpen`), so it carries its own
        // `.storageBlob` reason. `recovering` absorbs the decode error here because
        // the substitute (an empty profile) cannot itself fail.
        var persisted = recovering(step: .storageBlob) {
            try storage.load()
        } onFailure: { error in
            // MOVE the undecodable bytes aside so the primary path is clean for the
            // fresh save at the end of open; the empty profile then opens cleanly.
            let ref = try? quarantine.quarantine(
                [.file(storage.snapshotURL)], reason: .storageBlob, error: error
            )
            report.recordHealed(.storageBlob, quarantine: ref)
            return .empty
        }

        // STEP 1 — open the core. If the persisted profile cannot be opened at all
        // (a sealed identity the new core rejects, a corrupt snapshot, a wedged
        // database), this is the deepest recovery: MOVE the persisted snapshot and
        // the SQLite database aside — never deleting them — and open a FRESH
        // profile so the person lands in onboarding instead of a dead RETRY.
        // The deepest step uses an explicit do/catch (not `recovering`) because
        // its recovery — a fresh open — can itself throw, and that genuinely-
        // unrecoverable case must rethrow to the launch surface's "Start fresh"
        // rather than being absorbed.
        let profile: MobileProfile
        do {
            profile = try openCore(
                persisted: persisted, keyStore: keyStore, databasePath: databasePath
            )
        } catch {
            let ref = try? quarantine.quarantine(
                Self.coreArtifacts(snapshot: storage.snapshotURL, databasePath: databasePath),
                reason: .profileOpen,
                error: error
            )
            report.recordHealed(.profileOpen, quarantine: ref)
            persisted = .empty
            // `openFresh` falls back to in-memory; if even that throws, it rethrows
            // here and the launch surface offers "Start fresh".
            profile = try openFresh(databasePath: databasePath)
        }

        if persisted.space != nil {
            // STEP 2 — restore the space. A space the core will not rebuild (a
            // schema change, a namespace it now refuses) must not brick the
            // launch: preserve the pre-drop snapshot as a quarantine blob, DROP the
            // space from the working state, keep the identity, and continue.
            recovering(step: .space) {
                try restoreSpace(profile: profile, persisted: persisted, keyStore: keyStore)
                // STEP 3 — replay the saved bundles (Phase 1 local alerts + Phase 3
                // synced imports). A single bundle the core rejects is SKIPPED and
                // quarantined, not fatal — one poison bundle must not kill the rest,
                // the same rule the app-data / trust loops below already follow. The
                // recovery reason follows the bundle's PROVENANCE: a synced import is
                // set aside as `.syncImport`, a local alert as `.alertReplay`, so the
                // record is honest about where the bad bytes came from.
                for alert in persisted.alerts {
                    let step: RecoveryStep = alert.fromSync ? .syncImport : .alertReplay
                    recovering(step: step) {
                        let preview = try profile.inspectBytes(
                            bytes: alert.bundle, route: "protected-local-reload")
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
                    } onFailure: { error in
                        let ref = try? quarantine.quarantine(
                            [.blob(name: alert.fromSync ? "synced.bundle" : "alert.bundle",
                                   alert.bundle)],
                            reason: step,
                            error: error
                        )
                        report.recordDropped(step, quarantine: ref)
                    }
                }
            } onFailure: { error in
                // The space itself would not restore. Preserve the pre-drop
                // snapshot bytes as a blob (the live file is about to be rewritten
                // without the space), drop the space from the working state, and
                // persist the drop so the identity stays durable. The alerts stay
                // on disk (dormant without a space) rather than being erased.
                let ref = try? quarantine.quarantine(
                    storage.rawSnapshotBytes().map { [.blob(name: "profile-snapshot.json", $0)] } ?? [],
                    reason: .space,
                    error: error
                )
                report.recordDropped(.space, quarantine: ref)
                persisted.space = nil
                persisted.demoBundle = nil
                try? storage.save(persisted)
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
        // entry point is set aside and RECORDED (Phase 4) rather than silently
        // excluded — a starter pack that stopped installing is a signal worth
        // surfacing, not a mystery-missing tool.
        let appRuntime = profile.appRuntime()
        var installedApps: [InstalledApp] = []
        for pack in starterPacks {
            recovering(step: .appPack) {
                installedApps.append(try installPack(
                    appRuntime: appRuntime, manifest: pack.manifest, bundle: pack.bundle))
            } onFailure: { error in
                let ref = try? quarantine.quarantine(
                    [.blob(name: "starter-manifest.bin", pack.manifest),
                     .blob(name: "starter-bundle.bin", pack.bundle)],
                    reason: .appPack, error: error)
                report.recordDropped(.appPack, quarantine: ref)
            }
        }

        // Then the apps other people carried here. They go back in through the
        // same install as the starter catalog — Rust re-verifies the pair, so a
        // snapshot that was tampered with on disk is refused, not trusted. A pack
        // that will not install (tampered bytes, an incompatible core) is carried
        // user data: set it aside + RECORD it (Phase 4), skip it, keep the rest.
        for pack in persisted.carriedApps {
            recovering(step: .appPack) {
                installedApps.append(try installPack(
                    appRuntime: appRuntime, manifest: pack.manifest, bundle: pack.bundle))
            } onFailure: { error in
                let ref = try? quarantine.quarantine(
                    [.blob(name: "\(pack.appIDHex)-manifest.bin", pack.manifest),
                     .blob(name: "\(pack.appIDHex)-bundle.bin", pack.bundle)],
                    reason: .appPack, error: error)
                report.recordDropped(.appPack, quarantine: ref)
            }
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
        // corrupt bundle is set aside + RECORDED (Phase 4) and the rest replay —
        // one poison receipt never aborts the open or loses the others silently.
        for bundle in persisted.appDataBundles {
            recovering(step: .appData) {
                try appRuntime.replayAppDataBundle(bytes: bundle)
            } onFailure: { error in
                let ref = try? quarantine.quarantine(
                    [.blob(name: "app-data.bundle", bundle)], reason: .appData, error: error)
                report.recordDropped(.appData, quarantine: ref)
            }
        }

        let repository = RiotProfileRepository(
            profile: profile,
            storage: storage,
            keyStore: keyStore,
            appRuntime: appRuntime,
            installed: installedApps,
            persisted: persisted,
            recoveryReport: report,
            quarantine: quarantine
        )
        if persisted.sealedIdentity == nil {
            persisted.sealedIdentity = try repository.sealCurrentIdentity()
            repository.persisted = persisted
            try storage.save(persisted)
        }
        return repository
    }

    /// Rebuilds the listed space in the freshly-opened core: replays the demo
    /// bundle when in demo mode, else re-joins the public space. Throwing here is
    /// what `open`'s STEP 2 catches to degrade — drop the space, keep the identity.
    private static func restoreSpace(
        profile: MobileProfile,
        persisted: PersistedProfile,
        keyStore: WrappingKeyStore
    ) throws {
        guard let space = persisted.space else { return }
        if let demoBundle = persisted.demoBundle {
            // Demo mode survives a relaunch by REPLAYING THE BUNDLE, not by
            // re-joining the namespace. `join_public_space` would list an empty
            // space — the seeded alerts live in Rust's in-memory store and are
            // gone — and it would never set the demo-mode state that
            // `hide_demo_space` needs to put the person's own identity back.
            // Handing the same signed bytes to the same import restores all three
            // (the listing, the entries, the borrowed author), and it is
            // idempotent because the entries are content-addressed.
            _ = try profile.loadDemoSpace(bytes: demoBundle)
        } else {
            _ = try withWrappingKey(from: keyStore) { wrappingKey in
                try profile.joinPublicSpace(
                    space: PublicSpace(
                        namespaceId: space.namespaceID, title: space.title, isPublic: true),
                    wrappingKey: wrappingKey
                )
            }
        }
    }

    /// The files to MOVE aside on the deepest recovery: the persisted snapshot
    /// plus, when a durable store is in use, the SQLite database and its
    /// write-ahead sidecars (a half-written WAL/SHM/journal is exactly what can
    /// wedge the next open). Only paths that exist are relocated.
    private static func coreArtifacts(snapshot: URL, databasePath: String?) -> [RecoveryArtifact] {
        var artifacts: [RecoveryArtifact] = [.file(snapshot)]
        if let databasePath {
            for suffix in ["", "-wal", "-shm", "-journal"] {
                artifacts.append(.file(URL(fileURLWithPath: databasePath + suffix)))
            }
        }
        return artifacts
    }

    /// Opens the core from whatever the snapshot holds — the sealed identity if
    /// there is one (validated to the 112-byte envelope the core writes), else a
    /// new local profile. Any throw here means the persisted profile cannot be
    /// opened at all, and `open` treats it as the deepest recovery.
    private static func openCore(
        persisted: PersistedProfile,
        keyStore: WrappingKeyStore,
        databasePath: String?
    ) throws -> MobileProfile {
        if let sealedIdentity = persisted.sealedIdentity {
            guard sealedIdentity.count == 112 else { throw RepositoryError.invalidSealedIdentity }
            return try withWrappingKey(from: keyStore) { wrappingKey in
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
        }
        return try openFresh(databasePath: databasePath)
    }

    /// Opens a brand-new local profile on the primary paths — the fresh start
    /// after a quarantine, or the first-ever open. Prefers the durable database;
    /// if that path cannot be opened (unwritable directory, a database the store
    /// still refuses) it falls back to the in-memory profile so the launch still
    /// reaches a usable state rather than throwing.
    private static func openFresh(databasePath: String?) throws -> MobileProfile {
        guard let databasePath else { return try openLocalProfile() }
        do {
            return try openLocalProfileWithDatabase(dbPath: databasePath)
        } catch {
            return try openLocalProfile()
        }
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
        // ALWAYS the 32-byte Keychain wrapping key (never an empty/keyless key):
        // `withWrappingKey` loads-or-creates it and guards `count == 32`, so a
        // real shipping user's join seals the displaced author INLINE and never
        // reaches core's keyless unsealed-parking fallback (Risk 13). The keyless
        // path exists only for ephemeral `open_local_profile()` test/demo builds.
        let joined = try Self.withWrappingKey(from: keyStore) { wrappingKey in
            try profile.joinPublicSpace(
                space: PublicSpace(
                    namespaceId: space.namespaceID, title: space.title, isPublic: true),
                wrappingKey: wrappingKey
            )
        }
        persisted.space = RiotSpace(namespaceID: joined.namespaceId, title: joined.title)
        persisted.sealedIdentity = try sealCurrentIdentity()
        try storage.save(persisted)
        // The join already sealed any displaced author inline; this flush then
        // seals the active author and persists the registry for durability.
        try? persistCommunities()
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

    /// Joins a SECOND (or further) public community while already holding one — the
    /// MANUAL, share-reference path only, and deliberately SEPARATE from
    /// ``joinSpace``.
    ///
    /// `joinSpace` stays single-community on purpose: the nearby-adopt flow calls
    /// it (`SpacePairing` → `host.joinSpace`), so relaxing it would silently make
    /// the NEARBY path multi-community too — a distinct, independently-reviewed
    /// security surface. This method is the only multi-community door, opened by an
    /// explicit paste-a-reference action, never by a peer on the wire.
    ///
    /// It routes to core's multi-community `join_public_space`, which PARKS the
    /// currently-active community's author in the registry and mints a FRESH,
    /// unlinkable communal author for the target namespace. Re-joining the
    /// community that is already active is idempotent (core returns early without
    /// minting). Exactly like `joinSpace`, a join that regenerates the author
    /// RE-SEALS the identity — otherwise the next launch restores the pre-join
    /// identity and mints a different subspace on every relaunch (see `joinSpace`)
    /// — and re-claims the display name the regenerated author orphaned. The
    /// freshly parked author is sealed IMMEDIATELY via ``persistCommunities`` so
    /// the unsealed-in-RAM window is minimal (Risk 13).
    @discardableResult
    public func joinAdditionalCommunity(
        _ space: RiotSpace,
        descriptorEntryID: String
    ) throws -> CommunityRow {
        // Join through `joinNewswireCommunity` so the joined community's registry
        // row CARRIES the descriptor handle from the share reference (Risk 15) —
        // otherwise it is a dead follow whose Home can never reproject. Keyed via
        // the Keychain wrapping key so the displaced author is sealed inline
        // (Risk 13), exactly like `joinSpace`.
        let joined = try Self.withWrappingKey(from: keyStore) { wrappingKey in
            try profile.joinNewswireCommunity(
                space: PublicSpace(
                    namespaceId: space.namespaceID, title: space.title, isPublic: true),
                descriptorEntryId: descriptorEntryID,
                wrappingKey: wrappingKey
            )
        }
        persisted.space = RiotSpace(namespaceID: joined.namespaceId, title: joined.title)
        persisted.sealedIdentity = try sealCurrentIdentity()
        try storage.save(persisted)
        try persistCommunities()
        reclaimDisplayName()
        guard let active = try activeCommunity() else { throw RepositoryError.noCurrentSpace }
        return active
    }

    /// Decodes a pasted `riot://newswire/join/v1/...` share reference to its
    /// coordinates (namespace + descriptor entry + digest). A thin, testable seam
    /// over the core codec; it refuses anything that is not a canonical reference.
    public func decodeShareReference(_ encoded: String) throws -> NewswireShareReference {
        try newswireDecodeShareReference(encoded: encoded)
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
                // Tagged `fromSync` so a later replay that the core rejects is
                // quarantined under `.syncImport` (its true provenance), not
                // `.alertReplay` (Phase 3).
                self.persisted.alerts.append(PersistedAlert(bundle: bundle, fromSync: true))
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

// MARK: - Multiple communities (Unit 3)

/// The registry seam over the Unit-3 FFI. Switch and persist seal/unseal
/// per-community authors, so they route through the SAME profile wrapping key
/// this device already holds in the platform secure store (iOS Keychain) for the
/// primary sealed identity — the key is loaded transiently and reset after use.
/// Real shipping users therefore get durable SEALED per-community identity; no
/// raw secret is ever exposed, and no new key or secure store is introduced.
extension RiotProfileRepository: CommunityRegistry {
    /// The held communities for the chooser. The core already returns rows for
    /// unavailable/quarantined communities (it never drops a row), so one corrupt
    /// community cannot brick this list. The one remaining brick risk is a
    /// registry whose whole persisted form failed to decode — if that ever throws
    /// here, degrade to an EMPTY chooser + RECORD it (Phase 2) rather than a dead
    /// launch: the person still reaches a usable app with an honest recovery
    /// signal, never a crash.
    public func listCommunities() throws -> [CommunityRow] {
        recovering(step: .community) {
            try profile.listCommunities()
        } onFailure: { error in
            let ref = try? quarantine.quarantine(
                [.blob(name: "community-registry.json",
                       Data("registry unreadable".utf8))],
                reason: .community, error: error)
            recoveryReport.recordDropped(.community, quarantine: ref)
            return []
        }
    }

    public func activeCommunity() throws -> CommunityRow? {
        try profile.activeCommunity()
    }

    @discardableResult
    public func switchToCommunity(namespaceID: String) throws -> CommunityRow {
        // Per-community open/reproject (Phase 2). The core loads (unseals) the
        // target's at-rest author here; if that author is corrupt it quarantines
        // it (never drops it) and answers `CommunityUnavailable` WITHOUT leaving
        // the current community — so a broken community can never brick the
        // registry or the others. From this Swift boundary a corrupt author and an
        // otherwise-unopenable target are the SAME signal, so both route through
        // the recovery core: RECORD the event + set a manifest aside (never a
        // silent heal), then rethrow so the shell surfaces the unavailable state
        // in place. Explicit do/catch (not `recovering`) because this must rethrow
        // to the caller, exactly as the deepest profile-open step does.
        do {
            let row = try Self.withWrappingKey(from: keyStore) { wrappingKey in
                try profile.switchCommunity(namespaceId: namespaceID, wrappingKey: wrappingKey)
            }
            // The registry is now on `row`; mirror it into the persisted single-slot
            // so `currentSpace` (and the reprojection that reads it) reflects the
            // community just switched to, not the one we came from. Without this a
            // switch leaves the previous community's title on screen.
            persisted.space = RiotSpace(namespaceID: row.namespaceId, title: row.title)
            try storage.save(persisted)
            return row
        } catch {
            recordCommunityUnavailable(namespaceID: namespaceID, error: error)
            throw error
        }
    }

    public func archiveCommunity(namespaceID: String) throws {
        do {
            try profile.archiveCommunity(namespaceId: namespaceID)
        } catch {
            recordCommunityUnavailable(namespaceID: namespaceID, error: error)
            throw error
        }
    }

    @discardableResult
    public func restoreCommunity(namespaceID: String) throws -> CommunityRow {
        do {
            return try profile.restoreCommunity(namespaceId: namespaceID)
        } catch {
            recordCommunityUnavailable(namespaceID: namespaceID, error: error)
            throw error
        }
    }

    /// Records a community that could not open/reproject through the recovery core:
    /// a manifest naming which community + why is set aside (the core has already
    /// preserved the community's at-rest author for recovery — this is the honest
    /// Swift-side record the recovery view lists), and the event lands in the
    /// shared `RecoveryReport` so the shell can surface it. Never deletes anything.
    private func recordCommunityUnavailable(namespaceID: String, error: Error) {
        let record = ["namespaceID": namespaceID]
        let bytes = (try? JSONSerialization.data(withJSONObject: record)) ?? Data(namespaceID.utf8)
        let ref = try? quarantine.quarantine(
            [.blob(name: "community-\(namespaceID.prefix(16)).json", bytes)],
            reason: .community, error: error)
        recoveryReport.recordDropped(.community, quarantine: ref)
    }

    /// Seals every session-held community author under the secure-store wrapping
    /// key so the held communities survive a reopen. Called after create/join,
    /// alongside identity sealing.
    public func persistCommunities() throws {
        try Self.withWrappingKey(from: keyStore) { wrappingKey in
            try profile.persistCommunities(wrappingKey: wrappingKey)
        }
    }

    public func communityRegistryQuarantined() throws -> Bool {
        try profile.communityRegistryQuarantined()
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

    /// Installs a tool from a manifest+bundle pair the organizer chose from a
    /// file — the "Add a tool" flow. Rust's `installApp` is the integrity oracle;
    /// we then decode the bundle for serving and confirm its entry point before
    /// retaining a resolver. The pair is written to the profile snapshot as a
    /// carried pack so it survives a relaunch (the store is in-memory), and the
    /// held-apps change is posted so Tools refreshes.
    ///
    /// Installing turns NOTHING on. The tool joins the held apps UNTRUSTED, so the
    /// review sheet (`AppReviewSheet`) still stands between it and a WebView —
    /// exactly like `getCarriedApp`. There is no auto-trust here by design.
    @discardableResult
    public func installApp(manifest: Data, bundle: Data) throws -> RiotSpaceApp {
        // Admission first: a pair Rust refuses is never retained or written to disk.
        let record = try appRuntime.installApp(manifestBytes: manifest, bundleBytes: bundle)
        let app = try Self.retain(record: record, bundle: bundle)

        if let existing = installed.firstIndex(where: {
            $0.record.appId.lowercased() == record.appId.lowercased()
        }) {
            installed[existing] = app
        } else {
            installed.append(app)
        }

        let pack = PersistedAppPack(
            appIDHex: record.appId.lowercased(),
            manifest: manifest,
            bundle: bundle
        )
        persisted.carriedApps.removeAll { $0.appIDHex == pack.appIDHex }
        persisted.carriedApps.append(pack)
        try storage.save(persisted)

        // Held and saved; anything showing held apps (the Tools card, the only
        // route to Open) is stale as of this line. Posted after the save so no
        // observer can read a set a later throw would undo — same ordering as
        // getCarriedApp.
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

    /// Signs an editorial action (feature, verify, correct, hide, tombstone,
    /// retract) on an existing post, importing it into the store. Core is the
    /// authorization boundary: it REFUSES to sign an action whose signer is not in
    /// the descriptor's editorial roster, so this THROWS for a non-editor — UI
    /// visibility is never the gate. The reason/replacement text must already obey
    /// the closed field table (the surface validates first, and core validates
    /// again).
    @discardableResult
    func createNewswireEditorialAction(
        spaceDescriptorEntryID: String,
        targetEntryID: String,
        kind: NewswireEditorialActionKind,
        reason: String?,
        correctionText: String?
    ) throws -> NewswireSignedRecord {
        try profile.createNewswireEditorialAction(input: NewswireEditorialActionInput(
            spaceDescriptorEntryId: spaceDescriptorEntryID,
            targetEntryId: targetEntryID,
            kind: kind,
            reason: reason,
            correctionText: correctionText
        ))
    }

    /// Signs a communal reply to `parentEntryID` and imports it through the same
    /// preview/plan/commit path as a post. A reply is communal — no editorial
    /// role is required — so core admits it for any member of the space; it is
    /// dropped from the projection if the parent post is not held.
    @discardableResult
    func createNewswireComment(
        spaceDescriptorEntryID: String,
        parentEntryID: String,
        body: String,
        language: String
    ) throws -> NewswireSignedRecord {
        try profile.createNewswireComment(
            spaceDescriptorEntryId: spaceDescriptorEntryID,
            parentEntryId: parentEntryID,
            body: body,
            language: language
        )
    }

    /// Toggles this profile's communal reaction of `kind` on `parentEntryID` and
    /// imports it through the same preview/plan/commit path as a reply. Like a
    /// reply a reaction is communal — no editorial role is required — so core
    /// admits it for any member of the space; `active: false` retracts this
    /// author's reaction of that kind (latest-wins per author). `kind` is one of
    /// the closed wire names (`support`/`solidarity`/`important`/`grief`); an
    /// unknown name is refused by core as invalid input.
    @discardableResult
    func toggleNewswireReaction(
        spaceDescriptorEntryID: String,
        parentEntryID: String,
        kind: String,
        active: Bool
    ) throws -> NewswireSignedRecord {
        try profile.toggleNewswireReaction(
            spaceDescriptorEntryId: spaceDescriptorEntryID,
            parentEntryId: parentEntryID,
            kind: kind,
            active: active
        )
    }

    /// The collective view of a newswire space: the open wire (all non-expired
    /// posts, newest-first) and the front page (ordinary posts with an active
    /// Feature action). `Hidden`/`Tombstoned` posts arrive with `body == nil`.
    func projectNewswire(spaceDescriptorEntryID: String) throws -> NewswireProjectionView {
        try profile.projectNewswireSpace(spaceDescriptorEntryId: spaceDescriptorEntryID)
    }

    /// The Known-contributors (People) surface of a newswire space: every
    /// distinct author of a signed record it holds, each rendered as `name ·
    /// tag`, with the recognized organizer marked by the namespace coordinate.
    /// Derived from the community's records — not a membership roster.
    func projectNewswireContributors(spaceDescriptorEntryID: String) throws -> [NewswireContributor] {
        try profile.projectNewswireContributors(spaceDescriptorEntryId: spaceDescriptorEntryID)
    }

    /// The digest-bound share/join reference for a newswire space this profile
    /// holds. `encoded` is the canonical `riot://newswire/join/v1/...` string
    /// (link or QR payload); `contentDigest` binds the descriptor's canonical
    /// bytes, so a substituted community name or roster is detectable on import.
    func newswireShareReference(spaceDescriptorEntryID: String) throws -> NewswireShareReference {
        try profile.newswireShareReference(spaceDescriptorEntryId: spaceDescriptorEntryID)
    }

    /// True iff `subjectID` may take editorial actions in the space identified by
    /// `spaceDescriptorEntryID` — core's descriptor-authenticated roster answer
    /// (Unit 4a), the SAME authority core enforces at admission. An unknown /
    /// not-yet-synced descriptor returns `false`, never a throw.
    func newswireIsEditor(spaceDescriptorEntryID: String, subjectID: String) throws -> Bool {
        try profile.newswireIsEditor(descriptorEntryId: spaceDescriptorEntryID, subjectId: subjectID)
    }
}

/// `RiotProfileRepository` is the live source of the People surface — it hands
/// the FFI-projected contributors straight to `PeopleSurfaceModel`.
extension RiotProfileRepository: NewswireContributorProjecting {}

/// The live sources for the 2A community shell. Create-community signs both the
/// app-trust backing space (`createBackingSpace`) and the newswire descriptor
/// (`createNewswireCommunity`); Home projects the community's wire
/// (`NewswireProjecting`). Each just forwards to a method that already exists.
extension RiotProfileRepository: CommunityBackingSpaceCreating {
    @discardableResult
    public func createBackingSpace(name: String) throws -> RiotSpace {
        try createPublicSpace(title: name)
    }
}

extension RiotProfileRepository: NewswireSpaceCreating {
    @discardableResult
    public func createNewswireCommunity(
        name: String,
        summary: String,
        editorialRoster: [String]
    ) throws -> NewswireSignedRecord {
        try createNewswireSpace(
            name: name,
            summary: summary,
            editorialRoster: editorialRoster
        )
    }
}

extension RiotProfileRepository: NewswireProjecting {}

/// The live signer for the editorial surface — it hands the action straight to
/// core, whose roster check (not any UI state) is what actually authorizes it.
extension RiotProfileRepository: NewswireEditorialActing {}

/// The live signer for a communal reply — it forwards to core, which admits the
/// reply for any member of the space (no editorial role) and drops it from the
/// projection if the parent post is not held.
extension RiotProfileRepository: NewswireCommenting {}

/// The live signer for a communal reaction — it forwards to core, which admits
/// the reaction for any member of the space (no editorial role) and retracts this
/// author's reaction of that kind on `active: false`. The same communal contract
/// as a reply.
extension RiotProfileRepository: NewswireReacting {}

/// The live owner-moderation signer — it loads the device wrapping key
/// transiently (reset after use) and hands the sealed masthead + action to core,
/// which signs at O:/mod/ under the owner cap and auto-publishes the coupled
/// heartbeat. Ownership is proven by possession of `sealedRoot` + the wrapping
/// key; core refuses a masthead the key cannot open.
extension RiotProfileRepository: SiteModerationAuthoring {
    @discardableResult
    public func authorSiteModeration(
        sealedRoot: Data,
        action: SiteModerationAction
    ) throws -> SiteModerationOutcome {
        try Self.withWrappingKey(from: keyStore) { wrappingKey in
            try profile.createSiteModerationAction(
                sealedRoot: sealedRoot,
                wrappingKey: wrappingKey,
                action: action
            )
        }
    }
}

/// The live editor-authority read for the editorial surface — it forwards to
/// core's descriptor-authenticated roster answer (Unit 4a), the same authority
/// core enforces at admission. Visibility only; core is the signing gate.
extension RiotProfileRepository: NewswireEditorAuthorityChecking {}

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
