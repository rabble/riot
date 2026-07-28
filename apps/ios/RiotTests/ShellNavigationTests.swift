import XCTest
@testable import RiotKit

/// The community-first shell (Unit 2A): four routes — Home, Tools, People,
/// Nearby — with deterministic Home shortcuts, a relocated profile/settings
/// split, keyboard navigation, focus restoration, a dirty-draft guard, launch +
/// recovery states, and a create-community flow that signs a `SpaceDescriptorV1`
/// with a founding editorial roster. The old five debug-shaped surfaces
/// (Spaces/Apps/Board/Post/Connect) are gone.
final class ShellNavigationTests: XCTestCase {
    func testEveryComposerOriginUsesOneOpenState() {
        var state = ComposerPresentationState.closed
        for origin in ComposerOrigin.allCases {
            state.open(origin)
            XCTAssertEqual(state, .open(origin))
            state.close()
            XCTAssertEqual(state, .closed)
        }
    }

    // MARK: - The four community routes

    func testTheShellExposesExactlyTheFourCommunityRoutes() {
        XCTAssertEqual(
            RiotDestination.phoneTabs,
            [.home, .tools, .people, .nearby],
            "the community shell has exactly Home, Tools, People, Nearby — in canonical order"
        )
        XCTAssertEqual(
            RiotDestination.phoneTabs.map(\.title),
            ["Home", "Tools", "People", "Nearby"]
        )
        XCTAssertEqual(
            RiotDestination.phoneTabs.map(\.tabTitle),
            ["Home", "Tools", "People", "Nearby"]
        )
        // Each route has its own distinct icon (selection is never by color alone,
        // §4.6 — icon + label + shape all differ).
        let images = RiotDestination.phoneTabs.map(\.systemImage)
        XCTAssertEqual(Set(images).count, images.count, "each route has a distinct icon")
    }

    // MARK: - Command-1…4 and Escape

    func testCommandNumbersSelectTheFourDestinationsInCanonicalOrder() {
        XCTAssertEqual(RiotDestination.home.commandNumber, 1)
        XCTAssertEqual(RiotDestination.tools.commandNumber, 2)
        XCTAssertEqual(RiotDestination.people.commandNumber, 3)
        XCTAssertEqual(RiotDestination.nearby.commandNumber, 4)

        XCTAssertEqual(RiotDestination.forCommandNumber(1), .home)
        XCTAssertEqual(RiotDestination.forCommandNumber(2), .tools)
        XCTAssertEqual(RiotDestination.forCommandNumber(3), .people)
        XCTAssertEqual(RiotDestination.forCommandNumber(4), .nearby)
        // Out of range selects nothing rather than wrapping.
        XCTAssertNil(RiotDestination.forCommandNumber(0))
        XCTAssertNil(RiotDestination.forCommandNumber(5))
    }

    func testEscapeReturnsFromAToolOnlyWhenSafe() {
        // No tool open: Escape is not the shell's to handle.
        XCTAssertEqual(ShellEscapeAction.action(isToolOpen: false, hasUnsavedWork: false), .ignore)
        XCTAssertEqual(ShellEscapeAction.action(isToolOpen: false, hasUnsavedWork: true), .ignore)
        // A tool open with no unsaved work: Escape returns.
        XCTAssertEqual(ShellEscapeAction.action(isToolOpen: true, hasUnsavedWork: false), .returnFromTool)
        // A tool open with unsaved work: Escape must confirm, never discard silently.
        XCTAssertEqual(ShellEscapeAction.action(isToolOpen: true, hasUnsavedWork: true), .confirmDiscard)
    }

    // MARK: - Home hierarchy inside an active community

    func testHomeNamesThePlaceAndNeverReprintsTheCommunityName() {
        // The persistent community header already carries the community name with
        // the chooser chevron. Home is INSIDE that header, so repeating the name
        // spends the screen's best row saying what the row above it just said.
        for name in ["River City Wire", "Home", "", "Riot"] {
            XCTAssertNotEqual(
                HomeHeaderTitle.title(forCommunityNamed: name),
                name,
                "Home names the place within the community, not the community"
            )
        }
        XCTAssertEqual(HomeHeaderTitle.title(forCommunityNamed: "River City Wire"), "What's happening")
        XCTAssertEqual(HomeHeaderTitle.eyebrow, "Community")
    }

