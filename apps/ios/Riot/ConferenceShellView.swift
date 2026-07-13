import SwiftUI
import RiotKit

extension View {
    /// Mounts a running app over the shell. `fullScreenCover` is a UIKit-era API
    /// that does not exist on macOS, where a sheet is the platform's equivalent
    /// for taking over the window; the app itself (`AppRuntimeView`) is the same
    /// RiotKit view on both.
    @ViewBuilder
    func riotAppCover<Item: Identifiable, Content: View>(
        item: Binding<Item?>,
        @ViewBuilder content: @escaping (Item) -> Content
    ) -> some View {
        #if os(macOS)
        sheet(item: item, content: content)
        #else
        fullScreenCover(item: item, content: content)
        #endif
    }
}

struct ConferenceShellView: View {
    @ObservedObject var model: RiotAppModel
    /// Observed separately from `model` so a tab tap re-renders this shell only,
    /// and not the four other destination views kept alive in the ZStack below.
    /// See the performance contract on `RiotNavigationModel`.
    @ObservedObject private var navigation: RiotNavigationModel
    @Environment(\.colorScheme) private var colorScheme

    init(model: RiotAppModel) {
        _model = ObservedObject(wrappedValue: model)
        _navigation = ObservedObject(wrappedValue: model.navigation)
    }

    var body: some View {
        VStack(spacing: 0) {
            ZStack {
                ForEach(RiotDestination.phoneTabs) { destination in
                    NavigationStack {
                        destinationView(destination)
                    }
                    .opacity(navigation.destination == destination ? 1 : 0)
                    .allowsHitTesting(navigation.destination == destination)
                }
            }
            connectionDisclosureBar
            RiotTabBar(selection: $navigation.destination)
        }
        .background(RiotTheme.paper(for: colorScheme).ignoresSafeArea())
        .alert("Riot couldn’t finish that", isPresented: errorBinding) {
            Button("OK") { model.dismissError() }
        } message: {
            Text(model.errorMessage ?? "Unknown local error")
        }
    }

