import Foundation
import SwiftUI

/// The four destinations inside a selected community (community-first
/// navigation design §"Navigation and platform behavior"). This replaces the
/// old five debug-shaped surfaces (Spaces/Apps/Board/Post/Connect): Riot is now
/// organized around a community, and a person answers "what is happening here?"
/// (Home) and "what can we do together?" (Tools/People/Nearby). Order is
/// canonical — it is the tab-bar order on iPhone and the sidebar order on macOS,
/// and it is what `Command-1…4` select.
public enum RiotDestination: String, CaseIterable, Identifiable, Sendable {
    case home
    case tools
    case people
    case nearby

    public var id: Self { self }

    public static let phoneTabs = allCases

    public var title: String {
        switch self {
        case .home: "Home"
        case .tools: "Tools"
        case .people: "People"
        case .nearby: "Nearby"
        }
    }

    public var tabTitle: String {
        switch self {
        case .home: "Home"
        case .tools: "Tools"
        case .people: "People"
        case .nearby: "Nearby"
        }
    }

    public var systemImage: String {
        switch self {
        case .home: "house"
        case .tools: "square.grid.2x2"
        case .people: "person.2"
        case .nearby: "antenna.radiowaves.left.and.right"
        }
    }

    /// The `Command-N` accelerator that selects this destination (nav design:
    /// "Command-1…4 select destinations"). Home is 1, and the number follows
    /// canonical order, so the shortcut is stable no matter the platform.
    public var commandNumber: Int {
        switch self {
        case .home: 1
        case .tools: 2
        case .people: 3
        case .nearby: 4
        }
    }

    /// The destination a `Command-N` press selects, or `nil` for an out-of-range
    /// number. Pure so the keyboard map is provable without a live window.
    public static func forCommandNumber(_ number: Int) -> RiotDestination? {
        allCases.first { $0.commandNumber == number }
    }
}

public enum RiotConnectionStatus: Equatable, Sendable {
    case offline
    case nearby(String)
}

/// A missing or unreadable built-in starter pack. The starter catalog is a
/// fixed set of eight tool pairs (`STARTER_CATALOG` in
/// `crates/riot-core/src/apps/starter.rs`); a pair that is listed but not
/// bundled is a build defect, never a silent drop.
public struct StarterCatalogError: Error, Equatable {
    public enum Pack: String, Sendable { case manifest, bundle }
    public let slug: String
    public let pack: Pack
    public var technicalDetails: String {
        "starter pack '\(slug).\(pack.rawValue)' is not bundled"
    }
}

/// The surfaced recovery state for a catalog/package failure (nav design §4.7):
/// a fixed error code plus technical details. The UI shows Retry and hides the
/// details behind a "Technical details" disclosure — never a raw internal error.
public struct StarterCatalogFailure: Equatable, Sendable {
    /// Stable, user-reportable code for the catalog-unavailable state.
    public static let catalogUnavailableCode = "RIOT-CATALOG-UNAVAILABLE"
    public let code: String
    public let technicalDetails: String
}

/// Which tab is on screen, on its own observable object.
///
/// PERFORMANCE CONTRACT: this deliberately does NOT live on `RiotAppModel`. The
/// shell keeps all five destination views alive at once (a ZStack toggling
/// opacity, so each tab's state — the compose draft, the nearby session's
/// `@StateObject` — survives a switch), and every one of those views observes
/// `RiotAppModel`. `@ObservedObject` subscribes to an object's
/// `objectWillChange`, not to individual properties, so publishing `destination`
/// from the app model made a single tab tap re-evaluate all five screen bodies.
/// Keeping selection on a separate object means a tab tap only notifies what
/// actually depends on it: the shell (for opacity) and the directory (which
/// syncs when it becomes visible).
@MainActor
public final class RiotNavigationModel: ObservableObject {
    @Published public var destination: RiotDestination = .home

    public init() {}
}

/// Whether a phone that has just gained a space has to announce it AGAIN, by
/// starting to look afresh.
///
/// The advertisement does not carry the space — `SpacePairing` reads
/// `currentSpace` LIVE when it shakes hands. So a phone that is merely `.looking`
/// needs nothing: whichever peer turns up next will be told about the space then.
/// Restarting there would only churn discovery.
///
/// What is genuinely stuck is a handshake that ALREADY ran and concluded there was
/// nothing to share. Two spaceless phones settle on `.nothingToShare` and the
/// session ends — and the ended session leaves the peer still selected, so
/// auto-connect will never re-dial them. The organizer then taps "Create space" on
/// a phone that has already had its one and only conversation with the phone
/// beside it, and the space they just made can never reach it. Only a fresh look
/// clears that and lets the two of them talk again.
///
/// Pure, because a guard is exactly what got this wrong before: auto-connect once
/// required `.idle` and so never fired at all, `.looking` being the state
/// discovery actually leaves behind. The states that must refuse:
///
/// - `.idle`: discovery never started, or the person tapped "Stop looking". A
///   space appearing must not put them on the air behind their back.
/// - Anything mid-session. Joining a peer's space is ITSELF a nil -> space change
///   on the joiner, and it lands while the sync carrying it is still running — a
///   restart there would tear down the very session doing the work.
public enum NearbyReannounceGate {
    public static func needsReannounce(state: NearbyConnectionState) -> Bool {
        switch state {
        // A session that ran, shared nothing, and left this phone unable to re-dial
        // the peer it was just talking to.
        case .nothingToShare, .differentSpace, .outOfRange, .failed:
            return true
        case .idle, .looking, .confirm, .connecting, .joinSpace,
             .gettingLatest, .preview, .caughtUp, .alreadyCurrent:
            return false
        }
    }
}

@MainActor
public final class RiotAppModel: ObservableObject {
    /// Tab selection. Observe this — not the app model — for destination changes;
    /// see the performance contract on `RiotNavigationModel`.
    public let navigation = RiotNavigationModel()

    /// Passthrough so callers (and `select`) keep reading and writing
    /// `model.destination`. Not `@Published`: the storage lives on `navigation`,
    /// and republishing it here would reintroduce the all-tabs re-render.
    public var destination: RiotDestination {
        get { navigation.destination }
        set { navigation.destination = newValue }
    }

