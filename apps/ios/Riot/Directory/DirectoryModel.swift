import Foundation

/// The app directory as the storefront reads and writes it: the computed
/// listings, the apps whose bytes this profile actually holds, and the two
/// outward actions. The storefront reaches Rust only through this protocol, so
/// all of its logic runs in tests with no FFI behind it — the same shape as
/// Android's `DirectoryPort`.
public protocol DirectoryPorting: AnyObject {
    var currentSpace: RiotSpace? { get }
    func directoryListings() throws -> [DirectoryListing]
    func installedApps() throws -> [RiotSpaceApp]
    func endorseApp(appID: Data, note: String, retract: Bool) throws
    func shareApp(appID: Data) throws
}

/// One app as the directory shows it: what it is, what it can do, who vouches
/// for it, and what this person may do with it right now. Every string here is
/// already in the plain language the surface renders — the row is the whole
/// product decision, which is why it is built by a pure function the tests can
/// call without a profile.
public struct RiotDirectoryRow: Identifiable, Equatable, Sendable {
    /// What this profile can do with the app at this moment.
    public enum Availability: Equatable, Sendable {
        /// Held on this device and turned on in this space.
        case open(RiotSpaceApp)
        /// Held on this device, but no organizer has turned it on here yet.
        case review(RiotSpaceApp)
        /// Turned on here, but its bytes have not finished arriving.
        case arriving
        /// Listed only: this profile cannot open it yet.
        case listedOnly
    }

    public let appID: Data
    public let appIDHex: String
    public let name: String
    public let version: String
    public let description: String
    public let permissions: [String]
    public let badges: [String]
    /// "Recommended by …", or nil when nobody has — the surface stays silent
    /// rather than printing a zero.
    public let endorsement: String?
    public let availability: Availability
    public let canRecommend: Bool
    public let canShare: Bool

    public var id: String { appIDHex }
}

public extension RiotDirectoryRow {
    /// Lowercase hex, matching the Rust FFI's own encoding of app ids. The
    /// directory addresses apps by raw bytes while the installed-app store keys
    /// on hex text, so this is the one seam between them.
    static func hex(_ bytes: Data) -> String {
        bytes.map { String(format: "%02x", $0) }.joined()
    }

    /// True when a recognized organizer of the current space trusts this app —
    /// the signal that flips a row from "Review" to "Open".
    static func trustedInCurrentSpace(listing: DirectoryListing, space: RiotSpace?) -> Bool {
        guard let space else { return false }
        let namespace = space.namespaceID.lowercased()
        return listing.trustedInSpaces.contains { hex($0) == namespace }
    }

    /// Whether this profile may recommend the app. Endorsement speaks for a
    /// space that already trusts the app (design spec), so it is offered only
    /// where the app is on in the current space.
    static func canRecommend(listing: DirectoryListing, space: RiotSpace?) -> Bool {
        trustedInCurrentSpace(listing: listing, space: space)
    }

    /// Who vouches for this app, counting only the endorsing groups this profile
    /// has actually met by name and folding the rest into an anonymous count.
    /// Nil when nobody has endorsed it.
    static func endorsementSummary(met: Int, unmet: Int) -> String? {
        guard met + unmet > 0 else { return nil }
        var parts: [String] = []
        if met > 0 {
            parts.append(met == 1 ? "1 group you’ve met" : "\(met) groups you’ve met")
        }
        if unmet > 0 {
            parts.append("\(unmet) you haven’t met")
        }
        return "Recommended by " + parts.joined(separator: ", ")
    }

    /// Builds the row for one listing. `installed` is the locally held app with
    /// the same id, or nil when its bytes have not arrived yet — a carried app
    /// this profile can list but not open until sync brings them.
    static func make(
        listing: DirectoryListing,
        installed: RiotSpaceApp?,
        space: RiotSpace?
    ) -> RiotDirectoryRow {
        let trusted = trustedInCurrentSpace(listing: listing, space: space)

        var badges: [String] = []
        if listing.builtIn { badges.append("Built in") }
        if trusted { badges.append("On in this space") }
        if !listing.bundlePresent { badges.append("Still arriving from your group") }

        let availability: Availability
        switch (installed, trusted, listing.bundlePresent) {
        case let (.some(app), true, _): availability = .open(app)
        case let (.some(app), false, _): availability = .review(app)
        case (.none, true, false): availability = .arriving
        default: availability = .listedOnly
        }

        return RiotDirectoryRow(
            appID: listing.appId,
            appIDHex: hex(listing.appId),
            name: listing.name,
            version: listing.version,
            description: listing.description,
            permissions: listing.permissions,
            badges: badges,
            endorsement: endorsementSummary(
                met: listing.endorsingMetSubspaces.count,
                unmet: Int(listing.endorsingUnmetCount)
            ),
            availability: availability,
            canRecommend: canRecommend(listing: listing, space: space),
            canShare: space != nil
        )
    }
}

/// Storefront logic with no SwiftUI or FFI types of its own — it reaches the
/// directory surface and the locally held apps only through `DirectoryPorting`,
/// so it runs entirely in unit tests (the twin of Android's
/// `DirectoryController`).
@MainActor
public final class RiotDirectoryModel: ObservableObject {
    @Published public private(set) var rows: [RiotDirectoryRow] = []
    @Published public private(set) var errorMessage: String?
    /// Plain-language receipt for the last action ("Recommended Checklist"),
    /// shown until the person leaves the surface.
    @Published public private(set) var confirmation: String?

    private var port: DirectoryPorting?

    public init(port: DirectoryPorting? = nil) {
        self.port = port
    }

    /// Binds the surface to the opened profile. The shell renders every tab
    /// before `bootstrap` has opened one, so the port arrives after the view.
    public func attach(port: DirectoryPorting?) {
        guard self.port == nil else { return }
        self.port = port
    }

    /// Recomputes the directory. Rust assembles it on demand, so this is the
    /// only way the surface learns that an app was carried in, turned on, or
    /// endorsed.
    public func refresh() {
        guard let port else {
            rows = []
            return
        }
        do {
            let installed = try port.installedApps()
            let space = port.currentSpace
            rows = try port.directoryListings().map { listing in
                let hex = RiotDirectoryRow.hex(listing.appId)
                return RiotDirectoryRow.make(
                    listing: listing,
                    installed: installed.first { $0.appIDHex.lowercased() == hex },
                    space: space
                )
            }
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    public func recommend(_ row: RiotDirectoryRow, note: String) {
        perform(confirming: "Recommended \(row.name)") { port in
            try port.endorseApp(appID: row.appID, note: note, retract: false)
        }
    }

    public func share(_ row: RiotDirectoryRow) {
        guard let title = port?.currentSpace?.title else { return }
        perform(confirming: "Shared \(row.name) to \(title)") { port in
            try port.shareApp(appID: row.appID)
        }
    }

    public func clearConfirmation() {
        confirmation = nil
    }

    private func perform(confirming receipt: String, _ action: (DirectoryPorting) throws -> Void) {
        guard let port else { return }
        do {
            try action(port)
            confirmation = receipt
            errorMessage = nil
            refresh()
        } catch {
            confirmation = nil
            errorMessage = String(describing: error)
        }
    }
}
