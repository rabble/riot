import Foundation

/// Why a pasted or scanned string was refused as a site follow ticket, mapped to
/// actionable copy by the sheet. Parallel to ``JoinReferenceError`` — but a SITE
/// ticket's real trust check (signature + expiry) is the core's `follow_site`, so
/// this layer only screens the scheme and length before the FFI verify.
public enum FollowSiteError: Error, Equatable {
    /// A scanned/pasted payload that is not a `riot://site/v1/...` ticket at all.
    case notASiteTicket
    /// Exceeds the sane bound — refused before any FFI work so a hostile QR code
    /// (or pasted blob) cannot make the app chew on megabytes.
    case tooLong
}

/// One row on the followed-sites list, projected from core's `FollowedSiteRow`
/// (a uniffi record). Pure + `Equatable` so the list is unit-testable without a
/// device: the view renders exactly these fields and makes no trust decision of
/// its own — in particular it HONORS the core's fetch-time arti gate rather than
/// re-deciding it.
public struct FollowedSiteDisplay: Equatable, Identifiable {
    public let root: String
    public let title: String
    public let stateLabel: String
    /// True iff the site requires an unavailable transport (`require:arti`). The
    /// row then shows "requires Tor — unavailable" and offers NO fetch button.
    public let transportBlocked: Bool
    /// The HTTPS URL to pull the owner-signed bundle from, or `nil`. A blocked row
    /// always projects `nil` here (the core gate nulls it), so `canRefresh` is
    /// false and no clearnet IP can leak to a mirror.
    public let fetchURL: String?

    public var id: String { root }

    /// Whether a "Refresh from site" action is offered: only when the site is not
    /// transport-blocked AND carries a URL to pull from.
    public var canRefresh: Bool { !transportBlocked && fetchURL != nil }

    public init(
        root: String,
        title: String,
        stateLabel: String,
        transportBlocked: Bool,
        fetchURL: String?
    ) {
        self.root = root
        self.title = title
        self.stateLabel = stateLabel
        self.transportBlocked = transportBlocked
        self.fetchURL = fetchURL
    }

    /// Project a core `FollowedSiteRow`. A transport-blocked row keeps NO fetch
    /// URL locally either — belt and suspenders over the core gate.
    public init(_ row: FollowedSiteRow) {
        self.init(
            root: row.root,
            title: row.title,
            stateLabel: FollowedSiteDisplay.label(forState: row.state),
            transportBlocked: row.transportBlocked,
            fetchURL: row.transportBlocked ? nil : row.fetchUrl)
    }

    /// Human copy for the honest row-state token core hands back.
    static func label(forState state: String) -> String {
        switch state {
        case "available": return "Up to date"
        case "pending-first-sync": return "Waiting for first sync"
        case "transport-blocked": return "Requires Tor — unavailable"
        case "degraded": return "Needs attention"
        default: return state
        }
    }
}

/// Pure, camera-free screening for the follow-a-site-by-ticket flow. Unlike a
/// community join reference (decoded locally by ``JoinReferenceModel``), a SITE
/// ticket's signature and expiry are verified in the core `follow_site` FFI — so
/// this model only enforces the `riot://site/v1/` scheme and a length bound
/// before the string is handed to the FFI. Unit-testable without a device.
public final class FollowSiteModel {
    static let siteScheme = "riot://site/v1/"
    /// A canonical ticket is well under this; the bound only exists to refuse a
    /// hostile oversize payload before the FFI ever sees it.
    static let maxLength = 4096

    public init() {}

    /// Screen a pasted/scanned ticket BEFORE the FFI verify. Enforces the scheme
    /// and length bound; the real trust check (signature, expiry) is the core's.
    /// Returns the trimmed ticket ready for `follow_site`.
    public func screen(ticket string: String) throws -> String {
        let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.count <= Self.maxLength else { throw FollowSiteError.tooLong }
        guard trimmed.hasPrefix(Self.siteScheme) else { throw FollowSiteError.notASiteTicket }
        return trimmed
    }

    /// Decode a 64-char lowercase-hex site root to its 32 bytes, or `nil` if it is
    /// not exactly 32 bytes of hex. Used to hand `import_followed_site_bundle` the
    /// followed root it re-verifies the pulled bytes against.
    public static func hexBytes(_ hex: String) -> [UInt8]? {
        guard hex.count == 64 else { return nil }
        var out = [UInt8]()
        out.reserveCapacity(32)
        var index = hex.startIndex
        while index < hex.endIndex {
            let next = hex.index(index, offsetBy: 2)
            guard let byte = UInt8(hex[index..<next], radix: 16) else { return nil }
            out.append(byte)
            index = next
        }
        return out
    }
}
