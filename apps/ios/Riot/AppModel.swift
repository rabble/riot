import Foundation
import SwiftUI

public enum RiotDestination: String, CaseIterable, Identifiable, Sendable {
    case spaces
    case directory
    case board
    case compose
    case connection

    public var id: Self { self }

    public static let phoneTabs = allCases

    public var title: String {
        switch self {
        case .spaces: "Spaces"
        case .directory: "App directory"
        case .board: "Incident board"
        case .compose: "Compose & sign"
        case .connection: "Connection"
        }
    }

    public var tabTitle: String {
        switch self {
        case .spaces: "Spaces"
        case .directory: "Apps"
        case .board: "Board"
        case .compose: "Compose"
        case .connection: "Connect"
        }
    }

    public var systemImage: String {
        switch self {
        case .spaces: "square.stack.3d.up"
        case .directory: "square.grid.2x2"
        case .board: "exclamationmark.bubble"
        case .compose: "square.and.pencil"
        case .connection: "antenna.radiowaves.left.and.right"
        }
    }
}

public enum RiotConnectionStatus: Equatable, Sendable {
    case offline
    case nearby(String)
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
    @Published public var destination: RiotDestination = .spaces

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
        starterPacks: [(manifest: Data, bundle: Data)]? = nil
    ) {
        guard repository == nil else { return }
        do {
            let base = try storageDirectory ?? Self.defaultStorageDirectory()
            let storage = try ProtectedProfileStorage(fileURL: base.appendingPathComponent("riot-profile.json"))
            let repository = try RiotProfileRepository.open(
                storage: storage,
                keyStore: keyStore,
                starterPacks: starterPacks ?? Self.loadStarterPacks()
            )
            self.repository = repository
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
            entries = try repository.currentEntries()
            isDemoMode = repository.isDemoSpaceLoaded
            me = try repository.me()
            claimedName = repository.claimedName
            noteArrivals()
            refreshApps()
            refreshDisplayNames()
            refreshOrganizerState()
        }
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
            destination = .board
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

    private func refreshApps() {
        apps = (try? repository?.spaceApps()) ?? []
    }

    public func sign(headline: String, description: String, aiAssisted: Bool) {
        perform {
            guard let repository, let space else { return }
            let expiry = UInt64(Date().timeIntervalSince1970) + 3_600
            _ = try repository.signAlert(
                in: space,
                draft: AlertDraft(
                    expiresAt: expiry,
                    headline: headline,
                    description: description,
                    sourceClaims: ["Local conference participant"],
                    aiAssisted: aiAssisted
                )
            )
            entries = try repository.currentEntries()
            // An alert you signed yourself did not arrive from anyone: it stamps
            // onto the board like any other entry, but it must not buzz in your
            // hand as though someone else had posted it.
            noteArrivals()
            arrivals = []
            destination = .board
        }
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

    /// The frozen starter catalog to install on open. A pair that cannot be read
    /// is dropped (Rust remains the integrity oracle for the bytes we do read),
    /// so a missing artifact leaves the Tools list empty rather than failing
    /// `bootstrap`.
    /// Mirrors `STARTER_APPS` in `crates/riot-core/src/apps/starter.rs`, in the
    /// same order. Adding a starter app means adding it here AND adding its two
    /// `.cbor` artifacts to the Riot target's Resources build phase — a pack
    /// that is listed but not bundled is silently dropped below.
    private static let starterAppNames = ["checklist", "supply-board", "roll-call", "quick-poll"]

    private static func loadStarterPacks() -> [(manifest: Data, bundle: Data)] {
        starterAppNames.compactMap { app in
            guard let manifest = loadPackData(named: "\(app).manifest"),
                  let bundle = loadPackData(named: "\(app).bundle")
            else { return nil }
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
