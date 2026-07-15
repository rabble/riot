import SwiftUI
import RiotKit

/// The community-first shell (Unit 2A). Riot is organized around a community:
/// once one is selected, a person answers "what is happening here?" (Home) and
/// "what can we do together?" (Tools / People / Nearby). Before a community
/// exists — or while the profile opens, or when a retained community cannot be
/// opened — the shell shows a launch or in-place recovery surface, never a blank
/// screen. The old five debug-shaped surfaces are gone.
struct ConferenceShellView: View {
    @ObservedObject var model: RiotAppModel

    var body: some View {
        Group {
            if let failure = model.starterCatalogFailure {
                CatalogFailureView(failure: failure, onRetry: model.retryStarterCatalog)
            } else {
                switch model.launchState {
                case .loading:
                    ShellRecoveryView(state: .profileStoreLoading, onPrimary: {}, onSecondary: nil)
                case .noCommunity:
                    LaunchView(model: model)
                case let .unavailable(unavailable):
                    ShellRecoveryView(
                        state: .communityUnavailable(unavailable),
                        onPrimary: model.retryCommunity,
                        onSecondary: { model.select(.nearby) }
                    )
                case let .community(community):
                    CommunityShellView(model: model, community: community)
                }
            }
        }
        .alert("Riot couldn’t finish that", isPresented: errorBinding) {
            Button("OK") { model.dismissError() }
        } message: {
            Text(model.errorMessage ?? "Unknown local error")
        }
    }

    private var errorBinding: Binding<Bool> {
        Binding(
            get: { model.errorMessage != nil },
            set: { if !$0 { model.dismissError() } }
        )
    }
}

// MARK: - Launch (no community)

/// The no-community launch surface: Create a community / Find one nearby, with
/// the display name offered inline and skippable, plus the demo space. The
/// community name is required to create; the display name is not.
private struct LaunchView: View {
    @ObservedObject var model: RiotAppModel
    @Environment(\.colorScheme) private var colorScheme
    @State private var communityName = ""
    @State private var displayName = ""
    @State private var demoFailure: String?

    private var trimmedCommunity: String {
        communityName.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                RiotCard {
                    VStack(alignment: .leading, spacing: 12) {
                        eyebrow("Get started")
                        Text("You’re not in a community yet.")
                            .font(.riot(.body, size: 20, relativeTo: .title3))
                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                            .accessibilityAddTraits(.isHeader)

                        // Display name — offered inline, skippable.
                        TextField("Your name (optional)", text: $displayName)
                            .font(.riot(.body, size: 17, relativeTo: .body))
                            .accessibilityIdentifier("launch-display-name")
                        Button("Save name") { model.setDisplayName(displayName.trimmingCharacters(in: .whitespacesAndNewlines)) }
                            .buttonStyle(.riotSecondary)
                            .disabled(displayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                            .accessibilityIdentifier("launch-save-display-name")

                        Divider().overlay(RiotTheme.line(for: colorScheme))

                        TextField("Community name", text: $communityName)
                            .font(.riot(.body, size: 17, relativeTo: .body))
                            .accessibilityIdentifier("community-name-field")
                        Button("Create a community") {
                            model.createCommunity(
                                CommunityCreationRequest(
                                    name: trimmedCommunity,
                                    // The founding collective's initial editorial
                                    // selection: the founder, threaded explicitly
                                    // so the community is not silently pinned to
                                    // core's single-editor default.
                                    editorialRoster: model.me.map { [$0.id] } ?? []
                                )
                            )
                        }
                        .buttonStyle(.riotPrimary)
                        .disabled(trimmedCommunity.isEmpty)
                        .accessibilityIdentifier("create-community")

                        Button("Find one nearby") { model.select(.nearby) }
                            .buttonStyle(.riotSecondary)
                            .accessibilityIdentifier("find-nearby")
                    }
                }

                loadDemoCard
                if let demoFailure {
                    Text(demoFailure)
                        .font(.riot(.body, size: 13, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.pink(for: colorScheme))
                        .accessibilityIdentifier("demo-failure")
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Riot", "Welcome")
        .onAppear { displayName = model.claimedName ?? "" }
    }

    /// The seeded Riverside space, offered where a person will find it — right
    /// where they are deciding what to do with no community of their own.
    private var loadDemoCard: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 12) {
                eyebrow("Or try it out")
                Text("Start from a community that already has people in it — alerts, a part-done checklist, and a tool to open.")
                    .font(.riot(.body, size: 13, relativeTo: .caption))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                Button("Load the demo space (Riverside Tenants Union)") { loadDemoSpace() }
                    .buttonStyle(.riotSecondary)
                    .accessibilityIdentifier("demo-load")
            }
        }
    }