    @Published public private(set) var space: RiotSpace?

    /// The signed newswire `SpaceDescriptorV1` entry id of the selected
    /// community, when this device knows it. It is captured from
    /// `createNewswireSpace` on create; a loaded or joined community carries
    /// `nil` (there is no descriptor-discovery accessor in the MVP FFI), and
    /// Home then shows the honest no-updates recovery state. See
    /// ``CommunityContext/newswireDescriptorEntryID``.
    @Published public private(set) var newswireDescriptorEntryID: String?

    /// Set when a retained community cannot be opened, so the shell can render
    /// the §4.7 community-unavailable recovery in place rather than a blank
    /// screen. `nil` on the ordinary paths.
    @Published public private(set) var communityUnavailable: CommunityUnavailable?
    @Published public private(set) var entries: [RiotEntry] = []
    @Published public private(set) var apps: [RiotSpaceApp] = []
    @Published public private(set) var connectionStatus: RiotConnectionStatus = .offline
    @Published public private(set) var errorMessage: String?

    /// Every person this device can name, keyed by lowercase hex subspace id and
    /// already rendered by core (`"Ana · a3f91122"`).
    ///
    /// Resolved ONCE per change rather than per row: a board redraw must not turn
    /// into sixty FFI calls, and — more importantly — a view that resolved names
    /// lazily while drawing would be mutating the model from inside its own body.
    /// Read it through ``rendered(for:)``, which is the only accessor.
    @Published public private(set) var displayNames: [String: String] = [:]

    /// True when this phone is showing the seeded demo space. The shell reads it
    /// to decide whether the finale banner exists at all.
    @Published public private(set) var isDemoMode = false

    /// Who this person is, as everyone else will see them: `"Ana · a3f91122"`
    /// once they have named themselves, `"member · a3f91122"` before that.
    ///
    /// Nil only before the profile is open. The tag half is not decoration — it
    /// is what keeps two people who both call themselves Ana apart — so this is
    /// carried as the whole `RiotPerson` and drawn through ``RiotPerson/rendered``.
    @Published public private(set) var me: RiotPerson?

    /// What this person last typed as their name, so the field they edit it in
    /// starts where they left it. Nil if they have never claimed one.
    ///
    /// Never drawn — it is a bare claim, not a rendering. ``me`` is what gets
    /// shown.
    @Published public private(set) var claimedName: String?

    /// Why the last attempt to claim a name did not take, in words a person can
    /// act on. Nil when the name saved, and cleared as soon as they try again.
    ///
    /// Separate from ``errorMessage`` on purpose: a name that is too long is a
    /// thing to fix in the field they are already typing in, not an alert that
    /// interrupts them.
    @Published public private(set) var nameError: String?

    /// The entries that appeared in the LAST reload and were not on this phone
    /// before it — the six alerts crossing to phone B, an update landing while
    /// the board is open. Empty on the first read out of the profile, because
    /// entries that were already on disk did not arrive from anywhere.
    ///
    /// The board watches this to stamp and to buzz. It is deliberately not "every
    /// entry the view has not drawn yet": a relaunch must not feel like six people
    /// posting at once.
    @Published public private(set) var arrivals: Set<String> = []

    /// True once the profile is open — i.e. there is an identity to advertise
    /// with and a space (or the honest absence of one) to announce.
    ///
    /// The Connection screen gates discovery on this and must not start without
    /// it. The shell builds every tab at launch (they all live in one ZStack, so
    /// their state survives a tab switch), so `ConnectionStatusView.onAppear`
    /// fires while `bootstrap` is still opening the profile off the window's
    /// `.task`. A phone that began advertising in that window paired with no
    /// repository behind it and announced a nil space — and a peer cannot adopt a
    /// space that was never announced, which is exactly how a fresh phone failed
    /// to join the organizer's.
    ///
    /// Published AFTER ``reload``, never before: the space has to be readable by
    /// the time this opens the gate, or the gate opens onto the same nil announce
    /// it exists to prevent.
    @Published public private(set) var isProfileOpen = false

    /// Set when the built-in starter catalog could not be loaded on open. The
    /// shell renders the §4.7 recovery state from this (Retry + Technical
    /// details behind a disclosure); `nil` means the catalog loaded cleanly.
    @Published public private(set) var starterCatalogFailure: StarterCatalogFailure?

    /// Set when `open` had to SELF-HEAL to reach a usable state — the persisted
    /// profile was quarantined and a fresh one opened, a space could not be
    /// restored, or one or more saved alerts were skipped. The shell surfaces a
    /// dismissible, non-fatal notice from this ("We couldn't restore your previous
    /// data — it's been saved aside") instead of the old dead RETRY. `nil` on a
    /// clean open. See ``RiotAppModel/recoveryNoticeMessage`` for the wording.
    @Published public private(set) var recoveryNotice: RecoveryReport?

    /// The arguments of the last `bootstrap` call, retained so `retryStarterCatalog`
    /// can re-attempt the one-time install after a catalog failure.
    private var lastBootstrapArgs: (
        storageDirectory: URL?,
        keyStore: WrappingKeyStore,
        starterPacks: [(manifest: Data, bundle: Data)]?,
        starterPackResolver: ((String) -> Data?)?
    )?

    /// Nil until the profile has been read once. That first read is the baseline,
    /// not an arrival — see ``arrivals``.
    private var knownEntryIDs: Set<String>?

    private var repository: RiotProfileRepository?

    /// Read-only handle for the runtime host, which needs the live repository to
    /// mount a trusted app's WebView. Exposed instead of widening the stored
    /// property so callers cannot swap the repository out from under the model.
    public var profileRepository: RiotProfileRepository? { repository }

    /// Kept alive for this model's lifetime; the app model IS the Tools card's
    /// source, so it listens for as long as it exists.
    ///
    /// `nonisolated(unsafe)` so `deinit` — which is nonisolated — can unregister it.
    /// Safe by construction: written once in `init` and read once in `deinit`, never
    /// concurrently, and `NotificationCenter` is itself thread-safe.
    private nonisolated(unsafe) var heldAppsObserver: NSObjectProtocol?
    private nonisolated(unsafe) var committedStoreObserver: NSObjectProtocol?