    @MainActor
    func testEveryRouteCanBecomeTheVisibleDestination() {
        let model = RiotAppModel()
        for destination in RiotDestination.phoneTabs {
            model.select(destination)
            XCTAssertEqual(model.destination, destination)
        }
    }

    // MARK: - Performance contract: selection lives off the app model

    /// The tab-lifecycle performance contract (nav design): route selection lives
    /// on its own observable object, NOT on `RiotAppModel`, so a tab tap does not
    /// re-evaluate every route body. A fresh model starts on Home.
    @MainActor
    func testRouteSelectionLivesOnItsOwnObjectAndStartsOnHome() {
        let model = RiotAppModel()
        XCTAssertEqual(model.navigation.destination, .home)
        XCTAssertEqual(model.destination, .home, "the model passes through to the navigation object")

        model.navigation.destination = .tools
        XCTAssertEqual(model.destination, .tools, "writing the navigation object is what moves the route")
    }

    // MARK: - Deterministic Home shortcuts

    private static func app(_ name: String, id: String, trusted: Bool) -> RiotSpaceApp {
        RiotSpaceApp(
            appIDHex: id,
            name: name,
            description: "",
            version: "1",
            permissions: [],
            trusted: trusted
        )
    }

    /// Walk canonical catalog order and take the first four APPROVED tools,
    /// continuing past unapproved ones rather than leaving a hole.
    func testHomeShortcutsAreTheFirstFourApprovedToolsInOrderNeverLeavingAHole() {
        // Canonical order with unapproved tools interleaved at positions 1 and 3.
        let catalog = [
            Self.app("Checklist", id: "a1", trusted: true),
            Self.app("Needs & Offers", id: "a2", trusted: false), // unapproved — skipped
            Self.app("Events", id: "a3", trusted: true),
            Self.app("Decisions", id: "a4", trusted: false), // unapproved — skipped
            Self.app("Chat", id: "a5", trusted: true),
            Self.app("Dispatches", id: "a6", trusted: true),
            Self.app("Wiki", id: "a7", trusted: true),
        ]

        let shortcuts = HomeShortcuts.deterministic(from: catalog)

        XCTAssertEqual(
            shortcuts.map(\.name),
            ["Checklist", "Events", "Chat", "Dispatches"],
            "the unapproved tools are skipped and the order is preserved — never a hole"
        )
        XCTAssertEqual(shortcuts.count, HomeShortcuts.count)
        XCTAssertTrue(shortcuts.allSatisfy(\.trusted), "a shortcut is only ever an approved tool")
    }

    func testHomeShortcutsAreShorterThanFourWhenFewerToolsAreApproved() {
        let catalog = [
            Self.app("Checklist", id: "a1", trusted: true),
            Self.app("Needs & Offers", id: "a2", trusted: false),
        ]
        let shortcuts = HomeShortcuts.deterministic(from: catalog)
        XCTAssertEqual(shortcuts.map(\.name), ["Checklist"], "never padded with an unapproved tool")
    }

    // MARK: - Profile / community settings relocation

    /// The header keeps two separate labeled paths: the avatar opens Your
    /// profile; a distinct gear opens Community settings. They are never one
    /// combined menu.
    func testProfileAndCommunitySettingsAreTwoDistinctLabeledPaths() {
        XCTAssertEqual(ShellIdentityDestination.yourProfile.label, "Your profile")
        XCTAssertEqual(ShellIdentityDestination.communitySettings.label, "Community settings")
        XCTAssertNotEqual(
            ShellIdentityDestination.yourProfile.label,
            ShellIdentityDestination.communitySettings.label
        )
        // Distinct triggers (avatar vs gear) and distinct accessibility handles.
        XCTAssertNotEqual(
            ShellIdentityDestination.yourProfile.systemImage,
            ShellIdentityDestination.communitySettings.systemImage
        )
        XCTAssertEqual(ShellIdentityDestination.yourProfile.accessibilityID, "your-profile")
        XCTAssertEqual(ShellIdentityDestination.communitySettings.accessibilityID, "community-settings")
    }

