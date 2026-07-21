import Foundation

public enum NearbyStrings {
    public static let devicesTitle = "Nearby devices"
    public static let syncedPeopleTitle = "People you’ve synced with"
    public static let stopLabel = "Stop"
    public static let deviceSummary =
        "Discovery runs automatically nearby over Bluetooth or your local network."

    public static func addUpdates(_ count: Int) -> String {
        "Add \(count) update\(count == 1 ? "" : "s")"
    }
}

// MARK: - Community context

/// The selected community, as the shell reads it. Views bind to this value —
/// never to a singleton space — so Unit 3 can swap the selection source (a
/// `RiotDatabase`-backed multi-community registry) without touching a single
/// route view. Slices 0–2 support exactly one selected public communal
/// community; this type is already shaped for the multi-community future.
public struct CommunityContext: Equatable, Sendable, Identifiable {
    /// The person-facing community name — what the header and chooser show.
    public let name: String
    /// The community's Willow namespace id (the app-trust / apps / nearby key).
    public let namespaceID: String
    /// The signed newswire `SpaceDescriptorV1` entry id, when this device knows
    /// it. It is returned by `createNewswireSpace` and captured on create; a
    /// loaded or joined community whose descriptor id has not been surfaced
    /// through FFI carries `nil`, and Home then shows the honest no-updates
    /// recovery state rather than inventing content. (There is no descriptor
    /// discovery accessor in the MVP FFI — that arrives with Unit 3's registry.)
    public let newswireDescriptorEntryID: String?
    /// True only when core says this profile is the recognized organizer of this
    /// community — the namespace coordinate, never a self-claim. Drives whether
    /// organizer-only affordances (Add a tool, Community settings governance)
    /// appear at all.
    public let isOrganizer: Bool

    /// Communities are addressed by their namespace; a re-render keyed on this
    /// stays stable across a name edit.
    public var id: String { namespaceID }

    public init(
        name: String,
        namespaceID: String,
        newswireDescriptorEntryID: String?,
        isOrganizer: Bool
    ) {
        self.name = name
        self.namespaceID = namespaceID
        self.newswireDescriptorEntryID = newswireDescriptorEntryID
        self.isOrganizer = isOrganizer
    }
}

// MARK: - Launch state

/// What the shell shows before any route is drawn (nav design "Truthful product
/// transition" + §4.7 recovery). Exactly one of these is true at launch, and
/// none of them is ever a blank screen.
public enum ShellLaunchState: Equatable, Sendable {
    /// The profile/store is still opening. Accessible progress and a bounded
    /// wait, never a fake empty state (§4.7 profile/store loading).
    case loading
    /// No retained community. Offer Create a community / Find one nearby, with
    /// the display name inline and skippable (nav design first-use).
    case noCommunity
    /// One retained community — open its Home directly.
    case community(CommunityContext)
    /// A retained community that cannot open — recovery in place (Retry / Find
    /// nearby / Remove after confirmation), never a blank space (§4.7).
    case unavailable(CommunityUnavailable)
}

/// The record of a community that could not open, preserved so recovery has
/// something to act on (nav design: "preserves its record with recovery
/// actions").
public struct CommunityUnavailable: Equatable, Sendable {
    /// The remembered community name, so recovery names what it is recovering.
    public let name: String
    /// A stable, user-reportable code for the technical-details disclosure —
    /// never a raw internal error string.
    public let code: String

    public static let unavailableCode = "RIOT-COMMUNITY-UNAVAILABLE"

    public init(name: String, code: String = CommunityUnavailable.unavailableCode) {
        self.name = name
        self.code = code
    }
}

/// The selection seam Unit 3 replaces with a multi-community registry. Kept
/// deliberately narrow — the shell reads a launch state and nothing else — so
/// the swap is invisible to every route view. `@MainActor` because the live
/// conformer (`RiotAppModel`) is, and the shell reads it on the main actor.
@MainActor
public protocol CommunitySelecting {
    var launchState: ShellLaunchState { get }
}

// MARK: - First-run onboarding

