import SwiftUI

/// Named, goal-oriented copy for the organizer's trust decision.
struct AppReviewSheetCopy: Equatable {
    let title: String
    let confirmation: String
    let memberReason: String
    let legacyReason: String
    let failure: String

    init(context: RiotDirectoryActionContext) {
        title = "Add \(context.appName) to \(context.communityTitle)?"
        confirmation = "Add to \(context.communityTitle)"
        memberReason = "Only an organizer of \(context.communityTitle) can add this tool."
        legacyReason = "This profile can’t add tools to \(context.communityTitle). "
            + "Start a new profile to organize a community."
        failure = "Couldn’t add \(context.appName) to \(context.communityTitle). "
            + "Nothing changed. Try again."
    }
}

/// Pure approval state so duplicate suppression and dismissal are testable
/// without presenting SwiftUI.
enum AppReviewSheetSubmissionState: Equatable {
    case idle
    case adding
    case failed(message: String)
    case savedNeedsRestart(message: String)
    case succeeded

    var canSubmit: Bool {
        switch self {
        case .idle, .failed:
            true
        case .adding, .savedNeedsRestart, .succeeded:
            false
        }
    }

    var shouldDismiss: Bool {
        self == .succeeded
    }

    var isAdding: Bool {
        self == .adding
    }

    var message: String? {
        switch self {
        case let .failed(message), let .savedNeedsRestart(message):
            message
        case .idle, .adding, .succeeded:
            nil
        }
    }

    @discardableResult
    mutating func begin() -> Bool {
        guard canSubmit else { return false }
        self = .adding
        return true
    }

    mutating func finish(_ result: RiotToolApprovalResult) {
        guard self == .adding else { return }
        switch result {
        case .added:
            self = .succeeded
        case let .notAdded(message):
            self = .failed(message: message)
        case let .savedNeedsRestart(message):
            self = .savedNeedsRestart(message: message)
        }
    }
}

/// The ordered, capability-sensitive content of the sheet. Keeping permissions
/// and the approval model in one value makes it impossible for a refactor to
/// place the security explanation after the decision control unnoticed.
struct AppReviewSheetPresentation: Equatable {
    enum Element: Equatable {
        case permissions([String])
        case unavailable(String)
        case approval(title: String, isEnabled: Bool, accessibilityHint: String?)

        var isApproval: Bool {
            if case .approval = self { return true }
            return false
        }
    }

    let copy: AppReviewSheetCopy
    let elements: [Element]

    init(
        permissions: [String],
        context: RiotDirectoryActionContext,
        capability: RiotDirectoryApprovalCapability,
        submissionState: AppReviewSheetSubmissionState
    ) {
        let copy = AppReviewSheetCopy(context: context)
        self.copy = copy

        var elements: [Element] = [.permissions(permissions)]
        if case let .savedNeedsRestart(message) = submissionState {
            elements.append(
                .approval(
                    title: copy.confirmation,
                    isEnabled: false,
                    accessibilityHint: message
                )
            )
            self.elements = elements
            return
        }
        switch capability {
        case .organizer:
            elements.append(
                .approval(
                    title: submissionState.isAdding ? "Adding…" : copy.confirmation,
                    isEnabled: submissionState.canSubmit,
                    accessibilityHint: submissionState.message
                )
            )
        case .member:
            elements.append(.unavailable(copy.memberReason))
        case .unavailable:
            elements.append(.unavailable(copy.legacyReason))
        }
        self.elements = elements
    }
}

/// The organizer's trust-decision moment for one app, in plain language — never
/// the words bundle, signature, namespace, or sync. Approving adds the tool to
/// the exact community captured by `context`.
public struct AppReviewSheet: View {
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.dismiss) private var dismiss
    @AccessibilityFocusState private var approvalFocused: Bool

    private let context: RiotDirectoryActionContext
    private let app: RiotSpaceApp
    private let capability: RiotDirectoryApprovalCapability
    private let onApprove: (RiotDirectoryActionContext) -> RiotToolApprovalResult
    private let onCancel: () -> Void

    @State private var submissionState: AppReviewSheetSubmissionState = .idle

    public init(
        context: RiotDirectoryActionContext,
        app: RiotSpaceApp,
        capability: RiotDirectoryApprovalCapability,
        onApprove: @escaping (RiotDirectoryActionContext) -> RiotToolApprovalResult,
        onCancel: @escaping () -> Void
    ) {
        self.context = context
        self.app = app
        self.capability = capability
        self.onApprove = onApprove
        self.onCancel = onCancel
    }

    public var body: some View {
        let presentation = AppReviewSheetPresentation(
            permissions: app.permissions,
            context: context,
            capability: capability,
            submissionState: submissionState
        )

        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text(presentation.copy.title)
                    .font(.riot(.poster, size: 32, relativeTo: .largeTitle))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                Text(app.description)
                    .font(.riot(.body, size: 17, relativeTo: .body))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))

                ForEach(Array(presentation.elements.enumerated()), id: \.offset) { _, element in
                    elementView(element)
                }

                Button(capability == .organizer ? "Not now" : "Close") {
                    onCancel()
                    dismiss()
                }
                .buttonStyle(.riotSecondary)
            }
            .padding(20)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(RiotTheme.paper(for: colorScheme).ignoresSafeArea())
    }

    @ViewBuilder
    private func elementView(_ element: AppReviewSheetPresentation.Element) -> some View {
        switch element {
        case let .permissions(permissions):
            RiotCard {
                VStack(alignment: .leading, spacing: 8) {
                    Text("This tool can")
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .textCase(.uppercase)
                        .tracking(1)
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    ForEach(permissions, id: \.self) { permission in
                        Text(permission)
                            .font(.riot(.body, size: 15, relativeTo: .body))
                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    }
                }
            }
        case let .unavailable(reason):
            Text(reason)
                .font(.riot(.body, size: 15, relativeTo: .body))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                .accessibilityIdentifier("approve-unavailable")
        case let .approval(title, isEnabled, accessibilityHint):
            if let accessibilityHint {
                Text(accessibilityHint)
                    .font(.riot(.body, size: 15, relativeTo: .body))
                    .foregroundStyle(RiotTheme.pink(for: colorScheme))
                    .accessibilityIdentifier("approve-failure")
            }
            Button(action: approve) {
                Text(title)
                    .frame(maxWidth: .infinity)
            }
                .buttonStyle(.riotPrimary)
                .disabled(!isEnabled)
                .accessibilityIdentifier("approve-app")
                .accessibilityHint(Text(accessibilityHint ?? ""))
                .accessibilityFocused($approvalFocused)
        }
    }

    private func approve() {
        guard submissionState.begin() else { return }
        approvalFocused = false

        let result = onApprove(context)
        submissionState.finish(result)
        if result.wasAdded {
            dismiss()
        } else {
            DispatchQueue.main.async {
                approvalFocused = true
            }
        }
    }
}