    public init() {
        observeHeldApps()
        observeCommittedStoreChanges()
    }

    init(testError: String) {
        errorMessage = testError
        observeHeldApps()
        observeCommittedStoreChanges()
    }

    /// Re-read the held apps whenever ANY surface takes one up.
    ///
    /// The directory performs the get and refreshes ITSELF; without this the app
    /// model never hears, `apps` stays stale, and Tools says "No tools yet" about
    /// an app the profile is holding — and since Tools is the only route to Open,
    /// the app is then reachable from nowhere.
    ///
    /// Delivered on the posting thread (`queue: nil`) and posted from the main
    /// actor, so the refresh lands synchronously: the card is right on the very
    /// next render rather than a frame later.
    private func observeHeldApps() {
        heldAppsObserver = NotificationCenter.default.addObserver(
            forName: .riotHeldAppsDidChange,
            object: nil,
            queue: nil
        ) { [weak self] _ in
            guard Thread.isMainThread else {
                Task { @MainActor [weak self] in self?.refreshApps() }
                return
            }
            MainActor.assumeIsolated {
                self?.refreshApps()
            }
        }
    }

    /// Re-read native space state after a store mutation has committed.
    ///
    /// WebViews already observe this signal and redraw their app data. Alerts
    /// use `RiotAppModel.entries` instead, so without the matching refresh a
    /// freshly joined phone receives the board on disk but shows an empty board
    /// until relaunch.
    private func observeCommittedStoreChanges() {
        committedStoreObserver = NotificationCenter.default.addObserver(
            forName: AppRuntimeView.dataChangedNotification,
            object: nil,
            queue: nil
        ) { [weak self] _ in
            guard Thread.isMainThread else {
                Task { @MainActor [weak self] in self?.refreshFromStore() }
                return
            }
            MainActor.assumeIsolated {
                self?.refreshFromStore()
            }
        }
    }

    deinit {
        if let heldAppsObserver {
            NotificationCenter.default.removeObserver(heldAppsObserver)
        }
        if let committedStoreObserver {
            NotificationCenter.default.removeObserver(committedStoreObserver)
        }
    }

    public var connectionDisclosure: String {
        switch connectionStatus {
        case .offline: "Offline · local device only"
        case let .nearby(peer): "Nearby · \(peer)"
        }
    }

    public func select(_ destination: RiotDestination) {
        self.destination = destination
    }

    public func dismissError() {
        errorMessage = nil
    }

    /// Dismisses the non-fatal self-healing notice once the person has seen it.
    /// The recovery already happened — this only clears the banner.
    public func dismissRecoveryNotice() {
        recoveryNotice = nil
    }

    /// The plain-language, non-fatal notice for a self-healing open, or `nil` when
    /// nothing was recovered. Honest about WHAT was dropped and clear that the data
    /// was saved aside, never deleted.
    public var recoveryNoticeMessage: String? {
        guard let recovery = recoveryNotice else { return nil }
        if recovery.quarantinedProfile {
            return "We couldn't restore your previous data, so we started fresh. "
                + "Your old data has been saved aside on this device, not deleted."
        }
        var parts: [String] = []
        if recovery.spaceDropped {
            parts.append("your community couldn't be restored")
        }
        if recovery.alertsSkipped > 0 {
            let n = recovery.alertsSkipped
            parts.append("\(n) saved \(n == 1 ? "alert" : "alerts") couldn't be reloaded")
        }
        guard !parts.isEmpty else { return nil }
        return "We couldn't fully restore your previous data — "
            + parts.joined(separator: " and ")
            + ". It's been saved aside on this device, not deleted."
    }

    /// Re-attempts the one-time profile open with the same arguments the last
    /// `bootstrap` used. The escape from the old dead RETRY: `open` now
    /// self-heals, so a retry that reaches it lands the person in a usable app
    /// rather than re-failing forever. No-ops once the profile is open.
    public func retryBootstrap() {
        guard repository == nil, let args = lastBootstrapArgs else { return }
        errorMessage = nil
        bootstrap(
            storageDirectory: args.storageDirectory,
            keyStore: args.keyStore,
            starterPacks: args.starterPacks,
            starterPackResolver: args.starterPackResolver
        )
    }

    /// "Start fresh": the last-resort recovery for a genuinely-unrecoverable
    /// error that survived every in-`open` degrade. It QUARANTINES the persisted
    /// snapshot and database aside — never deleting them, so the data stays on
    /// disk for later inspection — then re-opens a fresh profile. This is what
    /// makes the launch error surface impossible to permanently brick: there is
    /// always an action that reaches a usable state.
    public func resetAndRecover() {
        errorMessage = nil
        // Move the persisted state aside through the shared recovery core so the
        // re-open cannot trip on it again — and so the reset is recorded and
        // preserved (never deleted) exactly like an automatic quarantine.
        // Best-effort: a failure here must not itself become a new dead-end, and
        // `open`'s own recovery is the backstop.
        if let args = lastBootstrapArgs,
           let base = try? (args.storageDirectory ?? Self.defaultStorageDirectory()) {
            let databasePath = base.appendingPathComponent("riot.db").path
            let artifacts: [RecoveryArtifact] =
                [.file(base.appendingPathComponent("riot-profile.json"))]
                + ["", "-wal", "-shm", "-journal"].map {
                    .file(URL(fileURLWithPath: databasePath + $0))
                }
            _ = try? RecoveryQuarantine(storageDirectory: base)
                .quarantine(artifacts, reason: .startFresh, error: nil)
        }
        repository = nil
        retryBootstrap()
    }

    public func openNearbySyncBoundary() throws -> MobileSyncSessionBoundary {
        guard let repository else { throw RepositoryError.profileClosed }
        return try repository.openSyncBoundary()
    }