/// The first-run guided path, as a pure decision the shell reads off real state.
/// A brand-new person opens Riot with no identity and no community; the shell
/// already resolves that to `ShellLaunchState.noCommunity`, so onboarding does
/// not invent a second "have they onboarded?" flag — it derives from the same
/// launch state. The moment a person has a community they are in the shell, so
/// onboarding is never shown in front of it again (a person who later leaves
/// every community returns to no-community and is guided back in — the honest
/// outcome, since they still need a community to do anything).
public enum Onboarding {
    /// True only when the shell is showing the no-community launch state — the
    /// one moment the guided path belongs in front of the shell. Loading, an
    /// open community, and in-place recovery are all explicitly not first-run.
    public static func isFirstRun(_ launchState: ShellLaunchState) -> Bool {
        launchState == .noCommunity
    }
}

/// The two short screens of the first-run path (activists in the field, not a
/// wizard): a welcome that says what Riot is, then setup where a person names
/// themselves and creates or joins a community. Setup is the last step — the
/// flow does not end on a screen, it ends by landing in the shell with a real
/// community, so there is deliberately no third step and no "finish" button.
public enum OnboardingStep: Int, CaseIterable, Equatable, Sendable {
    /// What Riot is, in plain indymedia terms. Offers a general "Get started"
    /// path and a direct "Join with a link or QR" path; setup carries the
    /// chosen intent so the direct-join path can present the real join sheet
    /// instead of offering nearby as an onboarding exit.
    case welcome
    /// Name yourself (skippable) and create or join a community (required to
    /// leave onboarding). Reuses the display-name and create/join paths.
    case setup

    /// Where the flow begins.
    public static let first: OnboardingStep = .welcome

    /// The next step, or `nil` when this is the last one (setup completes into a
    /// real community, not another screen).
    public var next: OnboardingStep? {
        OnboardingStep(rawValue: rawValue + 1)
    }

    /// The previous step, or `nil` at the first step (nowhere back to).
    public var back: OnboardingStep? {
        rawValue == 0 ? nil : OnboardingStep(rawValue: rawValue - 1)
    }
}

/// One beat of the paired "How Riot works" story. The story is the single
/// ordered mental model shared between the app's first-run explainer and the
/// marketing homepage, so a person who meets Riot either way hears the same
/// trust boundaries in the same order.
public struct OnboardingExplainerPoint: Equatable, Sendable {
    public let title: String
    public let body: String

    init(title: String, body: String) {
        self.title = title
        self.body = body
    }
}

/// The canonical five-beat story. Order and exact phrasing are pinned by
/// `ShellNavigationTests.testExplainerStoryPinsOrderedTrustBoundaries` and by
/// the cross-surface marketing contract, because each beat deliberately
/// separates what the app verifies from what a browser mirror could lie about.
/// Do not reorder or rephrase without updating both of those checks.
public enum OnboardingExplainerStory {
    public static let points: [OnboardingExplainerPoint] = [
        OnboardingExplainerPoint(
            title: "No central account or publishing server",
            body: "Your identity is a cryptographic key, not a service login. Volunteer seeds, anchors, and mirrors can run on servers, but none owns your identity or is the single place Riot must publish."
        ),
        OnboardingExplainerPoint(
            title: "Publishing moves peer to peer",
            body: "Signed posts move between phones and volunteer seeds. Peer-to-peer does not mean anonymous: devices and infrastructure may observe connections."
        ),
        OnboardingExplainerPoint(
            title: "Many mirrors, not one site",
            body: "Websites are replaceable views, not the authority. A mirror can display altered text or false attribution, but it cannot produce an independently synced signed record Riot accepts as the claimed author."
        ),
        OnboardingExplainerPoint(
            title: "Signed records, checked in the app",
            body: "Riot checks the signature and authorization of the independently synced record. That establishes who signed an unchanged admitted record—not whether its claims are true, current, complete, safe, or endorsed."
        ),
        OnboardingExplainerPoint(
            title: "Web for reach; the app for provenance",
            body: "Use the web to reach readers. When provenance matters, read the independently synced record in Riot instead of trusting what a mirror displayed."
        ),
    ]
}

