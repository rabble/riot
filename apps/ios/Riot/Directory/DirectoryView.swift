import SwiftUI
import UniformTypeIdentifiers

public enum ToolStrings {
    public static let emptyTitle = "No tools yet"
    public static let availableToAdd = "Available to add"
    public static let moreTools = "More tools"
    public static let retry = "Try again"
    public static let chooseCommunity = "Choose a community to see its tools"

    public static func headerEyebrow(communityTitle: String) -> String {
        communityTitle
    }

    public static func inCommunity(communityTitle: String) -> String {
        "In \(communityTitle)"
    }

    public static func emptyMessage(communityTitle: String) -> String {
        "Tools added to \(communityTitle) will appear here."
    }

    public static func inlineEmpty(communityTitle: String) -> String {
        "No tools in \(communityTitle) yet"
    }

    public static func loading(communityTitle: String) -> String {
        "Loading tools for \(communityTitle)…"
    }

    public static func recommend(communityTitle: String) -> String {
        "Recommend to \(communityTitle)"
    }

    public static func retractRecommendation(communityTitle: String) -> String {
        "Take back recommendation from \(communityTitle)"
    }

    public static func makeAvailable(communityTitle: String) -> String {
        "Make available in \(communityTitle)"
    }

    public static func retryAccessibilityLabel(communityTitle: String) -> String {
        "Try tools for \(communityTitle) again"
    }

    public static let userFacingVocabulary = [
        emptyTitle,
        availableToAdd,
        moreTools,
        retry,
    ]
}

struct DirectoryScreenState {
    let snapshot: RiotDirectorySnapshot
    let scopedError: String?

    var inlineEmptyMessage: String? {
        guard snapshot.inCommunity.isEmpty else { return nil }
        return ToolStrings.inlineEmpty(communityTitle: snapshot.communityTitle)
    }
}

private struct DirectoryReviewItem: Identifiable {
    let rowID: String
    let context: RiotDirectoryActionContext
    let app: RiotSpaceApp

    var id: String {
        "\(context.namespaceID.lowercased())-\(context.selectionGeneration)-\(rowID)"
    }
}

struct DirectoryApprovalFlowState: Equatable {
    private(set) var pendingFocusAppID: String?
    private var preservesSheetDuringRecovery = false

    mutating func record(_ result: RiotToolApprovalResult, appIDHex: String) {
        switch result {
        case .added:
            pendingFocusAppID = appIDHex
            preservesSheetDuringRecovery = false
        case .notAdded:
            pendingFocusAppID = nil
            preservesSheetDuringRecovery = false
        case .savedNeedsRestart:
            pendingFocusAppID = nil
            preservesSheetDuringRecovery = true
        }
    }

    /// Called only after the namespace changed. A failed durable reopen clears
    /// the projection to nil; that is recovery, not the person selecting another
    /// community, so its restart guidance stays on screen.
    func shouldCancelSheet(currentNamespaceID: String?) -> Bool {
        !(preservesSheetDuringRecovery && currentNamespaceID == nil)
    }

    mutating func consumeFocusOnDismiss() -> String? {
        defer { reset() }
        return pendingFocusAppID
    }

    mutating func reset() {
        pendingFocusAppID = nil
        preservesSheetDuringRecovery = false
    }
}

/// The tool shelf for one selected community. Its snapshot, header, sections,
/// and every action are captured for the same selection generation, so a
/// selection change cannot leave another community's tools under this header.
public struct DirectoryView: View {
    @ObservedObject private var model: RiotAppModel
    @ObservedObject private var navigation: RiotNavigationModel
    @StateObject private var directory = RiotDirectoryModel()
    @Environment(\.colorScheme) private var colorScheme
    @State private var reviewing: DirectoryReviewItem?
    @State private var notes: [String: String] = [:]
    @State private var isImportingManifest = false
    @State private var isImportingBundle = false
    @State private var pendingManifest: Data?
    @State private var pendingImportContext: RiotDirectoryImportContext?
    @State private var approvalFlow = DirectoryApprovalFlowState()
    @AccessibilityFocusState private var focusedToolID: String?
    private let onOpen: (RiotSpaceApp) -> Void

