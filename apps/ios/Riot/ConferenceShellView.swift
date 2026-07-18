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
                    // The launch load. Its Retry re-attempts the (now self-healing)
                    // open, and — because it must never be a permanent dead-end —
                    // "Start fresh" quarantines the persisted state aside and opens
                    // clean. See ``RiotAppModel/resetAndRecover``.
                    ShellRecoveryView(
                        state: .profileStoreLoading,
                        onPrimary: model.retryBootstrap,
                        onSecondary: model.resetAndRecover,
                        secondaryLabelOverride: "Start fresh"
                    )
                case .noCommunity:
                    OnboardingView(model: model)
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
        .safeAreaInset(edge: .top) {
            if let message = model.recoveryNoticeMessage {
                RecoveryNoticeBanner(message: message, onDismiss: model.dismissRecoveryNotice)
            }
        }
        .alert("Riot couldn’t finish that", isPresented: errorBinding) {
            // Never only a dead OK/RETRY: "Start fresh" quarantines the persisted
            // data aside (never deletes it) and re-opens a fresh profile, so a
            // genuinely-unrecoverable error still reaches a usable state.
            Button("Start fresh") { model.resetAndRecover() }
            Button("OK") { model.dismissError() }
        } message: {
            Text(model.errorMessage ?? "Unknown local error")
        }
        .sheet(item: openOutcomeBinding) { outcome in
            OpenInRiotVerifyView(outcome: outcome, onClose: model.dismissOpenOutcome)
        }
    }

    private var errorBinding: Binding<Bool> {
        Binding(
            get: { model.errorMessage != nil },
            set: { if !$0 { model.dismissError() } }
        )
    }

    /// Presents the verify outcome of an "Open in Riot" link — but not for a plain
    /// home/masthead link, which only navigates. Dismissing clears the pending
    /// outcome on the model.
    private var openOutcomeBinding: Binding<RiotOpenOutcome?> {
        Binding(
            get: {
                if case .openedHome = model.openOutcome { return nil }
                return model.openOutcome
            },
            set: { if $0 == nil { model.dismissOpenOutcome() } }
        )
    }
}

// MARK: - "Open in Riot" verify result

/// The honest verify result of an "Open in Riot" deep link (web = reach, app =
/// truth). A "Verified in Riot" badge appears ONLY for a post this device holds as
/// its own signed, signature-verified record — never a fake checkmark for content
/// the app has not cryptographically checked. The other states are equally honest:
/// a post not yet synced cannot be verified, and a community this device does not
/// follow has nothing to verify against until it joins and syncs.
struct OpenInRiotVerifyView: View {
    let outcome: RiotOpenOutcome
    let onClose: () -> Void

    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: symbolName)
                .font(.system(size: 44))
                .foregroundStyle(symbolColor)
                .accessibilityHidden(true)
            Text(title)
                .font(.title2.weight(.bold))
                .multilineTextAlignment(.center)
            if let headline {
                Text(headline)
                    .font(.headline)
                    .multilineTextAlignment(.center)
            }
            Text(explanation)
                .font(.body)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
            Spacer()
            Button(action: onClose) {
                Text("Done")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
        }
        .padding(28)
        .presentationDetents([.medium])
    }

    private var symbolName: String {
        switch outcome {
        case .verified: "checkmark.seal.fill"
        case .postNotHeld: "clock.badge.questionmark"
        case .notFollowing: "person.crop.circle.badge.plus"
        case .openedHome: "house.fill"
        }
    }

    private var symbolColor: Color {
        switch outcome {
        case .verified: .green
        case .postNotHeld, .notFollowing, .openedHome: .secondary
        }
    }

    private var title: String {
        switch outcome {
        case .verified: "Verified in Riot"
        case .postNotHeld: "Not on your device yet"
        case .notFollowing: "You don’t follow this community"
        case .openedHome: "Opened in Riot"
        }
    }

    private var headline: String? {
        if case let .verified(_, _, headline) = outcome { return headline }
        return nil
    }

    private var explanation: String {
        switch outcome {
        case .verified:
            return "Riot holds this post as a signed record for this community and "
                + "verified its signature when it synced. The web copy matches a "
                + "post Riot cryptographically checked — a mirror can’t forge that."
        case .postNotHeld:
            return "Riot can’t verify this post because it hasn’t synced to your "
                + "device yet. Open this community, let it sync, then follow the "
                + "link again to check the signature."
        case .notFollowing:
            return "There’s nothing here for Riot to check against yet. Join this "
                + "community and let it sync — then Riot can verify its posts "
                + "against the signed records themselves, not the web copy."
        case .openedHome:
            return "Opened this community’s Home."
        }
    }
}

