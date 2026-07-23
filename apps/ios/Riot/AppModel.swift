import Foundation
import OSLog
import SwiftUI

/// The app's built-in "known relay + known community": the deployed GCP anchor
/// relay's stable NodeId and a root-signed ReadCommitted ticket for a community
/// already committed on that relay. Baked in so the app can pull a real community
/// out of the box — no IP, no port, no manual paste. The relay is dialed by its
/// NodeId ALONE: iroh relay + pkarr/DNS discovery resolves the address.
///
/// Lives in RiotKit (not the app target) because ``RiotAppModel/syncFromRelay()``
/// drives the pull; the visible `AnchorRelaySyncCard` reads this too.
public enum AnchorRelayDefaults {
    /// The deployed relay's stable NodeId (64 hex) — the whole dial hint.
    public static let relayNodeId =
        "60ab7b416b0ef0b8088cd64a3ef01edd598dcc5bb7a4df03145f957fec2432d8"

    /// A root-signed ReadCommitted ticket (hex) for a community already committed
    /// on the relay (O masthead + C comments + W newswire wire). Re-baked
    /// 2026-07-24 by reseeding the live relay with a real newswire community
    /// (demo_host over discovery); community root (W)
    /// 452760690dc2b6d0d73c3ce5a1b9985751def04945d3d7d00121cff42e9ef544
    /// ("River City Wire" — 3 posts from distinct people). Durable 89-day ticket.
    public static let communityTicketHex =
        "83028c58207f6c42e7988f6ee2654cf3e1177c614086d54e0dcd9f1905c8460083036472c358207f6c42e7988f6ee2654cf3e1177c614086d54e0dcd9f1905c8460083036472c3582026f1ad8ff8789248f171487257cc5a0a0e6d17f24469ad107377d961f6b78a8a5820452760690dc2b6d0d73c3ce5a1b9985751def04945d3d7d00121cff42e9ef54458204ee5784092f6176e5599d68dd31d7de1d2c2b970f504e0975ac78994f77ebb951a6a62989f026c726571756972655f6e6f6e656c726571756972655f6e6f6e65011a6a62983b1a6a62a6af5840112e56fe6383b87b8c5900e0b9f739bd41cba9d8bb182b5b09dea05e3c068005ea1a57640b9ea9156b410f0f0a96f0569ca52946a240ee92c42b583435fddd06"

    /// A human name for the built-in community, shown when its own signed
    /// descriptor doesn't carry one. A real newswire descriptor name overrides it.
    public static let communityDisplayName = "River City Wire"

    /// Decode the baked ticket hex to bytes.
    public static var communityTicket: Data { data(fromHex: communityTicketHex) }

    static func data(fromHex hex: String) -> Data {
        var data = Data(capacity: hex.count / 2)
        var index = hex.startIndex
        while index < hex.endIndex {
            let next = hex.index(index, offsetBy: 2)
            guard let byte = UInt8(hex[index..<next], radix: 16) else { return Data() }
            data.append(byte)
            index = next
        }
        return data
    }
}

/// The human outcome of a relay pull, as the card reads it. Leads with the
/// COMMUNITY — its name, how many people and posts are there — not with protocol
/// counts. `namespaceID` is set (and ``isWalkInReady`` true) only when the
/// community was adopted into the chooser and can actually be opened.
public struct RelaySyncResult: Equatable, Sendable {
    public let communityName: String
    public let namespaceID: String?
    public let peopleCount: Int
    public let postCount: Int
    public let syncedAt: Date
    public let isWalkInReady: Bool

    public init(
        communityName: String,
        namespaceID: String?,
        peopleCount: Int,
        postCount: Int,
        syncedAt: Date,
        isWalkInReady: Bool
    ) {
        self.communityName = communityName
        self.namespaceID = namespaceID
        self.peopleCount = peopleCount
        self.postCount = postCount
        self.syncedAt = syncedAt
        self.isWalkInReady = isWalkInReady
    }
}

/// Developer-facing trail for a relay pull — the namespace ids, verified/rejected
/// counts, and refusal strings stay HERE (os_log), never on the person's screen.
enum RelaySyncLog {
    private static let logger = Logger(subsystem: "net.protest.riot", category: "anchor-relay")