    private var connectionDisclosureBar: some View {
        VStack(spacing: 3) {
            // Who this person is, on every screen. The name is not printed bare —
            // `rendered` is the sanctioned `Ana · a3f91122` — and it is NOT
            // uppercased with the rest of the bar, because the half after the dot
            // is lowercase hex off their key and has to read as what it is.
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

    @ViewBuilder
    private func destinationView(_ destination: RiotDestination) -> some View {
        switch destination {
        case .spaces: SpacesView(model: model)
        case .directory: AppDirectoryTab(model: model)
        case .board: IncidentBoardView(model: model)
        case .compose: ComposeReviewSignView(model: model)
        case .connection: ConnectionStatusView(model: model)
        }
    }

    private var errorBinding: Binding<Bool> {
        Binding(
            get: { model.errorMessage != nil },
            set: { isPresented in
                if !isPresented { model.dismissError() }
            }
        )
    }
}

private struct SpacesView: View {
    @ObservedObject var model: RiotAppModel
    @Environment(\.colorScheme) private var colorScheme
    @State private var title = "Berlin Mutual Aid"
    @State private var reviewing: RiotSpaceApp?
    @State private var running: RiotSpaceApp?
    /// The name being typed, seeded from the claim this person last made so
    /// editing starts where they left off rather than from an empty field.
    @State private var name = ""
    /// Why loading or removing the seeded space did not work, in one sentence.
    @State private var demoFailure: String?

    private var trimmedName: String {
        name.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                youCard
                RiotCard {
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Public incident space")
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .textCase(.uppercase)
                            .tracking(1)
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        if let space = model.space {
                            LabeledContent("Title", value: space.title)
                            IdentifierRow(label: "Namespace", value: space.namespaceID)
                            Text("Public content · fixed incident-board/1 renderer")
                                .font(.riot(.body, size: 13, relativeTo: .caption))
                                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            if model.isDemoMode {
                                removeDemoSection
                            }
                        } else {
                            TextField("Space title", text: $title)
                                .font(.riot(.body, size: 17, relativeTo: .body))
                            Button("Create public space") { model.createSpace(title: title) }
                                .buttonStyle(.riotPrimary)
                            loadDemoSection
                        }
                        if let demoFailure {
                            Text(demoFailure)
                                .font(.riot(.body, size: 13, relativeTo: .caption))
                                .foregroundStyle(RiotTheme.pink(for: colorScheme))
                                .accessibilityIdentifier("demo-failure")
                        }
                    }
                }
                if model.space != nil {
                    toolsCard
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Riot", "Spaces")
        .sheet(item: $reviewing) { app in
            AppReviewSheet(
                app: app,
                onApprove: {
                    model.trustApp(appID: app.appIDHex)
                    reviewing = nil
                },
                onCancel: { reviewing = nil }
            )
        }
        .riotAppCover(item: $running) { app in
            if let repository = model.profileRepository {
                AppRuntimeView(
                    repository: repository,
                    appIDHex: app.appIDHex,
                    appName: app.name,
                    onClose: { running = nil }
                )
            } else {
                Color.clear.onAppear { running = nil }
            }
        }
    }

    /// The seeded Riverside Tenants Union space, offered where a person will
    /// actually find it: right under "create a space", at the moment they have
    /// none and are deciding what to do.
    ///
    /// It used to be reachable only by long-pressing a version string, which is
    /// the kind of thing you can only find if you already know it is there.
    /// Discoverable beats clever: this is a stage prop, but a stage prop nobody
    /// can pick up is just a missing feature.
    ///
    /// Offered ONLY when there is no space, because the import is additive and
    /// refuses to displace a space the person already has — a button that could
    /// only fail is not an offer.
    @ViewBuilder
    private var loadDemoSection: some View {
        Divider().overlay(RiotTheme.line(for: colorScheme))
        Text("Or start from a space that already has people in it — six alerts, a part-done checklist, and a tool in the directory.")
            .font(.riot(.body, size: 13, relativeTo: .caption))
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        Button("Load the demo space (Riverside Tenants Union)") { loadDemoSpace() }
            .buttonStyle(.riotSecondary)
            .accessibilityIdentifier("demo-load")
    }

    /// The way back out, so a rehearsal can be run twice.
    ///
    /// The copy says "hiding is not erasing" because that is the truth — the
    /// store is append-only and the entries stay in it, inert — and a presenter
    /// who expects a wipe and gets a hide will find out on stage.
    @ViewBuilder
    private var removeDemoSection: some View {
        Divider().overlay(RiotTheme.line(for: colorScheme))
        RiotBadge("Demo space")
        Text(DemoModeCopy.hideExplanation)
            .font(.riot(.body, size: 13, relativeTo: .caption))
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        Button(DemoModeCopy.hide) { hideDemoSpace() }
            .buttonStyle(.riotSecondary)
            .accessibilityIdentifier("demo-hide")
    }

    /// Imports the seeded bundle through the ORDINARY import path — the same
    /// `inspect → plan → commit` a bundle from a phone across the room takes. A
    /// missing resource and a refused import say the same sentence, because the
    /// difference matters to us and not to the person holding the phone.
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

    private func hideDemoSpace() {
        demoFailure = nil
        do {
            try model.demoLoader?.hideDemoSpace()
        } catch {
            demoFailure = "Couldn’t hide the demo space."
        }
    }

    /// "This is me." The one place a person says who they are.
    ///
    /// It leads the first screen deliberately: everything else here — a space, an
    /// alert, an app someone carried over — is signed BY someone, and until this
    /// is filled in that someone is `member · a3f91122` to every device in the
    /// room.
    ///
    /// What is echoed back is core's rendering, not what they typed. Seeing
    /// `Ana · a3f91122` (and not just "Ana") is the point: the tag is the part
    /// that actually comes from their key, and it is what keeps them apart from
    /// the second Ana in the room.
    private var youCard: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 12) {
                Text("You")
                    .font(.riot(.mono, size: 12, relativeTo: .caption))
                    .textCase(.uppercase)
                    .tracking(1)
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))

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
                    .textFieldStyle(.plain)
                    .accessibilityIdentifier("my-name-field")

