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
        Text(model.connectionDisclosure)
            .font(.riot(.mono, size: 11, relativeTo: .caption2))
            .textCase(.uppercase)
            .tracking(0.5)
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
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

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
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
                        } else {
                            TextField("Space title", text: $title)
                                .font(.riot(.body, size: 17, relativeTo: .body))
                            Button("Create public space") { model.createSpace(title: title) }
                                .buttonStyle(.riotPrimary)
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
    @State private var aiAssisted = true

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
                        Text("Review before signing")
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .textCase(.uppercase)
                            .tracking(1)
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        Text("Signing publishes this alert into your local public space. A model cannot press this button or sync for you.")
                            .font(.riot(.body, size: 15, relativeTo: .callout))
                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        Button("Review complete — sign locally") {
                            model.sign(headline: headline, description: details, aiAssisted: aiAssisted)
                        }
                        .buttonStyle(.riotPrimary)
                        .disabled(model.space == nil || headline.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    }
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Draft, review, sign", "Compose & sign")
    }
}

private struct ConnectionStatusView: View {
    @ObservedObject var model: RiotAppModel
    @StateObject private var nearby = NearbyTransportController()
    @Environment(\.colorScheme) private var colorScheme
    /// The peer whose profile is open. Tapping a device opens their profile;
    /// inviting them from there starts the connection that shares your space.
    @State private var inspecting: DiscoveredPhone?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                RiotBadge(nearby.state.message, stamped: true)
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
                onInvite: { _ in
                    // Inviting = connect to them, which shares your space so their
                    // device can join it. They still confirm on their side.
                    nearby.requestConnection(to: phone)
                    inspecting = nil
                },
                onClose: { inspecting = nil }
            )
        }
        .onAppear {
            // A phone that joins a peer's space gains a space, a board, and a set
            // of apps it did not have a moment ago — none of which this screen is
            // the source of. Re-read the profile when that happens.
            nearby.onSpaceJoined = { model.refreshFromStore() }
            // Look for peers as soon as this screen appears, and connect to
            // whatever we find. Nobody should have to tap to meet the phone
            // next to them.
            if nearby.state == .idle {
                nearby.findNearby(host: model.nearbySpaceHost)
            }
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
