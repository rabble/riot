import SwiftUI

/// Follow a composite indymedia site by pasting its `riot://site/v1/...` ticket or
/// (on iOS) scanning its QR, and manage the sites you already follow. Mirrors the
/// community ``JoinByReferenceSheet`` flow, but a site ticket is verified in the
/// core `follow_site` FFI (signature + expiry), so there is no local preview card —
/// a screened ticket is followed directly and the row appears in the list below.
///
/// Each followed row can pull the owner-signed bundle over HTTPS ("Refresh from
/// site") and import it — the pulled bytes are UNTRUSTED until the core re-verifies
/// every entry (owner cap + Following-gate + family-gate). A site that requires an
/// unavailable transport (`require:arti`) shows "requires Tor — unavailable" and
/// offers NO fetch button: the sheet HONORS the core's fetch-time arti gate, never
/// re-implements it.
public struct FollowSiteSheet: View {
    @ObservedObject private var model: RiotAppModel
    private let onClose: () -> Void

    private let tickets = FollowSiteModel()

    @State private var pasted = ""
    @State private var screenedTicket: String?
    @State private var errorText: String?
    @State private var refreshing: Set<String> = []
    /// Per-site "Imported N records" feedback, keyed by root, shown after a
    /// successful refresh so the pull's payoff is visible.
    @State private var lastImport: [String: Int] = [:]

    #if os(iOS)
    @State private var mode: Mode = .paste
    @State private var cameraDenied = false

    private enum Mode: String, CaseIterable, Identifiable {
        case paste = "Paste"
        case scan = "Scan"
        var id: String { rawValue }
    }
    #endif

    public init(model: RiotAppModel, onClose: @escaping () -> Void) {
        self.model = model
        self.onClose = onClose
    }