    private func loadDemoSpace() {
        demoFailure = nil
        guard let loader = model.demoLoader, let bytes = DemoFixture.bytes() else {
            demoFailure = DemoModeCopy.missingFixture
            return
        }
        do {
            _ = try loader.loadDemoSpace(bytes: bytes)
        } catch {
            demoFailure = DemoModeCopy.loadFailed
        }
    }

    private func eyebrow(_ text: String) -> some View {
        Text(text)
            .font(.riot(.mono, size: 12, relativeTo: .caption))
            .textCase(.uppercase)
            .tracking(1)
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
    }
}

// MARK: - Recovery surfaces (§4.7)

/// The generic §4.7 recovery surface: a plain-language message, a primary action
/// that is always useful, and an optional secondary — never a blank screen, and
/// never a raw internal error. `ShellRecoveryState` owns the copy so the tests
/// pin it.
private struct ShellRecoveryView: View {
    let state: ShellRecoveryState
    let onPrimary: () -> Void
    let onSecondary: (() -> Void)?
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        VStack(spacing: 16) {
            if case .profileStoreLoading = state {
                ProgressView()
                    .accessibilityLabel("Opening your profile")
            }
            Text(state.message)
                .font(.riot(.body, size: 17, relativeTo: .headline))
                .multilineTextAlignment(.center)
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                .accessibilityAddTraits(.isHeader)
            Button(state.primaryActionLabel, action: onPrimary)
                .buttonStyle(.riotPrimary)
                .frame(minHeight: 44)
            if let secondary = state.secondaryActionLabel, let onSecondary {
                Button(secondary, action: onSecondary)
                    .buttonStyle(.riotSecondary)
                    .frame(minHeight: 44)
            }
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityIdentifier(state.accessibilityID)
    }
}

/// The §4.7 catalog/package-failure recovery: Retry plus a fixed error code
/// behind a Technical-details disclosure — never a raw internal error.
private struct CatalogFailureView: View {
    let failure: StarterCatalogFailure
    let onRetry: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var showingTechnical = false

    var body: some View {
        VStack(spacing: 16) {
            Text("Riot couldn’t load its built-in tools.")
                .font(.riot(.body, size: 17, relativeTo: .headline))
                .multilineTextAlignment(.center)
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                .accessibilityAddTraits(.isHeader)
            Button("Retry", action: onRetry)
                .buttonStyle(.riotPrimary)
                .frame(minHeight: 44)
            DisclosureGroup(isExpanded: $showingTechnical) {
                VStack(alignment: .leading, spacing: 6) {
                    Text(failure.code).font(.riot(.mono, size: 12, relativeTo: .caption))
                    Text(failure.technicalDetails)
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .textSelection(.enabled)
                }
            } label: {
                Text("Technical details")
                    .font(.riot(.mono, size: 12, relativeTo: .caption))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            }
            .accessibilityIdentifier("catalog-technical-details")
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityIdentifier("recovery-catalog-failed")
    }
}

// MARK: - The community shell (adaptive)

/// The selected-community shell. On iPhone it is a bottom tab bar over the four
/// routes; on macOS it is a `NavigationSplitView` whose detail pane shows the
/// selected route or a running tool — a tool never opens as a modal sheet on the
/// Mac. Route selection lives on `RiotNavigationModel` (the performance
/// contract); the running tool, its focus restoration, and the identity sheets
/// live here.
private struct CommunityShellView: View {
    @ObservedObject var model: RiotAppModel
    @ObservedObject private var navigation: RiotNavigationModel
    let community: CommunityContext

    /// The composer and People projection are built once from the selected
    /// community, so a post draft and the resolved contributors survive route
    /// switches. Rebuilt only when the shell is recreated for a new community.
    @StateObject private var composer: PostUpdateViewModel
    @StateObject private var people: PeopleSurfaceModel

    /// The one community-scoped Nearby coordinator (nav design §"Nearby security
    /// and lifecycle": "The selected community owns one community-scoped Nearby
    /// coordinator"). It lives HERE, above the four routes, not inside the Nearby
    /// route — so it survives Home/Tools/People/Nearby routing. When the shell is
    /// recreated for a different community, this instance is torn down (its
    /// callbacks cancelled on disappear) and the new community gets its own.
    @StateObject private var nearby = NearbyTransportController()

