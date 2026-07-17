import Foundation

// MARK: - Recovery system (boundary-agnostic core)
//
// Riot must never brick and never silently lose data. Any corrupt, partial, or
// version-incompatible persisted state auto-recovers to a usable app,
// quarantines what it can't restore (moved aside, timestamped, with a manifest —
// NEVER deleted), and honestly reports what happened.
//
// This file is the REUSABLE CORE every restore/import boundary shares:
//   • `RecoveryQuarantine` — moves files / writes blobs aside + writes a manifest
//     and an append-only log.
//   • `recovering(step:_:onFailure:)` — the resilient-restore helper applied at
//     every boundary, so one bad unit degrades instead of taking down its
//     siblings.
//   • `RecoveryReport` — what a boundary healed / dropped / quarantined, surfaced
//     to the shell.
//
// Phase 1 applies it to `ProfileRepository.open`. It is deliberately
// boundary-agnostic so Phases 2–5 (per-community reproject, sync import,
// app-drop, storage-blob load) plug in without reworking the core: a new boundary
// adds a `RecoveryStep` and calls `recovering(...)`.

/// A named restore/import unit. Boundary-agnostic: a future boundary adds its own
/// static without editing a central switch. `id` is filesystem-safe (it names the
/// quarantine directory); `label` is plain language for the user-facing notice.
public struct RecoveryStep: Equatable, Hashable, Sendable {
    public let id: String
    public let label: String

    public init(id: String, label: String) {
        self.id = id
        self.label = label
    }

    // Phase 1 — profile open.
    public static let profileOpen = RecoveryStep(id: "profile-open", label: "your saved profile")
    public static let space = RecoveryStep(id: "space", label: "your community")
    public static let alertReplay = RecoveryStep(id: "alert-replay", label: "a saved alert")
    /// A user-initiated "Start fresh": the persisted state is quarantined aside
    /// (never deleted) so the next open starts clean.
    public static let startFresh = RecoveryStep(id: "start-fresh", label: "your saved profile")

    // Phase 2 — per-community open / reproject. One community that will not open
    // (a corrupt at-rest author, an unreadable registry) is set aside as
    // unavailable so the registry and the OTHER communities survive.
    public static let community = RecoveryStep(id: "community", label: "one of your communities")

    // Phase 3 — sync import. A bundle that arrived over sync and the core now
    // rejects on replay is quarantined + recorded, distinct from a locally
    // authored alert (`alertReplay`), so its provenance is honest.
    public static let syncImport = RecoveryStep(id: "sync-import", label: "a synced update")

    // Phase 4 — app-drop / app data. A neighbour-carried app pack (or a starter
    // pack) that will not install, and a committed app-data bundle the core
    // rejects on replay — each set aside + recorded instead of silently skipped.
    public static let appPack = RecoveryStep(id: "app-pack", label: "an app")
    public static let appData = RecoveryStep(id: "app-data", label: "an app's saved data")

    // Phase 5 — the protected-storage blob itself. The earliest boundary: the
    // persisted bytes will not decode at all, so they are set aside and the open
    // starts from an empty profile rather than throwing before it begins.
    public static let storageBlob = RecoveryStep(id: "storage-blob", label: "your saved profile")
}

/// What to set aside. A `file` is MOVED (used when the primary path must be left
/// clean, e.g. a wedged snapshot/database before a fresh open); a `blob` is
/// WRITTEN from bytes already in hand (used when the live file stays in place and
/// only the un-restorable fragment is preserved, e.g. a rejected alert bundle).
public enum RecoveryArtifact: Sendable {
    case file(URL)
    case blob(name: String, Data)
}

/// The on-disk record written beside every quarantined item, so an auto-fix is
/// inspectable later (a recovery UI, a bug report) even headless.
public struct QuarantineManifest: Codable, Equatable, Sendable {
    /// The `RecoveryStep.id` that failed.
    public let reason: String
    /// The plain-language `RecoveryStep.label`.
    public let reasonLabel: String
    /// ISO8601 capture time.
    public let timestamp: String
    /// The filenames relocated/written into the quarantine directory.
    public let artifacts: [String]
    /// The underlying error, stringified — the "why" for a human reading it back.
    public let error: String?
    /// The app version that quarantined it, so a schema jump is diagnosable.
    public let appVersion: String
}

/// A handle to one quarantined item: its directory and the manifest inside it.
public struct QuarantineRef: Equatable, Sendable {
    public let directory: URL
    public let manifest: QuarantineManifest

