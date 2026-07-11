import SwiftUI
import RiotKit

struct ConferenceShellView: View {
    @ObservedObject var model: RiotAppModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        VStack(spacing: 0) {
            ZStack {
                ForEach(RiotDestination.phoneTabs) { destination in
                    NavigationStack {
                        destinationView(destination)
                    }
                    .opacity(model.destination == destination ? 1 : 0)
                    .allowsHitTesting(model.destination == destination)
                }
            }
            connectionDisclosureBar
            RiotTabBar(selection: $model.destination)
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
        case .board: IncidentBoardView(model: model)
        case .compose: ComposeReviewSignView(model: model)
        case .importPreview: ImportPreviewView(model: model)
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
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Riot", "Spaces")
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

private struct ImportPreviewView: View {
    @ObservedObject var model: RiotAppModel
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                RiotCard {
                    VStack(alignment: .leading, spacing: 10) {
                        RiotBadge("Nothing is accepted automatically")
                        Text("Nearby and file transports will place signed public entries here. You choose what enters this local space.")
                            .font(.riot(.body, size: 15, relativeTo: .callout))
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    }
                }
                if model.importEntries.isEmpty {
                    RiotEmptyState(
                        title: "No pending import",
                        message: "Incoming signed entries will appear here for you to preview before they touch your board."
                    )
                } else {
                    ForEach(model.importEntries) { entry in
                        RiotCard {
                            VStack(alignment: .leading, spacing: 8) {
                                Text(entry.headline)
                                    .font(.riot(.body, size: 16, relativeTo: .headline))
                                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                                IdentifierRow(label: "Signer", value: entry.signerID)
                            }
                        }
                    }
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Preview first", "Import preview")
    }
}

private struct ConnectionStatusView: View {
    @ObservedObject var model: RiotAppModel
    @StateObject private var nearby = NearbyTransportController()
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                RiotBadge(nearby.state.message, stamped: true)
                RiotCard {
                    VStack(alignment: .leading, spacing: 14) {
                        Text("Connections stay between nearby phones. Riot never switches this nearby session to the internet.")
                            .font(.riot(.body, size: 15, relativeTo: .callout))
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        if nearby.state == .idle || nearby.state == .failed {
                            Button("Find nearby phones") {
                                nearby.findNearby { try model.openNearbySyncBoundary() }
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
                            Text("Phones")
                                .font(.riot(.mono, size: 12, relativeTo: .caption))
                                .textCase(.uppercase)
                                .tracking(1)
                                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                            ForEach(nearby.phones) { phone in
                                Button(phone.friendlyName) { nearby.requestConnection(to: phone) }
                                    .buttonStyle(.riotSecondary)
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
        .confirmationDialog(
            nearby.state.message,
            isPresented: Binding(
                get: { if case .confirm = nearby.state { return true }; return false },
                set: { if !$0 { nearby.cancelConnection() } }
            )
        ) {
            Button("Confirm") { nearby.confirmConnection() }
            Button("Cancel", role: .cancel) { nearby.cancelConnection() }
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