/// Which welcome path a person chose, carried into setup so the direct-join
/// path can present the real join sheet immediately rather than treating
/// nearby as an onboarding exit. `general` is the plain "Get started" flow.
public enum OnboardingSetupIntent: Equatable, Sendable {
    case general
    case join
}

// MARK: - Deterministic Home shortcuts

/// The four Home tool shortcuts. Deterministic by construction: walk the
/// canonical catalog order (the order `spaceApps()` returns, which Rust owns and
/// 0A froze) and take the first four APPROVED tools, continuing past unapproved
/// ones rather than leaving a mysterious hole (nav design "Home shortcuts are
/// deterministic"). Organizer pinning and local recency are deferred — a stable
/// order beats an unexplained one.
public enum HomeShortcuts {
    /// The default number of shortcuts Home shows.
    public static let count = 4

    /// The first `limit` approved (trusted) apps in the given canonical order.
    /// An unapproved app is skipped, not left as a gap, so the result never has
    /// a hole. Fewer than `limit` approved apps yields a shorter list, never a
    /// padded one.
    public static func deterministic(
        from apps: [RiotSpaceApp],
        limit: Int = HomeShortcuts.count
    ) -> [RiotSpaceApp] {
        Array(apps.filter { $0.trusted }.prefix(limit))
    }
}

// MARK: - Profile / community settings relocation

/// The two distinct, labeled identity paths the header exposes (nav design: the
/// avatar opens **Your profile**; a separate gear opens **Community settings**).
/// They are deliberately different actions with different labels and different
/// triggers — never one ambiguous combined menu — so a person can never mistake
/// editing their own identity for changing the community, on iPhone or in the
/// macOS sidebar footer.
public enum ShellIdentityDestination: String, CaseIterable, Sendable, Identifiable {
    case yourProfile
    case communitySettings

    public var id: String { rawValue }

    /// The label shown on the control and read by VoiceOver.
    public var label: String {
        switch self {
        case .yourProfile: "Your profile"
        case .communitySettings: "Community settings"
        }
    }

    /// The control that opens it: the avatar for the profile, a gear for the
    /// community. Distinct triggers, so the two paths never collapse.
    public var systemImage: String {
        switch self {
        case .yourProfile: "person.crop.circle"
        case .communitySettings: "gearshape"
        }
    }

    /// The accessibility identifier the shell stamps on the control and the
    /// tests assert against.
    public var accessibilityID: String {
        switch self {
        case .yourProfile: "your-profile"
        case .communitySettings: "community-settings"
        }
    }
}

// MARK: - Keyboard: Command-1…4 and Escape

/// What the Escape key does from inside a running tool (nav design: "Escape
/// returns from a tool when it is safe"). Escape is only ever a return when a
/// tool is actually open, and only when there is no unsaved work; a dirty draft
/// routes through the Stay-or-Discard confirmation instead of discarding
/// silently.
public enum ShellEscapeAction: Equatable, Sendable {
    /// No tool is open — Escape does nothing shell-level.
    case ignore
    /// Safe to leave the tool: close it and restore focus to the invoking card.
    case returnFromTool
    /// The tool (or a post draft) has unsaved work — confirm before leaving.
    case confirmDiscard

    /// The action for the current shell state.
    public static func action(isToolOpen: Bool, hasUnsavedWork: Bool) -> ShellEscapeAction {
        guard isToolOpen else { return .ignore }
        return hasUnsavedWork ? .confirmDiscard : .returnFromTool
    }
}

/// Mounted tools are pushed inside the iPhone Tools stack, but replace the
/// selected split-detail route on macOS. Only the latter must be torn down when
/// a sidebar/keyboard route changes.
public enum ToolRoutePolicy {
    public static var closesMountedToolBeforeRoute: Bool {
        #if os(macOS)
        true
        #else
        false
        #endif
    }
}

// MARK: - Focus restoration

/// Remembers which tool card launched the running tool so focus returns to it
/// when the tool closes (nav design + §4.6: "focus returns to the invoking tool
/// card"). A value, so it is testable without a live focus system.
public struct ToolFocusRestoration: Equatable, Sendable {
    /// The id of the card that opened the current tool, or `nil` when no tool is
    /// open.
    public private(set) var invokingToolID: String?