    /// The profile a nearby pairing acts on: it announces the space this phone is
    /// in, and joins the peer's if this phone has none.
    public var nearbySpaceHost: NearbySpaceHost? { repository }

    /// Re-reads everything the store owns. Called after this phone joins a peer's
    /// space, where the profile gains a space — and, once the sync lands, a board
    /// and a set of apps — without the person having done anything on this screen.
    public func refreshFromStore() {
        space = repository?.currentSpace
        entries = (try? repository?.currentEntries()) ?? []
        // Joining regenerates the author, so this person's own tag is not what it
        // was a moment ago (the repository re-claims their name under the new one).
        // Re-read who they are, or the identity on screen is the pre-join one.
        me = try? repository?.me()
        refreshApps()
        refreshDisplayNames()
        // Joining a space makes this person a MEMBER of it, not its organizer.
        refreshOrganizerState()
    }

    /// Opens (or restores) the on-device profile and installs the starter tools.
    ///
    /// `storageDirectory`, `keyStore`, and `starterPacks` all carry their
    /// production defaults; tests override them to isolate storage, skip the
    /// Keychain, and force the starter set (an empty list emulates the packs
    /// failing to load). When `starterPacks` is nil the packs are read from the
    /// app bundle, falling back to the source tree on a DEBUG simulator run.
    public func bootstrap(
        storageDirectory: URL? = nil,
        keyStore: WrappingKeyStore = KeychainWrappingKeyStore(),
        starterPacks: [(manifest: Data, bundle: Data)]? = nil,
        starterPackResolver: ((String) -> Data?)? = nil
    ) {
        guard repository == nil else { return }
        lastBootstrapArgs = (storageDirectory, keyStore, starterPacks, starterPackResolver)

        // Resolve the starter catalog first. A missing/unreadable built-in is a
        // loud, recoverable failure (§4.7 Catalog/package failed) — never a
        // silently short Tools surface, and never a raw internal error string.
        let resolvedPacks: [(manifest: Data, bundle: Data)]
        if let starterPacks {
            resolvedPacks = starterPacks
        } else {
            do {
                resolvedPacks = try Self.loadStarterPacks(
                    resolve: starterPackResolver ?? { Self.loadPackData(named: $0) }
                )
            } catch let error as StarterCatalogError {
                starterCatalogFailure = StarterCatalogFailure(
                    code: StarterCatalogFailure.catalogUnavailableCode,
                    technicalDetails: error.technicalDetails
                )
                return
            } catch {
                starterCatalogFailure = StarterCatalogFailure(
                    code: StarterCatalogFailure.catalogUnavailableCode,
                    technicalDetails: String(describing: error)
                )
                return
            }
        }

        do {
            let base = try storageDirectory ?? Self.defaultStorageDirectory()
            let storage = try ProtectedProfileStorage(fileURL: base.appendingPathComponent("riot-profile.json"))
            // Durable SQLite store: spaces and accepted entries survive a
            // relaunch without replaying bundles. Falls back to in-memory if
            // the directory isn't writable.
            let databasePath = base.appendingPathComponent("riot.db").path
            let repository = try RiotProfileRepository.open(
                storage: storage,
                keyStore: keyStore,
                starterPacks: resolvedPacks,
                databasePath: databasePath
            )
            self.repository = repository
            // A self-healing open lands the person in a usable app; surface an
            // honest, non-fatal notice about what it had to drop rather than the
            // old dead RETRY. `nil` on a clean open.
            recoveryNotice = repository.recovery
            demoLoader = RiotDemoSpaceLoader(repository: repository, model: self)
            // Headless two-node testing: with RIOT_SEED_SPACE=1 a fresh phone
            // opens a space on launch, so one scripted instance can host a space
            // for another (fresh) instance to auto-join and sync. Seed BEFORE
            // reload so the space exists by the time `me` is published and the
            // readiness gate opens discovery — otherwise the host advertises
            // spaceless. Off by default; opening a space is a person's decision.
            if repository.currentSpace == nil,
               ProcessInfo.processInfo.environment["RIOT_SEED_SPACE"] == "1" {
                _ = try? repository.createPublicSpace(title: "Test Space")
            }
            reload()
            // LAST, and only on the success path: this is what lets the Connection
            // screen start advertising, and it must not open until the space above
            // is readable. See ``isProfileOpen``.
            isProfileOpen = true
        } catch {
            errorMessage = String(describing: error)
        }
    }

    /// Recovery action for the §4.7 "Catalog/package failed" state: clears the
    /// failure and re-attempts the one-time starter install with the same
    /// arguments the last `bootstrap` used. Safe to call repeatedly —
    /// `bootstrap` no-ops once the profile is open.
    public func retryStarterCatalog() {
        guard let args = lastBootstrapArgs else { return }
        starterCatalogFailure = nil
        bootstrap(
            storageDirectory: args.storageDirectory,
            keyStore: args.keyStore,
            starterPacks: args.starterPacks,
            starterPackResolver: args.starterPackResolver
        )
    }

    /// Re-reads everything the screens draw from the open profile: the listed
    /// space, its board, its tools, and the names of the people on both.
    ///
    /// This is the one refresh path. Anything that changes the store from outside
    /// the model — a sync round landing, demo mode loading the seeded space —
    /// calls it, which is what makes entries that arrived over the air show up on
    /// the board without a relaunch.
    public func reload() {
        guard let repository else { return }
        perform {
            space = repository.currentSpace
            // The newswire descriptor id is persisted per community (CommunityRow),
            // not only captured at create time. Re-derive it on every reload so a
            // restored, switched, or joined community projects its wire instead of
            // degrading to the "updates unavailable" state. Previously only
            // createCommunity set this, so any community reached after an app
            // relaunch or a switch had a permanently dead newswire.
            if let namespaceID = space?.namespaceID {
                newswireDescriptorEntryID = (try? repository.listCommunities())?
                    .first { $0.namespaceId == namespaceID }?
                    .descriptorEntryId
            } else {
                newswireDescriptorEntryID = nil
            }
            entries = try repository.currentEntries()
            isDemoMode = repository.isDemoSpaceLoaded
            me = try repository.me()
            claimedName = repository.claimedName
            noteArrivals()
            refreshApps()
            refreshDisplayNames()
            refreshOrganizerState()
            refreshCommunities()
        }
    }