    public var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    followCard
                    if let errorText {
                        Text(errorText)
                            .font(.riot(.body, size: 13, relativeTo: .caption))
                            .foregroundStyle(.red)
                            .accessibilityIdentifier("follow-site-error")
                    }
                    followedList
                }
                .padding(20)
            }
            .riotHeader(eyebrow: "Follow", "Follow a site")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done", action: onClose)
                        .accessibilityIdentifier("follow-site-done")
                }
            }
            .onAppear { model.reloadFollowedSites() }
        }
    }

    // MARK: - Follow (paste / scan)

    private var followCard: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 10) {
                #if os(iOS)
                Picker("How", selection: $mode) {
                    ForEach(Mode.allCases) { Text($0.rawValue).tag($0) }
                }
                .pickerStyle(.segmented)
                .accessibilityIdentifier("follow-site-mode-picker")
                .onChange(of: mode) { _, _ in screenedTicket = nil; errorText = nil }

                switch mode {
                case .paste:
                    pasteField
                case .scan:
                    scanArea
                }
                #else
                // macOS has no camera target: paste only.
                pasteField
                #endif

                if screenedTicket != nil {
                    Button("Follow this site") { commitFollow() }
                        .buttonStyle(.riotPrimary)
                        .accessibilityIdentifier("follow-site-confirm")
                }
            }
        }
    }

    private var pasteField: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Paste a site ticket someone shared")
                .font(.riot(.body, size: 13, relativeTo: .caption))
                .foregroundStyle(.secondary)
            pasteTextField
                .autocorrectionDisabled()
                .accessibilityIdentifier("follow-site-field")
                .onChange(of: pasted) { _, newValue in screenPasted(newValue) }
        }
    }

    /// A ticket is never capitalized; the auto-capitalization modifier is iOS-only,
    /// so it is applied only there — macOS shares this source.
    private var pasteTextField: some View {
        let field = TextField("riot://site/v1/…", text: $pasted, axis: .vertical)
        #if os(iOS)
        return field.textInputAutocapitalization(.never)
        #else
        return field
        #endif
    }

    private func screenPasted(_ string: String) {
        guard !string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            screenedTicket = nil
            errorText = nil
            return
        }
        do {
            screenedTicket = try tickets.screen(ticket: string)
            errorText = nil
        } catch {
            screenedTicket = nil
            errorText = Self.copy(for: error)
        }
    }

    #if os(iOS)
    @ViewBuilder private var scanArea: some View {
        if cameraDenied {
            cameraRecoveryCard
        } else {
            QRScannerView(
                onScanned: { screenScanned($0) },
                onPermissionDenied: { cameraDenied = true }
            )
            .frame(height: 260)
            .clipShape(RoundedRectangle(cornerRadius: 12))
            .accessibilityIdentifier("follow-site-scanner")
            Text("Point the camera at a site's QR code. You can also paste a ticket.")
                .font(.riot(.body, size: 13, relativeTo: .caption))
                .foregroundStyle(.secondary)
        }
    }

    private var cameraRecoveryCard: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(CameraPermissionRecovery.message)
                .font(.riot(.body, size: 14, relativeTo: .body))
            if let url = CameraPermissionRecovery.settingsURL {
                Link("Open Settings", destination: url)
                    .buttonStyle(.riotSecondary)
            }
            Button("Paste a ticket instead") { mode = .paste }
                .buttonStyle(.riotSecondary)
                .accessibilityIdentifier("follow-site-use-paste")
        }
        .accessibilityIdentifier("follow-site-camera-denied")
    }

    private func screenScanned(_ string: String) {
        do {
            screenedTicket = try tickets.screen(ticket: string)
            errorText = nil
        } catch {
            screenedTicket = nil
            errorText = Self.copy(for: error)
        }
    }
    #endif

    private func commitFollow() {
        guard let ticket = screenedTicket else { return }
        model.followSite(ticket: ticket)
        if model.errorMessage == nil {
            pasted = ""
            screenedTicket = nil
            errorText = nil
        } else {
            errorText = model.errorMessage
        }
    }

    // MARK: - Followed sites list

    private var displays: [FollowedSiteDisplay] {
        model.followedSites.map(FollowedSiteDisplay.init)
    }

    @ViewBuilder private var followedList: some View {
        if displays.isEmpty {
            Text("You aren't following any sites yet. Paste a site ticket above to start.")
                .font(.riot(.body, size: 13, relativeTo: .caption))
                .foregroundStyle(.secondary)
                .accessibilityIdentifier("follow-site-empty")
        } else {
            VStack(alignment: .leading, spacing: 12) {
                Text("Following")
                    .font(.riot(.monoBold, size: 13, relativeTo: .caption))
                    .foregroundStyle(.secondary)
                ForEach(displays) { followedRow($0) }
            }
        }
    }

    private func followedRow(_ display: FollowedSiteDisplay) -> some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 8) {
                Text(display.title)
                    .font(.riot(.monoBold, size: 16, relativeTo: .headline))
                    .accessibilityIdentifier("follow-site-row-title")
                Text(display.stateLabel)
                    .font(.riot(.body, size: 12, relativeTo: .caption))
                    .foregroundStyle(.secondary)
                if display.transportBlocked {
                    Text("Requires Tor — unavailable")
                        .font(.riot(.body, size: 12, relativeTo: .caption))
                        .foregroundStyle(.orange)
                        .accessibilityIdentifier("follow-site-row-blocked")
                } else if display.canRefresh, let url = display.fetchURL {
                    if refreshing.contains(display.root) {
                        ProgressView()
                            .accessibilityIdentifier("follow-site-row-refreshing")
                    } else {
                        Button("Refresh from site") {
                            refresh(root: display.root, url: url)
                        }
                        .buttonStyle(.riotSecondary)
                        .accessibilityIdentifier("follow-site-row-refresh")
                    }
                    if let count = lastImport[display.root] {
                        Text("Imported \(count) record\(count == 1 ? "" : "s")")
                            .font(.riot(.body, size: 12, relativeTo: .caption))
                            .foregroundStyle(.secondary)
                            .accessibilityIdentifier("follow-site-row-imported")
                    }
                }
            }
        }
        .accessibilityIdentifier("follow-site-row")
    }

    private func refresh(root: String, url: String) {
        refreshing.insert(root)
        errorText = nil
        Task {
            let imported = await model.refreshFollowedSite(root: root, fetchURL: url)
            refreshing.remove(root)
            if let imported {
                lastImport[root] = imported
                errorText = nil
            } else {
                lastImport[root] = nil
                errorText = model.errorMessage
            }
        }
    }

    private static func copy(for error: Error) -> String {
        switch error as? FollowSiteError {
        case .notASiteTicket:
            return "That isn't a Riot site ticket."
        case .tooLong:
            return "That ticket is too long to be a Riot site ticket."
        case .none:
            return "That ticket isn't valid. Check that you copied all of it."
        }
    }
}