    // MARK: - Focus restoration

    /// Opening a tool records the invoking card; closing hands that id back so
    /// focus returns to it (§4.6 "focus returns to the invoking tool card").
    func testFocusReturnsToTheInvokingToolCard() {
        var focus = ToolFocusRestoration()
        XCTAssertNil(focus.invokingToolID)

        focus.open(toolID: "checklist-card")
        XCTAssertEqual(focus.invokingToolID, "checklist-card")

        XCTAssertEqual(focus.close(), "checklist-card", "closing restores focus to the opener")
        XCTAssertNil(focus.invokingToolID, "and the tool is no longer open")
        XCTAssertNil(focus.close(), "closing again restores nothing")
    }

    // MARK: - Dirty-draft guard before a community change

    func testChangingCommunityWithAnUnsavedDraftMustConfirmStayOrDiscard() {
        XCTAssertEqual(CommunityChangeGuard.decision(hasUnsavedDraft: false), .proceed)
        XCTAssertEqual(
            CommunityChangeGuard.decision(hasUnsavedDraft: true),
            .confirm(StayOrDiscardPrompt())
        )
        XCTAssertEqual(StayOrDiscardPrompt.stayLabel, "Stay")
        XCTAssertEqual(StayOrDiscardPrompt.discardLabel, "Discard draft")
    }

    // MARK: - Launch states

    @MainActor
    func testLaunchStartsLoadingUntilTheProfileIsOpen() {
        let model = RiotAppModel()
        XCTAssertEqual(model.launchState, .loading, "no fake empty state before the profile opens")
    }

    @MainActor
    func testAnOpenProfileWithNoCommunityLaunchesToTheNoCommunityState() throws {
        let directory = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let model = RiotAppModel()
        model.bootstrap(storageDirectory: directory, keyStore: TestWrappingKeyStore(), starterPacks: [])

        XCTAssertTrue(model.isProfileOpen)
        XCTAssertEqual(model.launchState, .noCommunity)
        XCTAssertNil(model.community)
    }

    @MainActor
    func testOneRetainedCommunityLaunchesDirectlyToItsHome() throws {
        let directory = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let model = RiotAppModel()
        model.bootstrap(storageDirectory: directory, keyStore: TestWrappingKeyStore(), starterPacks: [])

        model.createSpace(title: "Riverside Tenants Union")

        let community = try XCTUnwrap(model.community)
        XCTAssertEqual(community.name, "Riverside Tenants Union")
        XCTAssertEqual(model.launchState, .community(community))
        XCTAssertEqual(model.destination, .home, "a community opens to its Home")
    }

    @MainActor
    func testAnUnavailableCommunityRecoversInPlaceAndIsNeverBlank() throws {
        let directory = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let model = RiotAppModel()
        model.bootstrap(storageDirectory: directory, keyStore: TestWrappingKeyStore(), starterPacks: [])
        model.createSpace(title: "Fire Watch")

        model.markCommunityUnavailable(CommunityUnavailable(name: "Fire Watch"))
        XCTAssertEqual(
            model.launchState,
            .unavailable(CommunityUnavailable(name: "Fire Watch"))
        )

        // Retry clears the unavailable state and returns to the community in place.
        model.retryCommunity()
        XCTAssertNotNil(model.community)
        if case .unavailable = model.launchState {
            XCTFail("Retry must leave the community-unavailable state")
        }
    }

    // MARK: - First-run onboarding gate + steps