    /// Re-reads the held communities for the chooser. A failure leaves the last
    /// list rather than blanking the chooser.
    private func refreshCommunities() {
        guard let repository else { return }
        if let rows = try? repository.listCommunities() {
            communities = rows.map { CommunityChooserRow.from($0) }
        }
    }

    /// Opens the Level-1 community chooser (Command-K / the community-name control).
    public func openCommunityChooser() { isCommunityChooserPresented = true }

    /// Dismisses the chooser without changing communities.
    public func dismissCommunityChooser() { isCommunityChooserPresented = false }

    /// Switches to another held community. The switch cancels in-flight work and
    /// fails closed in core; here we reload everything the screens draw so the
    /// board, tools, people, and organizer state all reflect the new community,
    /// then land on Home. A community that cannot open surfaces the §4.7
    /// community-unavailable recovery in place rather than a blank screen.
    public func switchCommunity(namespaceID: String) {
        guard let repository else { return }
        do {
            let row = try repository.switchToCommunity(namespaceID: namespaceID)
            communityUnavailable = nil
            isCommunityChooserPresented = false
            destination = .home
            reload()
            _ = row
        } catch {
            // A row that could not open (archived / quarantined / no author) is
            // preserved with recovery, never dropped or shown as a raw error.
            let name = communities.first { $0.namespaceID == namespaceID }?.name ?? "This community"
            markCommunityUnavailable(CommunityUnavailable(name: name))
            isCommunityChooserPresented = false
        }
    }

    /// Follows a SECOND community from a pasted `riot://newswire/join/v1/...`
    /// share reference (Unit 3D — manual multi-community join). Decodes the
    /// reference to its namespace, joins as a fresh unlinkable member (parking the
    /// current community, never replacing it), and reprojects the shell onto the
    /// joined community. The reference carries only coordinates, so the community
    /// is "pending first sync" — its descriptor and content arrive over sync; the
    /// shell shows that honestly rather than fabricating a feed. A malformed or
    /// incomplete reference is refused into ``errorMessage`` and changes nothing.
    public func joinAdditionalCommunity(shareReference: String) {
        guard let repository else { return }
        do {
            let reference = try repository.decodeShareReference(shareReference)
            _ = try repository.joinAdditionalCommunity(
                RiotSpace(
                    namespaceID: reference.namespaceId,
                    title: CommunityShareJoin.provisionalTitle(namespaceID: reference.namespaceId)
                ),
                descriptorEntryID: reference.descriptorEntryId
            )
            errorMessage = nil
            communityUnavailable = nil
            isCommunityChooserPresented = false
            destination = .home
            reload()
        } catch {
            errorMessage = Self.joinRefusal
        }
    }

    /// The outcome of the last `riot://open?...` verify link the app was handed,
    /// or `nil` when none is pending. The shell presents it as an HONEST verify
    /// result (see ``RiotOpenOutcome``) — a "verified" badge only for a post this
    /// device holds as its own signed, signature-verified record. Dismissing clears it.
    @Published public private(set) var openOutcome: RiotOpenOutcome?

    /// Routes an incoming `riot://` deep link. An `open` link surfaces a community
    /// and, for a per-post link, VERIFIES the post against this device's own synced
    /// signed records (never a fake checkmark — see ``RiotOpenOutcome``). A `join`
    /// reference is handed to the established `decodeShareReference` join path
    /// (``joinAdditionalCommunity(shareReference:)``), not duplicated.
    public func handleDeepLink(_ url: URL) {
        guard let link = RiotDeepLink.parse(url) else { return }
        switch link {
        case let .joinReference(encoded):
            joinAdditionalCommunity(shareReference: encoded)
        case let .openSpace(namespace, entry):
            openFromDeepLink(namespace: namespace, entry: entry)
        }
    }

    /// Dismisses the pending verify outcome.
    public func dismissOpenOutcome() { openOutcome = nil }

    /// Opens the community a `riot://open?...` link names and resolves the honest
    /// verify outcome. If this device does not follow the community there is
    /// nothing to verify against, so it offers to join and verify after sync. If it
    /// does, the community's Home opens and — for a per-post link — the post is
    /// checked against the community's projected, signature-verified wire, so a
    /// mirror cannot make the app confirm a post it never synced.
    private func openFromDeepLink(namespace: String, entry: String?) {
        guard let repository else { return }
        let communities = (try? repository.listCommunities()) ?? []
        guard let row = communities.first(where: {
            $0.namespaceId.caseInsensitiveCompare(namespace) == .orderedSame
        }) else {
            openOutcome = .notFollowing(namespace: namespace, entry: entry)
            return
        }

        // Bring the named community on screen. Only switch when it is not already
        // selected — a switch regenerates the author, which a re-open must not do.
        if space?.namespaceID.caseInsensitiveCompare(row.namespaceId) != .orderedSame {
            switchCommunity(namespaceID: row.namespaceId)
        } else {
            destination = .home
        }

        // Verify the post (if any) against this device's OWN signature-verified
        // wire — presence there means the record passed core's Ed25519-checking
        // import path, which is the whole anti-forgery guarantee.
        var held: Set<String> = []
        var headlines: [String: String] = [:]
        if entry != nil, let descriptor = row.descriptorEntryId,
           let projection = try? repository.projectNewswire(spaceDescriptorEntryID: descriptor) {
            let posts = projection.openWire + projection.frontPage + projection.earlier
            for post in posts {
                let key = post.entryId.lowercased()
                held.insert(key)
                if let headline = post.headline { headlines[key] = headline }
            }
        }
        openOutcome = RiotDeepLinkResolver.resolveOpen(
            namespace: row.namespaceId,
            entry: entry,
            followsNamespace: true,
            heldEntryIDs: held,
            headlineForEntry: { headlines[$0] })
    }