    /// The tool running in the detail pane (macOS) / full-screen (iPhone), and
    /// the card that opened it, so focus returns there on close.
    @State private var runningTool: RiotSpaceApp?
    @State private var focus = ToolFocusRestoration()

    @State private var identitySheet: ShellIdentityDestination?
    /// Presented when a community-change is requested with an unsaved draft.
    @State private var confirmingLeave = false

    @Environment(\.colorScheme) private var colorScheme

    init(model: RiotAppModel, community: CommunityContext) {
        _model = ObservedObject(wrappedValue: model)
        _navigation = ObservedObject(wrappedValue: model.navigation)
        self.community = community

        let me = model.me ?? RiotPerson(id: "", displayName: "member", tag: "")
        let publisher: NewswirePostPublishing = model.profileRepository ?? UnavailablePublisher()
        let postingCommunity = PostingCommunity(
            name: community.name,
            spaceDescriptorEntryID: community.newswireDescriptorEntryID ?? ""
        )
        _composer = StateObject(wrappedValue: PostUpdateViewModel(
            identity: .persistent(me),
            community: postingCommunity,
            publisher: publisher,
            draftStore: UserDefaultsPostDraftStore(communityID: community.id)
        ))

        let projector: NewswireContributorProjecting = model.profileRepository ?? UnavailableProjector()
        _people = StateObject(wrappedValue: PeopleSurfaceModel(
            projector: projector,
            spaceDescriptorEntryID: community.newswireDescriptorEntryID ?? ""
        ))
    }

    /// Whether there is unsaved work that must be confirmed before a community
    /// change (nav design + §4.6). A non-empty post draft is unsaved work.
    private var hasUnsavedDraft: Bool { !composer.currentDraft.isEmpty }

    var body: some View {
        adaptiveShell
            .background(keyboardShortcuts)
            // Leaving/switching this community cancels the old coordinator's
            // pairing, transfer, and callbacks before the shell is rebuilt for the
            // next community (nav design §"Nearby security and lifecycle").
            .onDisappear { nearby.stop() }
            .sheet(item: $identitySheet) { destination in
                switch destination {
                case .yourProfile:
                    YourProfileSheet(model: model, onClose: { identitySheet = nil })
                case .communitySettings:
                    CommunitySettingsSheet(
                        model: model,
                        community: community,
                        onLeave: requestLeaveCommunity,
                        onClose: { identitySheet = nil }
                    )
                }
            }
            .confirmationDialog(
                StayOrDiscardPrompt.title,
                isPresented: $confirmingLeave,
                titleVisibility: .visible
            ) {
                Button(StayOrDiscardPrompt.discardLabel, role: .destructive) { model.leaveCommunity() }
                Button(StayOrDiscardPrompt.stayLabel, role: .cancel) {}
            }
    }

    // MARK: Adaptive presentation

    @ViewBuilder
    private var adaptiveShell: some View {
        #if os(macOS)
        macShell
        #else
        phoneShell
        #endif
    }

    #if os(macOS)
    /// macOS: sidebar of the four routes + identity footer; the detail pane shows
    /// the selected route or the running tool. A tool is NEVER a modal sheet.
    private var macShell: some View {
        NavigationSplitView {
            VStack(spacing: 0) {
                List(RiotDestination.phoneTabs, selection: sidebarSelection) { destination in
                    Label(destination.title, systemImage: destination.systemImage)
                        .tag(destination)
                        .accessibilityIdentifier("route-\(destination.rawValue)")
                }
                Divider()
                identityFooter
                    .padding(12)
            }
            .navigationTitle(community.name)
        } detail: {
            if let tool = runningTool, let repository = model.profileRepository {
                AppRuntimeView(
                    repository: repository,
                    appIDHex: tool.appIDHex,
                    appName: tool.name,
                    onClose: closeTool
                )
                .onExitCommand(perform: escape)
            } else {
                routeView(navigation.destination)
            }
        }
    }

    private var sidebarSelection: Binding<RiotDestination?> {
        Binding(
            get: { navigation.destination },
            set: { if let value = $0 { changeRoute(to: value) } }
        )
    }

