import SwiftUI
import RiotKit

/// Narrow, UUID-gated seams for deterministic UI automation. A production
/// launch cannot activate these flags accidentally because it has no run ID.
private enum RiotUIAutomationEnvironment {
    static func isEnabled(_ key: String) -> Bool {
        let environment = ProcessInfo.processInfo.environment
        guard
            environment[key] == "1",
            let runID = environment["RIOT_UI_TEST_RUN_ID"],
            UUID(uuidString: runID) != nil
        else { return false }
        return true
    }
}

/// The community-first shell (Unit 2A). Riot is organized around a community:
/// once one is selected, a person answers "what is happening here?" (Home) and
/// "what can we do together?" (Tools / People / Nearby). Before a community
/// exists — or while the profile opens, or when a retained community cannot be
/// opened — the shell shows a launch or in-place recovery surface, never a blank
/// screen. The old five debug-shaped surfaces are gone.
struct ConferenceShellView: View {
    @ObservedObject var model: RiotAppModel
    private let notifierFactory: @MainActor () -> LocalNotifier

    init(
        model: RiotAppModel,
        notifierFactory: @escaping @MainActor () -> LocalNotifier = LocalNotifier.makeDefault
    ) {
        self.model = model
        self.notifierFactory = notifierFactory
    }

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
                    CommunityShellView(
                        model: model,
                        community: community,
                        notifierFactory: notifierFactory
                    )
                        .id(community.id)
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
    @State private var displayName = ""
    @State private var demoFailure: String?
    @State private var showNameError = false
    @State private var isCreatePresented = false
    @State private var isJoinPresented = false
    @AccessibilityFocusState private var nameErrorFocused: Bool

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                RiotCard {
                    VStack(alignment: .leading, spacing: 12) {
                        eyebrow("Set up")
                        Text("Enter a community")
                            .font(.riot(.body, size: 20, relativeTo: .title3))
                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                            .accessibilityAddTraits(.isHeader)

                        TextField("Your name (optional)", text: $displayName)
                            .font(.riot(.body, size: 17, relativeTo: .body))
                            .accessibilityIdentifier("launch-display-name")
                        Text("This self-claimed name is saved on this device and shared with future peers.")
                            .font(.riot(.body, size: 13, relativeTo: .caption))
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            .accessibilityIdentifier("launch-display-name-disclosure")

                        if showNameError, let nameError = model.nameError {
                            Text(nameError)
                                .font(.riot(.body, size: 13, relativeTo: .caption))
                                .foregroundStyle(RiotTheme.pink(for: colorScheme))
                                .accessibilityIdentifier("launch-name-error")
                                .accessibilityFocused($nameErrorFocused)
                        }

                        Button("Join with a link or QR") {
                            perform(.join) { isJoinPresented = true }
                        }
                        .buttonStyle(.riotPrimary)
                        .accessibilityIdentifier("launch-join-by-reference")

                        Button("Create a community") { isCreatePresented = true }
                            .buttonStyle(.riotSecondary)
                            .accessibilityIdentifier("create-community")

                        Button("Try the Riverside demo") {
                            perform(.demo, action: loadDemoSpace)
                        }
                        .buttonStyle(.riotSecondary)
                        .accessibilityIdentifier("demo-load")

                        Text(OnboardingPresentation.nearbyNote)
                            .font(.riot(.body, size: 13, relativeTo: .caption))
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            .accessibilityIdentifier("launch-nearby-note")
                    }
                }

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
            JoinByReferenceSheet(model: model, onClose: { isJoinPresented = false })
        }
        .sheet(isPresented: $isCreatePresented) {
            OnboardingCreateCommunitySheet(
                model: model,
                displayName: displayName,
                onClose: { isCreatePresented = false }
            )
        }
        .onAppear { displayName = model.claimedName ?? "" }
    }

    private func perform(_ exit: OnboardingExit, action: @escaping () -> Void) {
        let result = OnboardingExitGate.perform(
            exit,
            displayName: displayName,
            saveName: model.setDisplayName,
            proceed: { _ in action() }
        )
        showNameError = result == .nameSaveFailed
        nameErrorFocused = showNameError
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

/// First-run creation owns only the community-name field. The optional display
/// name remains visible on setup and is saved by the same fail-closed exit gate
/// immediately before creation.
private struct OnboardingCreateCommunitySheet: View {
    @ObservedObject var model: RiotAppModel
    let displayName: String
    let onClose: () -> Void
    @Environment(\.colorScheme) private var colorScheme
    @State private var communityName = ""
    @State private var showNameError = false
    @AccessibilityFocusState private var nameErrorFocused: Bool

    private var trimmedCommunity: String {
        communityName.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                RiotCard {
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Name your community")
                            .font(.riot(.body, size: 17, relativeTo: .body))
                        Text("You’ll be its founding organizer and first editor. You can invite others later.")
                            .font(.riot(.body, size: 13, relativeTo: .caption))
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            .accessibilityIdentifier("create-community-founding-disclosure")
                        TextField("Community name", text: $communityName)
                            .font(.riot(.body, size: 17, relativeTo: .body))
                            .accessibilityIdentifier("create-community-name-field")
                        if showNameError, let nameError = model.nameError {
                            Text(nameError)
                                .font(.riot(.body, size: 13, relativeTo: .caption))
                                .foregroundStyle(RiotTheme.pink(for: colorScheme))
                                .accessibilityIdentifier("create-community-name-error")
                                .accessibilityFocused($nameErrorFocused)
                        }
                        Button("Create a community", action: create)
                            .buttonStyle(.riotPrimary)
                            .disabled(trimmedCommunity.isEmpty)
                            .accessibilityIdentifier("create-community-confirm")
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

    private func create() {
        let result = OnboardingExitGate.perform(
            .create,
            displayName: displayName,
            saveName: model.setDisplayName,
            proceed: { _ in
                model.createCommunity(
                    CommunityCreationRequest(
                        name: trimmedCommunity,
                        editorialRoster: model.me.map { [$0.id] } ?? []
                    )
                )
            }
        )
        showNameError = result == .nameSaveFailed
        nameErrorFocused = showNameError
        if result == .proceeded, model.community != nil {
            onClose()
        }
    }
}

// MARK: - Self-healing recovery notice

/// The honest, non-fatal notice shown after a self-healing open recovered
/// something. Dismissible — the recovery already happened and the app is usable;
/// this only tells the person what was set aside (never deleted). Never a dead
/// error.
/// The owner moderation sheet: author a Revoke/Tombstone at O:/mod/, review the
/// complete (untruncated) identifiers, and sign — core auto-publishes the coupled
/// heartbeat. Shown only to an owner (the model is constructed with the site's
/// sealed masthead). Mirrors the editorial-action sheet. On success it surfaces the
/// signed action + heartbeat records; their `signedBytes` are the propagation
/// payload the app hands onward (owned-namespace /mod/ has no automatic sync yet),
/// so they are shown, never silently dropped.
struct SiteModerationSheet: View {
    @ObservedObject var model: SiteModerationModel
    let onClose: () -> Void
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        composer
            .riotHeader(eyebrow: "Moderate", "Owner moderation")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Close", action: onClose) }
            }
    }

    private var composer: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Picker("Action", selection: $model.draft.kind) {
                    ForEach(SiteModerationTargetKind.allCases) { kind in
                        Text(kind.label).tag(kind)
                    }
                }
                .accessibilityIdentifier("mod-kind-picker")

                switch model.draft.kind {
                case .revoke:
                    field("Author key", text: $model.draft.authorKey, id: "mod-author-key")
                case .tombstone:
                    field("Namespace", text: $model.draft.targetNamespace, id: "mod-target-namespace")
                    field("Target entry", text: $model.draft.targetEntry, id: "mod-target-entry")
                }

                reviewCard
                signButton
                outcomeNotice
            }
            .padding(20)
        }
    }

    @ViewBuilder
    private var reviewCard: some View {
        if case let .success(review) = model.review() {
            RiotCard {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Review before signing")
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .textCase(.uppercase)
                        .tracking(1)
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    ForEach(review.rows, id: \.label) { row in
                        VStack(alignment: .leading, spacing: 3) {
                            Text(row.label)
                                .font(.riot(.mono, size: 11, relativeTo: .caption2))
                                .textCase(.uppercase)
                                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            Text(row.value)
                                .font(.riot(.mono, size: 13, relativeTo: .footnote))
                                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                                .textSelection(.enabled)
                        }
                    }
                }
            }
            .accessibilityIdentifier("mod-review")
        }
    }

    @ViewBuilder
    private var signButton: some View {
        let isReady: Bool = {
            if case .success = model.review() { return true }
            return false
        }()
        Button("Sign and publish") {
            if case .signed = model.sign() { /* stays open to show the outcome */ }
        }
        .buttonStyle(.riotPrimary)
        .frame(minHeight: 44)
        .disabled(!isReady)
        .accessibilityIdentifier("mod-sign")
    }

    @ViewBuilder
    private var outcomeNotice: some View {
        switch model.lastSignOutcome {
        case let .signed(outcome):
            VStack(alignment: .leading, spacing: 4) {
                Text("Signed. A fresh moderation heartbeat was published.")
                    .font(.riot(.body, size: 13, relativeTo: .caption))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                // The signed bytes are the propagation payload — surfaced, not dropped.
                Text("Share \(outcome.action.signedBytes.count + outcome.epoch.signedBytes.count) bytes to sync this to followers.")
                    .font(.riot(.mono, size: 11, relativeTo: .caption2))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            }
            .accessibilityIdentifier("mod-signed")
        case let .invalid(violation):
            Text(violation.message)
                .font(.riot(.body, size: 13, relativeTo: .caption))
                .foregroundStyle(RiotTheme.pink(for: colorScheme))
                .accessibilityIdentifier("mod-violation")
        case .rejected:
            Text("That action was not accepted. Your draft is kept.")
                .font(.riot(.body, size: 13, relativeTo: .caption))
                .foregroundStyle(RiotTheme.pink(for: colorScheme))
                .accessibilityIdentifier("mod-rejected")
        case .none:
            EmptyView()
        }
    }

    private func field(_ label: String, text: Binding<String>, id: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label)
                .font(.riot(.mono, size: 12, relativeTo: .caption))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            TextField(label, text: text, axis: .vertical)
                .font(.riot(.mono, size: 13, relativeTo: .footnote))
                .accessibilityIdentifier(id)
        }
    }
}