    /// The manifest file itself, for a recovery UI to open/export.
    public var manifestURL: URL { directory.appendingPathComponent("manifest.json") }
}

/// The one component every boundary uses to set data aside. Copy-on-write and
/// fail-safe: a `file` artifact is MOVED (the source is never deleted until it is
/// safely relocated — `FileManager.moveItem` only removes the source after the
/// destination exists), and NOTHING is ever deleted on the user's behalf. Discard
/// is an explicit, separate user action a recovery UI performs — never here.
public final class RecoveryQuarantine {
    private let root: URL
    private let logURL: URL
    private let appVersion: String
    private let fileManager: FileManager

    /// - Parameter storageDirectory: the app-storage folder; quarantines land in
    ///   `<storageDirectory>/quarantine/`.
    public init(
        storageDirectory: URL,
        appVersion: String = RecoveryQuarantine.bundleShortVersion(),
        fileManager: FileManager = .default
    ) {
        self.root = storageDirectory.appendingPathComponent("quarantine", isDirectory: true)
        self.logURL = root.appendingPathComponent("recovery.log")
        self.appVersion = appVersion
        self.fileManager = fileManager
    }

    /// Moves/writes the given artifacts into a fresh timestamped directory, writes
    /// a `manifest.json`, appends a line to the rolling `recovery.log`, and returns
    /// a handle. Tolerant by design: an artifact that no longer exists is skipped
    /// (the manifest records what actually made it), and a manifest is written even
    /// when nothing relocated — so the fault is logged even for an in-memory-only
    /// failure. Throws only if the quarantine directory itself cannot be created;
    /// a quarantine that cannot even be created must surface, not be swallowed.
    @discardableResult
    public func quarantine(
        _ artifacts: [RecoveryArtifact],
        reason: RecoveryStep,
        error: Error?
    ) throws -> QuarantineRef {
        let now = Date()
        let dir = root.appendingPathComponent(
            "\(Self.stamp(now))-\(reason.id)-\(UUID().uuidString.prefix(8))",
            isDirectory: true
        )
        try fileManager.createDirectory(
            at: dir,
            withIntermediateDirectories: true,
            attributes: [.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication]
        )

        var relocated: [String] = []
        for artifact in artifacts {
            switch artifact {
            case let .file(source):
                guard fileManager.fileExists(atPath: source.path) else { continue }
                let destination = dir.appendingPathComponent(source.lastPathComponent)
                // Move — copy-on-write: the source is only removed once the
                // destination exists. Never a delete-before-relocate.
                try? fileManager.moveItem(at: source, to: destination)
                if fileManager.fileExists(atPath: destination.path) {
                    relocated.append(destination.lastPathComponent)
                }
            case let .blob(name, data):
                let destination = dir.appendingPathComponent(name)
                if (try? data.write(to: destination, options: .atomic)) != nil {
                    relocated.append(name)
                }
            }
        }

        let manifest = QuarantineManifest(
            reason: reason.id,
            reasonLabel: reason.label,
            timestamp: ISO8601DateFormatter().string(from: now),
            artifacts: relocated,
            error: error.map { String(describing: $0) },
            appVersion: appVersion
        )
        writeManifest(manifest, into: dir)
        appendLog(reason: reason, timestamp: manifest.timestamp, directory: dir, error: manifest.error)
        return QuarantineRef(directory: dir, manifest: manifest)
    }

    /// Every quarantined item on disk, newest first — the source a recovery UI
    /// lists. Skips any directory without a readable manifest.
    public func list() throws -> [QuarantineRef] {
        guard fileManager.fileExists(atPath: root.path) else { return [] }
        let dirs = try fileManager.contentsOfDirectory(
            at: root, includingPropertiesForKeys: nil
        ).filter { (try? $0.resourceValues(forKeys: [.isDirectoryKey]))?.isDirectory == true }
        let refs = dirs.compactMap { dir -> QuarantineRef? in
            let manifestURL = dir.appendingPathComponent("manifest.json")
            guard let data = try? Data(contentsOf: manifestURL),
                  let manifest = try? JSONDecoder().decode(QuarantineManifest.self, from: data)
            else { return nil }
            return QuarantineRef(directory: dir, manifest: manifest)
        }
        return refs.sorted { $0.manifest.timestamp > $1.manifest.timestamp }
    }

    /// The directory holding one quarantined item's files, for inspect/export.
    public func open(_ ref: QuarantineRef) -> URL { ref.directory }

    // MARK: - Private

    private func writeManifest(_ manifest: QuarantineManifest, into dir: URL) {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        guard let data = try? encoder.encode(manifest) else { return }
        try? data.write(to: dir.appendingPathComponent("manifest.json"), options: .atomic)
    }

