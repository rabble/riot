import SwiftUI

/// Follow a community by pasting its `riot://newswire/join/v1/...` link or (on iOS)
/// scanning its QR. The sheet decodes with ``JoinReferenceModel``, shows an HONEST
/// pre-sync preview (namespace only — the reference carries no name), and on confirm
/// commits through ``RiotAppModel/commitJoin(preview:)`` (which switches instead of
/// duplicating an already-held community). A malformed link renders actionable copy
/// and changes nothing; a denied camera falls back to paste via
/// ``CameraPermissionRecovery``.
///
/// Presented from the Launch screen and the community chooser, so both entry points
/// run the same code and the same core call.
public struct JoinByReferenceSheet: View {
    @ObservedObject private var model: RiotAppModel
    private let onClose: () -> Void

    private let references = JoinReferenceModel()

    @State private var pasted = ""
    @State private var preview: JoinPreview?
    @State private var errorText: String?

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
                    #if os(iOS)
                    Picker("How", selection: $mode) {
                        ForEach(Mode.allCases) { Text($0.rawValue).tag($0) }
                    }
                    .pickerStyle(.segmented)
                    .accessibilityIdentifier("join-mode-picker")

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

                    if let preview {
                        previewCard(preview)
                    }
                    if let errorText {
                        Text(errorText)
                            .font(.riot(.body, size: 13, relativeTo: .caption))
                            .foregroundStyle(.red)
                            .accessibilityIdentifier("join-reference-error")
                    }
                }
                .padding(20)
            }
            .riotHeader(eyebrow: "Follow", "Join with a link")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done", action: onClose)
                        .accessibilityIdentifier("join-reference-done")
                }
            }
        }
    }

    // MARK: - Paste

    private var pasteField: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 8) {
                Text("Paste a link someone shared")
                    .font(.riot(.body, size: 13, relativeTo: .caption))
                    .foregroundStyle(.secondary)
                pasteTextField
                    .autocorrectionDisabled()
                    .accessibilityIdentifier("join-reference-field")
                    .onChange(of: pasted) { _, newValue in previewPasted(newValue) }
            }
        }
    }

    /// A share reference is never capitalized; the auto-capitalization modifier is
    /// iOS-only, so it is applied only there — macOS shares this source.
    private var pasteTextField: some View {
        let field = TextField("riot://newswire/join/v1/…", text: $pasted, axis: .vertical)
        #if os(iOS)
        return field.textInputAutocapitalization(.never)
        #else
        return field
        #endif
    }

    private func previewPasted(_ string: String) {
        guard !string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            preview = nil
            errorText = nil
            return
        }
        do {
            preview = try references.preview(fromPastedString: string)
            errorText = nil
        } catch {
            preview = nil
            errorText = Self.copy(for: error)
        }
    }

    // MARK: - Scan (iOS only)

    #if os(iOS)
    @ViewBuilder private var scanArea: some View {
        if cameraDenied {
            cameraRecoveryCard
        } else {
            QRScannerView(
                onScanned: { previewScanned($0) },
                onPermissionDenied: { cameraDenied = true }
            )
            .frame(height: 260)
            .clipShape(RoundedRectangle(cornerRadius: 12))
            .accessibilityIdentifier("join-reference-scanner")
            Text("Point the camera at a community's QR code. You can also paste a link.")
                .font(.riot(.body, size: 13, relativeTo: .caption))
                .foregroundStyle(.secondary)
        }
    }

    private var cameraRecoveryCard: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 10) {
                Text(CameraPermissionRecovery.message)
                    .font(.riot(.body, size: 14, relativeTo: .body))
                if let url = CameraPermissionRecovery.settingsURL {
                    Link("Open Settings", destination: url)
                        .buttonStyle(.riotSecondary)
                }
                Button("Paste a link instead") { mode = .paste }
                    .buttonStyle(.riotSecondary)
                    .accessibilityIdentifier("join-reference-use-paste")
            }
        }
        .accessibilityIdentifier("join-reference-camera-denied")
    }

    private func previewScanned(_ string: String) {
        do {
            preview = try references.preview(fromScannedString: string)
            errorText = nil
        } catch {
            preview = nil
            errorText = Self.copy(for: error)
        }
    }
    #endif

    // MARK: - Preview + commit

    private func previewCard(_ preview: JoinPreview) -> some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 10) {
                Text("Join community")
                    .font(.riot(.monoBold, size: 17, relativeTo: .headline))
                Text(preview.shortNamespace)
                    .font(.riot(.mono, size: 12, relativeTo: .caption))
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("join-reference-namespace")
                // Honest: the reference carries no name, so we promise nothing about
                // the community's name or posts until it syncs.
                Text("Its name and posts arrive on first sync.")
                    .font(.riot(.body, size: 13, relativeTo: .caption))
                    .foregroundStyle(.secondary)
                Button("Join this community") {
                    model.commitJoin(preview: preview)
                    if model.errorMessage == nil { onClose() }
                }
                .buttonStyle(.riotPrimary)
                .accessibilityIdentifier("join-reference-confirm")
            }
        }
        .accessibilityIdentifier("join-reference-preview")
    }

    private static func copy(for error: Error) -> String {
        switch error as? JoinReferenceError {
        case .notARiotJoinLink:
            return "That isn't a Riot community link."
        case .tooLong:
            return "That link is too long to be a Riot community link."
        case .decodeFailed, .none:
            return "That link isn't valid. Check that you copied all of it."
        }
    }
}