    static func pullLanded(root: String, imported: Int, namespaces: [NamespacePullOutcome]) {
        logger.log("anchor-relay: durable pull landed root=\(root, privacy: .public) imported=\(imported, privacy: .public)")
        for ns in namespaces {
            logger.log("anchor-relay: ns=\(ns.namespaceId, privacy: .public) verified=\(ns.verified, privacy: .public) imported=\(ns.imported, privacy: .public) rejected=\(ns.rejected, privacy: .public) refusal=\(ns.refusal ?? "none", privacy: .public)")
        }
    }

    static func adoptFailed(namespace: String, error: Error) {
        logger.error("anchor-relay: adopt failed ns=\(namespace, privacy: .public): \(error.localizedDescription, privacy: .public)")
    }

    static func discovered(_ candidates: [SyncedCommunityCandidate]) {
        logger.log("anchor-relay: discovered \(candidates.count, privacy: .public) candidate(s)")
        for c in candidates {
            logger.log("anchor-relay: candidate ns=\(c.namespaceId, privacy: .public) descriptor=\(c.descriptorEntryId ?? "nil", privacy: .public) name=\(c.name ?? "nil", privacy: .public) posts=\(c.postCount, privacy: .public) alerts=\(c.alertCount, privacy: .public) people=\(c.contributorCount, privacy: .public)")
        }
    }

    static func discoverFailed(error: Error) {
        logger.error("anchor-relay: discover failed: \(error.localizedDescription, privacy: .public)")
    }
}

/// The three supported ways to leave first-run setup. Nearby exchange requires
/// an active community, so it is deliberately not an onboarding exit.
public enum OnboardingExit: CaseIterable, Equatable, Sendable {
    case join
    case create
    case demo
}

public enum OnboardingExitResult: Equatable, Sendable {
    case proceeded
    case nameSaveFailed
}

public enum OnboardingPresentation {
    public static let actionOrder: [OnboardingExit] = [.join, .create, .demo]
    public static let nearbyNote =
        "Nearby exchange is available after you enter a community."
}

/// One fail-closed dispatcher for every setup exit. A blank optional name is
/// skipped; a typed name must be saved before any community action may begin.
public enum OnboardingExitGate {
    public static func perform(
        _ exit: OnboardingExit,
        displayName: String,
        saveName: (String) -> Bool,
        proceed: (OnboardingExit) -> Void
    ) -> OnboardingExitResult {
        let name = displayName.trimmingCharacters(in: .whitespacesAndNewlines)
        if !name.isEmpty, !saveName(name) {
            return .nameSaveFailed
        }
        proceed(exit)
        return .proceeded
    }
}

public enum CommunityTransitionReason: Equatable, Sendable {
    case preserveDraft
    case discardDraft
}

public struct CommunityTransitionPreparation: Equatable, Sendable {
    public let reason: CommunityTransitionReason
    public let transportMustContinue: Bool

    public init(reason: CommunityTransitionReason, transportMustContinue: Bool = false) {
        self.reason = reason
        self.transportMustContinue = transportMustContinue
    }
}

public enum CommunityDraftTransition {
    public static func apply(
        _ reason: CommunityTransitionReason,
        persist: () -> Void,
        clear: () -> Void
    ) {
        switch reason {
        case .preserveDraft: persist()
        case .discardDraft: clear()
        }
    }
}

/// Orders community-scoped teardown before repository mutation. Registration is
/// tokened so an old shell cannot clear a newer shell's handler on disappear.
public final class CommunityTransitionGate {
    public struct Token: Equatable, Sendable {
        fileprivate let id: UUID
    }

    private var active: (
        token: Token,
        prepare: (CommunityTransitionPreparation) -> Void,
        recover: () -> Void
    )?

    public init() {}

    public func register(
        _ prepare: @escaping (CommunityTransitionReason) -> Void
    ) -> Token {
        registerPreparation({ prepare($0.reason) })
    }