    public init(model: RiotAppModel, onOpen: @escaping (RiotSpaceApp) -> Void) {
        _model = ObservedObject(wrappedValue: model)
        _navigation = ObservedObject(wrappedValue: model.navigation)
        self.onOpen = onOpen
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                status
                if let snapshot = directory.snapshot {
                    scopedContent(snapshot)
                } else if directory.isLoading, let title = directory.selectedCommunityTitle {
                    ProgressView(ToolStrings.loading(communityTitle: title))
                        .font(.riot(.body, size: 15, relativeTo: .body))
                } else if directory.errorMessage == nil, !directory.isLoading {
                    Text(ToolStrings.chooseCommunity)
                        .font(.riot(.body, size: 15, relativeTo: .body))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                }
            }
            .padding(20)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(RiotTheme.paper(for: colorScheme))
        .riotHeader(
            eyebrow: directory.selectedCommunityTitle.map {
                ToolStrings.headerEyebrow(communityTitle: $0)
            },
            "Tools"
        )
        .onAppear(perform: sync)
        .onChange(of: navigation.destination) { _, destination in
            if destination == .tools {
                sync()
            } else {
                directory.clearConfirmation()
                reviewing = nil
                approvalFlow.reset()
            }
        }
        .onChange(of: model.apps) { _, _ in refresh() }
        .onChange(of: model.space) { previous, current in
            if previous?.namespaceID.caseInsensitiveCompare(current?.namespaceID ?? "")
                != .orderedSame
            {
                if approvalFlow.shouldCancelSheet(currentNamespaceID: current?.namespaceID) {
                    reviewing = nil
                    approvalFlow.reset()
                }
                pendingManifest = nil
                pendingImportContext = nil
                isImportingManifest = false
                isImportingBundle = false
                focusedToolID = nil
            }
            refresh()
        }
        .fileImporter(isPresented: $isImportingManifest, allowedContentTypes: [.data]) { result in
            guard
                let importContext = pendingImportContext,
                (try? directory.validate(importContext)) != nil,
                case let .success(url) = result,
                let bytes = Self.readSecurityScoped(url)
            else {
                resetImport()
                return
            }
            pendingManifest = bytes
            isImportingBundle = true
        }
        .fileImporter(isPresented: $isImportingBundle, allowedContentTypes: [.data]) { result in
            defer { resetImport() }
            guard let importContext = pendingImportContext,
                  (try? directory.validate(importContext)) != nil,
                  case let .success(url) = result,
                  let manifest = pendingManifest,
                  let bundle = Self.readSecurityScoped(url)
            else {
                return
            }
            guard let installed = model.installTool(manifest: manifest, bundle: bundle) else {
                return
            }
            guard let prepared = try? directory.prepareImportedTool(installed) else {
                return
            }
            reviewing = DirectoryReviewItem(
                rowID: prepared.row.appIDHex,
                context: prepared.context,
                app: prepared.app
            )
        }
        .sheet(item: $reviewing, onDismiss: {
            reviewing = nil
            if let appID = approvalFlow.consumeFocusOnDismiss() {
                DispatchQueue.main.async {
                    focusedToolID = appID
                }
            }
        }) { item in
            AppReviewSheet(
                context: item.context,
                app: item.app,
                capability: approvalCapability,
                onApprove: approve,
                onCancel: { reviewing = nil }
            )
        }
    }

    @ViewBuilder
    private func scopedContent(_ snapshot: RiotDirectorySnapshot) -> some View {
        let screenState = DirectoryScreenState(
            snapshot: snapshot,
            scopedError: directory.errorMessage
        )
        toolSection(
            title: ToolStrings.inCommunity(communityTitle: snapshot.communityTitle),
            identifier: "directory-section-in-community"
        ) {
            if let inlineEmptyMessage = screenState.inlineEmptyMessage {
                Text(inlineEmptyMessage)
                    .font(.riot(.mono, size: 13, relativeTo: .footnote))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    .accessibilityIdentifier("directory-empty-inline")
            } else {
                ForEach(snapshot.inCommunity) { row in
                    card(for: row, communityTitle: snapshot.communityTitle)
                }
            }
        }

        if !snapshot.availableToAdd.isEmpty {
            toolSection(
                title: ToolStrings.availableToAdd,
                identifier: "directory-section-available"
            ) {
                ForEach(snapshot.availableToAdd) { row in
                    card(for: row, communityTitle: snapshot.communityTitle)
                }
            }
        }

        if !snapshot.moreTools.isEmpty {
            moreToolsSection {
                ForEach(Array(snapshot.moreTools.enumerated()), id: \.offset) { _, action in
                    switch action {
                    case let .importVerifiedPair(title):
                        Button(title, action: beginImport)
                            .buttonStyle(.riotSecondary)
                            .frame(minHeight: 44)
                            .accessibilityIdentifier("directory-import-tool")
                    }
                }
            }
        }
    }

    private func moreToolsSection<Content: View>(
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Rectangle()
                .fill(RiotTheme.inkSoft(for: colorScheme))
                .frame(height: 1)
                .opacity(0.45)
                .padding(.top, 8)
            Text(ToolStrings.moreTools)
                .font(.riot(.mono, size: 14, relativeTo: .headline))
                .textCase(.uppercase)
                .tracking(1)
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                .accessibilityAddTraits(.isHeader)
            content()
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("directory-section-more-tools")
    }

    private func toolSection<Content: View>(
        title: String,
        identifier: String,
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title)
                .font(.riot(.poster, size: 24, relativeTo: .title2))
                .textCase(.uppercase)
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                .accessibilityAddTraits(.isHeader)
                .overlay(alignment: .bottomLeading) {
                    Rectangle()
                        .fill(RiotTheme.blue(for: colorScheme))
                        .frame(width: 48, height: 3)
                        .offset(y: 5)
                }
            content()
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(identifier)
    }

    @ViewBuilder
    private var status: some View {
        if let confirmation = directory.confirmation {
            RiotBadge(confirmation, stamped: true)
        }
        if let errorMessage = directory.errorMessage {
            RiotCard {
                VStack(alignment: .leading, spacing: 12) {
                    Text(errorMessage)
                        .font(.riot(.body, size: 15, relativeTo: .body))
                        .foregroundStyle(RiotTheme.pink(for: colorScheme))
                    if let title = directory.failedCommunityTitle {
                        Button(ToolStrings.retry) { directory.retry() }
                            .buttonStyle(.riotSecondary)
                            .frame(minHeight: 44)
                            .accessibilityLabel(
                                ToolStrings.retryAccessibilityLabel(communityTitle: title)
                            )
                            .accessibilityIdentifier("directory-retry")
                    }
                }
            }
        }
        if let importError = model.errorMessage, directory.errorMessage == nil {
            Text(importError)
                .font(.riot(.body, size: 15, relativeTo: .body))
                .foregroundStyle(RiotTheme.pink(for: colorScheme))
        }
    }

    private func card(for row: RiotDirectoryRow, communityTitle: String) -> some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 12) {
                Text(row.name)
                    .font(.riot(.body, size: 18, relativeTo: .headline))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                Text(row.description)
                    .font(.riot(.body, size: 15, relativeTo: .body))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                primaryAction(for: row)
                if !row.badges.isEmpty {
                    badges(row.badges)
                }
                DisclosureGroup("Details for \(row.name)") {
                    VStack(alignment: .leading, spacing: 12) {
                        LabeledContent("Version", value: row.version)
                        if !row.permissions.isEmpty {
                            permissions(row.permissions)
                        }
                        if let endorsement = row.endorsement {
                            Text(endorsement)
                                .font(.riot(.body, size: 13, relativeTo: .caption))
                                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        }
                        secondaryActions(for: row, communityTitle: communityTitle)
                    }
                    .padding(.top, 8)
                }
                .accessibilityIdentifier("directory-details-\(row.appIDHex)")
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(row.accessibilityIdentifier)
    }

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
            Text("This tool can:")
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
    private func primaryAction(for row: RiotDirectoryRow) -> some View {
        switch row.primaryAction {
        case let .open(title):
            Button(action: { open(row) }) {
                Text(title)
                    .frame(maxWidth: .infinity)
            }
                .buttonStyle(.riotPrimary)
                .frame(minHeight: 44)
                .accessibilityIdentifier("directory-open-\(row.appIDHex)")
                .accessibilityFocused($focusedToolID, equals: row.appIDHex)
        case let .add(title):
            Button(action: { add(row) }) {
                Text(title)
                    .frame(maxWidth: .infinity)
            }
                .buttonStyle(.riotPrimary)
                .frame(minHeight: 44)
                .accessibilityIdentifier("directory-add-\(row.appIDHex)")
        case let .ask(title):
            Text(title)
                .font(.riot(.body, size: 15, relativeTo: .body))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                .accessibilityIdentifier("directory-ask-\(row.appIDHex)")
        case let .unavailable(message):
            Text(message)
                .font(.riot(.body, size: 15, relativeTo: .body))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                .accessibilityIdentifier("directory-unavailable-\(row.appIDHex)")
        }
    }

    @ViewBuilder
    private func secondaryActions(
        for row: RiotDirectoryRow,
        communityTitle: String
    ) -> some View {
        if row.endorsedByMe {
            Button(ToolStrings.retractRecommendation(communityTitle: communityTitle)) {
                guard let context = row.actionContext else { return }
                directory.retract(row, context: context)
            }
            .buttonStyle(.riotSecondary)
            .frame(minHeight: 44)
            .accessibilityIdentifier("directory-retract-\(row.appIDHex)")
        } else if row.canRecommend {
            TextField("Why you recommend it (optional)", text: note(for: row))
                .font(.riot(.body, size: 15, relativeTo: .body))
            Button(ToolStrings.recommend(communityTitle: communityTitle)) {
                guard let context = row.actionContext else { return }
                directory.recommend(
                    row,
                    note: notes[row.appIDHex] ?? "",
                    context: context
                )
                notes[row.appIDHex] = ""
            }
            .buttonStyle(.riotSecondary)
            .frame(minHeight: 44)
            .accessibilityIdentifier("directory-recommend-\(row.appIDHex)")
        }

        if row.canMakeAvailable {
            Button(ToolStrings.makeAvailable(communityTitle: communityTitle)) {
                guard let context = row.actionContext else { return }
                directory.makeAvailable(row, context: context)
            }
            .buttonStyle(.riotSecondary)
            .frame(minHeight: 44)
            .accessibilityIdentifier("directory-make-available-\(row.appIDHex)")
        }
    }

    private func open(_ row: RiotDirectoryRow) {
        guard let context = row.actionContext else { return }
        do {
            onOpen(try directory.prepareOpen(row, context: context))
        } catch {
            // `prepareOpen` owns the named, retryable message and preserves the
            // last good same-community snapshot.
        }
    }

    private func add(_ row: RiotDirectoryRow) {
        guard let context = row.actionContext else { return }
        do {
            let app = try directory.prepareAdd(row, context: context)
            reviewing = DirectoryReviewItem(rowID: row.appIDHex, context: context, app: app)
        } catch {
            // `prepareAdd` owns the named, retryable message and leaves the tool
            // in Available to add.
        }
    }

    private func approve(_ context: RiotDirectoryActionContext) -> RiotToolApprovalResult {
        directory.detach()
        let result = model.approveTool(
            appID: context.appIDHex,
            expectedNamespaceID: context.namespaceID
        )
        directory.attach(port: model.profileRepository)
        refresh()

        switch result {
        case .added:
            directory.confirmAdded(context)
            approvalFlow.record(.added, appIDHex: context.appIDHex)
            return .added
        case .savedNeedsRestart:
            approvalFlow.record(result, appIDHex: context.appIDHex)
            return result
        case .notAdded:
            let named = RiotToolApprovalResult.notAdded(
                message: AppReviewSheetCopy(context: context).failure
            )
            approvalFlow.record(named, appIDHex: context.appIDHex)
            return named
        }
    }

    private var approvalCapability: RiotDirectoryApprovalCapability {
        if model.canApproveApps {
            return .organizer
        }
        return model.isLegacyProfile ? .unavailable : .member
    }

    private func sync() {
        directory.attach(port: model.profileRepository)
        refresh()
    }

    private func refresh() {
        directory.refresh(approval: approvalCapability)
    }

    private func beginImport() {
        guard let context = directory.captureImportContext() else { return }
        pendingImportContext = context
        pendingManifest = nil
        isImportingManifest = true
    }

    private func resetImport() {
        pendingManifest = nil
        pendingImportContext = nil
        isImportingManifest = false
        isImportingBundle = false
    }

    private static func readSecurityScoped(_ url: URL) -> Data? {
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        return try? Data(contentsOf: url)
    }

    private func note(for row: RiotDirectoryRow) -> Binding<String> {
        Binding(
            get: { notes[row.appIDHex] ?? "" },
            set: { notes[row.appIDHex] = $0 }
        )
    }
}