    /// Onboarding is the first-run guided path. It is shown ONLY when the profile
    /// is open and there is no community yet — the same `.noCommunity` launch
    /// state the shell already derives from real state. It is never shown while
    /// the profile is loading, while a community is open, or during recovery — so
    /// once a person has a community, onboarding never reappears in front of the
    /// shell.
    func testOnboardingIsFirstRunOnlyForTheNoCommunityLaunchState() {
        XCTAssertTrue(Onboarding.isFirstRun(.noCommunity))

        let community = CommunityContext(
            name: "Riverside Tenants Union",
            namespaceID: "ns-riverside",
            newswireDescriptorEntryID: nil,
            isOrganizer: true
        )
        XCTAssertFalse(Onboarding.isFirstRun(.community(community)))
        XCTAssertFalse(Onboarding.isFirstRun(.loading))
        XCTAssertFalse(
            Onboarding.isFirstRun(.unavailable(CommunityUnavailable(name: "Fire Watch")))
        )
    }

    /// The guided path is two SHORT screens (activists in the field, not a
    /// wizard): a welcome that says what Riot is, then setup where you name
    /// yourself and create or join a community. Setup is the last step — the flow
    /// ends by landing in the shell (a real community), never on a further screen.
    func testOnboardingStepsAreWelcomeThenSetup() {
        XCTAssertEqual(OnboardingStep.first, .welcome)
        XCTAssertEqual(OnboardingStep.welcome.next, .setup)
        XCTAssertNil(OnboardingStep.setup.next, "setup is the last step; completion is a real community")
        XCTAssertEqual(OnboardingStep.setup.back, .welcome)
        XCTAssertNil(OnboardingStep.welcome.back, "welcome is the first step; there is nowhere back to")
    }

    /// The paired five-beat story pins Riot's trust boundaries in a single
    /// ordered list, shared between the app's first-run explainer and the
    /// marketing homepage. The order and the exact phrases matter: each beat
    /// names what is actually verified versus what a mirror could lie about, so
    /// drift here would re-introduce the unsafe "safe to read from / app is
    /// proof" claims the story was written to replace.
    func testExplainerStoryPinsOrderedTrustBoundaries() {
        XCTAssertEqual(
            OnboardingExplainerStory.points.map(\.title),
            [
                "No central account or publishing server",
                "Publishing moves peer to peer",
                "Many mirrors, not one site",
                "Signed records, checked in the app",
                "Web for reach; the app for provenance",
            ]
        )

        let copy = OnboardingExplainerStory.points.map(\.body).joined(separator: " ")
        XCTAssertTrue(copy.contains("does not mean anonymous"))
        XCTAssertTrue(copy.contains("display altered text"))
        XCTAssertTrue(copy.contains("false attribution"))
        XCTAssertTrue(copy.contains("accepts as the claimed author"))
        XCTAssertTrue(copy.contains("independently synced record"))
        XCTAssertTrue(copy.contains("not whether its claims are true"))
        XCTAssertFalse(copy.contains("safe to read from"))
        XCTAssertFalse(copy.contains("cannot alter it"))
        XCTAssertFalse(copy.contains("app is proof"))
    }

    /// The welcome screen offers two distinct paths into setup: a general
    /// "get started" and a direct "join with a link or QR". Setup must be able
    /// to tell them apart so the direct-join intent can present the real join
    /// sheet immediately, instead of offering nearby as an onboarding exit.
    func testWelcomeSetupIntentsAreDistinct() {
        XCTAssertNotEqual(OnboardingSetupIntent.general, .join)
    }

    func testSetupOrderAndUnsupportedNearbyBoundary() {
        XCTAssertEqual(OnboardingPresentation.actionOrder, [.join, .create, .demo])
        XCTAssertEqual(
            OnboardingPresentation.nearbyNote,
            "Nearby exchange is available after you enter a community."
        )
    }

    func testNonEmptyNameFailureBlocksEveryExit() {
        for exit in OnboardingExit.allCases {
            var performed: [OnboardingExit] = []
            let result = OnboardingExitGate.perform(
                exit,
                displayName: "Ana",
                saveName: { _ in false },
                proceed: { performed.append($0) }
            )

            XCTAssertEqual(result, .nameSaveFailed)
            XCTAssertEqual(performed, [])
        }
    }