    public init() {}

    /// Record that `toolID` opened a tool — the card focus must return to.
    public mutating func open(toolID: String) {
        invokingToolID = toolID
    }

    /// Close the tool and hand back the id focus should return to (or `nil` if
    /// nothing was open).
    @discardableResult
    public mutating func close() -> String? {
        defer { invokingToolID = nil }
        return invokingToolID
    }
}

// MARK: - Dirty-draft guard before a community change

/// The choice a person is given when they try to change communities (or leave a
/// tool) with unsaved work (nav design + §4.6: "switching communities with a
/// non-empty draft requires choose Stay or Discard draft"). A pure decision, so
/// the guard is provable without any UI.
public struct StayOrDiscardPrompt: Equatable, Sendable {
    public static let title = "You have an unsaved draft"
    public static let stayLabel = "Stay"
    public static let discardLabel = "Discard draft"
}

/// Whether a pending community change may proceed, or must first confirm.
public enum CommunityChangeDecision: Equatable, Sendable {
    /// No unsaved work — the change proceeds immediately.
    case proceed
    /// Unsaved work — present Stay or Discard draft before changing anything.
    case confirm(StayOrDiscardPrompt)
}

/// Guards a community change on unsaved draft state. A non-empty tool or post
/// draft must never be silently lost by a switch.
public enum CommunityChangeGuard {
    public static func decision(hasUnsavedDraft: Bool) -> CommunityChangeDecision {
        hasUnsavedDraft ? .confirm(StayOrDiscardPrompt()) : .proceed
    }
}

// MARK: - Recovery states (§4.7)

/// The community-first navigation design's recovery-state contract (§4.7 / nav
/// design lines 362–375), as a closed set of states this shell implements. Every
/// state has a useful primary action and omits unavailable role actions — and
/// none is ever a blank screen or a raw internal error.
///
/// The subset owned by this unit (2A): profile/store loading, no updates, no
/// tools (role-explained), no community, and community unavailable. The others
/// belong to the units named in the plan (catalog → 0A, sync/permissions → 2B,
/// stale session → 0C, post failure → 1B).
public enum ShellRecoveryState: Equatable, Sendable {
    /// Profile/store still opening — accessible progress, bounded wait, Retry.
    case profileStoreLoading
    /// A community with no updates yet — Post the first update / Find nearby.
    case noUpdates
    /// A community with no tools — the role is explained, never a dead button:
    /// an organizer sees "Add a tool"; a member sees "Find nearby".
    case noTools(isOrganizer: Bool)
    /// No retained community — Create a community / Find one nearby.
    case noCommunity
    /// A retained community that cannot open — Retry / Find nearby / Remove
    /// after confirmation, never blank.
    case communityUnavailable(CommunityUnavailable)

    /// The primary action label — always present, always actionable.
    public var primaryActionLabel: String {
        switch self {
        case .profileStoreLoading: "Retry"
        case .noUpdates: "Post the first update"
        case let .noTools(isOrganizer): isOrganizer ? "Add a tool" : "Find nearby"
        case .noCommunity: "Create a community"
        case .communityUnavailable: "Retry"
        }
    }

    /// The secondary action label, when the state offers one.
    public var secondaryActionLabel: String? {
        switch self {
        case .profileStoreLoading: nil
        case .noUpdates: "Find nearby"
        case .noTools: nil
        case .noCommunity: "Find one nearby"
        case .communityUnavailable: "Find nearby"
        }
    }

    /// The plain-language explanation shown above the actions — never a raw
    /// internal error.
    public var message: String {
        switch self {
        case .profileStoreLoading:
            return "Opening your profile…"
        case .noUpdates:
            return "No updates have arrived yet."
        case let .noTools(isOrganizer):
            return isOrganizer
                ? "No tools are available here yet. Add one so everyone can use it."
                : "No tools are available here yet. Find people nearby to bring some over."
        case .noCommunity:
            return "You're not in a community yet."
        case let .communityUnavailable(unavailable):
            return "\(unavailable.name) couldn't be opened."
        }
    }