    private var identityFooter: some View {
        VStack(alignment: .leading, spacing: 8) {
            Button { identitySheet = .yourProfile } label: {
                Label(ShellIdentityDestination.yourProfile.label,
                      systemImage: ShellIdentityDestination.yourProfile.systemImage)
            }
            .accessibilityIdentifier(ShellIdentityDestination.yourProfile.accessibilityID)
            Button { identitySheet = .communitySettings } label: {
                Label(ShellIdentityDestination.communitySettings.label,
                      systemImage: ShellIdentityDestination.communitySettings.systemImage)
            }
            .accessibilityIdentifier(ShellIdentityDestination.communitySettings.accessibilityID)
        }
        .buttonStyle(.plain)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
    #else
    /// iPhone: a header with the community name and the two identity controls,
    /// the four routes kept alive in a ZStack (the tab-lifecycle performance
    /// contract), a connection bar, and the bottom tab bar. Opening a tool
    /// presents it full-screen — never the phone's card sheet for a tool.
    private var phoneShell: some View {
        VStack(spacing: 0) {
            communityHeader
            ZStack {
                ForEach(RiotDestination.phoneTabs) { destination in
                    NavigationStack {
                        routeView(destination)
                    }
                    .opacity(navigation.destination == destination ? 1 : 0)
                    .allowsHitTesting(navigation.destination == destination)
                }
            }
            connectionDisclosureBar
            RiotTabBar(selection: tabSelection)
        }
        .background(RiotTheme.paper(for: colorScheme).ignoresSafeArea())
        .fullScreenCover(item: $runningTool) { tool in
            if let repository = model.profileRepository {
                AppRuntimeView(
                    repository: repository,
                    appIDHex: tool.appIDHex,
                    appName: tool.name,
                    onClose: closeTool
                )
            } else {
                Color.clear.onAppear { closeTool() }
            }
        }
    }

    private var tabSelection: Binding<RiotDestination> {
        Binding(
            get: { navigation.destination },
            set: { changeRoute(to: $0) }
        )
    }

    private var communityHeader: some View {
        HStack(spacing: 12) {
            Button { identitySheet = .yourProfile } label: {
                Image(systemName: ShellIdentityDestination.yourProfile.systemImage)
                    .font(.system(size: 22))
            }
            .accessibilityLabel(ShellIdentityDestination.yourProfile.label)
            .accessibilityIdentifier(ShellIdentityDestination.yourProfile.accessibilityID)

            Text(community.name)
                .font(.riot(.body, size: 18, relativeTo: .headline))
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                .frame(maxWidth: .infinity, alignment: .leading)
                .accessibilityAddTraits(.isHeader)

            Button { identitySheet = .communitySettings } label: {
                Image(systemName: ShellIdentityDestination.communitySettings.systemImage)
                    .font(.system(size: 20))
            }
            .accessibilityLabel(ShellIdentityDestination.communitySettings.label)
            .accessibilityIdentifier(ShellIdentityDestination.communitySettings.accessibilityID)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(RiotTheme.paper2(for: colorScheme))
    }

    private var connectionDisclosureBar: some View {
        VStack(spacing: 3) {
            if let me = model.me {
                Text("You are \(me.rendered)")
                    .font(.riot(.mono, size: 11, relativeTo: .caption2))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    .accessibilityIdentifier("identity-chip")
            }
            Text(model.connectionDisclosure)
                .font(.riot(.mono, size: 11, relativeTo: .caption2))
                .textCase(.uppercase)
                .tracking(0.5)
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
        .background(RiotTheme.paper2(for: colorScheme))
        .overlay(alignment: .top) {
            Rectangle().fill(RiotTheme.line(for: colorScheme)).frame(height: 1)
        }
    }
    #endif

    // MARK: Routes

    @ViewBuilder
    private func routeView(_ destination: RiotDestination) -> some View {
        switch destination {
        case .home:
            HomeRouteView(
                model: model,
                composer: composer,
                onOpenTool: openTool
            )
        case .tools:
            DirectoryView(model: model, onOpen: { openTool($0, fromCardID: "tool-\($0.appIDHex)") })
        case .people:
            PeopleView(model: people, onPostUpdate: { changeRoute(to: .home) })
        case .nearby:
            ConnectionStatusView(model: model, nearby: nearby)
        }
    }

    // MARK: Tool lifecycle + focus

    private func openTool(_ app: RiotSpaceApp, fromCardID cardID: String) {
        focus.open(toolID: cardID)
        runningTool = app
    }

    private func closeTool() {
        _ = focus.close()
        runningTool = nil
    }

    private func escape() {
        switch ShellEscapeAction.action(isToolOpen: runningTool != nil, hasUnsavedWork: false) {
        case .returnFromTool: closeTool()
        case .ignore, .confirmDiscard: break
        }
    }

    // MARK: Community change guard

    private func changeRoute(to destination: RiotDestination) {
        model.select(destination)
    }

    private func requestLeaveCommunity() {
        identitySheet = nil
        switch CommunityChangeGuard.decision(hasUnsavedDraft: hasUnsavedDraft) {
        case .proceed: model.leaveCommunity()
        case .confirm: confirmingLeave = true
        }
    }

    // MARK: Keyboard: Command-1…4

    private var keyboardShortcuts: some View {
        ForEach(RiotDestination.phoneTabs) { destination in
            Button("") { changeRoute(to: destination) }
                .keyboardShortcut(
                    KeyEquivalent(Character("\(destination.commandNumber)")),
                    modifiers: .command
                )
                .frame(width: 0, height: 0)
                .opacity(0)
                .accessibilityHidden(true)
        }
    }
}

// MARK: - Home route

/// Home answers "what is happening here?" and "what can we do together?": the
/// community's updates, Post an update as a primary action, and four
/// deterministic tool shortcuts. An empty updates feed shows the actionable
/// no-updates recovery, never a blank list.
private struct HomeRouteView: View {
    @ObservedObject var model: RiotAppModel
    @ObservedObject var composer: PostUpdateViewModel
    let onOpenTool: (RiotSpaceApp, String) -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var inspecting: RiotEntry?

    private var shortcuts: [RiotSpaceApp] { HomeShortcuts.deterministic(from: model.apps) }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                shortcutsCard
                updatesSection
                PostUpdateView(model: composer)
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Community", model.space?.title ?? "Home")
        .sheet(item: $inspecting) { entry in
            AlertDetailView(entry: entry, onClose: { inspecting = nil })
        }
    }

    @ViewBuilder
    private var shortcutsCard: some View {
        if !shortcuts.isEmpty {
            RiotCard {
                VStack(alignment: .leading, spacing: 12) {
                    eyebrow("Do useful work")
                    ForEach(shortcuts) { app in
                        Button {
                            onOpenTool(app, "home-shortcut-\(app.appIDHex)")
                        } label: {
                            HStack {
                                Image(systemName: "square.grid.2x2")
                                Text(app.name).font(.riot(.body, size: 17, relativeTo: .body))
                                Spacer()
                            }
                        }
                        .buttonStyle(.riotSecondary)
                        .accessibilityIdentifier("home-shortcut-\(app.name)")
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var updatesSection: some View {
        if model.entries.isEmpty {
            ShellRecoveryInline(state: .noUpdates, onPrimary: {}, onSecondary: { model.select(.nearby) })
        } else {
            RiotCard {
                VStack(alignment: .leading, spacing: 12) {
                    eyebrow("What is happening")
                    ForEach(model.entries) { entry in
                        Button { inspecting = entry } label: {
                            VStack(alignment: .leading, spacing: 6) {
                                Text(entry.headline)
                                    .font(.riot(.body, size: 17, relativeTo: .headline))
                                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                                if entry.aiAssisted {
                                    RiotBadge("AI-assisted · human reviewed and signed")
                                }
                                Text("Created \(Date(timeIntervalSince1970: TimeInterval(entry.createdAt)), style: .relative)")
                                    .font(.riot(.mono, size: 11, relativeTo: .caption2))
                                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                        }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("update-\(entry.entryID)")
                    }
                }
            }
        }
    }

    private func eyebrow(_ text: String) -> some View {
        Text(text)
            .font(.riot(.mono, size: 12, relativeTo: .caption))
            .textCase(.uppercase)
            .tracking(1)
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
    }
}

/// The §4.7 recovery rows rendered inline inside a route (Home's no-updates, no
/// tools), rather than as a full-screen surface. Same copy source, so the tests
/// pin it once.
private struct ShellRecoveryInline: View {
    let state: ShellRecoveryState
    let onPrimary: () -> Void
    let onSecondary: (() -> Void)?
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 10) {
                Text(state.message)
                    .font(.riot(.body, size: 15, relativeTo: .callout))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                Button(state.primaryActionLabel, action: onPrimary)
                    .buttonStyle(.riotPrimary)
                    .frame(minHeight: 44)
                if let secondary = state.secondaryActionLabel, let onSecondary {
                    Button(secondary, action: onSecondary)
                        .buttonStyle(.riotSecondary)
                        .frame(minHeight: 44)
                }
            }
        }
        .accessibilityIdentifier(state.accessibilityID)
    }
}

// MARK: - Identity sheets

/// "Your profile" — editing this person's identity, moved out of the way of
/// everyday community content (nav design: manage identity in context).
private struct YourProfileSheet: View {
    @ObservedObject var model: RiotAppModel
    let onClose: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var name = ""

    private var trimmed: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if let me = model.me {
                    Text(me.rendered)
                        .font(.riot(.body, size: 20, relativeTo: .title3))
                        .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        .textSelection(.enabled)
                        .accessibilityIdentifier("my-rendered-name")
                    Text("This is how you appear to everyone you sync with. Choose the name; the characters after the dot come from your key, so two people who both pick “Ana” are still told apart.")
                        .font(.riot(.body, size: 13, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                }
                TextField("Your name", text: $name)
                    .font(.riot(.body, size: 17, relativeTo: .body))
                    .accessibilityIdentifier("my-name-field")
                Button("Save name") { model.setDisplayName(trimmed) }
                    .buttonStyle(.riotPrimary)
                    .disabled(trimmed.isEmpty)
                    .accessibilityIdentifier("save-my-name")
                if let nameError = model.nameError {
                    Text(nameError)
                        .font(.riot(.body, size: 13, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.pink(for: colorScheme))
                        .accessibilityIdentifier("my-name-error")
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "You", ShellIdentityDestination.yourProfile.label)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) { Button("Done", action: onClose) }
        }
        .onAppear { name = model.claimedName ?? "" }
    }
}

/// "Community settings" — About, sync health, and Technical details for members;
/// organizer-only governance (leaving the community here in 2A) appears only when
/// core says this profile organizes.
private struct CommunitySettingsSheet: View {
    @ObservedObject var model: RiotAppModel
    let community: CommunityContext
    let onLeave: () -> Void
    let onClose: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var showingTechnical = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                RiotCard {
                    VStack(alignment: .leading, spacing: 10) {
                        eyebrow("About")
                        LabeledContent("Community", value: community.name)
                        LabeledContent("Your role", value: community.isOrganizer ? "Organizer" : "Member")
                        LabeledContent("Connection", value: model.connectionDisclosure)
                    }
                }
                DisclosureGroup(isExpanded: $showingTechnical) {
                    RiotCard {
                        IdentifierRow(label: "Namespace", value: community.namespaceID)
                    }
                    .padding(.top, 8)
                } label: {
                    Text("Technical details")
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                }
                .accessibilityIdentifier("community-technical-details")

                Button("Leave this community", role: .destructive, action: onLeave)
                    .buttonStyle(.riotSecondary)
                    .accessibilityIdentifier("leave-community")
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Community", ShellIdentityDestination.communitySettings.label)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) { Button("Done", action: onClose) }
        }
    }

    private func eyebrow(_ text: String) -> some View {
        Text(text)
            .font(.riot(.mono, size: 12, relativeTo: .caption))
            .textCase(.uppercase)
            .tracking(1)
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
    }
}

// MARK: - Unavailable seams (never reached with an open profile)

/// A publisher used only if the profile is somehow closed when the shell builds
/// — it fails closed rather than force-unwrapping. The live conformer is the
/// repository.
private struct UnavailablePublisher: NewswirePostPublishing {
    func publishNewswirePost(_ request: PostUpdateRequest) throws -> NewswireSignedRecord {
        throw RepositoryError.profileClosed
    }
}

private struct UnavailableProjector: NewswireContributorProjecting {
    func projectNewswireContributors(spaceDescriptorEntryID: String) throws -> [NewswireContributor] {
        throw RepositoryError.profileClosed
    }
}

// MARK: - Nearby (existing surface, now the Nearby route)

private struct ConnectionStatusView: View {
    @ObservedObject var model: RiotAppModel
    /// Owned by `CommunityShellView` (community-scoped, survives routing); this
    /// route only observes it.
    @ObservedObject var nearby: NearbyTransportController
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.openURL) private var openURL
    @State private var inspecting: DiscoveredPhone?
    @State private var inspectingPerson: RiotPerson?

    private var syncedPeople: [RiotPerson] {
        guard let repository = model.profileRepository,
              let me = try? repository.me() else { return [] }
        return model.displayNames.keys
            .filter { $0.lowercased() != me.id.lowercased() }
            .compactMap { try? repository.person(idHex: $0) }
            .sorted { $0.rendered < $1.rendered }
    }

    private func startDiscoveryWhenReady() {
        guard model.isProfileOpen, nearby.state == .idle else { return }
        nearby.findNearby(host: model.nearbySpaceHost)
    }

    private func reannounceSpaceIfStuck() {
        guard model.space != nil, NearbyReannounceGate.needsReannounce(state: nearby.state) else { return }
        nearby.findNearby(host: model.nearbySpaceHost)
    }

    private var syncSentence: String {
        switch nearby.state {
        case .connecting: "Connecting…"
        case .gettingLatest: "Getting the latest from them…"
        case let .preview(count, _):
            "\(count) new thing\(count == 1 ? "" : "s") to bring over — review them below"
        case .caughtUp:
            if let count = nearby.itemsBroughtOver, count > 0 {
                "Synced · \(count) new thing\(count == 1 ? "" : "s") arrived"
            } else {
                "Synced · you both have the same things"
            }
        case .alreadyCurrent: "Synced · nothing new to bring over"
        case .differentSpace: "They are in a different space, so nothing was shared"
        case .outOfRange: "They went out of range"
        case .failed: "The connection failed — try again"
        default: "Connected"
        }
    }

    /// The §4.7 "Bluetooth/local-network denied" recovery: what still works
    /// offline plus an Open Settings deep link — never a raw permission error.
    private var permissionRecoveryCard: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 10) {
                Text("Nearby needs permission")
                    .font(.riot(.body, size: 17, relativeTo: .headline))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    .accessibilityAddTraits(.isHeader)
                Text(NearbyPermissionRecovery.message)
                    .font(.riot(.body, size: 14, relativeTo: .callout))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                if let url = NearbyPermissionRecovery.settingsURL {
                    Button("Open Settings") { openURL(url) }
                        .buttonStyle(.riotPrimary)
                        .frame(minHeight: 44)
                        .accessibilityIdentifier("nearby-open-settings")
                }
            }
        }
        .accessibilityIdentifier("nearby-permission-denied")
    }