    func testEmptyAndSuccessfullySavedNameCoverEveryExit() {
        for exit in OnboardingExit.allCases {
            var performed: [OnboardingExit] = []
            var saved: [String] = []

            XCTAssertEqual(
                OnboardingExitGate.perform(
                    exit,
                    displayName: "  ",
                    saveName: { _ in
                        XCTFail("an empty optional name must not be saved")
                        return false
                    },
                    proceed: { performed.append($0) }
                ),
                .proceeded
            )
            XCTAssertEqual(performed, [exit])

            performed = []
            XCTAssertEqual(
                OnboardingExitGate.perform(
                    exit,
                    displayName: "  Ana  ",
                    saveName: {
                        saved.append($0)
                        return true
                    },
                    proceed: { performed.append($0) }
                ),
                .proceeded
            )
            XCTAssertEqual(saved, ["Ana"])
            XCTAssertEqual(performed, [exit])
        }
    }

    /// The gate is derived from real state, not a separate flag: a fresh profile
    /// with no community is first-run, and creating a community ends first-run —
    /// so the shell, not onboarding, is what shows from then on.
    @MainActor
    func testCreatingACommunityEndsFirstRun() throws {
        let directory = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let model = RiotAppModel()
        model.bootstrap(storageDirectory: directory, keyStore: TestWrappingKeyStore(), starterPacks: [])

        XCTAssertTrue(Onboarding.isFirstRun(model.launchState), "a fresh profile with no community is first-run")

        model.createSpace(title: "Riverside Tenants Union")
        XCTAssertFalse(Onboarding.isFirstRun(model.launchState), "a community ends first-run; the shell shows now")
    }

    /// The offlineStale "Try again" resolver seam (Unit 7): re-deriving the active
    /// community's newswire descriptor from the registry returns the id a create
    /// just persisted, and `nil` once there is no selected community — which is what
    /// drives the wire's forward-path (rejoin / sync) state instead of a silent loop.
    @MainActor
    func testRederivedNewswireDescriptorReadsTheRegistryForTheActiveCommunity() throws {
        let directory = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let model = RiotAppModel()
        model.bootstrap(storageDirectory: directory, keyStore: TestWrappingKeyStore(), starterPacks: [])

        // No selected community yet → the resolver is honest about having nothing.
        XCTAssertNil(model.rederivedNewswireDescriptorID())

        model.createCommunity(CommunityCreationRequest(name: "Fire Watch", summary: "east side"))
        let created = try XCTUnwrap(model.newswireDescriptorEntryID,
                                    "create signs a descriptor for the active community")
        XCTAssertEqual(model.rederivedNewswireDescriptorID(), created,
                       "the on-demand re-derive reads the same registry row reload() does")

        model.leaveCommunity()
        XCTAssertNil(model.rederivedNewswireDescriptorID(),
                     "no selected community → nil → the wire's forward-path state, not a loop")
    }

    // MARK: - Create a community signs a descriptor with a founding roster

    /// Create-community signs an immutable `SpaceDescriptorV1` via
    /// `createNewswireSpace`, threading the founding collective's chosen editorial
    /// roster — an empty roster would make every user-created community
    /// permanently single-editor. The backing app-trust space is created too.
    func testCreatingACommunitySignsADescriptorCarryingTheFoundingEditorialRoster() throws {
        let backing = StubBackingSpaceCreator(namespaceID: "ns-fire")
        let descriptor = StubNewswireSpaceCreator(entryID: "descriptor-entry-1")
        let coordinator = CommunityCreationCoordinator(backing: backing, descriptor: descriptor)

        let roster = ["editor-key-a", "editor-key-b"]
        let context = try coordinator.create(
            CommunityCreationRequest(name: "Fire Watch", summary: "east side", editorialRoster: roster)
        )

        // The backing space was created…
        XCTAssertEqual(backing.createdNames, ["Fire Watch"])
        // …and the descriptor was signed with the chosen roster, not a defaulted one.
        XCTAssertEqual(descriptor.receivedRosters, [roster])
        XCTAssertEqual(descriptor.receivedNames, ["Fire Watch"])

        // The resulting context carries the descriptor id (so Home can project)
        // and marks the founder the organizer.
        XCTAssertEqual(context.name, "Fire Watch")
        XCTAssertEqual(context.namespaceID, "ns-fire")
        XCTAssertEqual(context.newswireDescriptorEntryID, "descriptor-entry-1")
        XCTAssertTrue(context.isOrganizer, "the founder signs under their own namespace, so they organize")
    }