                Button("Save name") { model.setDisplayName(trimmedName) }
                    .buttonStyle(.riotPrimary)
                    .disabled(trimmedName.isEmpty)
                    .accessibilityIdentifier("save-my-name")

                // Core is the only thing that judges a name, so this is core's
                // refusal put into words, not a rule re-implemented up here.
                if let nameError = model.nameError {
                    Text(nameError)
                        .font(.riot(.body, size: 13, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.pink(for: colorScheme))
                        .accessibilityIdentifier("my-name-error")
                }
            }
        }
        .onAppear { name = model.claimedName ?? "" }
        // Only fires when the stored claim actually changes — saving a name sets it
        // to what is already typed, so this never yanks the field out from under
        // someone mid-edit.
        .onChange(of: model.claimedName) { _, claimed in name = claimed ?? "" }
    }

    private var toolsCard: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 12) {
                Text("Tools")
                    .font(.riot(.mono, size: 12, relativeTo: .caption))
                    .textCase(.uppercase)
                    .tracking(1)
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                if model.apps.isEmpty {
                    Text("No tools yet.")
                        .font(.riot(.body, size: 13, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                }
                ForEach(model.apps) { app in
                    HStack {
                        Text(app.name)
                            .font(.riot(.body, size: 17, relativeTo: .body))
                        Spacer()
                        if app.trusted {
                            Button("Open") { running = app }
                                .buttonStyle(.riotPrimary)
                                .accessibilityIdentifier("open-\(app.name)")
                        } else {
                            RiotBadge("New")
                            Button("Review") { reviewing = app }
                                .buttonStyle(.riotSecondary)
                                .accessibilityIdentifier("review-\(app.name)")
                        }
                    }
                }
            }
        }
    }
}

/// Hosts the app directory and, on top of it, the runtime for an app opened from
/// there. `AppRuntimeView` re-checks trust as it mounts, so an "Open" the
/// directory offered a moment ago still cannot run an app whose trust was
/// withdrawn in between.
private struct AppDirectoryTab: View {
    @ObservedObject var model: RiotAppModel
    @State private var running: RiotSpaceApp?

    var body: some View {
        DirectoryView(model: model, onOpen: { running = $0 })
            .riotAppCover(item: $running) { app in
                if let repository = model.profileRepository {
                    AppRuntimeView(
                        repository: repository,
                        appIDHex: app.appIDHex,
                        appName: app.name,
                        onClose: { running = nil }
                    )
                } else {
                    Color.clear.onAppear { running = nil }
                }
            }
    }
}

private struct IncidentBoardView: View {
    @ObservedObject var model: RiotAppModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        Group {
            if model.entries.isEmpty {
                RiotEmptyState(
                    title: "No alerts yet",
                    message: "Create and review an alert on this device. It stays local until you explicitly sync it."
                )
            } else {
                ScrollView {
                    VStack(spacing: 12) {
                        ForEach(model.entries) { entry in
                            RiotCard {
                                VStack(alignment: .leading, spacing: 10) {
                                    Text(entry.headline)
                                        .font(.riot(.body, size: 17, relativeTo: .headline))
                                        .foregroundStyle(RiotTheme.ink(for: colorScheme))
                                    if entry.aiAssisted {
                                        RiotBadge("AI-assisted · human reviewed and signed")
                                    }
                                    Text("Created \(Date(timeIntervalSince1970: TimeInterval(entry.createdAt)), style: .relative)")
                                        .font(.riot(.mono, size: 11, relativeTo: .caption2))
                                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                                    IdentifierRow(label: "Entry", value: entry.entryID)
                                    IdentifierRow(label: "Signer", value: entry.signerID)
                                }
                            }
                        }
                    }
                    .padding(20)
                }
            }
        }
        .riotHeader(eyebrow: "Public incident space", model.space?.title ?? "Incident board")
    }
}