    private func appendLog(reason: RecoveryStep, timestamp: String, directory: URL, error: String?) {
        let line = "\(timestamp)\t\(reason.id)\t\(directory.lastPathComponent)\t\(error ?? "")\n"
        guard let data = line.data(using: .utf8) else { return }
        if let handle = try? FileHandle(forWritingTo: logURL) {
            defer { try? handle.close() }
            _ = try? handle.seekToEnd()
            try? handle.write(contentsOf: data)
        } else {
            try? data.write(to: logURL, options: .atomic)
        }
    }

    /// A filename-safe timestamp (ISO8601 with colons swapped for dashes).
    private static func stamp(_ date: Date) -> String {
        ISO8601DateFormatter().string(from: date).replacingOccurrences(of: ":", with: "-")
    }

    public static func bundleShortVersion() -> String {
        (Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String) ?? "unknown"
    }
}

/// The resilient-restore helper. Applied at EVERY boundary so resilience is
/// systematic rather than remembered case-by-case: run `body`; if it throws, hand
/// the error to `onFailure`, which quarantines + records + returns a degraded
/// value. `onFailure` is non-throwing on purpose — this helper fully ABSORBS the
/// failure so one bad unit never takes down its siblings or the whole open, and a
/// caller never has to remember `try`/`catch` around a degrade.
///
/// The one genuinely-unrecoverable step (the deepest profile open, where even a
/// fresh open could fail) uses an explicit `do/catch` instead, so it can rethrow
/// to the launch surface's "Start fresh".
@discardableResult
public func recovering<T>(
    step: RecoveryStep,
    _ body: () throws -> T,
    onFailure: (Error) -> T
) -> T {
    do {
        return try body()
    } catch {
        return onFailure(error)
    }
}

/// What a self-healing open/import had to do. `nil`-equivalent is `isClean`.
/// `healed` = a step that failed but was recovered to a working substitute (a
/// quarantined profile replaced by a fresh one). `dropped` = a step left out (a
/// space that would not restore, an alert the core rejected). `quarantined` = the
/// refs for everything set aside. The shell reads this for an honest notice and a
/// recovery view lists `quarantined`.
public final class RecoveryReport: @unchecked Sendable {
    public private(set) var healed: [RecoveryStep] = []
    public private(set) var dropped: [RecoveryStep] = []
    public private(set) var quarantined: [QuarantineRef] = []

    public init() {}

    /// A step that failed but recovered to a usable substitute.
    public func recordHealed(_ step: RecoveryStep, quarantine ref: QuarantineRef?) {
        healed.append(step)
        if let ref { quarantined.append(ref) }
    }

    /// A step whose data could not be restored and was left out (degraded).
    public func recordDropped(_ step: RecoveryStep, quarantine ref: QuarantineRef?) {
        dropped.append(step)
        if let ref { quarantined.append(ref) }
    }

    /// True when nothing had to be recovered — a clean open.
    public var isClean: Bool {
        healed.isEmpty && dropped.isEmpty && quarantined.isEmpty
    }

    // MARK: - Convenience for the launch notice (Phase 1 shape)

    /// The whole persisted profile was quarantined and a fresh one opened.
    public var quarantinedProfile: Bool { healed.contains(.profileOpen) }
    /// The persisted space would not restore and was dropped from the working state.
    public var spaceDropped: Bool { dropped.contains(.space) }
    /// How many individual saved alerts the core rejected and were skipped.
    public var alertsSkipped: Int { dropped.filter { $0 == .alertReplay }.count }
    /// Where the preserved bytes went (the first quarantine directory), if any.
    public var quarantineLocation: URL? { quarantined.first?.directory }

    // MARK: - Convenience for Phases 2–5

    /// A community that could not open/reproject was set aside as unavailable.
    public var communityUnavailable: Bool { dropped.contains(.community) }
    /// How many synced bundles the core rejected on replay and were skipped.
    public var syncImportsSkipped: Int { dropped.filter { $0 == .syncImport }.count }
    /// How many app packs (carried or starter) failed to install and were set aside.
    public var appPacksSkipped: Int { dropped.filter { $0 == .appPack }.count }
    /// How many committed app-data bundles the core rejected on replay were skipped.
    public var appDataSkipped: Int { dropped.filter { $0 == .appData }.count }
    /// The undecodable persisted blob was set aside and the open started empty.
    public var storageBlobQuarantined: Bool { healed.contains(.storageBlob) }
}
