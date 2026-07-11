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
    /// Takes an app this profile already carries — one that arrived from a
    /// neighbour — and admits it to this device's runtime, so it can be reviewed
    /// and then opened. Throws when its bytes have not all arrived, which is the
    /// only reason the person is ever told no.
    func getCarriedApp(appID: Data) throws -> RiotSpaceApp
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
        /// Carried in whole by someone you synced with, but this device has not
        /// taken it up yet — the person can, in one tap.
        case get
        /// Its bytes have not finished arriving, so there is nothing to take yet.
        case arriving
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

    /// The reverse: the raw id the directory's actions take, from the hex text
    /// the held-app store keys on. Anything that is not a whole number of hex
    /// bytes is not an id at all.
    static func bytes(hex: String) -> Data? {
        guard hex.count.isMultiple(of: 2) else { return nil }
        var bytes = Data(capacity: hex.count / 2)
        var index = hex.startIndex
        while index < hex.endIndex {
            let next = hex.index(index, offsetBy: 2)
            guard let byte = UInt8(hex[index..<next], radix: 16) else { return nil }
            bytes.append(byte)
            index = next
        }
        return bytes
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
    /// the same id, or nil when this device has not taken the app up yet: either
    /// it is here in full and one tap away (`.get`), or its bytes are still
    /// crossing from the group that carries it (`.arriving`).
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

        // A listing whose bytes this profile carries but has not taken up is the
        // whole point of the directory: it is a neighbour's app, present in full,
        // and the row must offer to get it rather than dead-end on a badge.
        let availability: Availability
        switch (installed, trusted, listing.bundlePresent) {
        case let (.some(app), true, _): availability = .open(app)
        case let (.some(app), false, _): availability = .review(app)
        case (.none, _, true): availability = .get
        case (.none, _, false): availability = .arriving
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

    /// The row for an app this device holds that the directory does not list.
    ///
    /// The directory is assembled from the built-in catalog and the live app
    /// index, and the index lives in a store that does not survive a relaunch. An
    /// app someone carried here and this person kept is re-installed on open from
    /// the profile's own copy of its bytes — so it is genuinely held and openable
    /// while no listing speaks for it. Without this row it would silently
    /// disappear from the surface that offered it in the first place.
    static func held(_ app: RiotSpaceApp, space: RiotSpace?) -> RiotDirectoryRow {
        RiotDirectoryRow(
            appID: bytes(hex: app.appIDHex) ?? Data(),
            appIDHex: app.appIDHex.lowercased(),
            name: app.name,
            version: app.version,
            description: app.description,
            permissions: app.permissions,
            badges: app.trusted ? ["On in this space"] : [],
            // Nobody's recommendation reaches this row: the endorsements that
            // would speak for it live in the same index that is gone.
            endorsement: nil,
            availability: app.trusted ? .open(app) : .review(app),
            canRecommend: app.trusted && space != nil,
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
            let listed = try port.directoryListings().map { listing in
                let hex = RiotDirectoryRow.hex(listing.appId)
                return RiotDirectoryRow.make(
                    listing: listing,
                    installed: installed.first { $0.appIDHex.lowercased() == hex },
                    space: space
                )
            }
            // Apps this device holds that no listing speaks for — see
            // `RiotDirectoryRow.held`. They keep their place on this surface
            // instead of vanishing at the next launch.
            let listedIDs = Set(listed.map(\.appIDHex))
            let unlisted = installed
                .filter { !listedIDs.contains($0.appIDHex.lowercased()) }
                .map { RiotDirectoryRow.held($0, space: space) }
            rows = listed + unlisted
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    /// Takes up an app a neighbour carried to this device. It is not turned on by
    /// getting it: the row flips to Review, and the app still runs nothing until
    /// this person approves it.
    public func get(_ row: RiotDirectoryRow) {
        guard let port else { return }
        do {
            _ = try port.getCarriedApp(appID: row.appID)
            confirmation = "Got \(row.name) — review it before it runs"
            errorMessage = nil
            refresh()
        } catch {
            confirmation = nil
            errorMessage = Self.getFailureMessage(name: row.name, error: error)
        }
    }

    /// The only refusal the core makes here is "not all of it is here" — an app
    /// that never arrived, or whose bytes are still crossing. Say that, rather
    /// than an error code, and never pretend the app was taken up.
    static func getFailureMessage(name: String, error: Error) -> String {
        if (error as? MobileError) == .AppRejected {
            return "\(name) isn’t all here yet. Sync with the group carrying it, then try again."
        }
        return "Couldn’t get \(name): \(String(describing: error))"
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