    private var inboundConfirmTitle: String {
        if case let .confirm(name) = nearby.state { return "Accept connection from \(name)?" }
        return ""
    }

    @ViewBuilder
    private var connectedCard: some View {
        if let peer = nearby.connectedPeer {
            RiotCard {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Connected to")
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .textCase(.uppercase)
                        .tracking(1)
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    Text(peer)
                        .font(.riot(.body, size: 20, relativeTo: .title3))
                        .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        .accessibilityIdentifier("connected-peer")
                    Text(syncSentence)
                        .font(.riot(.body, size: 14, relativeTo: .callout))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        .accessibilityIdentifier("connected-sync-state")
                }
            }
        }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                RiotBadge(nearby.state.message, stamped: true)
                if nearby.permissionDenied { permissionRecoveryCard }
                connectedCard
                RiotCard {
                    VStack(alignment: .leading, spacing: 14) {
                        Text("Connections stay between devices near you — over Bluetooth, or the local network you are both on. Riot never sends this session over the internet.")
                            .font(.riot(.body, size: 15, relativeTo: .callout))
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        if nearby.state == .idle || nearby.state == .failed {
                            Button("Find nearby devices") {
                                nearby.findNearby(host: model.nearbySpaceHost)
                            }
                            .buttonStyle(.riotPrimary)
                        } else {
                            Button("Stop looking", role: .cancel) { nearby.stop() }
                                .buttonStyle(.riotSecondary)
                        }
                        if case .preview = nearby.state {
                            Button("Add them") { nearby.addPreviewedContent() }
                                .buttonStyle(.riotPrimary)
                            Button("Not now", role: .cancel) { nearby.rejectPreviewedContent() }
                                .buttonStyle(.riotSecondary)
                        }
                    }
                }
                if !nearby.phones.isEmpty {
                    RiotCard {
                        VStack(alignment: .leading, spacing: 10) {
                            Text("Devices")
                                .font(.riot(.mono, size: 12, relativeTo: .caption))
                                .textCase(.uppercase)
                                .tracking(1)
                                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            ForEach(nearby.phones) { phone in
                                Button(phone.friendlyName) { inspecting = phone }
                                    .buttonStyle(.riotSecondary)
                                    .accessibilityIdentifier("peer-\(phone.friendlyName)")
                            }
                        }
                    }
                }
                if !syncedPeople.isEmpty {
                    RiotCard {
                        VStack(alignment: .leading, spacing: 10) {
                            Text("People")
                                .font(.riot(.mono, size: 12, relativeTo: .caption))
                                .textCase(.uppercase)
                                .tracking(1)
                                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            Text("People you have synced with. Tap to see who they are and what they carry.")
                                .font(.riot(.body, size: 13, relativeTo: .caption))
                                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            ForEach(syncedPeople) { person in
                                Button(person.rendered) { inspectingPerson = person }
                                    .buttonStyle(.riotSecondary)
                                    .accessibilityIdentifier("person-\(person.id)")
                            }
                        }
                    }
                }
                RiotCard {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("On this device")
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .textCase(.uppercase)
                            .tracking(1)
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        LabeledContent("Signed alerts", value: "\(model.entries.count)")
                        LabeledContent("Renderer", value: "incident-board/1")
                    }
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Transport", "Nearby")
        .sheet(item: $inspecting) { phone in
            PeerProfileView(
                model: model,
                peerName: phone.friendlyName,
                isConnected: nearby.connectedPeer == phone.friendlyName,
                onInvite: { _ in
                    nearby.requestConnection(to: phone)
                    inspecting = nil
                },
                onClose: { inspecting = nil }
            )
        }
        .sheet(item: $inspectingPerson) { person in
            PeerProfileView(
                model: model,
                peerName: person.rendered,
                authoredName: person.rendered,
                onClose: { inspectingPerson = nil }
            )
        }
        .onAppear {
            nearby.onSpaceJoined = { model.refreshFromStore() }
            startDiscoveryWhenReady()
        }
        .onChange(of: model.isProfileOpen) { _, _ in startDiscoveryWhenReady() }
        .onChange(of: model.space) { _, _ in reannounceSpaceIfStuck() }
        .onChange(of: nearby.state) { _, state in
            guard case .nothingToShare = state, model.space != nil else { return }
            nearby.findNearby(host: model.nearbySpaceHost)
        }
        // A peer asked to pair. Discovery never auto-accepts — the human here must
        // say yes before the connection is made or any community disclosed.
        .confirmationDialog(
            inboundConfirmTitle,
            isPresented: Binding(
                get: { if case .confirm = nearby.state { return true }; return false },
                set: { if !$0 { nearby.cancelConnection() } }
            ),
            titleVisibility: .visible
        ) {
            Button("Accept") { nearby.confirmConnection() }
            Button("Decline", role: .cancel) { nearby.cancelConnection() }
        }
        .confirmationDialog(
            nearby.state.message,
            isPresented: Binding(
                get: { if case .joinSpace = nearby.state { return true }; return false },
                set: { if !$0 { nearby.declineJoinSpace() } }
            )
        ) {
            Button("Join") { nearby.confirmJoinSpace() }
            Button("Not now", role: .cancel) { nearby.declineJoinSpace() }
        }
        .onChange(of: nearby.state) { _, state in
            if case .joinSpace = state,
               ProcessInfo.processInfo.environment["RIOT_AUTO_CONFIRM"] == "1" {
                nearby.confirmJoinSpace()
            }
        }
    }
}