    func testAnEmptyRosterStillThreadsThroughAsTheSingleEditorDefault() throws {
        let backing = StubBackingSpaceCreator(namespaceID: "ns")
        let descriptor = StubNewswireSpaceCreator(entryID: "d")
        let coordinator = CommunityCreationCoordinator(backing: backing, descriptor: descriptor)
        _ = try coordinator.create(CommunityCreationRequest(name: "Solo"))
        XCTAssertEqual(descriptor.receivedRosters, [[]], "an empty roster is passed through verbatim")
    }

    func testACommunityCreationRequestRequiresANameButNotADisplayName() {
        XCTAssertTrue(CommunityCreationRequest(name: "Uganda").hasName)
        XCTAssertFalse(CommunityCreationRequest(name: "   ").hasName)
        XCTAssertFalse(CommunityCreationRequest(name: "").hasName)
    }

    // MARK: - Recovery-state contract (§4.7)

    func testEveryRecoveryStateHasAUsefulPrimaryActionAndIsNeverBlank() {
        // No community.
        XCTAssertEqual(ShellRecoveryState.noCommunity.primaryActionLabel, "Create a community")
        XCTAssertEqual(ShellRecoveryState.noCommunity.secondaryActionLabel, "Find one nearby")
        // No updates.
        XCTAssertEqual(ShellRecoveryState.noUpdates.primaryActionLabel, "Post the first update")
        XCTAssertEqual(ShellRecoveryState.noUpdates.secondaryActionLabel, "Find nearby")
        // Profile/store loading — accessible progress + Retry, no fake empty.
        XCTAssertEqual(ShellRecoveryState.profileStoreLoading.primaryActionLabel, "Retry")
        // Community unavailable — Retry + Find nearby, never blank.
        let unavailable = ShellRecoveryState.communityUnavailable(CommunityUnavailable(name: "Fire Watch"))
        XCTAssertEqual(unavailable.primaryActionLabel, "Retry")
        XCTAssertEqual(unavailable.secondaryActionLabel, "Find nearby")
        XCTAssertTrue(unavailable.message.contains("Fire Watch"))

        // Every state has a non-empty message and a stable a11y id.
        for state in [
            ShellRecoveryState.profileStoreLoading,
            .noUpdates,
            .noTools(isOrganizer: true),
            .noCommunity,
            unavailable,
        ] {
            XCTAssertFalse(state.message.isEmpty)
            XCTAssertFalse(state.accessibilityID.isEmpty)
        }
    }

    /// The no-tools state explains the role and never renders a dead button: an
    /// organizer is told to Add a tool; a member is told to Find nearby.
    func testTheNoToolsStateExplainsTheRoleAndNeverShowsADeadButton() {
        XCTAssertEqual(ShellRecoveryState.noTools(isOrganizer: true).primaryActionLabel, "Add a tool")
        XCTAssertEqual(ShellRecoveryState.noTools(isOrganizer: false).primaryActionLabel, "Find nearby")
        XCTAssertNotEqual(
            ShellRecoveryState.noTools(isOrganizer: true).message,
            ShellRecoveryState.noTools(isOrganizer: false).message,
            "the role is explained differently, never a generic dead end"
        )
    }

    // MARK: - Performance: a starter tool opens under 500 ms (simulator-relative)