// MARK: - First-run onboarding (no community)

/// The first-run guided path. It is the shell's `.noCommunity` surface, so it is
/// shown exactly when `Onboarding.isFirstRun` is true and is dismissed the moment
/// a community exists (the launch state flips to `.community` and the shell
/// takes over). Two short screens — a welcome that says what Riot is, then setup
/// where you name yourself and create or join — because activists in the field
/// need a path, not a wizard. It reuses the display-name path and the
/// create/paste-to-join logic; it adds no community or identity machinery.
private struct OnboardingView: View {
    @ObservedObject var model: RiotAppModel
    @State private var step: OnboardingStep = .first

    var body: some View {
        switch step {
        case .welcome:
            OnboardingWelcomeView(onContinue: { step = .setup })
        case .setup:
            OnboardingSetupView(model: model, onBack: { step = .welcome })
        }
    }
}

/// Screen one: what Riot is, in plain indymedia terms. One action — get started.
private struct OnboardingWelcomeView: View {
    @Environment(\.colorScheme) private var colorScheme
    let onContinue: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                RiotCard {
                    VStack(alignment: .leading, spacing: 12) {
                        eyebrow("Welcome")
                        Text("Publish to your community.")
                            .font(.riot(.body, size: 24, relativeTo: .title2))
                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                            .accessibilityAddTraits(.isHeader)
                        Text("Reach the web, prove it in the app. No servers, no accounts.")
                            .font(.riot(.body, size: 17, relativeTo: .body))
                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        Text("Riot is a place to report what's happening where you are — an update, an alert, a call to show up — and have your community carry it. Your posts are signed by you and shared device to device. The web can mirror them; the proof stays in the app.")
                            .font(.riot(.body, size: 15, relativeTo: .callout))
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))

                        Button("Get started", action: onContinue)
                            .buttonStyle(.riotPrimary)
                            .accessibilityIdentifier("onboarding-get-started")
                    }
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Riot", "Welcome")
    }

    private func eyebrow(_ text: String) -> some View {
        Text(text)
            .font(.riot(.mono, size: 12, relativeTo: .caption))
            .textCase(.uppercase)
            .tracking(1)
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
    }
}

/// Screen two: name yourself (skippable) and create or join a community
/// (required to leave onboarding). The display name reuses the existing
/// `setDisplayName` path; create reuses `createCommunity`; join reuses the exact
/// paste-to-join sheet the community chooser uses. Nothing here is new identity
/// or community machinery — it is a flow layer that routes into what exists.
private struct OnboardingSetupView: View {
    @ObservedObject var model: RiotAppModel
    @Environment(\.colorScheme) private var colorScheme
    let onBack: () -> Void
    @State private var communityName = ""
    @State private var displayName = ""
    @State private var demoFailure: String?
    @State private var isJoinPresented = false