// MARK: - Update detail

/// The full signed detail behind a Home update: what it says, when it is good
/// for, and — only if asked — the identifiers that prove it. `AlertDetail`
/// (RiotKit) owns what appears where, so the rule that full ids stay behind
/// **Technical details** is pinned by tests rather than by this view.
private struct AlertDetailView: View {
    let entry: RiotEntry
    let onClose: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var showingTechnical = false

    private var detail: AlertDetail { AlertDetail(entry: entry) }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text(detail.headline)
                    .font(.riot(.body, size: 22, relativeTo: .largeTitle))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                if detail.aiAssisted {
                    RiotBadge("AI-assisted · human reviewed and signed")
                }
                RiotCard {
                    VStack(alignment: .leading, spacing: 10) {
                        ForEach(detail.summary, id: \.label) { row in
                            IdentifierRow(label: row.label, value: row.value)
                        }
                    }
                }
                DisclosureGroup(isExpanded: $showingTechnical) {
                    RiotCard {
                        VStack(alignment: .leading, spacing: 10) {
                            ForEach(detail.technical, id: \.label) { row in
                                IdentifierRow(label: row.label, value: row.value)
                            }
                        }
                    }
                    .padding(.top, 10)
                } label: {
                    Text(AlertDetail.technicalDisclosureTitle)
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                }
                .accessibilityIdentifier("alert-technical-details")
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Signed update", detail.headline)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Close", action: onClose)
            }
        }
    }
}

private struct IdentifierRow: View {
    @Environment(\.colorScheme) private var colorScheme
    let label: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label)
                .font(.riot(.mono, size: 11, relativeTo: .caption2))
                .textCase(.uppercase)
                .tracking(0.5)
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            Text(value)
                .font(.riot(.mono, size: 13, relativeTo: .footnote))
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                .textSelection(.enabled)
        }
    }
}
