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
    private let canApprove: Bool
    private let isLegacyProfile: Bool
    private let onApprove: () -> Void
    private let onCancel: () -> Void

    /// The honest sentence for someone who cannot approve, or nil when they can.
    ///
    /// Two different people land here and they need opposite advice. A member is
    /// fine — the organizer turns apps on, and that is the design. A pre-organizer
    /// ("legacy") profile is not fine, and no amount of asking will help: nothing
    /// in the app can make it an organizer, so the only true thing to say is that
    /// a new profile is needed. Saying either sentence to the other person is a lie.
    static func unavailableReason(canApprove: Bool, isLegacyProfile: Bool) -> String? {
        guard !canApprove else { return nil }
        if isLegacyProfile {
            return "This profile was made before communities had organizers, so it can’t "
                + "approve tools for this community. Start a new profile to organize one."
        }
        return "Only the organizer of this community can turn a tool on here."
    }

    /// - Parameters:
    ///   - canApprove: whether this person is the space's organizer. When false the
    ///     approve button is NOT DRAWN: a button that cannot succeed is how the
    ///     original bug felt — the tap did nothing, and said nothing.
    ///   - isLegacyProfile: whether they can never organize any space, which picks
    ///     which honest sentence is shown in the button's place.
    public init(
        app: RiotSpaceApp,
        canApprove: Bool = true,
        isLegacyProfile: Bool = false,
        onApprove: @escaping () -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.app = app
        self.canApprove = canApprove
        self.isLegacyProfile = isLegacyProfile
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
                        Text("This tool can")
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
                if let reason = Self.unavailableReason(
                    canApprove: canApprove,
                    isLegacyProfile: isLegacyProfile
                ) {
                    Text(reason)
                        .font(.riot(.body, size: 15, relativeTo: .body))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        .accessibilityIdentifier("approve-unavailable")
                } else {
                    Button("Let this community use this tool") { onApprove() }
                        .buttonStyle(.riotPrimary)
                        .accessibilityIdentifier("approve-app")
                }
                Button(canApprove ? "Not now" : "Close") { onCancel() }
                    .buttonStyle(.riotSecondary)
            }
            .padding(20)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(RiotTheme.paper(for: colorScheme).ignoresSafeArea())
    }
}