    /// The <500 ms starter-tool-open gate, measured with `measure(metrics:)` on
    /// the model path a tap triggers — resolving the deterministic Home shortcut
    /// and building the focus/route to open it. Recorded SIMULATOR-RELATIVE on the
    /// iPhone 17 Pro sim (OS 26.2, arm64); physical-device timing is
    /// assumed-not-proven, since this repo's harness is the simulator (§8.3).
    func testAStarterToolOpensUnderHalfASecondSimRelative() {
        let apps = (0..<8).map { Self.app("Tool \($0)", id: "id-\($0)", trusted: true) }
        measure(metrics: [XCTClockMetric()]) {
            for _ in 0..<1_000 {
                let shortcuts = HomeShortcuts.deterministic(from: apps)
                var focus = ToolFocusRestoration()
                if let first = shortcuts.first {
                    focus.open(toolID: first.id)
                    _ = focus.close()
                }
            }
        }
    }

    // MARK: - Retained behaviours that still hold

    @MainActor
    func testConnectionStartsExplicitlyOffline() {
        let model = RiotAppModel()
        XCTAssertEqual(model.connectionStatus, .offline)
        XCTAssertEqual(model.connectionDisclosure, "Not connected")
    }

    func testNearbyUsesTruthfulCompactVocabularyAndOfferedCount() {
        XCTAssertEqual(NearbyStrings.devicesTitle, "Nearby devices")
        XCTAssertEqual(NearbyStrings.syncedPeopleTitle, "People you’ve synced with")
        XCTAssertEqual(NearbyStrings.stopLabel, "Stop")
        XCTAssertEqual(NearbyStrings.addUpdates(3), "Add 3 updates")
        XCTAssertEqual(NearbyStrings.addUpdates(1), "Add 1 update")
        XCTAssertFalse(NearbyStrings.deviceSummary.localizedCaseInsensitiveContains("renderer"))
        XCTAssertFalse(NearbyStrings.syncedPeopleTitle.contains("Recently synced"))
    }

    @MainActor
    func testDismissingAnAlertClearsItsBackingError() {
        let model = RiotAppModel(testError: "InvalidInput")
        model.dismissError()
        XCTAssertNil(model.errorMessage)
    }

    // MARK: - Looking closer at an update (AlertDetail still backs Home update detail)

    private static func entry(
        headline: String = "Medic tent moved to the north gate",
        validFrom: UInt64? = 1_720_000_500,
        aiAssisted: Bool = false
    ) -> RiotEntry {
        RiotEntry(
            entryID: String(repeating: "a", count: 64),
            namespaceID: String(repeating: "b", count: 64),
            signerID: String(repeating: "c", count: 64),
            headline: headline,
            createdAt: 1_720_000_000,
            validFrom: validFrom,
            expiresAt: 1_720_003_600,
            aiAssisted: aiAssisted
        )
    }

    func testTheAlertDetailKeepsFullIdentifiersBehindTechnicalDetails() {
        let entry = Self.entry()
        let detail = AlertDetail(entry: entry)

        XCTAssertEqual(detail.headline, "Medic tent moved to the north gate")
        let onOpen = detail.summary.map(\.value).joined(separator: " ")
        for identifier in [entry.entryID, entry.namespaceID, entry.signerID] {
            XCTAssertFalse(
                onOpen.contains(identifier),
                "a full identifier must not be shown before Technical details is opened"
            )
        }
        XCTAssertEqual(detail.summary.map(\.label), ["Created", "Valid from", "Expires"])
        XCTAssertEqual(AlertDetail.technicalDisclosureTitle, "Technical details")
        XCTAssertEqual(detail.technical.map(\.label), ["Entry", "Namespace", "Signer"])
        XCTAssertEqual(
            detail.technical.map(\.value),
            [entry.entryID, entry.namespaceID, entry.signerID],
            "the ids are shown whole — a truncated id proves nothing"
        )
    }

    func testAnAlertWithNoStartTimeShowsNoValidFromRow() {
        let detail = AlertDetail(entry: Self.entry(validFrom: nil))
        XCTAssertEqual(detail.summary.map(\.label), ["Created", "Expires"])
    }