/// The composite-site read surface: the honest-degradation banner (the SAME
/// `RecoveryNoticeBanner` the shell uses) over the accountable rows. When the
/// model reports a hold — critically, `.moderationLoading` — the rows are visually
/// GATED (dimmed and non-interactive behind the banner) so not-yet-trustworthy
/// content is never presented as clean. The hold is a security control enforced by
/// the model (`isContentHeld`), independent of whether the banner is dismissed.
struct CompositeSiteReadView: View {
    private let model: CompositeSiteReadModel
    @State private var bannerDismissed = false
    @Environment(\.colorScheme) private var colorScheme

    init(model: CompositeSiteReadModel) {
        self.model = model
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            if let message = model.bannerMessage, !bannerDismissed {
                RecoveryNoticeBanner(message: message, onDismiss: { bannerDismissed = true })
                    .accessibilityIdentifier("composite-degradation-banner")
            }
            if model.items.isEmpty {
                Text("No posts on this site yet.")
                    .font(.riot(.body, size: 14, relativeTo: .footnote))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            } else {
                VStack(alignment: .leading, spacing: 10) {
                    ForEach(model.items) { row in compositeRow(row) }
                }
                .opacity(model.isContentHeld ? 0.4 : 1)
                .disabled(model.isContentHeld)
                .accessibilityIdentifier(
                    model.isContentHeld ? "composite-content-held" : "composite-content-shown")
            }
        }
        .accessibilityIdentifier("composite-site-read")
    }

    @ViewBuilder
    private func compositeRow(_ row: CompositeItemRow) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            switch row.display {
            case .ordinary:
                Text(row.tier.label)
                    .font(.riot(.body, size: 15, relativeTo: .headline))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
            case .hiddenInterstitial:
                Text(NewswireTreatmentCopy.hiddenTitle)
                    .font(.riot(.body, size: 13, relativeTo: .caption))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            case .tombstoned:
                Text(NewswireTreatmentCopy.tombstoneTitle)
                    .font(.riot(.body, size: 13, relativeTo: .caption))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            }
            Text(row.authorTag)
                .font(.riot(.mono, size: 11, relativeTo: .caption2))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityIdentifier("composite-item-\(row.id)")
    }
}