    /// Core and the codec both answer a bad paste with the same opaque error, so
    /// the sentence names the likely causes rather than guessing at one.
    private static let joinRefusal =
        "Riot couldn’t join from that link. It may be incomplete or not a Riot community reference — check you pasted the whole thing and try again."

    /// Seals every held community's author under the secure-store wrapping key so
    /// the communities survive a reopen. Best-effort — called after create; a
    /// failure does not block the create.
    private func persistCommunitiesQuietly() {
        try? repository?.persistCommunities()
    }

    /// Claims a name for this person — the one thing on this screen that decides
    /// how they appear to everyone they sync with.
    ///
    /// Core owns the rules (see `RiotProfileRepository.setDisplayName`), so this
    /// does not pre-validate: it lets core refuse and translates that refusal.
    /// The refusal is deliberately not routed through ``errorMessage`` — see
    /// ``nameError``.
    public func setDisplayName(_ name: String) {
        guard let repository else { return }
        do {
            try repository.setDisplayName(name)
            nameError = nil
            // The claim changed how this person renders, and they are in their own
            // name map — re-read both, so the field they just typed into echoes
            // back the `Ana · a3f91122` their neighbour is about to see.
            me = try repository.me()
            claimedName = repository.claimedName
            refreshDisplayNames()
        } catch {
            nameError = Self.nameRefusal
            // The name on screen is still the one core holds — this attempt
            // changed nothing — so leave `me` alone rather than clearing it.
        }
    }

    /// Core answers both "that name is not usable" and "a sync is in flight" with
    /// the same `InvalidInput`, and there is no third field to tell them apart. So
    /// the sentence names both causes rather than guessing at one and being
    /// confidently wrong at the person.
    private static let nameRefusal =
        "Riot couldn’t save that name. It may be too long, or Riot may be syncing with someone right now — wait a moment and try again."

    /// Works out which of the entries now on the board were not on this phone a
    /// moment ago. The first read is the baseline (see ``arrivals``).
    private func noteArrivals() {
        let ids = Set(entries.map(\.id))
        arrivals = knownEntryIDs.map { ids.subtracting($0) } ?? []
        knownEntryIDs = ids
    }

    /// The rendered name for a signer, or nil if this device cannot name them.
    ///
    /// Nil is a real answer and the caller must honour it by drawing NOTHING. It
    /// must never fall back to the raw id: a 64-character key is not a name, and
    /// showing one is precisely the failure the display-name work exists to end.
    public func rendered(for signerID: String) -> String? {
        displayNames[signerID.lowercased()]
    }

    /// The attribution line for a board row: `"Posted by Ana · a3f91122"`.
    public func postedBy(_ entry: RiotEntry) -> String? {
        rendered(for: entry.signerID).map { "Posted by \($0)" }
    }

    /// Names every person the current board and directory can point at.
    ///
    /// Two sources, in this order: the profile cards this device holds (one call,
    /// every name it knows), and then — for any signer with no profile card yet —
    /// core's own fallback for that id (`member · a3f91122`). The second pass is
    /// what keeps a row signed by someone whose profile has not synced yet from
    /// falling back to hex.
    private func refreshDisplayNames() {
        guard let repository else { return }
        var names = (try? repository.displayNames()) ?? [:]
        for signer in Set(entries.map { $0.signerID.lowercased() }) where names[signer] == nil {
            guard let person = try? repository.person(idHex: signer) else { continue }
            names[signer] = person.rendered
        }
        displayNames = names
    }

    /// Demo mode's port onto the live profile, or nil before one is open.
    ///
    /// The repository is what conforms — it persists the loaded space and the
    /// bundle, so the demo survives the phone being put down between loading it
    /// backstage and walking on. This wrapper exists only to pull the model back
    /// in step afterwards; without it the seeded board would be sitting in Rust
    /// with nothing on screen showing it.
    public private(set) var demoLoader: DemoSpaceLoading?