    func testTheAIAssistanceFlagReachesTheDetail() {
        XCTAssertFalse(AlertDetail(entry: Self.entry()).aiAssisted)
        XCTAssertTrue(AlertDetail(entry: Self.entry(aiAssisted: true)).aiAssisted)
    }

    // MARK: - Discovery readiness (unchanged: a phone must not advertise a nil space)

    @MainActor
    func testAPhoneIsNotReadyToAdvertiseUntilItsProfileIsOpen() throws {
        let directory = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let model = RiotAppModel()

        XCTAssertFalse(model.isProfileOpen)
        XCTAssertNil(model.nearbySpaceHost)

        model.bootstrap(storageDirectory: directory, keyStore: TestWrappingKeyStore(), starterPacks: [])

        XCTAssertTrue(model.isProfileOpen)
        XCTAssertNotNil(model.nearbySpaceHost)
    }

    @MainActor
    func testASpaceCreatedAfterLookingBeganIsVisibleToTheHostDiscoveryIsHolding() throws {
        let directory = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let model = RiotAppModel()
        model.bootstrap(storageDirectory: directory, keyStore: TestWrappingKeyStore(), starterPacks: [])

        let host = try XCTUnwrap(model.nearbySpaceHost)
        XCTAssertNil(host.currentSpace)

        model.createSpace(title: "Fire Watch")

        XCTAssertEqual(host.currentSpace?.title, "Fire Watch")
        XCTAssertEqual(model.space?.title, "Fire Watch")
    }

    func testASpaceArrivingLaterIsReannouncedOnlyWhereAHandshakeIsAlreadyStuck() {
        XCTAssertTrue(NearbyReannounceGate.needsReannounce(state: .nothingToShare))
        XCTAssertTrue(NearbyReannounceGate.needsReannounce(state: .failed))
        XCTAssertFalse(NearbyReannounceGate.needsReannounce(state: .looking))
        XCTAssertFalse(
            NearbyReannounceGate.needsReannounce(state: .joinSpace(title: "Fire Watch", name: "PATIENT BROOM"))
        )
        XCTAssertFalse(NearbyReannounceGate.needsReannounce(state: .gettingLatest(name: "PATIENT BROOM")))
        XCTAssertFalse(NearbyReannounceGate.needsReannounce(state: .preview(count: 6, name: "PATIENT BROOM")))
        XCTAssertFalse(NearbyReannounceGate.needsReannounce(state: .connecting))
        XCTAssertFalse(NearbyReannounceGate.needsReannounce(state: .caughtUp))
        XCTAssertFalse(NearbyReannounceGate.needsReannounce(state: .idle))
    }

    private static func temporaryProfileDirectory() throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("riot-shell-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }
}

// MARK: - Create-community stubs

/// Records the backing-space creations so the two-step create is provable
/// without a live store.
private final class StubBackingSpaceCreator: CommunityBackingSpaceCreating {
    private let namespaceID: String
    private(set) var createdNames: [String] = []

    init(namespaceID: String) { self.namespaceID = namespaceID }

    func createBackingSpace(name: String) throws -> RiotSpace {
        createdNames.append(name)
        return RiotSpace(namespaceID: namespaceID, title: name)
    }
}

/// Records the roster passed to `createNewswireSpace`, so the RED test proves the
/// founding editorial selection is threaded through rather than defaulted away.
private final class StubNewswireSpaceCreator: NewswireSpaceCreating {
    private let entryID: String
    private(set) var receivedRosters: [[String]] = []
    private(set) var receivedNames: [String] = []

    init(entryID: String) { self.entryID = entryID }

    func createNewswireCommunity(
        name: String,
        summary: String,
        editorialRoster: [String]
    ) throws -> NewswireSignedRecord {
        receivedNames.append(name)
        receivedRosters.append(editorialRoster)
        return NewswireSignedRecord(entryId: entryID, signedBytes: Data())
    }
}

private final class TestWrappingKeyStore: WrappingKeyStore {
    private var key: Data?

    func loadOrCreateWrappingKey() throws -> Data {
        if let key { return key }
        let created = Data(repeating: 0x5a, count: 32)
        key = created
        return created
    }
}
