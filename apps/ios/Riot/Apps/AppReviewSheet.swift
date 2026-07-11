import SwiftUI

/// The organizer's trust-decision moment for one app, in plain language — never
/// the words bundle, signature, namespace, or sync. Approving trusts the app for
/// everyone in the space.
///
/// Shared by both surfaces that can reach an app: the Tools list inside a space
/// and the app directory. It is the same decision either way, so it is the same
/// sheet.
public struct AppReviewSheet: View {
    @Environment(\.colorScheme) private var colorScheme
    private let app: RiotSpaceApp
    private let onApprove: () -> Void
    private let onCancel: () -> Void

    public init(
        app: RiotSpaceApp,
        onApprove: @escaping () -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.app = app
        self.onApprove = onApprove
        self.onCancel = onCancel
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text(app.name)
                    .font(.riot(.poster, size: 32, relativeTo: .largeTitle))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                Text(app.description)
                    .font(.riot(.body, size: 17, relativeTo: .body))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                RiotCard {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("This app can")
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .textCase(.uppercase)
                            .tracking(1)
                            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        ForEach(app.permissions, id: \.self) { permission in
                            Text(permission)
                                .font(.riot(.body, size: 15, relativeTo: .body))
                                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        }
                    }
                }
                Button("Let everyone in this space use this") { onApprove() }
                    .buttonStyle(.riotPrimary)
                    .accessibilityIdentifier("approve-app")
                Button("Not now") { onCancel() }
                    .buttonStyle(.riotSecondary)
            }
            .padding(20)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(RiotTheme.paper(for: colorScheme).ignoresSafeArea())
    }
}