/// The honest-degradation notice banner (§4.7 recovery convention). Internal so
/// the composite-site read surface can reuse the exact same banner for its
/// moderation-loading / degraded states.
struct RecoveryNoticeBanner: View {
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
    @State private var composerPresentation: ComposerPresentationState = .closed
    @State private var transitionToken: CommunityTransitionGate.Token?
    @FocusState private var focusedComposerTrigger: ComposerOrigin?
    /// Presented when a community-change is requested with an unsaved draft.
    @State private var confirmingLeave = false

    /// Turns local store-changed events into new-content notifications: a system
    /// alert when the app is backgrounded, a subtle in-app banner when it is
    /// foregrounded. There is no server/push in this P2P app — the only trigger is
    /// a LOCAL event (accepted nearby sync, or foregrounding), both of which arrive
    /// through `AppRuntimeView.dataChangedNotification`.
    @StateObject private var notifier: LocalNotifier

    @Environment(\.colorScheme) private var colorScheme
    /// Foreground vs background — decides system-notify vs in-app banner.
    @Environment(\.scenePhase) private var scenePhase

    init(
        model: RiotAppModel,
        community: CommunityContext,
        notifierFactory: @MainActor () -> LocalNotifier
    ) {
        _model = ObservedObject(wrappedValue: model)
        _navigation = ObservedObject(wrappedValue: model.navigation)
        self.community = community
        let notifier = notifierFactory()
        _notifier = StateObject(wrappedValue: notifier)

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
            draftStore: UserDefaultsPostDraftStore(communityID: community.id),
            expectedCommunityID: community.id,
            contextProvider: model
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
            seenCursor: SeenCursorStore(),
            // Communal reply signer — the same repository, or nil (reply hidden)
            // when no profile is open.
            commenter: model.profileRepository
        ))
    }

    /// Whether there is unsaved work that must be confirmed before a community
    /// change (nav design + §4.6). A non-empty post draft is unsaved work.
    private var hasUnsavedDraft: Bool { !composer.currentDraft.isEmpty }

    var body: some View {
        adaptiveShell
            .background(keyboardShortcuts)
            // A subtle in-app banner when new content arrives while the app is on
            // screen — the foreground counterpart to a backgrounded system alert.
            .overlay(alignment: .top) { newContentBanner }
            // The one local trigger: an accepted nearby sync or a foregrounding both
            // post this. Recompute unread (reusing the seen-cursor) and let the
            // notifier decide system-notify / banner / nothing.
            .onReceive(NotificationCenter.default.publisher(for: AppRuntimeView.dataChangedNotification)) { _ in
                handleStoreChanged()
            }
            .onChange(of: model.me) { _, _ in
                composer.refreshPublishingContext()
            }
            .onChange(of: model.newswireDescriptorEntryID) { _, _ in
                composer.refreshPublishingContext()
            }
            // Leaving/switching this community cancels the old coordinator's
            // pairing, transfer, and callbacks before the shell is rebuilt for the
            // next community (nav design §"Nearby security and lifecycle").
            .onAppear { registerCommunityScope() }
            .onDisappear { unregisterCommunityScope() }
            .sheet(item: $identitySheet) { destination in
                NavigationStack {
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
            .sheet(isPresented: composerPresented) {
                NavigationStack {
                    PostUpdateView(
                        model: composer,
                        onPosted: handlePosted,
                        onDone: closeComposer
                    )
                }
            }
    }

    private var createCommunityBinding: Binding<Bool> {
        Binding(
            get: { model.isCreateCommunityRequested },
            set: { if !$0 { model.dismissCreateCommunity() } }
        )
    }

    private var composerPresented: Binding<Bool> {
        Binding(
            get: {
                if case .open = composerPresentation { return true }
                return false
            },
            set: { if !$0 { closeComposer() } }
        )
    }

    private func openComposer(_ origin: ComposerOrigin) {
        composer.refreshPublishingContext()
        composerPresentation.open(origin)
    }

    private func closeComposer() {
        let origin = composerPresentation.origin
        composerPresentation.close()
        focusedComposerTrigger = origin
    }

    private func handlePosted(_ update: PostedUpdate) {
        _ = update
        newswire.load()
        people.load()
        guard !RiotUIAutomationEnvironment.isEnabled(
            "RIOT_UI_TEST_SUPPRESS_NOTIFICATION_PERMISSION"
        ) else { return }
        Task {
            await Task.yield()
            await notifier.requestAuthorizationIfNeeded()
        }
    }

    private func registerCommunityScope() {
        guard transitionToken == nil else { return }
        nearby.onBeforeSpaceJoin = {
            model.communityTransitionGate.prepareForNearbyAdoption()
        }
        transitionToken = model.communityTransitionGate.registerPreparation(
            { preparation in prepareForCommunityTransition(preparation) },
            recover: rearmCommunityScopeCallbacks
        )
    }

    private func unregisterCommunityScope() {
        if let transitionToken {
            model.communityTransitionGate.unregister(transitionToken)
            self.transitionToken = nil
        }
        nearby.onBeforeSpaceJoin = nil
        nearby.onSpaceJoined = nil
        nearby.stop()
    }

    private func prepareForCommunityTransition(_ preparation: CommunityTransitionPreparation) {
        CommunityDraftTransition.apply(
            preparation.reason,
            persist: composer.persistDraft,
            clear: { UserDefaultsPostDraftStore(communityID: community.id).clear() }
        )
        composerPresentation.close()
        identitySheet = nil
        confirmingLeave = false
        runningTool = nil
        nearby.onBeforeSpaceJoin = nil
        nearby.onSpaceJoined = nil
        if !preparation.transportMustContinue {
            nearby.stop()
        }
    }

    private func rearmCommunityScopeCallbacks() {
        nearby.onBeforeSpaceJoin = {
            model.communityTransitionGate.prepareForNearbyAdoption()
        }
        nearby.onSpaceJoined = {
            model.refreshFromStore()
        }
    }

    /// A local store-changed event landed (accepted sync or foregrounding).
    /// Refresh the wire — which recomputes `newswire.unread` against the per-device
    /// seen cursor using the SAME what's-new logic Home draws from — then hand that
    /// fresh unread to the notifier along with the current foreground/background
    /// phase. Scoped to the selected community: that is the one wire this shell has
    /// projected. (Multi-community background notification would iterate every
    /// joined community's projection — deferred with the multi-community registry.)
    private func handleStoreChanged() {
        newswire.load()
        let phase: NotifierPhase = scenePhase == .active ? .active : .background
        Task {
            await notifier.evaluate(
                communityID: community.id,
                communityName: community.name,
                unread: newswire.unread,
                phase: phase
            )
        }
    }

    /// The foreground new-content banner: a subtle, tappable, auto-dismissing toast
    /// ("N new in <community>"). Tapping it (or a few seconds) clears it; the badge
    /// and delta on Home carry the durable cue. Nothing renders when there is no
    /// banner to show.
    @ViewBuilder
    private var newContentBanner: some View {
        if let banner = notifier.banner {
            Text(banner.text)
                .font(.riot(.mono, size: 13, relativeTo: .footnote))
                .foregroundStyle(RiotTheme.paper(for: colorScheme))
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .background(
                    Capsule().fill(RiotTheme.pink(for: colorScheme))
                )
                .padding(.top, 8)
                .shadow(radius: 6, y: 2)
                .onTapGesture { notifier.dismissBanner() }
                .transition(.move(edge: .top).combined(with: .opacity))
                .accessibilityIdentifier("new-content-banner")
                .task(id: banner.id) {
                    // Auto-dismiss after a few seconds; a route change or a tap
                    // clears it sooner. Cancelled if a newer banner replaces this one.
                    try? await Task.sleep(nanoseconds: 4_000_000_000)
                    notifier.dismissBanner()
                }
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

            Button { model.openCommunityChooser() } label: {
                HStack(spacing: 6) {
                    Text(community.name)
                        .font(.riot(.body, size: 15, relativeTo: .callout))
                        .lineLimit(1)
                    Image(systemName: "chevron.down")
                        .font(.system(size: 12, weight: .semibold))
                }
            }
            .accessibilityLabel("Your communities")
            .accessibilityIdentifier("community-name")
            .frame(minHeight: 44)

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
                newswire: newswire,
                onPostUpdate: { openComposer(.home) },
                onPostFirstUpdate: { openComposer(.emptyWire) },
                composerFocus: $focusedComposerTrigger,
                onOpenTool: openTool
            )
        case .tools:
            DirectoryView(model: model, onOpen: { openTool($0, fromCardID: "tool-\($0.appIDHex)") })
        case .people:
            PeopleView(
                model: people,
                onPostUpdate: { openComposer(.people) },
                composerFocus: $focusedComposerTrigger
            )
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
    @ObservedObject var newswire: NewswireSurfaceModel
    let onPostUpdate: () -> Void
    let onPostFirstUpdate: () -> Void
    let composerFocus: FocusState<ComposerOrigin?>.Binding
    let onOpenTool: (RiotSpaceApp, String) -> Void
    let alertClock: ActiveAlertsClock
    @Environment(\.colorScheme) private var colorScheme
    @State private var showRejoinSheet = false
    @State private var now: Date

    init(
        model: RiotAppModel,
        newswire: NewswireSurfaceModel,
        onPostUpdate: @escaping () -> Void,
        onPostFirstUpdate: @escaping () -> Void,
        composerFocus: FocusState<ComposerOrigin?>.Binding,
        onOpenTool: @escaping (RiotSpaceApp, String) -> Void,
        alertClock: ActiveAlertsClock = .live
    ) {
        _model = ObservedObject(wrappedValue: model)
        _newswire = ObservedObject(wrappedValue: newswire)
        self.onPostUpdate = onPostUpdate
        self.onPostFirstUpdate = onPostFirstUpdate
        self.composerFocus = composerFocus
        self.onOpenTool = onOpenTool
        self.alertClock = alertClock
        _now = State(initialValue: alertClock.now())
    }

    private var shortcuts: [RiotSpaceApp] { HomeShortcuts.deterministic(from: model.apps) }
    private var activeAlerts: ActiveAlertsPresentation {
        ActiveAlertsPresentation.from(
            model.entries,
            activeNamespaceID: model.space?.namespaceID ?? "",
            now: now
        )
    }
    private var sections: [HomePresentation.Section] {
        HomePresentation.sections(
            wireHasPosts: newswire.hasPosts,
            alerts: activeAlerts,
            hasTools: !shortcuts.isEmpty
        )
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if sections.contains(.activeAlerts) {
                    AlertsListView(
                        presentation: activeAlerts,
                        displayName: { model.rendered(for: $0) }
                    )
                }
                // The collective newswire — Front page, Open wire, and the
                // always-public Editorial history — is the answer to "what is
                // happening here?" It reads the same core projection every platform
                // does, so a reader sees the identical front page as its peers.
                // The offlineStale forward paths lead somewhere real: rejoin with a
                // link (Unit 1's sheet) or sync with a peer (the existing Nearby
                // screen) — never a dead no-op button and never a silent retry loop.
                if sections.contains(.post) {
                    Button("Post an update", action: onPostUpdate)
                        .buttonStyle(.riotPrimary)
                        .frame(minHeight: 44)
                        .focused(composerFocus, equals: .home)
                        .accessibilityIdentifier("home-post-update")
                }
                NewswireSurfaceView(
                    model: newswire,
                    onPostUpdate: onPostFirstUpdate,
                    onSyncWithPeer: { model.select(.nearby) },
                    onRejoinWithLink: { showRejoinSheet = true },
                    composerFocus: composerFocus
                )
                if sections.contains(.tools) {
                    shortcutsCard
                }
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
        .task(id: activeAlerts.nextExpiryDate) {
            guard let expiry = activeAlerts.nextExpiryDate else { return }
            do {
                now = try await ActiveAlertsExpiryRefresh.wait(
                    until: expiry,
                    clock: alertClock
                )
            } catch {}
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
                Button("Done", action: onClose)
                    .buttonStyle(.riotSecondary)
                    .accessibilityIdentifier("your-profile-done")
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

                Button("Done", action: onClose)
                    .buttonStyle(.riotSecondary)
                    .accessibilityIdentifier("community-settings-done")
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Community", ShellIdentityDestination.communitySettings.label)
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
        guard !RiotUIAutomationEnvironment.isEnabled(
            "RIOT_UI_TEST_DISABLE_NEARBY_AUTOSTART"
        ) else { return }
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
            "\(count) offered update\(count == 1 ? "" : "s") to review"
        case .caughtUp:
            if let count = nearby.itemsBroughtOver, count > 0 {
                "Synced · \(count) update\(count == 1 ? "" : "s") added"
            } else {
                "Synced · you both have the same updates"
            }
        case .alreadyCurrent: "Synced · nothing new to bring over"
        case .differentSpace: "They are in a different space, so nothing was shared"
        case .outOfRange: "They went out of range"
        case .failed: "The connection failed — try again"
        default: "Connected"
        }
    }

    private var previewCount: Int? {
        guard case let .preview(count, _) = nearby.state else { return nil }
        return Int(count)
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
                        Text(NearbyStrings.deviceSummary)
                            .font(.riot(.body, size: 15, relativeTo: .callout))
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        if nearby.state == .idle || nearby.state == .failed {
                            Button("Find nearby devices") {
                                nearby.findNearby(host: model.nearbySpaceHost)
                            }
                            .buttonStyle(.riotPrimary)
                            .frame(minHeight: 44)
                            .accessibilityIdentifier("nearby-find-devices")
                        } else {
                            Button(NearbyStrings.stopLabel, role: .cancel) { nearby.stop() }
                                .buttonStyle(.riotSecondary)
                                .frame(minHeight: 44)
                                .accessibilityIdentifier("nearby-stop-looking")
                        }
                        if let previewCount {
                            Button(NearbyStrings.addUpdates(previewCount)) {
                                nearby.addPreviewedContent()
                            }
                                .buttonStyle(.riotPrimary)
                                .frame(minHeight: 44)
                                .accessibilityIdentifier("nearby-add-updates")
                            Button("Not now", role: .cancel) { nearby.rejectPreviewedContent() }
                                .buttonStyle(.riotSecondary)
                                .frame(minHeight: 44)
                                .accessibilityIdentifier("nearby-reject-updates")
                        }
                    }
                }
                if !nearby.phones.isEmpty {
                    RiotCard {
                        VStack(alignment: .leading, spacing: 10) {
                            Text(NearbyStrings.devicesTitle)
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
                            Text(NearbyStrings.syncedPeopleTitle)
                                .font(.riot(.mono, size: 12, relativeTo: .caption))
                                .textCase(.uppercase)
                                .tracking(1)
                                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            Text("Open a person to review what they carry.")
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
            nearby.onBeforeSpaceJoin = {
                model.communityTransitionGate.prepareForNearbyAdoption()
            }
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
