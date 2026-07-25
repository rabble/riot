import Foundation
import OSLog

/// A secret store whose contents can be destroyed, not just read.
///
/// `WrappingKeyStore` only ever needed `loadOrCreateWrappingKey`; a wipe needs
/// the opposite verb, and needs to be able to put a key BACK if the person
/// undoes an accidental trigger.
public protocol DestructibleSecretStore: WrappingKeyStore {
    /// Removes the stored secret. Returns what was destroyed so a brief undo is
    /// possible; `nil` when there was nothing stored.
    @discardableResult
    func destroyWrappingKey() throws -> Data?

    /// Puts a previously destroyed secret back. Used only by undo.
    func restoreWrappingKey(_ key: Data) throws
}

/// The filesystem verbs a wipe needs, as a seam so the destructive path is
/// testable without destroying anything real.
public protocol FileRemoving {
    func fileExists(atPath path: String) -> Bool
    func removeItem(atPath path: String) throws
}

extension FileManager: FileRemoving {}

/// Everything on disk that must not survive a wipe.
public struct WipePaths: Equatable {
    public let databasePath: String
    public let profilePath: String
    /// The quarantine root. THIS IS NOT OPTIONAL: the recovery system
    /// deliberately preserves copies of profile state instead of deleting it
    /// (`RecoveryQuarantine`), so a wipe that skipped it would leave behind
    /// copies of exactly what it was asked to destroy.
    public let quarantineDirectory: String

    public init(databasePath: String, profilePath: String, quarantineDirectory: String) {
        self.databasePath = databasePath
        self.profilePath = profilePath
        self.quarantineDirectory = quarantineDirectory
    }

    /// Every path a wipe removes, database sidecars included — the `-wal` holds
    /// the most recent entries, so deleting only `riot.db` would leave the
    /// newest content on disk.
    public var allPaths: [String] {
        DatabaseFileProtection.sidecarSuffixes.map { databasePath + $0 }
            + [profilePath, quarantineDirectory]
    }
}

/// Emergency wipe: destroy this device's copy of the community and the identity
/// that signs for it.
///
/// SHAPE — the key dies first, the files die after the undo window.
///
/// Everything on disk is either encrypted under, or meaningless without, the
/// 32-byte wrapping key. Destroying that key is therefore an instant
/// crypto-erase: the sealed identity becomes unrecoverable ciphertext the
/// moment `arm()` returns, no matter how large the database is or how slow the
/// filesystem is.
///
/// The undo window then costs nothing in safety. The key is held ONLY in
/// memory during it — never rewritten to disk — so if the app is force-quit
/// mid-window (an adversary grabbing the phone, a battery pull) the key is
/// simply gone and the data stays unrecoverable. The failure mode of an
/// interrupted wipe is WIPED, never restored. That is the correct direction for
/// a duress feature.
///
/// `commit()` then removes the files, which is cleanup rather than the security
/// boundary — the data was already unreadable.
public final class EmergencyWipe {
    private static let logger = Logger(subsystem: "net.protest.riot", category: "emergency-wipe")

    private let keyStore: DestructibleSecretStore
    private let files: FileRemoving
    private let paths: WipePaths

    /// The destroyed key, held for the undo window. Memory only — writing it
    /// anywhere would defeat the crypto-erase.
    private var undoKey: Data?
    private(set) public var isArmed = false

    public init(
        keyStore: DestructibleSecretStore,
        paths: WipePaths,
        files: FileRemoving = FileManager.default
    ) {
        self.keyStore = keyStore
        self.paths = paths
        self.files = files
    }

    deinit {
        wipeRetainedKey()
    }

    /// Zeroes and drops the in-memory undo copy. Kept in one place so no path
    /// out of the wipe leaves the key sitting in memory.
    private func wipeRetainedKey() {
        guard var key = undoKey else { return }
        key.resetBytes(in: 0..<key.count)
        undoKey = nil
    }

    /// Destroys the wrapping key immediately. Files are left for `commit()`.
    ///
    /// Safe to call when already armed: the second call must not overwrite the
    /// retained key with `nil` and strand the undo.
    public func arm() throws {
        guard !isArmed else { return }
        let destroyed = try keyStore.destroyWrappingKey()
        undoKey = destroyed
        isArmed = true
        Self.logger.notice("emergency wipe armed; identity key destroyed")
    }

    /// Cancels an accidental trigger by putting the key back. Only possible
    /// because `commit()` has not run — nothing on disk has been touched yet.
    public func undo() throws {
        guard isArmed else { return }
        defer {
            wipeRetainedKey()
            isArmed = false
        }
        guard var key = undoKey else {
            // Armed with nothing stored (no identity yet): nothing to restore.
            return
        }
        try keyStore.restoreWrappingKey(key)
        key.resetBytes(in: 0..<key.count)
        Self.logger.notice("emergency wipe undone; identity key restored")
    }

    /// Removes every wipe path, then drops the retained key so undo is no
    /// longer possible. Returns the paths actually removed.
    ///
    /// Deletion continues past a failure: a locked or missing file must not
    /// leave the remaining paths on disk. The first error is rethrown after the
    /// rest have been attempted.
    @discardableResult
    public func commit() throws -> [String] {
        var removed: [String] = []
        var firstError: Error?
        for path in paths.allPaths where files.fileExists(atPath: path) {
            do {
                try files.removeItem(atPath: path)
                removed.append(path)
            } catch {
                if firstError == nil { firstError = error }
                Self.logger.error("emergency wipe could not remove \(path, privacy: .public)")
            }
        }
        wipeRetainedKey()
        isArmed = false
        Self.logger.notice("emergency wipe committed; \(removed.count) paths removed")
        if let firstError { throw firstError }
        return removed
    }
}
