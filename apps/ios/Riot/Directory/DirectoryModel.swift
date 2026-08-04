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
    func endorseApp(
        appID: Data,
        note: String,
        retract: Bool,
        expectedNamespaceID: String
    ) throws
    func endorseApp(appID: Data, note: String, retract: Bool) throws
    func shareApp(appID: Data, expectedNamespaceID: String) throws
    func shareApp(appID: Data) throws
    /// Takes up an app this profile already carries — one that arrived from a
    /// neighbour — and admits it to this device's runtime, so it can be reviewed
    /// and then opened. Throws when its bytes have not all arrived, which is the
    /// only reason the person is ever told no.
    func getCarriedApp(appID: Data, expectedNamespaceID: String) throws -> RiotSpaceApp
    func getCarriedApp(appID: Data) throws -> RiotSpaceApp
    /// The lowercase-hex app ids this profile has endorsed. Drives the
    /// "Take back recommendation" affordance: a row whose id is in this set was
    /// recommended by this person and can be retracted.
    var endorsedAppIDs: Set<String> { get }
}

public extension DirectoryPorting {
    /// Compatibility entry points for older, already-scoped callers. New UI
    /// actions carry the namespace they were rendered for all the way to the
    /// repository boundary.
    func endorseApp(appID: Data, note: String, retract: Bool) throws {
        guard let namespaceID = currentSpace?.namespaceID else {
            throw RepositoryError.noCurrentSpace
        }
        try endorseApp(
            appID: appID,
            note: note,
            retract: retract,
            expectedNamespaceID: namespaceID
        )
    }

    func shareApp(appID: Data) throws {
        guard let namespaceID = currentSpace?.namespaceID else {
            throw RepositoryError.noCurrentSpace
        }
        try shareApp(appID: appID, expectedNamespaceID: namespaceID)
    }

    func getCarriedApp(appID: Data) throws -> RiotSpaceApp {
        guard let namespaceID = currentSpace?.namespaceID else {
            throw RepositoryError.noCurrentSpace
        }
        return try getCarriedApp(appID: appID, expectedNamespaceID: namespaceID)
    }
}

public enum RiotDirectoryApprovalCapability: Equatable, Sendable {
    case organizer
    case member
    case unavailable
}

public enum RiotDirectoryActionError: Error, Equatable, Sendable {
    case staleSelection
}

/// Immutable identity for a tool action. Namespace alone is insufficient:
/// selecting A, then B, then A again must invalidate actions captured during
/// the first visit to A.
public struct RiotDirectoryActionContext: Equatable, Sendable {
    public let appID: Data
    public let appIDHex: String
    public let appName: String
    public let namespaceID: String
    public let communityTitle: String
    public let selectionGeneration: UInt64

    public init(
        appID: Data,
        appIDHex: String,
        appName: String,
        namespaceID: String,
        communityTitle: String,
        selectionGeneration: UInt64
    ) {
        self.appID = appID
        self.appIDHex = appIDHex
        self.appName = appName
        self.namespaceID = namespaceID
        self.communityTitle = communityTitle
        self.selectionGeneration = selectionGeneration
    }
}

/// The community visit captured before the first file picker opens. Both file
/// choices must complete for this same generation; A → B → A is stale even
/// though the namespace text matches again.
public struct RiotDirectoryImportContext: Equatable, Sendable {
    public let namespaceID: String
    public let communityTitle: String
    public let selectionGeneration: UInt64

    public init(
        namespaceID: String,
        communityTitle: String,
        selectionGeneration: UInt64
    ) {
        self.namespaceID = namespaceID
        self.communityTitle = communityTitle
        self.selectionGeneration = selectionGeneration
    }
}

public enum RiotDirectoryPrimaryAction: Equatable, Sendable {
    case open(title: String)
    case add(title: String)
    case ask(title: String)
    case unavailable(message: String)

    /// The complete human-readable label used by both the visible CTA and
    /// accessibility. It always names the tool or the selected community.
    public var title: String {
        switch self {
        case let .open(title), let .add(title), let .ask(title):
            title
        case let .unavailable(message):
            message
        }
    }
}

public enum RiotDirectoryDiscoveryAction: Equatable, Sendable {
    case importVerifiedPair(title: String)
}