private struct ComposeReviewSignView: View {
    @ObservedObject var model: RiotAppModel
    @Environment(\.colorScheme) private var colorScheme
    @State private var headline = "Water available at the east entrance"
    @State private var details = "Bring a bottle. Volunteers are refilling the tank."
    @State private var aiAssisted = false

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                RiotCard {
                    VStack(alignment: .leading, spacing: 14) {
                        Text("Draft")
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .textCase(.uppercase)
                            .tracking(1)
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        TextField("Headline", text: $headline, axis: .vertical)
                            .font(.riot(.body, size: 17, relativeTo: .body))
                        TextField("What people need to know", text: $details, axis: .vertical)
                            .font(.riot(.body, size: 15, relativeTo: .body))
                            .lineLimit(4...8)
                        Toggle("Started with model assistance", isOn: $aiAssisted)
                            .tint(RiotTheme.pink(for: colorScheme))
                    }
                }
                RiotCard {
                    VStack(alignment: .leading, spacing: 14) {
                        Text("Review before posting")
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .textCase(.uppercase)
                            .tracking(1)
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            .accessibilityIdentifier("post-review")
                        Text("Posting shares this update with \(model.space?.title ?? "this community"). Review it first; only you can post it.")
                            .font(.riot(.body, size: 15, relativeTo: .callout))
                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        Button("Post update") {
                            model.sign(headline: headline, description: details, aiAssisted: aiAssisted)
                        }
                        .buttonStyle(.riotPrimary)
                        .accessibilityIdentifier("post-update")
                        .disabled(model.space == nil || headline.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    }
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Share with your community", "Post an update")
    }
}

private struct ConnectionStatusView: View {
    @ObservedObject var model: RiotAppModel
    @StateObject private var nearby = NearbyTransportController()
    @Environment(\.colorScheme) private var colorScheme
    /// The peer whose profile is open. Tapping a device opens their profile;
    /// inviting them from there starts the connection that shares your space.
    @State private var inspecting: DiscoveredPhone?
    /// A person you have already synced with, opened from the People list — this
    /// one carries a real profile identity, so their collection populates.
    @State private var inspectingPerson: RiotPerson?

    /// Everyone this device can now name except yourself: the people you have
    /// synced with. Distinct from "Devices" (a transport-level friendly name you
    /// can connect to) — these are real profile identities whose collections are
    /// attributable. Resolved from the published name map so the list updates as
    /// profiles arrive.
    private var syncedPeople: [RiotPerson] {
        guard let repository = model.profileRepository,
              let me = try? repository.me() else { return [] }
        return model.displayNames.keys
            .filter { $0.lowercased() != me.id.lowercased() }
            .compactMap { try? repository.person(idHex: $0) }
            .sorted { $0.rendered < $1.rendered }
    }

    /// Begin looking for peers, but only once the profile is open — a phone with
    /// no repository behind it would advertise and pair with nothing to announce.
    /// Safe to call more than once; it no-ops unless idle and ready.
    ///
    /// Gated on ``RiotAppModel/isProfileOpen`` rather than on `me`: the thing a
    /// pairing needs is the REPOSITORY (it is what answers `currentSpace`), and
    /// that flag is published only after the space is readable.
    private func startDiscoveryWhenReady() {
        guard model.isProfileOpen, nearby.state == .idle else { return }
        nearby.findNearby(host: model.nearbySpaceHost)
    }

    /// This phone has a space it did not have when it started looking — the
    /// organizer opens the app and only THEN taps "Create space". If its one
    /// handshake with the phone beside it has already settled "nothing to share",
    /// that space can never reach them; look again, so the two of them shake hands
    /// afresh with a space in hand this time.
    ///
    /// `findNearby` resets the session before it restarts, which is what clears the
    /// still-selected peer that was blocking auto-connect from re-dialling.
    private func reannounceSpaceIfStuck() {
        guard model.space != nil, NearbyReannounceGate.needsReannounce(state: nearby.state) else { return }
        nearby.findNearby(host: model.nearbySpaceHost)
    }