    /// Where this instance keeps its profile.
    ///
    /// Every instance of the app shares one container — and therefore one
    /// `riot-profile.json`, one identity — so two windows on a Mac are the same
    /// person and syncing them is a no-op. `RIOT_PROFILE_ID` gives each instance
    /// its own profile so they are genuinely different people, which is what
    /// makes nearby sync testable on a single machine.
    ///
    /// It selects a SUBDIRECTORY of the container rather than taking a path:
    /// under App Sandbox the app cannot write to an arbitrary location like
    /// `/tmp/riot-a`, so a path override would fail at runtime on macOS. The
    /// container is shared by both instances, so subdirectories of it are legal
    /// for each. `RIOT_PROFILE_DIR` still takes an explicit path, for tests and
    /// unsandboxed hosts.
    private static func defaultStorageDirectory() throws -> URL {
        let environment = ProcessInfo.processInfo.environment
        if let path = environment["RIOT_PROFILE_DIR"], !path.isEmpty {
            let url = URL(fileURLWithPath: path, isDirectory: true)
            try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
            return url
        }
        let container = try FileManager.default.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        guard let id = environment["RIOT_PROFILE_ID"], !id.isEmpty else { return container }
        // Keep it a single path component: an id like "../../elsewhere" must not
        // walk out of the container.
        let safe = id.replacingOccurrences(of: "/", with: "_")
        let url = container.appendingPathComponent("instances", isDirectory: true)
            .appendingPathComponent(safe, isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    public func createSpace(title: String) {
        perform {
            guard let repository else { return }
            space = try repository.createPublicSpace(title: title)
            refreshApps()
            // Creating a space is what makes you its organizer.
            refreshOrganizerState()
            persistCommunitiesQuietly()
            refreshCommunities()
            destination = .home
        }
    }

    /// The selected community as the shell reads it: name + namespace from the
    /// backing space, the newswire descriptor id when known, and organizer
    /// status from core. `nil` when there is no selected community. Views bind to
    /// this — never to `space` directly — so Unit 3 can swap the selection
    /// source without touching a route view.
    public var community: CommunityContext? {
        guard let space else { return nil }
        return CommunityContext(
            name: space.title,
            namespaceID: space.namespaceID,
            newswireDescriptorEntryID: newswireDescriptorEntryID,
            isOrganizer: canApproveApps
        )
    }

    /// The "Your communities" chooser rows (Unit 3), most-recently-active first,
    /// in plain language. Refreshed by `reload()` from the registry; empty on a
    /// single-community device, which the chooser reads as "no switcher needed".
    @Published public private(set) var communities: [CommunityChooserRow] = []

    /// Whether the Level-1 community chooser is presented. `Command-K` opens it;
    /// selecting or dismissing closes it.
    @Published public var isCommunityChooserPresented = false

    /// The launch state the shell renders before any route: loading while the
    /// profile opens, no-community when there is none, the community's Home when
    /// there is one, or in-place recovery when a retained one cannot open. Never
    /// a blank screen (nav design §4.7).
    public var launchState: ShellLaunchState {
        if let communityUnavailable { return .unavailable(communityUnavailable) }
        guard isProfileOpen else { return .loading }
        guard let community else { return .noCommunity }
        return .community(community)
    }

    /// Creates a community: the founding collective's initial choices become a
    /// signed, immutable `SpaceDescriptorV1` (via `createNewswireSpace`, carrying
    /// the chosen editorial roster) plus the app-trust backing space that carries
    /// its tools and nearby coordinator. The creator is the founding organizer +
    /// editor by construction. Lands on Home.
    /// Marks the selected community unavailable, so the shell renders the §4.7
    /// community-unavailable recovery in place. The remembered name and a fixed
    /// code drive the recovery view; nothing is erased.
    public func markCommunityUnavailable(_ unavailable: CommunityUnavailable) {
        communityUnavailable = unavailable
    }

    /// Clears the community-unavailable state — the Retry recovery action, which
    /// re-attempts opening the community context that is already in memory.
    public func retryCommunity() {
        communityUnavailable = nil
    }

    /// Leaves the selected community, returning the shell to the no-community
    /// launch state. In Slices 0–2 this is a session-scoped view change (the
    /// signed records stay in the store); Unit 3 replaces it with real
    /// multi-community removal. Callers gate this on the dirty-draft
    /// Stay-or-Discard confirmation (``CommunityChangeGuard``) so unsaved work is
    /// never lost silently.
    public func leaveCommunity() {
        space = nil
        newswireDescriptorEntryID = nil
        communityUnavailable = nil
        destination = .home
    }

    public func createCommunity(_ request: CommunityCreationRequest) {
        guard let repository else { return }
        // A newswire SpaceDescriptorV1 requires a non-empty summary; core rejects
        // an empty one with InvalidInput. The founder form does not collect a
        // summary, so default it to the community name. Without this, create signs
        // no descriptor and the community launches with a permanently dead wire
        // ("updates unavailable"), which is exactly the newswire being invisible.
        let trimmedSummary = request.summary.trimmingCharacters(in: .whitespacesAndNewlines)
        let normalized = trimmedSummary.isEmpty
            ? CommunityCreationRequest(
                name: request.name,
                summary: request.name,
                editorialRoster: request.editorialRoster,
                approvedStarterAppIDs: request.approvedStarterAppIDs
            )
            : request
        let coordinator = CommunityCreationCoordinator(backing: repository, descriptor: repository)
        perform {
            let context = try coordinator.create(normalized)
            space = repository.currentSpace
            newswireDescriptorEntryID = context.newswireDescriptorEntryID
            communityUnavailable = nil
            refreshApps()
            refreshOrganizerState()
            // Seal the new community's author so it survives a reopen, then refresh
            // the chooser so it appears in "Your communities".
            persistCommunitiesQuietly()
            refreshCommunities()
            destination = .home
        }
    }

    /// True when this person may approve apps here — i.e. they are this space's
    /// organizer. The review sheet reads it to decide whether "Let everyone here
    /// use this" is offered at all.
    @Published public private(set) var canApproveApps = false

    /// True for a profile made before spaces had organizers. It can never approve
    /// an app for any space, so it needs different advice from a member: start a
    /// new profile, rather than ask the organizer.
    @Published public private(set) var isLegacyProfile = false

    private func refreshOrganizerState() {
        canApproveApps = (try? repository?.isOrganizer()) ?? false
        isLegacyProfile = !((try? repository?.canOrganize()) ?? true)
    }

    /// Why an approval could not happen, in words a person can act on.
    ///
    /// The refusals are real and stay. What must never happen again is the one
    /// rabble hit: `set_app_trust` returned `InvalidInput`, the sheet closed, and
    /// the app simply never appeared — no reason given, and none discoverable.
    static func approvalFailureMessage(_ error: Error) -> String {
        switch error as? MobileError {
        case .LegacyProfileCannotOrganize:
            return "This profile was made before spaces had organizers, so it can’t "
                + "approve apps for this space. Start a new profile to organize one."
        case .NotSpaceOrganizer:
            return "Only the organizer of this space can turn an app on here."
        default:
            return String(describing: error)
        }
    }

    /// Trusts an app in this space so everyone here can use it, then refreshes
    /// the listing so the row flips from "Review" to "Open".
    public func trustApp(appID: String) {
        guard let repository else { return }
        do {
            try repository.trustApp(appID: appID)
            errorMessage = nil
            refreshApps()
        } catch {
            // Not `perform`: its `String(describing:)` is exactly how "InvalidInput"
            // reached a person who had done nothing wrong.
            errorMessage = Self.approvalFailureMessage(error)
        }
        refreshOrganizerState()
    }

    /// Revokes trust for an app in the current space. Organizer-gated like
    /// `trustApp` (a member turning an app off has the same organizer gate as
    /// turning one on).
    public func untrustApp(appID: String) {
        guard let repository else { return }
        do {
            try repository.untrustApp(appID: appID)
            errorMessage = nil
            refreshApps()
        } catch {
            errorMessage = Self.approvalFailureMessage(error)
        }
        refreshOrganizerState()
    }

    private func refreshApps() {
        apps = (try? repository?.spaceApps()) ?? []
    }

    private func perform(_ operation: () throws -> Void) {
        do {
            try operation()
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    // MARK: - Starter packs

    /// The canonical starter catalog: the eight built-in tool slugs, in the
    /// exact order of `STARTER_CATALOG` in `crates/riot-core/src/apps/starter.rs`.
    /// Rust is the source of truth; `apps_starter.rs`'s ordered-catalog test and
    /// `StarterResourceTests` both fail if this list drifts from it or from the
    /// bundled resources. Adding a tool means adding it here AND registering its
    /// two `.cbor` artifacts in both Apple app targets and the RiotTests bundle.
    static let starterCatalog = [
        "checklist", "supply-board", "roll-call", "quick-poll",
        "chat", "dispatches", "wiki", "photo-wall",
    ]

    /// Loads every pair in the starter catalog. Unlike the previous `compactMap`,
    /// a pair that cannot be read is a **loud** failure: it throws rather than
    /// silently shrinking the Tools surface. A built-in that is listed but not
    /// bundled is a build defect, never a tool that quietly vanishes (§6 Unit 0A).
    /// `resolve` is injectable so the failure path is testable without unbundling
    /// a real artifact.
    static func loadStarterPacks(
        resolve: (String) -> Data? = { loadPackData(named: $0) }
    ) throws -> [(manifest: Data, bundle: Data)] {
        try starterCatalog.map { slug in
            guard let manifest = resolve("\(slug).manifest") else {
                throw StarterCatalogError(slug: slug, pack: .manifest)
            }
            guard let bundle = resolve("\(slug).bundle") else {
                throw StarterCatalogError(slug: slug, pack: .bundle)
            }
            return (manifest: manifest, bundle: bundle)
        }
    }

    private static func loadPackData(named name: String) -> Data? {
        for url in packURLs(named: name) {
            if let data = try? Data(contentsOf: url) { return data }
        }
        return nil
    }

    /// Candidate locations for a `.cbor` starter artifact, in order: the app
    /// bundle (device/release), then the checked-in fixtures resolved from this
    /// source file (DEBUG only — the path exists on the host filesystem a
    /// simulator shares, but not on a device).
    private static func packURLs(named name: String) -> [URL] {
        var urls: [URL] = []
        if let bundled = Bundle.main.url(forResource: name, withExtension: "cbor") {
            urls.append(bundled)
        }
        #if DEBUG
        urls.append(sourceTreeFixtures().appendingPathComponent("\(name).cbor"))
        #endif
        return urls
    }

    /// `fixtures/apps` resolved four levels up from this file at
    /// `apps/ios/Riot/AppModel.swift`, matching the repository tests' convention.
    private static func sourceTreeFixtures(file: StaticString = #filePath) -> URL {
        URL(fileURLWithPath: "\(file)")
            .deletingLastPathComponent() // Riot
            .deletingLastPathComponent() // ios
            .deletingLastPathComponent() // apps
            .deletingLastPathComponent() // repo root
            .appendingPathComponent("fixtures/apps")
    }
}

/// The shell reads its launch state through this seam; Unit 3 swaps the
/// conformer for a multi-community registry without touching a route view.
extension RiotAppModel: CommunitySelecting {}

/// Demo mode's port, wired to the live profile AND to the screens.
///
/// The import itself is the repository's (which is what persists it). All this
/// adds is the step the port cannot know about: telling the model to re-read, so
/// the seeded board is on screen the moment the sheet says it loaded.
///
/// It is deliberately NOT `@MainActor`, because `DemoSpaceLoading` is not: a
/// main-actor conformance to a non-isolated protocol does not compile under
/// Swift 6. The refresh hops back to the main actor itself, which is also why
/// `model` is safe to capture — a `@MainActor` class is `Sendable`.
public final class RiotDemoSpaceLoader: DemoSpaceLoading {
    private let repository: RiotProfileRepository
    private weak var model: RiotAppModel?

    public init(repository: RiotProfileRepository, model: RiotAppModel?) {
        self.repository = repository
        self.model = model
    }

    public func loadDemoSpace(bytes: Data) throws -> RiotSpace {
        let space = try repository.loadDemoSpace(bytes: bytes)
        reloadModel()
        return space
    }

    public func hideDemoSpace() throws {
        try repository.hideDemoSpace()
        reloadModel()
    }

    private func reloadModel() {
        let model = model
        Task { @MainActor in model?.reload() }
    }
}

/// What the signed-alert detail shows, as a value rather than a view — the
/// board's rows are read-only, so this is the whole product decision behind
/// tapping one, and it is pinned by tests without rendering anything.
///
/// The split is the point. A person who opens an alert sees the headline, the
/// AI-assistance flag and the validity window: the parts they can act on. The
/// 64-hex identifiers are evidence, not reading material, so they live behind a
/// **Technical details** disclosure that starts closed — full ids never lead a
/// surface (navigation design's accessibility contract).
public struct AlertDetail: Equatable, Sendable {
    public struct Row: Equatable, Sendable {
        public let label: String
        public let value: String
    }

    /// The disclosure a person has to open before any full identifier is shown.
    public static let technicalDisclosureTitle = "Technical details"

    public let headline: String
    public let aiAssisted: Bool
    /// Shown as soon as the sheet opens.
    public let summary: [Row]
    /// Shown only once **Technical details** is opened.
    public let technical: [Row]

    public init(entry: RiotEntry) {
        headline = entry.headline
        aiAssisted = entry.aiAssisted

        var visible = [Row(label: "Created", value: Self.timestamp(entry.createdAt))]
        if let validFrom = entry.validFrom {
            visible.append(Row(label: "Valid from", value: Self.timestamp(validFrom)))
        }
        visible.append(Row(label: "Expires", value: Self.timestamp(entry.expiresAt)))
        summary = visible

        technical = [
            Row(label: "Entry", value: entry.entryID),
            Row(label: "Namespace", value: entry.namespaceID),
            Row(label: "Signer", value: entry.signerID),
        ]
    }

    static func timestamp(_ epochSeconds: UInt64) -> String {
        Date(timeIntervalSince1970: TimeInterval(epochSeconds))
            .formatted(.dateTime.year().month().day().hour().minute())
    }
}
