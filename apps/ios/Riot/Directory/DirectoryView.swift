import SwiftUI
import UniformTypeIdentifiers

/// The discovery surface: every app this profile can see — built in, shared into
/// a space, or carried in by someone who synced with you — with what it does,
/// what it can reach, who recommends it, and the actions to review it, recommend
/// it, or pass it on.
///
/// Plain language only: the words install, bundle, signature, and namespace
/// never appear. Opening is handed back to the shell (`onOpen`) because mounting
/// an app is the host's trust-gated job, not this surface's.
public struct DirectoryView: View {
    @ObservedObject private var model: RiotAppModel
    /// Selection is observed explicitly because `RiotAppModel` no longer
    /// publishes it (see the performance contract on `RiotNavigationModel`).
    /// Without this the view would not re-render on a tab change and the
    /// `onChange(of: navigation.destination)` below — which is what syncs the
    /// directory when this tab becomes visible — would silently never fire.
    @ObservedObject private var navigation: RiotNavigationModel
    @StateObject private var directory = RiotDirectoryModel()
    @Environment(\.colorScheme) private var colorScheme
    @State private var reviewing: RiotSpaceApp?
    @State private var notes: [String: String] = [:]
    /// Two chained document picks — manifest, then bundle — mirroring Android's
    /// manifest-then-bundle order. `pendingManifest` carries the first pick's
    /// bytes across to the second.
    @State private var isImportingManifest = false
    @State private var isImportingBundle = false
    @State private var pendingManifest: Data?
    private let onOpen: (RiotSpaceApp) -> Void

    public init(model: RiotAppModel, onOpen: @escaping (RiotSpaceApp) -> Void) {
        _model = ObservedObject(wrappedValue: model)
        _navigation = ObservedObject(wrappedValue: model.navigation)
        self.onOpen = onOpen
    }