public struct RiotDirectorySnapshot: Equatable, Sendable {
    public let namespaceID: String
    public let communityTitle: String
    public let inCommunity: [RiotDirectoryRow]
    public let availableToAdd: [RiotDirectoryRow]
    public let moreTools: [RiotDirectoryDiscoveryAction]

    public init(
        namespaceID: String,
        communityTitle: String,
        inCommunity: [RiotDirectoryRow],
        availableToAdd: [RiotDirectoryRow],
        moreTools: [RiotDirectoryDiscoveryAction]
    ) {
        self.namespaceID = namespaceID
        self.communityTitle = communityTitle
        self.inCommunity = inCommunity
        self.availableToAdd = availableToAdd
        self.moreTools = moreTools
    }
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
    /// Community-scoped presentation facts. `availability` above deliberately
    /// remains the flat local-state compatibility contract used by peers.
    public let enabledInCurrentCommunity: Bool
    public let locallyResolvableVerifiedPair: Bool
    public let locallyAdmitted: Bool
    public let primaryAction: RiotDirectoryPrimaryAction
    public let actionContext: RiotDirectoryActionContext?
    public let accessibilityIdentifier: String
    public let canRecommend: Bool
    /// Whether this profile holds the exact verified manifest+bundle pair that
    /// can be published into the selected community.
    public let canMakeAvailable: Bool
    /// Source compatibility while the view moves to the outcome-oriented name.
    public var canShare: Bool { canMakeAvailable }
    /// True when this profile has endorsed the app — the signal that offers
    /// "Take back recommendation" instead of "Recommend".
    public let endorsedByMe: Bool

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
        space: RiotSpace?,
        approval: RiotDirectoryApprovalCapability = .member,
        selectionGeneration: UInt64 = 0,
        endorsedByMe: Bool = false
    ) -> RiotDirectoryRow {
        // `InstalledApp.trusted` is the runtime execution gate for the selected
        // community. On a durable reopen it can be restored before the directory
        // listing's derived `trustedInSpaces` marker catches up. Reconcile the two
        // verified projections so an already-authorized installed tool remains
        // immediately usable instead of falling back to Add.
        let trusted = trustedInCurrentSpace(listing: listing, space: space)
            || installed?.trusted == true
        let appIDHex = hex(listing.appId)

        var badges: [String] = []
        if listing.bundlePresent, installed != nil { badges.append("Works offline") }
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

        let context = actionContext(
            appID: listing.appId,
            appIDHex: appIDHex,
            name: listing.name,
            space: space,
            selectionGeneration: selectionGeneration
        )

        return RiotDirectoryRow(
            appID: listing.appId,
            appIDHex: appIDHex,
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
            enabledInCurrentCommunity: trusted,
            locallyResolvableVerifiedPair: listing.bundlePresent,
            locallyAdmitted: installed != nil,
            primaryAction: primaryAction(
                name: listing.name,
                enabled: trusted,
                complete: listing.bundlePresent,
                approval: approval,
                space: space
            ),
            actionContext: context,
            accessibilityIdentifier: "directory-tool-\(appIDHex)",
            canRecommend: trusted && space != nil,
            canMakeAvailable: space != nil && listing.bundlePresent,
            endorsedByMe: endorsedByMe
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
    static func held(
        _ app: RiotSpaceApp,
        space: RiotSpace?,
        approval: RiotDirectoryApprovalCapability = .member,
        selectionGeneration: UInt64 = 0,
        endorsedByMe: Bool = false
    ) -> RiotDirectoryRow {
        let appIDHex = app.appIDHex.lowercased()
        let appID = bytes(hex: appIDHex) ?? Data()
        return RiotDirectoryRow(
            appID: appID,
            appIDHex: appIDHex,
            name: app.name,
            version: app.version,
            description: app.description,
            permissions: app.permissions,
            badges: ["Works offline"],
            // Nobody's recommendation reaches this row: the endorsements that
            // would speak for it live in the same index that is gone.
            endorsement: nil,
            availability: app.trusted ? .open(app) : .review(app),
            enabledInCurrentCommunity: app.trusted,
            locallyResolvableVerifiedPair: true,
            locallyAdmitted: true,
            primaryAction: primaryAction(
                name: app.name,
                enabled: app.trusted,
                complete: true,
                approval: approval,
                space: space
            ),
            actionContext: actionContext(
                appID: appID,
                appIDHex: appIDHex,
                name: app.name,
                space: space,
                selectionGeneration: selectionGeneration
            ),
            accessibilityIdentifier: "directory-tool-\(appIDHex)",
            canRecommend: app.trusted && space != nil,
            canMakeAvailable: space != nil,
            endorsedByMe: endorsedByMe
        )
    }

    private static func actionContext(
        appID: Data,
        appIDHex: String,
        name: String,
        space: RiotSpace?,
        selectionGeneration: UInt64
    ) -> RiotDirectoryActionContext? {
        guard let space else { return nil }
        return RiotDirectoryActionContext(
            appID: appID,
            appIDHex: appIDHex,
            appName: name,
            namespaceID: space.namespaceID,
            communityTitle: space.title,
            selectionGeneration: selectionGeneration
        )
    }

    private static func primaryAction(
        name: String,
        enabled: Bool,
        complete: Bool,
        approval: RiotDirectoryApprovalCapability,
        space: RiotSpace?
    ) -> RiotDirectoryPrimaryAction {
        guard let space else {
            return .unavailable(message: "Choose a community to use \(name)")
        }
        if !complete {
            return .unavailable(message: "\(name) is still arriving in \(space.title)")
        }
        if enabled {
            return .open(title: "Open \(name)")
        }
        switch approval {
        case .organizer:
            return .add(title: "Add \(name) to \(space.title)")
        case .member:
            return .ask(title: "Ask an organizer to add \(name)")
        case .unavailable:
            return .unavailable(
                message: "This profile can’t add tools to \(space.title). Start a new profile to organize a community."
            )
        }
    }
}

/// Storefront logic with no SwiftUI or FFI types of its own — it reaches the
/// directory surface and the locally held apps only through `DirectoryPorting`,
/// so it runs entirely in unit tests (the twin of Android's
/// `DirectoryController`).
@MainActor
public final class RiotDirectoryModel: ObservableObject {
    @Published public private(set) var rows: [RiotDirectoryRow] = []
    @Published public private(set) var snapshot: RiotDirectorySnapshot?
    @Published public private(set) var selectedCommunityTitle: String?
    @Published public private(set) var isLoading = false
    @Published public private(set) var failedNamespace: String?
    @Published public private(set) var failedCommunityTitle: String?
    @Published public private(set) var errorMessage: String?
    /// Plain-language receipt for the last action ("Recommended Checklist"),
    /// shown until the person leaves the surface.
    @Published public private(set) var confirmation: String?

    private var port: DirectoryPorting?
    private var selectedNamespaceID: String?
    private var hasObservedSelection = false
    private var selectionGeneration: UInt64 = 0
    private var lastApproval: RiotDirectoryApprovalCapability = .member

    public init(port: DirectoryPorting? = nil) {
        self.port = port
    }

    /// Binds the surface to the opened profile. The shell renders every tab
    /// before `bootstrap` has opened one, so the port arrives after the view.
    /// Assignment is intentional: durable trust recovery can replace the
    /// repository instance while this view remains alive.
    public func attach(port: DirectoryPorting?) {
        self.port = port
    }

    /// Releases the repository before a trust decision. That decision may need
    /// to close and reopen the profile; retaining the old repository here would
    /// keep the SQLite-backed session alive across recovery.
    public func detach() {
        port = nil
        rows = []
        snapshot = nil
        selectedCommunityTitle = nil
        failedNamespace = nil
        failedCommunityTitle = nil
        errorMessage = nil
        isLoading = false
    }

    /// Recomputes the directory. Rust assembles it on demand, so this is the
    /// only way the surface learns that an app was carried in, turned on, or
    /// endorsed.
    public func refresh() {
        refresh(approval: .member)
    }

    /// Builds one atomic presentation snapshot for the selected community while
    /// retaining `rows` as the existing flat compatibility surface.
    public func refresh(approval: RiotDirectoryApprovalCapability) {
        lastApproval = approval
        guard let port else {
            rows = []
            snapshot = nil
            selectedCommunityTitle = nil
            failedNamespace = nil
            failedCommunityTitle = nil
            errorMessage = nil
            isLoading = false
            return
        }

        let space = port.currentSpace
        selectedCommunityTitle = space?.title
        observeSelection(space?.namespaceID)
        isLoading = true
        defer { isLoading = false }

        do {
            let installed = try port.installedApps()
            var installedByID: [String: RiotSpaceApp] = [:]
            var installedOrder: [String] = []
            for app in installed {
                let appIDHex = app.appIDHex.lowercased()
                if installedByID[appIDHex] == nil {
                    installedOrder.append(appIDHex)
                }
                // The same verified tool can be restored more than once. Keep
                // its first position stable while using the latest restoration.
                installedByID[appIDHex] = app
            }
            let endorsed = port.endorsedAppIDs
            var seenListingIDs = Set<String>()
            let listed = try port.directoryListings().compactMap { listing -> RiotDirectoryRow? in
                let appIDHex = RiotDirectoryRow.hex(listing.appId)
                guard seenListingIDs.insert(appIDHex).inserted else { return nil }
                return RiotDirectoryRow.make(
                    listing: listing,
                    installed: installedByID[appIDHex],
                    space: space,
                    approval: approval,
                    selectionGeneration: selectionGeneration,
                    endorsedByMe: endorsed.contains(appIDHex)
                )
            }
            // Apps this device holds that no listing speaks for — see
            // `RiotDirectoryRow.held`. They keep their place on this surface
            // instead of vanishing at the next launch.
            let listedIDs = Set(listed.map(\.appIDHex))
            let unlisted = installedOrder
                .compactMap { installedByID[$0] }
                .filter { !listedIDs.contains($0.appIDHex.lowercased()) }
                .map {
                    RiotDirectoryRow.held(
                        $0,
                        space: space,
                        approval: approval,
                        selectionGeneration: selectionGeneration,
                        endorsedByMe: endorsed.contains($0.appIDHex.lowercased())
                    )
                }
            let allRows = listed + unlisted
            rows = allRows

            if let space {
                snapshot = RiotDirectorySnapshot(
                    namespaceID: space.namespaceID,
                    communityTitle: space.title,
                    inCommunity: sorted(
                        allRows.filter(\.enabledInCurrentCommunity)
                    ),
                    availableToAdd: sorted(
                        allRows.filter { !$0.enabledInCurrentCommunity }
                    ),
                    moreTools: approval == .organizer
                        ? [.importVerifiedPair(title: "Add a tool from a file")]
                        : []
                )
            } else {
                snapshot = nil
            }
            failedNamespace = nil
            failedCommunityTitle = nil
            errorMessage = nil
        } catch {
            failedNamespace = space?.namespaceID
            failedCommunityTitle = space?.title
            if let space {
                errorMessage = "Couldn’t load tools for \(space.title)."
            } else {
                errorMessage = PlainFailureText.plain(for: error)
            }
        }
    }

    /// Repeats a failed load only if its namespace is still selected. A retry
    /// captured for one community can never silently retarget another.
    public func retry() {
        guard
            let failedNamespace,
            port?.currentSpace?.namespaceID.lowercased() == failedNamespace.lowercased()
        else { return }
        refresh(approval: lastApproval)
    }

    /// Refuses any action that was not captured for this exact visit to the
    /// currently selected community. Namespace and generation are both
    /// required so A → B → A cannot revive a first-A action.
    public func validate(_ context: RiotDirectoryActionContext) throws {
        guard
            let currentNamespace = port?.currentSpace?.namespaceID,
            currentNamespace.caseInsensitiveCompare(context.namespaceID) == .orderedSame,
            context.selectionGeneration == selectionGeneration
        else {
            throw RiotDirectoryActionError.staleSelection
        }
    }

    public func captureImportContext() -> RiotDirectoryImportContext? {
        guard let space = port?.currentSpace else { return nil }
        return RiotDirectoryImportContext(
            namespaceID: space.namespaceID,
            communityTitle: space.title,
            selectionGeneration: selectionGeneration
        )
    }

    public func validate(_ context: RiotDirectoryImportContext) throws {
        guard
            let currentNamespace = port?.currentSpace?.namespaceID,
            currentNamespace.caseInsensitiveCompare(context.namespaceID) == .orderedSame,
            context.selectionGeneration == selectionGeneration
        else {
            throw RiotDirectoryActionError.staleSelection
        }
    }

    /// Turns a just-verified local import into the same named permission gate
    /// used by every other disabled tool. Importing supplies bytes only; this
    /// method proves the tool remains in Available to add and untrusted.
    public func prepareImportedTool(
        _ installed: RiotSpaceApp
    ) throws -> (
        row: RiotDirectoryRow,
        app: RiotSpaceApp,
        context: RiotDirectoryActionContext
    ) {
        refresh(approval: lastApproval)
        guard
            let row = snapshot?.availableToAdd.first(where: {
                $0.appIDHex.caseInsensitiveCompare(installed.appIDHex) == .orderedSame
            }),
            let context = row.actionContext,
            case .add = row.primaryAction
        else {
            throw RiotDirectoryActionError.staleSelection
        }
        let app = try prepareAdd(row, context: context)
        guard !app.trusted else {
            throw RiotDirectoryActionError.staleSelection
        }
        return (row, app, context)
    }

    /// Receipt for the only successful end of the permission flow. The caller
    /// invokes this after reattaching and refreshing the repository projection.
    public func confirmAdded(_ context: RiotDirectoryActionContext) {
        guard (try? validate(context)) != nil else { return }
        confirmation = "Added \(context.appName) to \(context.communityTitle)"
        errorMessage = nil
    }

    /// Resolves an enabled tool to a locally admitted app. A directory listing
    /// can be enabled before this device has admitted the verified bytes; Open
    /// performs that admission lazily, then refreshes and proves the tool is
    /// still enabled before handing the app to the shell.
    public func prepareOpen(
        _ row: RiotDirectoryRow,
        context: RiotDirectoryActionContext
    ) throws -> RiotSpaceApp {
        let priorRows = rows
        let priorSnapshot = snapshot
        do {
            try validate(row, context: context)
            guard case .open = row.primaryAction else {
                throw RiotDirectoryActionError.staleSelection
            }
            let app = try admittedApp(for: row, context: context)
            try validate(row, context: context)
            refresh(approval: lastApproval)
            try validate(context)
            guard
                let refreshed = snapshot?.inCommunity.first(where: {
                    $0.appIDHex.caseInsensitiveCompare(context.appIDHex) == .orderedSame
                }),
                refreshed.enabledInCurrentCommunity,
                case let .open(refreshedApp) = refreshed.availability
            else {
                throw RiotDirectoryActionError.staleSelection
            }
            confirmation = nil
            errorMessage = nil
            return refreshedApp.appIDHex.caseInsensitiveCompare(app.appIDHex) == .orderedSame
                ? refreshedApp
                : app
        } catch {
            rows = priorRows
            snapshot = priorSnapshot
            confirmation = nil
            errorMessage =
                "Couldn’t open \(context.appName) in \(context.communityTitle). Nothing changed. Try again."
            throw error
        }
    }

    /// Resolves a disabled tool to a locally admitted, still-untrusted app for
    /// the permission sheet. This method never grants trust.
    public func prepareAdd(
        _ row: RiotDirectoryRow,
        context: RiotDirectoryActionContext
    ) throws -> RiotSpaceApp {
        let priorRows = rows
        let priorSnapshot = snapshot
        do {
            try validate(row, context: context)
            guard case .add = row.primaryAction else {
                throw RiotDirectoryActionError.staleSelection
            }
            _ = try admittedApp(for: row, context: context)
            try validate(row, context: context)
            refresh(approval: lastApproval)
            try validate(context)
            guard
                let refreshed = snapshot?.availableToAdd.first(where: {
                    $0.appIDHex.caseInsensitiveCompare(context.appIDHex) == .orderedSame
                }),
                !refreshed.enabledInCurrentCommunity,
                case .add = refreshed.primaryAction,
                case let .review(refreshedApp) = refreshed.availability,
                !refreshedApp.trusted
            else {
                throw RiotDirectoryActionError.staleSelection
            }
            confirmation = nil
            errorMessage = nil
            return refreshedApp
        } catch {
            rows = priorRows
            snapshot = priorSnapshot
            confirmation = nil
            errorMessage =
                "Couldn’t add \(context.appName) to \(context.communityTitle). Nothing changed. Try again."
            throw error
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
            refresh(approval: lastApproval)
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
        return "Couldn’t get \(name). \(PlainFailureText.plain(for: error))"
    }

    /// Why a tool action didn’t happen, in words a person can act on. The one
    /// refusal this model makes itself is "you moved on since you opened that" —
    /// which used to reach a person as the literal word `staleSelection`.
    static func actionFailureMessage(_ error: Error) -> String {
        if (error as? RiotDirectoryActionError) == .staleSelection {
            return "You moved to a different community while that was open. "
                + "Nothing changed — open the tool again from here."
        }
        return PlainFailureText.plain(for: error)
    }

    public func recommend(
        _ row: RiotDirectoryRow,
        note: String,
        context: RiotDirectoryActionContext
    ) {
        perform(confirming: "Recommended \(context.appName) to \(context.communityTitle)") { port in
            try self.validate(row, context: context)
            try port.endorseApp(
                appID: context.appID,
                note: note,
                retract: false,
                expectedNamespaceID: context.namespaceID
            )
        }
    }

    public func recommend(_ row: RiotDirectoryRow, note: String) {
        guard let context = row.actionContext else { return }
        recommend(row, note: note, context: context)
    }

    /// Withdraws this profile's recommendation of an app. Surfaces a receipt so
    /// the person sees the take-back landed, mirroring `recommend`.
    public func retract(
        _ row: RiotDirectoryRow,
        context: RiotDirectoryActionContext
    ) {
        perform(
            confirming:
                "Took back recommendation of \(context.appName) from \(context.communityTitle)"
        ) { port in
            try self.validate(row, context: context)
            try port.endorseApp(
                appID: context.appID,
                note: "",
                retract: true,
                expectedNamespaceID: context.namespaceID
            )
        }
    }

    public func retract(_ row: RiotDirectoryRow) {
        guard let context = row.actionContext else { return }
        retract(row, context: context)
    }

    public func makeAvailable(
        _ row: RiotDirectoryRow,
        context: RiotDirectoryActionContext
    ) {
        guard row.canMakeAvailable else { return }
        perform(confirming: "Made \(context.appName) available in \(context.communityTitle)") { port in
            try self.validate(row, context: context)
            try port.shareApp(
                appID: context.appID,
                expectedNamespaceID: context.namespaceID
            )
        }
    }

    public func share(_ row: RiotDirectoryRow) {
        guard let context = row.actionContext else { return }
        makeAvailable(row, context: context)
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
            refresh(approval: lastApproval)
        } catch {
            confirmation = nil
            errorMessage = Self.actionFailureMessage(error)
        }
    }

    private func validate(
        _ row: RiotDirectoryRow,
        context: RiotDirectoryActionContext
    ) throws {
        guard
            row.appID == context.appID,
            row.appIDHex.caseInsensitiveCompare(context.appIDHex) == .orderedSame
        else {
            throw RiotDirectoryActionError.staleSelection
        }
        try validate(context)
    }

    private func admittedApp(
        for row: RiotDirectoryRow,
        context: RiotDirectoryActionContext
    ) throws -> RiotSpaceApp {
        switch row.availability {
        case let .open(app), let .review(app):
            return app
        case .get:
            guard let port else { throw RiotDirectoryActionError.staleSelection }
            return try port.getCarriedApp(
                appID: context.appID,
                expectedNamespaceID: context.namespaceID
            )
        case .arriving:
            throw MobileError.AppRejected
        }
    }

    private func observeSelection(_ namespaceID: String?) {
        let normalized = namespaceID?.lowercased()
        if !hasObservedSelection || normalized != selectedNamespaceID {
            hasObservedSelection = true
            selectedNamespaceID = normalized
            selectionGeneration += 1
            snapshot = nil
            rows = []
            failedNamespace = nil
            failedCommunityTitle = nil
            errorMessage = nil
        }
    }

    private func sorted(_ values: [RiotDirectoryRow]) -> [RiotDirectoryRow] {
        values.sorted { lhs, rhs in
            let order = lhs.name.localizedCaseInsensitiveCompare(rhs.name)
            if order == .orderedSame {
                return lhs.appIDHex < rhs.appIDHex
            }
            return order == .orderedAscending
        }
    }
}