    private var trimmedCommunity: String {
        communityName.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                RiotCard {
                    VStack(alignment: .leading, spacing: 12) {
                        eyebrow("Set up")
                        Text("Name yourself, then start or join a community.")
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

                        Button("Join with a link or QR") { isJoinPresented = true }
                            .buttonStyle(.riotSecondary)
                            .accessibilityIdentifier("launch-join-by-reference")

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
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Back", action: onBack)
                    .accessibilityIdentifier("onboarding-back")
            }
        }
        .sheet(isPresented: $isJoinPresented) {
            // The same paste/QR join sheet the in-app chooser uses, so onboarding's
            // join path is the identical code and core call, never a duplicate.
            JoinByReferenceSheet(model: model, onClose: { isJoinPresented = false })
        }
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

// MARK: - Self-healing recovery notice

/// The honest, non-fatal notice shown after a self-healing open recovered
/// something. Dismissible — the recovery already happened and the app is usable;
/// this only tells the person what was set aside (never deleted). Never a dead
/// error.
private struct RecoveryNoticeBanner: View {
    let message: String
    let onDismiss: () -> Void
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(.orange)
                .accessibilityHidden(true)
            Text(message)
                .font(.riot(.body, size: 14, relativeTo: .footnote))
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 8)
            Button(action: onDismiss) {
                Image(systemName: "xmark")
                    .font(.system(size: 12, weight: .bold))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            }
            .accessibilityLabel("Dismiss")
        }
        .padding(12)
        .background(RiotTheme.paper2(for: colorScheme))
        .overlay(alignment: .bottom) {
            Rectangle().fill(.orange.opacity(0.4)).frame(height: 1)
        }
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("recovery-notice-banner")
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
    /// A secondary label for states whose own `secondaryActionLabel` is nil (the
    /// loading state), so the launch load can offer "Start fresh" without
    /// changing the copy the recovery-state tests pin.
    var secondaryLabelOverride: String? = nil
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
            if let secondary = secondaryLabelOverride ?? state.secondaryActionLabel,
               let onSecondary {
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
    @StateObject private var newswire: NewswireSurfaceModel

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

        let wireProjector: NewswireProjecting = model.profileRepository ?? UnavailableWireProjector()
        let editor: NewswireEditorialActing = model.profileRepository ?? UnavailableEditor()
        let authority: NewswireEditorAuthorityChecking = model.profileRepository ?? UnavailableEditor()
        _newswire = StateObject(wrappedValue: NewswireSurfaceModel(
            projector: wireProjector,
            editor: editor,
            authority: authority,
            spaceDescriptorEntryID: community.newswireDescriptorEntryID ?? "",
            communityName: community.name,
            myKeyHex: me.id,
            descriptorResolver: { [weak model] in model?.rederivedNewswireDescriptorID() },
            // Per-device seen state lives in the standard defaults, keyed per
            // community — never the Willow store, never the FFI.
            seenCursor: SeenCursorStore()
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
            // Level-1 "Your communities" — one action away via Command-K or the
            // community-name control; selecting a row switches, an unavailable row
            // recovers in place.
            .sheet(isPresented: $model.isCommunityChooserPresented) {
                CommunityChooserView(model: model)
            }
            // Join with a link or QR — raised from the chooser's "Join another" row
            // (and, on the launch screen, its own button). Presented at the shell so
            // both entry points share one sheet and one core call.
            .sheet(isPresented: $model.isJoinByReferencePresented) {
                JoinByReferenceSheet(model: model, onClose: model.dismissJoinByReference)
            }
            // Create another community — the chooser's "Create a community" row.
            .sheet(isPresented: createCommunityBinding) {
                CreateCommunitySheet(model: model, onClose: model.dismissCreateCommunity)
            }
    }

    private var createCommunityBinding: Binding<Bool> {
        Binding(
            get: { model.isCreateCommunityRequested },
            set: { if !$0 { model.dismissCreateCommunity() } }
        )
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
                // Custom sidebar rows instead of a List: the macOS List selection
                // highlight follows the system accent (blue) and can't be retinted
                // reliably, so we paint our own pink-on-select rows in the app's
                // own palette — fully coherent, no system accent.
                VStack(spacing: 4) {
                    ForEach(RiotDestination.phoneTabs) { destination in
                        let selected = navigation.destination == destination
                        Button { changeRoute(to: destination) } label: {
                            Label(destination.title, systemImage: destination.systemImage)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(.horizontal, 10)
                                .padding(.vertical, 7)
                                .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(selected ? RiotTheme.paper(for: colorScheme) : RiotTheme.ink(for: colorScheme))
                        .background(selected ? RiotTheme.pink(for: colorScheme) : Color.clear)
                        .clipShape(RoundedRectangle(cornerRadius: 6))
                        .accessibilityIdentifier("route-\(destination.rawValue)")
                        .accessibilityAddTraits(selected ? .isSelected : [])
                    }
                }
                .padding(8)
                Spacer()
                Divider()
                identityFooter
                    .padding(12)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RiotTheme.paper(for: colorScheme))
            .navigationTitle(community.name)
        } detail: {
            Group {
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
            // The route/tool views set their own paper on iOS via phoneShell;
            // on macOS the split-view detail is system-white unless we paint it.
            // Hide the scroll surface so the paper shows; constrain the content to
            // a readable centered column so it reads like a feed, not a stretched
            // phone; and back the whole pane paper.
            .scrollContentBackground(.hidden)
            .frame(maxWidth: 760)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(RiotTheme.paper(for: colorScheme).ignoresSafeArea())
        }
        // Kill the macOS system-blue accent everywhere else it might leak.
        .tint(RiotTheme.pink(for: colorScheme))
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
    /// contract), a connection bar, and the bottom tab bar. Opening a tool is a
    /// navigation PUSH onto the Tools stack — so the community header and the tab
    /// bar (both outside every route's stack) stay on screen and the person never
    /// loses context. A tool never covers the shell as a bare full-screen sheet.
    private var phoneShell: some View {
        VStack(spacing: 0) {
            communityHeader
            ZStack {
                ForEach(RiotDestination.phoneTabs) { destination in
                    NavigationStack {
                        routeView(destination)
                            // The running tool lives on the Tools stack only, so
                            // "Open" always lands under Tools with a "‹ Tools"
                            // back button, wherever it was invoked from.
                            .navigationTitle(destination == .tools ? "Tools" : "")
                            .navigationDestination(
                                item: destination == .tools ? toolNavigation : .constant(nil)
                            ) { tool in
                                toolHost(tool)
                            }
                    }
                    .opacity(navigation.destination == destination ? 1 : 0)
                    .allowsHitTesting(navigation.destination == destination)
                }
            }
            RiotTabBar(selection: tabSelection, unreadBadges: unreadBadges)
        }
        .background(RiotTheme.paper(for: colorScheme).ignoresSafeArea())
    }

    /// The per-tab unread badges. Home carries the newswire's unread count, but
    /// only while the reader is somewhere else — a "come back, there's new" cue,
    /// not a badge on the screen you are already looking at (Home marks itself seen
    /// on appear). Other routes carry none yet.
    private var unreadBadges: [RiotDestination: Int] {
        let homeUnread = navigation.destination == .home ? 0 : newswire.unread.count
        return homeUnread > 0 ? [.home: homeUnread] : [:]
    }

    /// Drives the tool push on the Tools stack. Tapping the automatic back button
    /// clears `runningTool`; routing that through this binding also runs the focus
    /// + session teardown, so a back tap and a programmatic close (invalidation,
    /// a missing repository) share the one close path.
    private var toolNavigation: Binding<RiotSpaceApp?> {
        Binding(
            get: { runningTool },
            set: { newValue in
                if newValue == nil, runningTool != nil { _ = focus.close() }
                runningTool = newValue
            }
        )
    }

    @ViewBuilder
    private func toolHost(_ tool: RiotSpaceApp) -> some View {
        if let repository = model.profileRepository {
            AppRuntimeView(
                repository: repository,
                appIDHex: tool.appIDHex,
                appName: tool.name,
                onClose: { toolNavigation.wrappedValue = nil }
            )
        } else {
            Color.clear.onAppear { closeTool() }
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

            Spacer()

            // Community switcher — icon only. The community name is shown once,
            // in the designed Home header, so this top row carries just the
            // controls. Keeps the `community-name` id the nav tests tap.
            Button { model.openCommunityChooser() } label: {
                Image(systemName: "rectangle.stack")
                    .font(.system(size: 20))
            }
            .accessibilityLabel("Your communities")
            .accessibilityIdentifier("community-name")

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

    #endif

    // MARK: Routes

    @ViewBuilder
    private func routeView(_ destination: RiotDestination) -> some View {
        switch destination {
        case .home:
            HomeRouteView(
                model: model,
                composer: composer,
                newswire: newswire,
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
        // Always surface the tool under Tools so it inherits that tab's context
        // (community header + tab bar stay put). A Home shortcut and the Tools
        // list therefore open a tool the same way — never a contextless cover.
        model.select(.tools)
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

    // MARK: Keyboard: Command-1…4 and Command-K

    private var keyboardShortcuts: some View {
        Group {
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
            // Command-K focuses community selection (nav design Slice 3).
            Button("") { model.openCommunityChooser() }
                .keyboardShortcut(
                    KeyEquivalent(CommunitySelectionShortcut.keyEquivalent),
                    modifiers: .command
                )
                .frame(width: 0, height: 0)
                .opacity(0)
                .accessibilityIdentifier(CommunitySelectionShortcut.accessibilityID)
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
    @ObservedObject var newswire: NewswireSurfaceModel
    let onOpenTool: (RiotSpaceApp, String) -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var showRejoinSheet = false

    private var shortcuts: [RiotSpaceApp] { HomeShortcuts.deterministic(from: model.apps) }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                shortcutsCard
                // The collective newswire — Front page, Open wire, and the
                // always-public Editorial history — is the answer to "what is
                // happening here?" It reads the same core projection every platform
                // does, so a reader sees the identical front page as its peers.
                // The offlineStale forward paths lead somewhere real: rejoin with a
                // link (Unit 1's sheet) or sync with a peer (the existing Nearby
                // screen) — never a dead no-op button and never a silent retry loop.
                NewswireSurfaceView(
                    model: newswire,
                    onSyncWithPeer: { model.select(.nearby) },
                    onRejoinWithLink: { showRejoinSheet = true }
                )
                // The single Home entry point for this community's signed alerts —
                // the only tappable alert surface (the Nearby count is a diagnostic).
                AlertsListView(entries: model.entries,
                               activeNamespaceID: model.space?.namespaceID ?? "",
                               displayName: { model.rendered(for: $0) })
                PostUpdateView(model: composer)
            }
            .padding(20)
        }
        // The persistent top bar already names the community; Home names the
        // PLACE within it ("what is happening here?") so the community name is
        // not printed twice on the same screen.
        .riotHeader(eyebrow: "Community", model.space?.title ?? "Home")
        .sheet(isPresented: $showRejoinSheet) {
            JoinByReferenceSheet(model: model, onClose: { showRejoinSheet = false })
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

/// Create another community from inside the app — the chooser's "Create a community"
/// row. Mirrors the onboarding create card and calls the same `createCommunity`, so
/// the in-app and first-run create paths are one flow, never a dead no-op.
private struct CreateCommunitySheet: View {
    @ObservedObject var model: RiotAppModel
    let onClose: () -> Void
    @State private var communityName = ""

    private var trimmed: String { communityName.trimmingCharacters(in: .whitespacesAndNewlines) }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    RiotCard {
                        VStack(alignment: .leading, spacing: 12) {
                            Text("Name your community")
                                .font(.riot(.body, size: 17, relativeTo: .body))
                            TextField("Community name", text: $communityName)
                                .font(.riot(.body, size: 17, relativeTo: .body))
                                .accessibilityIdentifier("create-community-name-field")
                            Button("Create a community") {
                                model.createCommunity(
                                    CommunityCreationRequest(
                                        name: trimmed,
                                        editorialRoster: model.me.map { [$0.id] } ?? []
                                    )
                                )
                                if model.errorMessage == nil { onClose() }
                            }
                            .buttonStyle(.riotPrimary)
                            .disabled(trimmed.isEmpty)
                            .accessibilityIdentifier("create-community-confirm")
                        }
                    }
                }
                .padding(20)
            }
            .riotHeader(eyebrow: "Community", "Create a community")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel", action: onClose)
                        .accessibilityIdentifier("create-community-cancel")
                }
            }
        }
    }
}

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
    @State private var isSharePresented = false

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

                Button("Share this community") { isSharePresented = true }
                    .buttonStyle(.riotSecondary)
                    .accessibilityIdentifier("share-community")

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
        .sheet(isPresented: $isSharePresented) {
            ShareCommunitySheet(
                community: community,
                resolveEncoded: { id in
                    guard let repository = model.profileRepository else {
                        throw RepositoryError.profileClosed
                    }
                    return try repository.newswireShareReference(spaceDescriptorEntryID: id).encoded
                },
                onClose: { isSharePresented = false }
            )
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

private struct UnavailableWireProjector: NewswireProjecting {
    func projectNewswire(spaceDescriptorEntryID: String) throws -> NewswireProjectionView {
        throw RepositoryError.profileClosed
    }
}

private struct UnavailableEditor: NewswireEditorialActing, NewswireEditorAuthorityChecking {
    func createNewswireEditorialAction(
        spaceDescriptorEntryID: String,
        targetEntryID: String,
        kind: NewswireEditorialActionKind,
        reason: String?,
        correctionText: String?
    ) throws -> NewswireSignedRecord {
        throw RepositoryError.profileClosed
    }

    func newswireIsEditor(spaceDescriptorEntryID: String, subjectID: String) throws -> Bool {
        throw RepositoryError.profileClosed   // no live profile ⇒ never an editor (load() maps to false)
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
                        LabeledContent("Alerts on this device", value: "\(model.entries.count)")
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