    public var body: some View {
        // Status (a load failure, a just-sent recommendation) renders above
        // both branches. A directory that failed to load has no rows, and
        // showing "No apps yet" there would tell the person there are no
        // apps when in truth we never managed to look.
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                status
                // Rendered in the always-visible part of the VStack, so it is the
                // Tools-route-header affordance AND the empty-state action in one:
                // an organizer with no tools yet is no longer at a dead end.
                if model.canApproveApps {
                    Button("Add a tool") { isImportingManifest = true }
                        .buttonStyle(.riotSecondary)
                        .accessibilityIdentifier("directory-add-tool")
                }
                if directory.rows.isEmpty {
                    RiotEmptyState(
                        title: "No apps yet",
                        message: "Apps your communities carry will show up here. Nothing runs until an organizer turns one on for a space."
                    )
                } else {
                    intro
                    ForEach(directory.rows) { row in
                        card(for: row)
                    }
                }
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "From your communities", "Tools")
        .onAppear(perform: sync)
        .onChange(of: navigation.destination) { _, destination in
            if destination == .tools { sync() } else { directory.clearConfirmation() }
        }
        .onChange(of: model.apps) { _, _ in directory.refresh() }
        .onChange(of: model.space) { _, _ in directory.refresh() }
        .fileImporter(isPresented: $isImportingManifest, allowedContentTypes: [.data]) { result in
            guard case let .success(url) = result, let bytes = Self.readSecurityScoped(url) else { return }
            pendingManifest = bytes
            isImportingBundle = true            // now pick the bundle
        }
        .fileImporter(isPresented: $isImportingBundle, allowedContentTypes: [.data]) { result in
            defer { pendingManifest = nil }
            guard case let .success(url) = result,
                  let manifest = pendingManifest,
                  let bundle = Self.readSecurityScoped(url) else { return }
            model.installTool(manifest: manifest, bundle: bundle)
            directory.refresh()                 // pull the new (untrusted) row into the list
        }
        .sheet(item: $reviewing) { app in
            AppReviewSheet(
                app: app,
                canApprove: model.canApproveApps,
                isLegacyProfile: model.isLegacyProfile,
                onApprove: {
                    model.trustApp(appID: app.appIDHex)
                    reviewing = nil
                    directory.refresh()
                },
                onCancel: { reviewing = nil }
            )
        }
    }

    /// Attaches the profile the first time it exists — the shell builds every tab
    /// before `bootstrap` has opened one — and recomputes the directory each time
    /// this tab is shown, so an app that just arrived is on screen.
    private func sync() {
        directory.attach(port: model.profileRepository)
        directory.refresh()
    }

    /// A `.fileImporter` URL is security-scoped: reading it outside the sandbox
    /// requires bracketing the read with start/stopAccessingSecurityScopedResource,
    /// else `Data(contentsOf:)` fails for files the app does not otherwise own.
    private static func readSecurityScoped(_ url: URL) -> Data? {
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        return try? Data(contentsOf: url)
    }

    private var intro: some View {
        Text("Every app your communities carry shows up here. Nothing runs until an organizer turns it on for a space.")
            .font(.riot(.body, size: 15, relativeTo: .callout))
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
    }

    /// Shown whether or not any rows loaded — see the note in `body`.
    @ViewBuilder private var status: some View {
        if let confirmation = directory.confirmation {
            RiotBadge(confirmation, stamped: true)
        }
        if let errorMessage = directory.errorMessage {
            Text(errorMessage)
                .font(.riot(.mono, size: 12, relativeTo: .caption))
                .foregroundStyle(RiotTheme.pink(for: colorScheme))
        }
    }

    private func card(for row: RiotDirectoryRow) -> some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 12) {
                Text("\(row.name) · \(row.version)")
                    .font(.riot(.body, size: 17, relativeTo: .headline))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                Text(row.description)
                    .font(.riot(.body, size: 15, relativeTo: .body))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                if !row.badges.isEmpty {
                    badges(row.badges)
                }
                if !row.permissions.isEmpty {
                    permissions(row.permissions)
                }
                if let endorsement = row.endorsement {
                    Text(endorsement)
                        .font(.riot(.body, size: 13, relativeTo: .caption))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                }
                actions(for: row)
            }
        }
    }

    /// Side by side when they fit, stacked when they don't — "Still arriving from
    /// your group" is too long to share a phone's width with the others.
    private func badges(_ labels: [String]) -> some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) {
                ForEach(labels, id: \.self) { RiotBadge($0) }
            }
            VStack(alignment: .leading, spacing: 8) {
                ForEach(labels, id: \.self) { RiotBadge($0) }
            }
        }
    }

    private func permissions(_ permissions: [String]) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("This app can:")
                .font(.riot(.mono, size: 12, relativeTo: .caption))
                .textCase(.uppercase)
                .tracking(1)
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            ForEach(permissions, id: \.self) { permission in
                Text("• \(permission)")
                    .font(.riot(.body, size: 15, relativeTo: .body))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
            }
        }
    }

    @ViewBuilder
    private func actions(for row: RiotDirectoryRow) -> some View {
        switch row.availability {
        case let .open(app):
            Button("Open \(row.name)") { onOpen(app) }
                .buttonStyle(.riotPrimary)
                .accessibilityIdentifier("directory-open-\(row.name)")
        case let .review(app):
            Button("Review \(row.name)") { reviewing = app }
                .buttonStyle(.riotSecondary)
                .accessibilityIdentifier("directory-review-\(row.name)")
        case .get:
            // The app is here in full, carried by someone this person synced
            // with; taking it up turns nothing on — Review still stands between
            // it and running.
            Button("Get \(row.name)") { directory.get(row) }
                .buttonStyle(.riotPrimary)
                .accessibilityIdentifier("directory-get-\(row.name)")
        case .arriving:
            Text("Still arriving from your group…")
                .font(.riot(.body, size: 13, relativeTo: .caption))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        }

        // Recommending speaks for a space that already trusts the app (design
        // spec), so it appears only once the app is on in this space. A row this
        // profile already endorsed offers the take-back instead.
        if row.endorsedByMe {
            Button("Take back recommendation") {
                directory.retract(row)
            }
            .buttonStyle(.riotSecondary)
            .accessibilityIdentifier("directory-retract-\(row.name)")
        } else if row.canRecommend {
            TextField("Why you recommend it (optional)", text: note(for: row))
                .font(.riot(.body, size: 15, relativeTo: .body))
            Button("Recommend") {
                directory.recommend(row, note: notes[row.appIDHex] ?? "")
                notes[row.appIDHex] = ""
            }
            .buttonStyle(.riotSecondary)
            .accessibilityIdentifier("directory-recommend-\(row.name)")
        }

        if row.canShare {
            Button("Share to this space") { directory.share(row) }
                .buttonStyle(.riotSecondary)
                .accessibilityIdentifier("directory-share-\(row.name)")
        }
    }

    private func note(for row: RiotDirectoryRow) -> Binding<String> {
        Binding(
            get: { notes[row.appIDHex] ?? "" },
            set: { notes[row.appIDHex] = $0 }
        )
    }
}