    public func registerPreparation(
        _ prepare: @escaping (CommunityTransitionPreparation) -> Void,
        recover: @escaping () -> Void = {}
    ) -> Token {
        let token = Token(id: UUID())
        active = (token, prepare, recover)
        return token
    }

    public func unregister(_ token: Token) {
        if active?.token == token { active = nil }
    }

    public func prepare(_ reason: CommunityTransitionReason) {
        active?.prepare(CommunityTransitionPreparation(reason: reason))
    }

    public func prepareForNearbyAdoption() {
        active?.prepare(CommunityTransitionPreparation(
            reason: .preserveDraft,
            transportMustContinue: true
        ))
    }

    public func recoverAfterFailedPreparation() {
        active?.recover()
    }
}

public enum ComposerOrigin: String, CaseIterable, Equatable, Sendable {
    case home
    case emptyWire
    case people
}

public enum ComposerPresentationState: Equatable, Sendable {
    case closed
    case open(ComposerOrigin)

    public mutating func open(_ origin: ComposerOrigin) {
        self = .open(origin)
    }

    public mutating func close() {
        self = .closed
    }

    public var origin: ComposerOrigin? {
        if case let .open(origin) = self { return origin }
        return nil
    }
}

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
    /// The composite indymedia sites this person follows (author-less rows from
    /// `list_followed_sites`), surfaced by ``FollowSiteSheet``. Distinct from
    /// communities — a followed site holds no posting author here.
    @Published public private(set) var followedSites: [FollowedSiteRow] = []
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
    public let communityTransitionGate = CommunityTransitionGate()

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
        reloadFollowedSites()
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

    /// Re-derives the active community's newswire descriptor id from the registry
    /// on demand — the offlineStale "Try again" path. Returns nil when the community
    /// still carries none (a nearby-joined community), which is what puts the wire
    /// into its forward-path (rejoin / sync) state instead of a silent re-loop.
    /// Publishes the fresh value so the shell and Home agree. This is the same
    /// `listCommunities()` derivation `reload()` performs, callable without a full
    /// reload so the wire picks up a descriptor that just landed.
    @discardableResult
    public func rederivedNewswireDescriptorID() -> String? {
        guard let repository, let namespaceID = space?.namespaceID else {
            newswireDescriptorEntryID = nil
            return nil
        }
        let derived = (try? repository.listCommunities())?
            .first { $0.namespaceId == namespaceID }?
            .descriptorEntryId
        newswireDescriptorEntryID = derived
        return derived
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

    /// Whether the join-by-reference sheet (paste / QR) is presented. Raised from the
    /// chooser's "Join another" row and the Launch screen so both entry points share
    /// one sheet and one core call.
    @Published public var isJoinByReferencePresented = false

    /// Whether the chooser's "Create a community" action asked the shell to present
    /// the create flow. A real intent, not a dead no-op.
    @Published public private(set) var isCreateCommunityRequested = false

    /// Chooser "Find one nearby": close the chooser and route to the Nearby surface
    /// (the wired replacement for the old dead `{}` no-op at the call site).
    public func findNearby() {
        isCommunityChooserPresented = false
        select(.nearby)
    }

    /// Chooser "Join another" / Launch "Join with a link or QR": present the
    /// join-by-reference sheet.
    public func requestJoinByReference() {
        isCommunityChooserPresented = false
        isJoinByReferencePresented = true
    }

    /// Dismisses the join-by-reference sheet.
    public func dismissJoinByReference() { isJoinByReferencePresented = false }

    /// Chooser "Create a community": close the chooser and ask the shell to present
    /// the create flow (the wired replacement for the old dead `{}` no-op).
    public func requestCreateCommunity() {
        isCommunityChooserPresented = false
        isCreateCommunityRequested = true
    }

    /// Dismisses the create flow the chooser raised.
    public func dismissCreateCommunity() { isCreateCommunityRequested = false }

    /// Switches to another held community. The switch cancels in-flight work and
    /// fails closed in core; here we reload everything the screens draw so the
    /// board, tools, people, and organizer state all reflect the new community,
    /// then land on Home. A community that cannot open surfaces the §4.7
    /// community-unavailable recovery in place rather than a blank screen.
    public func switchCommunity(namespaceID: String) {
        communityTransitionGate.prepare(.preserveDraft)
        guard let repository else {
            communityTransitionGate.recoverAfterFailedPreparation()
            return
        }
        do {
            let row = try repository.switchToCommunity(namespaceID: namespaceID)
            communityUnavailable = nil
            isCommunityChooserPresented = false
            destination = .home
            reload()
            _ = row
        } catch {
            communityTransitionGate.recoverAfterFailedPreparation()
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
        communityTransitionGate.prepare(.preserveDraft)
        guard let repository else {
            communityTransitionGate.recoverAfterFailedPreparation()
            return
        }
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
            communityTransitionGate.recoverAfterFailedPreparation()
            errorMessage = Self.joinRefusal
        }
    }

    /// Commits a previewed join from the join-by-reference sheet (paste or QR). The
    /// preview already passed `JoinReferenceModel` validation, so the raw string is
    /// known-good; this only decides between JOIN and SWITCH: a namespace already
    /// held routes to a switch (never a duplicate row), otherwise it follows the
    /// established `joinAdditionalCommunity` path. Business logic stays in the
    /// repository/FFI — this forwards.
    public func commitJoin(preview: JoinPreview) {
        guard let repository else { return }
        let held = (try? repository.listCommunities())?.map(\.namespaceId) ?? []
        if JoinReferenceModel().isAlreadyJoined(namespaceIdHex: preview.namespaceIdHex, within: held) {
            switchCommunity(namespaceID: preview.namespaceIdHex)
        } else {
            joinAdditionalCommunity(shareReference: preview.encoded)
        }
    }

    // MARK: - Followed composite sites (Option C HTTP-pull)

    /// Re-reads the followed-sites list from the store. Cheap metadata read; safe
    /// to call after a follow, a refresh-import, or a profile open.
    public func reloadFollowedSites() {
        followedSites = (try? repository?.listFollowedSites()) ?? []
    }

    /// Follows a composite indymedia site from a pasted/scanned `riot://site/v1/...`
    /// ticket. The CORE verifies the ticket's signature and expiry and persists the
    /// `Following` record (this layer only screened the scheme + length); a refusal
    /// surfaces in ``errorMessage`` and changes nothing. On success the followed
    /// list reloads so the new row is visible immediately.
    public func followSite(ticket rawTicket: String) {
        guard let repository else { return }
        do {
            let ticket = try FollowSiteModel().screen(ticket: rawTicket)
            _ = try repository.followSite(ticket: ticket)
            errorMessage = nil
            reloadFollowedSites()
        } catch {
            errorMessage = Self.followSiteRefusal
        }
    }

    /// Pulls the owner-signed bundle for a followed site over HTTPS and imports it.
    /// The pulled bytes are UNTRUSTED: `import_followed_site_bundle` re-verifies
    /// every entry (owner cap + Following-gate + family-gate) before anything lands,
    /// so a bad mirror can only serve stale/empty, never forge. A transport-blocked
    /// site is never routed here (its row offers no refresh and carries no URL). A
    /// network or import failure surfaces in ``errorMessage`` and changes nothing.
    ///
    /// Returns the number of records the core admitted + committed (the
    /// `ImportSummary` count) so the row can show honest "Imported N records"
    /// feedback that the pull actually landed; `nil` on any failure.
    @MainActor
    @discardableResult
    public func refreshFollowedSite(root: String, fetchURL: String) async -> Int? {
        guard let repository else { return nil }
        guard let url = URL(string: fetchURL),
              let rootBytes = FollowSiteModel.hexBytes(root) else {
            errorMessage = Self.followSiteFetchRefusal
            return nil
        }
        do {
            let (data, response) = try await URLSession.shared.data(from: url)
            guard let http = response as? HTTPURLResponse,
                  (200..<300).contains(http.statusCode) else {
                errorMessage = Self.followSiteFetchRefusal
                return nil
            }
            let summary = try repository.importFollowedSiteBundle(
                bytes: data, root: Data(rootBytes))
            errorMessage = nil
            reloadFollowedSites()
            return Int(summary.imported)
        } catch {
            errorMessage = Self.followSiteFetchRefusal
            return nil
        }
    }

    // MARK: - Relay pull (the "leave the room, still connected" path)

    /// The human outcome of the last relay pull, or `nil` before one runs. It
    /// leads with the COMMUNITY — its name, that you're now in it, how many people
    /// and posts are there — not with protocol counts. The card reads this to say
    /// "you're in River City News · 4 posts · 3 people" and to offer Open. When
    /// ``RelaySyncResult/isWalkInReady`` is true the community was adopted into the
    /// chooser and can be opened; when false the data landed durably but there was
    /// no community to walk into yet (surfaced honestly, never as a fake door).
    @Published public private(set) var relaySyncResult: RelaySyncResult?

    /// True while a relay pull is in flight, so the three card placements (Home,
    /// Transport, Onboarding) all reflect ONE shared pull rather than firing their
    /// own. Published so every placement disables its button together.
    @Published public private(set) var isRelaySyncing = false

    /// A plain-language reason the last pull could not connect, or `nil`. Separate
    /// from ``errorMessage`` so a relay hiccup speaks on the relay card, not as an
    /// app-wide alert.
    @Published public private(set) var relaySyncError: String?

    /// Per-community "last synced" wall-clock, keyed by lowercase namespace hex.
    /// Held in memory (published) — reassurance that the place is alive and
    /// current, read through ``lastSyncedText(for:)``.
    @Published public private(set) var lastSyncedByNamespace: [String: Date] = [:]

    /// Records that this community synced at `date`, so the card and the community
    /// can show "Synced just now".
    public func recordSynced(namespaceID: String, at date: Date = Date()) {
        lastSyncedByNamespace[namespaceID.lowercased()] = date
    }

    /// "Synced just now" / "Synced 5m ago" / "Synced 2h ago", or `nil` if this
    /// community has no recorded sync yet. Reassurance you're up to date with
    /// these people — not a transport receipt.
    public func lastSyncedText(for namespaceID: String) -> String? {
        guard let date = lastSyncedByNamespace[namespaceID.lowercased()] else { return nil }
        let seconds = max(0, Date().timeIntervalSince(date))
        if seconds < 45 { return "Synced just now" }
        if seconds < 3600 { return "Synced \(Int(seconds / 60))m ago" }
        if seconds < 86_400 { return "Synced \(Int(seconds / 3600))h ago" }
        return "Synced \(Int(seconds / 86_400))d ago"
    }

    /// Dismisses the relay pull result banner (the data stays; this only clears
    /// the card's success state).
    public func dismissRelaySyncResult() {
        relaySyncResult = nil
        relaySyncError = nil
    }

    /// Pulls the built-in community from the anchor relay INTO THE DURABLE profile
    /// and turns the result into a place the person can walk into.
    ///
    /// The whole point of routing this through the app model (rather than the old
    /// throwaway `AnchorRelaySyncModel`) is that the pull now lands in the
    /// persisted store: the community becomes real, appears in "Your communities",
    /// and survives a relaunch. After the network leg it discovers what actually
    /// arrived, adopts the richest community it found (a full newswire wire when
    /// there is one, otherwise an alert board), records "synced just now", and
    /// publishes a human result the card leads with. Nothing here weakens the
    /// pull's security — the repository call verifies every entry through the
    /// canonical gate exactly as before.
    @MainActor
    public func syncFromRelay() async {
        guard let repository, !isRelaySyncing else { return }
        isRelaySyncing = true
        relaySyncError = nil
        defer { isRelaySyncing = false }

        let nodeId = AnchorRelayDefaults.relayNodeId
        let ticket = AnchorRelayDefaults.communityTicket
        let now = UInt64(Date().timeIntervalSince1970)
        // The DURABLE profile handle (Sendable), so the blocking network leg runs
        // off the main actor while importing into the PERSISTED store.
        let durableProfile = repository.durableProfile

        // The network + verify + durable import leg, off the main actor.
        let outcome: AnchorSyncOutcome
        do {
            outcome = try await Task.detached {
                let net = try bindNetRuntime()
                return try net.syncWithAnchor(
                    profile: durableProfile,
                    anchorHint: nodeId,
                    ticketBytes: ticket,
                    nowUnix: now
                )
            }.value
        } catch {
            relaySyncError = Self.relaySyncFailure
            return
        }

        let imported = outcome.namespaces.reduce(0) { $0 + Int($1.imported) }
        RelaySyncLog.pullLanded(root: outcome.root, imported: imported, namespaces: outcome.namespaces)

        // What actually arrived, richest-first: a full newswire community (a wire
        // + people project) beats an alert-only namespace; among equals, more
        // content wins.
        let pulledNamespaces = outcome.namespaces.map(\.namespaceId)
        let candidates: [SyncedCommunityCandidate]
        do {
            candidates = try repository.discoverSyncedCommunities(
                pulledNamespaceIDs: pulledNamespaces)
        } catch {
            RelaySyncLog.discoverFailed(error: error)
            candidates = []
        }
        RelaySyncLog.discovered(candidates)
        let best = candidates.max { lhs, rhs in
            let l = (lhs.descriptorEntryId != nil ? 1 : 0, Int(lhs.postCount + lhs.alertCount))
            let r = (rhs.descriptorEntryId != nil ? 1 : 0, Int(rhs.postCount + rhs.alertCount))
            return l < r
        }

        let syncedAt = Date()
        guard let best else {
            // The data crossed the wire and persists, but nothing here is a
            // walk-into-able community yet. Say so honestly — no fake door.
            relaySyncResult = RelaySyncResult(
                communityName: AnchorRelayDefaults.communityDisplayName,
                namespaceID: nil,
                peopleCount: 0,
                postCount: imported,
                syncedAt: syncedAt,
                isWalkInReady: false
            )
            return
        }

        let name = best.name ?? AnchorRelayDefaults.communityDisplayName
        let people = Int(best.contributorCount)
        let posts = Int(max(best.postCount, best.alertCount))

        // Adopt it so it appears in "Your communities" and is navigable. If it is
        // already held (a re-sync), don't duplicate the row.
        let held = (try? repository.listCommunities())?.map(\.namespaceId) ?? []
        let alreadyHeld = held.contains {
            $0.caseInsensitiveCompare(best.namespaceId) == .orderedSame
        }
        do {
            if alreadyHeld {
                _ = try repository.switchToCommunity(namespaceID: best.namespaceId)
            } else if let descriptor = best.descriptorEntryId {
                _ = try repository.joinAdditionalCommunity(
                    RiotSpace(namespaceID: best.namespaceId, title: name),
                    descriptorEntryID: descriptor)
            } else {
                _ = try repository.adoptSyncedNamespace(
                    RiotSpace(namespaceID: best.namespaceId, title: name))
            }
            recordSynced(namespaceID: best.namespaceId, at: syncedAt)
            reload()
            errorMessage = nil
            relaySyncResult = RelaySyncResult(
                communityName: name,
                namespaceID: best.namespaceId,
                peopleCount: people,
                postCount: posts,
                syncedAt: syncedAt,
                isWalkInReady: true
            )
        } catch {
            // Adoption failed but the verified entries are already durable. Report
            // the connection + what arrived, without a door that won't open.
            RelaySyncLog.adoptFailed(namespace: best.namespaceId, error: error)
            relaySyncResult = RelaySyncResult(
                communityName: name,
                namespaceID: nil,
                peopleCount: people,
                postCount: posts,
                syncedAt: syncedAt,
                isWalkInReady: false
            )
        }
    }

    /// Walks the person into a relay-pulled community: selects it and lands on its
    /// Home, closing the loop from the relay card with no dead end. Also records a
    /// fresh sync stamp so the community reads "Synced just now" as you arrive.
    public func openSyncedCommunity(namespaceID: String) {
        recordSynced(namespaceID: namespaceID)
        switchCommunity(namespaceID: namespaceID)
        select(.home)
    }

    private static let relaySyncFailure =
        "Riot couldn’t reach the relay just now. It may be offline, or this device may have no internet; nothing on your device changed."

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

    private static let followSiteRefusal =
        "Riot couldn’t follow from that ticket. It may be expired, incomplete, or not a Riot site ticket — check you pasted the whole thing and try again."

    private static let followSiteFetchRefusal =
        "Riot couldn’t refresh from that site right now. The mirror may be unreachable or the bundle didn’t verify; nothing on this device changed."

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
    @discardableResult
    public func setDisplayName(_ name: String) -> Bool {
        guard let repository else {
            nameError = Self.nameRefusal
            return false
        }
        do {
            try repository.setDisplayName(name)
            nameError = nil
            // The claim changed how this person renders, and they are in their own
            // name map — re-read both, so the field they just typed into echoes
            // back the `Ana · a3f91122` their neighbour is about to see.
            me = try repository.me()
            claimedName = repository.claimedName
            refreshDisplayNames()
            return true
        } catch {
            nameError = Self.nameRefusal
            // The name on screen is still the one core holds — this attempt
            // changed nothing — so leave `me` alone rather than clearing it.
            return false
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
        communityTransitionGate.prepare(.preserveDraft)
        guard let repository else {
            communityTransitionGate.recoverAfterFailedPreparation()
            return
        }
        perform({
            space = try repository.createPublicSpace(title: title)
            refreshApps()
            // Creating a space is what makes you its organizer.
            refreshOrganizerState()
            persistCommunitiesQuietly()
            refreshCommunities()
            destination = .home
        }, onFailure: communityTransitionGate.recoverAfterFailedPreparation)
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
        communityTransitionGate.prepare(.preserveDraft)
        communityUnavailable = nil
    }

    /// Leaves the selected community, returning the shell to the no-community
    /// launch state. In Slices 0–2 this is a session-scoped view change (the
    /// signed records stay in the store); Unit 3 replaces it with real
    /// multi-community removal. Callers gate this on the dirty-draft
    /// Stay-or-Discard confirmation (``CommunityChangeGuard``) so unsaved work is
    /// never lost silently.
    public func leaveCommunity() {
        communityTransitionGate.prepare(.discardDraft)
        space = nil
        newswireDescriptorEntryID = nil
        communityUnavailable = nil
        destination = .home
    }

    public func createCommunity(_ request: CommunityCreationRequest) {
        communityTransitionGate.prepare(.preserveDraft)
        guard let repository else {
            communityTransitionGate.recoverAfterFailedPreparation()
            return
        }
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
        perform({
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
        }, onFailure: communityTransitionGate.recoverAfterFailedPreparation)
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

    /// Adds a tool the organizer chose from a file, then refreshes Tools so the
    /// new tool shows with its "Review" action. Installing turns nothing on — the
    /// tool is UNTRUSTED until the organizer approves it in `AppReviewSheet`; this
    /// method never trusts. A rejected file surfaces a plain message rather than
    /// the silent no-op that let a failed install "just not appear".
    public func installTool(manifest: Data, bundle: Data) {
        guard let repository else { return }
        do {
            _ = try repository.installApp(manifest: manifest, bundle: bundle)
            errorMessage = nil
            refreshApps()
        } catch {
            errorMessage = Self.toolImportFailureMessage(error)
        }
    }

    /// Why a chosen file could not be added as a tool, in words a person can act
    /// on. Deliberately not `approvalFailureMessage` — that copy is about the
    /// organizer trust gate, not a malformed file.
    static func toolImportFailureMessage(_ error: Error) -> String {
        "That file couldn’t be added as a tool. Choose the tool’s manifest, then its bundle."
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

    private func perform(
        _ operation: () throws -> Void,
        onFailure: (() -> Void)? = nil
    ) {
        do {
            try operation()
            errorMessage = nil
        } catch {
            onFailure?()
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

extension RiotAppModel: PublishingContextProviding {
    public func currentPublishingContext() -> PublishingContext? {
        guard let community, let me else { return nil }
        return PublishingContext(
            communityID: community.id,
            identity: .persistent(me),
            community: PostingCommunity(
                name: community.name,
                spaceDescriptorEntryID: community.newswireDescriptorEntryID ?? ""
            )
        )
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
