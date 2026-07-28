import Foundation

/// Applies an explicit data-protection class to the durable SQLite store.
///
/// WHY THIS EXISTS: `riot.db` is created by Rust (rusqlite) through
/// `openLocalProfileWithDatabase`, so no Swift code ever declares its
/// protection class. It therefore inherits the platform default — on iOS
/// `completeUntilFirstUserAuthentication`, which is what we want, but by
/// accident rather than by contract, and nothing pinned it. The sealed
/// identity file (`ProtectedProfileStorage`) has always set the class
/// explicitly; the database holding every entry and payload did not.
///
/// SQLite is not one file. A write-ahead-log database is `riot.db` plus its
/// `-wal`, `-shm`, and (in rollback mode) `-journal` sidecars, and RECENT
/// CONTENT LIVES IN THE `-wal`. Protecting only the main file would leave the
/// newest entries at the weaker default, so every sidecar is covered here.
///
/// Missing sidecars are not an error: `-wal`/`-shm` exist only while the
/// database is open in WAL mode, and `-journal` only in rollback mode. The
/// applier skips what is absent and reports what it touched.
public protocol FileAttributeWriter {
    func fileExists(atPath path: String) -> Bool
    func setAttributes(_ attributes: [FileAttributeKey: Any], ofItemAtPath path: String) throws
}

extension FileManager: FileAttributeWriter {}

public enum DatabaseFileProtection {
    /// The sidecar suffixes a SQLite database can have, plus the database itself
    /// (the empty suffix). Order is stable so the returned list is predictable.
    static let sidecarSuffixes = ["", "-wal", "-shm", "-journal"]

    /// The class we pin. `completeUntilFirstUserAuthentication` — NOT
    /// `complete` — because sync and notification work must keep reading the
    /// store while the screen is locked; `complete` would make the database
    /// unreadable in exactly those paths.
    public static let protectionClass = FileProtectionType.completeUntilFirstUserAuthentication

    /// Applies the protection class to the database and each sidecar that
    /// exists. Returns the paths actually updated, so a caller (or a test) can
    /// assert coverage instead of trusting a silent no-op.
    @discardableResult
    public static func apply(
        databasePath: String,
        using writer: FileAttributeWriter = FileManager.default
    ) throws -> [String] {
        var updated: [String] = []
        for suffix in sidecarSuffixes {
            let path = databasePath + suffix
            guard writer.fileExists(atPath: path) else { continue }
            try writer.setAttributes([.protectionKey: protectionClass], ofItemAtPath: path)
            updated.append(path)
        }
        return updated
    }
}