    /// What is happening with the person on the other end, in words. Reached only
    /// when there IS someone on the other end, so it never has to describe "no
    /// one" — that is the empty state's job.
    ///
    /// The count comes from the import that actually landed, so "6 things" means
    /// six things arrived, not six things were offered.
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

    /// Who this device is connected to RIGHT NOW, said plainly.
    ///
    /// The badge above can only ever describe a STATE ("All caught up"); this is
    /// the only thing on the screen that answers the question a person actually
    /// has, which is *caught up with whom*.
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
        .riotHeader(eyebrow: "Transport", "Connection")
        .sheet(item: $inspecting) { phone in
            PeerProfileView(
                model: model,
                peerName: phone.friendlyName,
                isConnected: nearby.connectedPeer == phone.friendlyName,
                onInvite: { _ in
                    // Inviting = connect to them, which shares your space so their
                    // device can join it. They still confirm on their side.
                    nearby.requestConnection(to: phone)
                    inspecting = nil
                },
                onClose: { inspecting = nil }
            )
        }
        .sheet(item: $inspectingPerson) { person in
            // A real synced identity: their rendered name is what the directory
            // attributes their apps to, so their collection actually populates.
            PeerProfileView(
                model: model,
                peerName: person.rendered,
                authoredName: person.rendered,
                onClose: { inspectingPerson = nil }
            )
        }
        .onAppear {
            // A phone that joins a peer's space gains a space, a board, and a set
            // of apps it did not have a moment ago — none of which this screen is
            // the source of. Re-read the profile when that happens.
            nearby.onSpaceJoined = { model.refreshFromStore() }
            startDiscoveryWhenReady()
        }
        // The profile opens asynchronously (`bootstrap` runs off the window's
        // `.task`), and this screen can appear first — every tab is built at
        // launch, so this `onAppear` fires even while the person is looking at
        // Spaces. Starting discovery before the profile is open makes a phone
        // advertise and pair with no identity and no space to announce — so wait
        // until it is ready, and start the moment it becomes ready.
        .onChange(of: model.isProfileOpen) { _, _ in startDiscoveryWhenReady() }
        // A space this phone did not have when it started looking: the organizer
        // opens the app, and only THEN taps "Create space". Discovery has been
        // running since launch, announcing "no space" to everyone who paired — so
        // announce the real one now, or the phone next to them can never hear
        // about the space they just made.
        //
        // This also fires on a joiner the moment it adopts a peer's space, which
        // is mid-sync and must NOT restart anything; `reannounceSpace` refuses
        // while a session is live.
        // A space this phone did not have when it started looking: the organizer
        // opens the app, and only THEN taps "Create space".
        .onChange(of: model.space) { _, _ in reannounceSpaceIfStuck() }
        .onChange(of: nearby.state) { _, state in
            // The other order: the space landed while a handshake was already in
            // flight, so that handshake announced nothing and settled — and the
            // space change above fired mid-session, where it was rightly refused.
            // Catch it here, as the session settles.
            //
            // Only `.nothingToShare`, and only with a space in hand. That is what
            // makes this terminate: it takes TWO spaceless phones to reach this
            // state, so with a space to announce the next handshake cannot land
            // back here. A `.failed` handshake is deliberately NOT retried from
            // here — it would fail, restart, and fail again, and the two phones
            // would spin against each other.
            guard case .nothingToShare = state, model.space != nil else { return }
            nearby.findNearby(host: model.nearbySpaceHost)
        }
        // This phone has no space and the one it just connected to does. Joining
        // is how a fresh phone becomes part of a community — but it is the
        // person's decision, named plainly, and nothing is joined until they make
        // it.
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
            // Headless bring-up: with RIOT_AUTO_CONFIRM=1 a phone accepts the
            // join-space step without a tap, so two instances can be driven all
            // the way through pair -> join -> sync from a script. Off by default;
            // joining a space is a deliberate act for a real person.
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