    /// The stable accessibility identifier for the recovery view, so the RED
    /// tests and VoiceOver both have a stable handle.
    public var accessibilityID: String {
        switch self {
        case .profileStoreLoading: "recovery-loading"
        case .noUpdates: "recovery-no-updates"
        case .noTools: "recovery-no-tools"
        case .noCommunity: "recovery-no-community"
        case .communityUnavailable: "recovery-community-unavailable"
        }
    }
}

// MARK: - Create a community

/// The founding collective's initial choices when creating a community (newswire
/// Data Flows step 2): the name, and the initial editorial public keys and
/// approved starter apps it chooses. The `editorialRoster` is threaded straight
/// into `createNewswireSpace` — an EMPTY roster keeps core's single-editor
/// default, so a real founding selection is what stops every user-created
/// community from being permanently single-editor. Roster *rotation* stays
/// deferred; initial *selection* does not.
public struct CommunityCreationRequest: Equatable, Sendable {
    public let name: String
    public let summary: String
    /// Hex-encoded 32-byte editorial subspace ids the founding collective picks.
    /// Empty means "just me" (core's default founding editor).
    public let editorialRoster: [String]
    /// The starter app ids the founding collective approves at creation.
    public let approvedStarterAppIDs: [String]

    public init(
        name: String,
        summary: String = "",
        editorialRoster: [String] = [],
        approvedStarterAppIDs: [String] = []
    ) {
        self.name = name
        self.summary = summary
        self.editorialRoster = editorialRoster
        self.approvedStarterAppIDs = approvedStarterAppIDs
    }

    /// Whether the name is postable (a community needs a name; the display name
    /// offered alongside it is skippable, the community name is not).
    public var hasName: Bool {
        !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
}

/// The signed-descriptor seam. `RiotProfileRepository` conforms (its Newswire
/// extension); tests inject a stub so create-community is provable without a
/// live store, exactly as 1B/1C do for posting and contributors.
public protocol NewswireSpaceCreating {
    @discardableResult
    func createNewswireCommunity(
        name: String,
        summary: String,
        editorialRoster: [String]
    ) throws -> NewswireSignedRecord
}

/// The app-trust backing space seam — the public space that carries the
/// community's apps, nearby coordinator, and (today) its incident content. Kept
/// separate from the descriptor seam so the two-step create is testable in
/// isolation.
public protocol CommunityBackingSpaceCreating {
    @discardableResult
    func createBackingSpace(name: String) throws -> RiotSpace
}

/// Performs the two-step community create and returns the resulting context.
/// Step 1 establishes the app-trust backing space (apps/tools/nearby machinery,
/// as today). Step 2 signs the immutable `SpaceDescriptorV1` via
/// `createNewswireSpace`, carrying the founding editorial roster, and captures
/// its entry id so Home and People can project the community's newswire. The
/// creator is the founding organizer + founding editor by construction
/// (`create_newswire_space` signs under the profile's own namespace).
public struct CommunityCreationCoordinator {
    private let backing: CommunityBackingSpaceCreating
    private let descriptor: NewswireSpaceCreating

    public init(backing: CommunityBackingSpaceCreating, descriptor: NewswireSpaceCreating) {
        self.backing = backing
        self.descriptor = descriptor
    }

    public func create(_ request: CommunityCreationRequest) throws -> CommunityContext {
        let space = try backing.createBackingSpace(name: request.name)
        let record = try descriptor.createNewswireCommunity(
            name: request.name,
            summary: request.summary,
            editorialRoster: request.editorialRoster
        )
        return CommunityContext(
            name: space.title,
            namespaceID: space.namespaceID,
            newswireDescriptorEntryID: record.entryId,
            // The founder signs the descriptor under their own namespace, so they
            // are the recognized organizer of the community they just created.
            isOrganizer: true
        )
    }
}

// MARK: - Home newswire projection seam

/// The one call Home needs for community updates: the newswire projection for a
/// descriptor. `RiotProfileRepository` conforms (its Newswire extension); tests
/// inject a stub. Home degrades to the no-updates recovery state when a
/// community has no known descriptor id or an empty wire.
public protocol NewswireProjecting {
    func projectNewswire(spaceDescriptorEntryID: String) throws -> NewswireProjectionView
}
