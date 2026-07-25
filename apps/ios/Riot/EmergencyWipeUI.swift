import SwiftUI

/// Copy for the wipe, in one place so the gesture, the settings button, and the
/// countdown say the same thing. Deliberately plain: someone reaching for this
/// is not in a mood to decode a euphemism.
public enum EmergencyWipeCopy {
    public static let settingsLabel = "Emergency wipe"
    public static let settingsExplanation =
        "Destroys this device's copy of the community and the identity that signs for it. "
        + "Other people keep what they already received. This cannot be undone once the "
        + "countdown finishes."
    public static let confirmTitle = "Wipe everything on this device?"
    public static let confirmAction = "Wipe now"
    public static let bannerTitle = "Wiping"
    public static let undo = "Undo"
    public static let doneTitle = "Wiped"
    public static let doneBody =
        "This device no longer holds the community or your identity for it."

    public static func countdown(_ seconds: Int) -> String {
        "Erasing in \(seconds)s"
    }
}

/// The countdown, shown over everything while the undo window runs.
///
/// It says "Erasing", not "About to erase": the identity key is ALREADY gone by
/// the time this appears. Undo restores it; the countdown governs the files.
public struct EmergencyWipeBanner: View {
    @ObservedObject var controller: EmergencyWipeController
    @Environment(\.colorScheme) private var colorScheme

    public init(controller: EmergencyWipeController) {
        self.controller = controller
    }

    public var body: some View {
        switch controller.state {
        case .idle:
            EmptyView()
        case let .counting(secondsRemaining):
            banner {
                HStack(spacing: 12) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(EmergencyWipeCopy.bannerTitle)
                            .font(.riot(.mono, size: 12, relativeTo: .caption))
                            .textCase(.uppercase)
                            .tracking(0.5)
                        Text(EmergencyWipeCopy.countdown(secondsRemaining))
                            .font(.riot(.body, size: 17, relativeTo: .headline))
                    }
                    Spacer(minLength: 8)
                    Button(EmergencyWipeCopy.undo) { controller.undo() }
                        .buttonStyle(.riotSecondary)
                        .frame(minHeight: 44)
                        .accessibilityIdentifier("emergency-wipe-undo")
                }
            }
            .accessibilityIdentifier("emergency-wipe-banner")
        case .wiped:
            banner {
                VStack(alignment: .leading, spacing: 2) {
                    Text(EmergencyWipeCopy.doneTitle)
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .textCase(.uppercase)
                        .tracking(0.5)
                    Text(EmergencyWipeCopy.doneBody)
                        .font(.riot(.body, size: 15, relativeTo: .body))
                }
            }
            .accessibilityIdentifier("emergency-wipe-done")
        }
    }

    @ViewBuilder
    private func banner<Content: View>(@ViewBuilder content: () -> Content) -> some View {
        content()
            .foregroundStyle(RiotTheme.ink(for: colorScheme))
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RiotTheme.paper2(for: colorScheme))
            .overlay(
                Rectangle()
                    .frame(height: 2)
                    .foregroundStyle(RiotTheme.pink(for: colorScheme)),
                alignment: .top
            )
    }
}

/// The settings route: discoverable, deliberate, and confirmed.
///
/// The gesture exists for speed under pressure; this exists so the feature can
/// be FOUND — a duress affordance nobody knows about protects nobody.
public struct EmergencyWipeButton: View {
    @ObservedObject var controller: EmergencyWipeController
    @State private var confirming = false
    @Environment(\.colorScheme) private var colorScheme

    public init(controller: EmergencyWipeController) {
        self.controller = controller
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Button(EmergencyWipeCopy.settingsLabel, role: .destructive) { confirming = true }
                .buttonStyle(.riotSecondary)
                .accessibilityIdentifier("emergency-wipe-settings")
            Text(EmergencyWipeCopy.settingsExplanation)
                .font(.riot(.body, size: 13, relativeTo: .caption))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        }
        .confirmationDialog(
            EmergencyWipeCopy.confirmTitle, isPresented: $confirming, titleVisibility: .visible
        ) {
            Button(EmergencyWipeCopy.confirmAction, role: .destructive) { controller.trigger() }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text(EmergencyWipeCopy.settingsExplanation)
        }
    }
}

/// Attaches both halves of the wipe UI — the triple-tap trigger and the
/// countdown banner — or nothing at all when there is no profile to wipe.
///
/// A modifier rather than two call sites so the gesture and the banner can never
/// drift apart: a surface that can start a wipe always shows its countdown.
public struct EmergencyWipeTriggers: ViewModifier {
    let controller: EmergencyWipeController?

    public init(controller: EmergencyWipeController?) {
        self.controller = controller
    }

    public func body(content: Content) -> some View {
        if let controller {
            content
                .emergencyWipeTripleTap(controller)
                .emergencyWipeBanner(controller)
        } else {
            content
        }
    }
}

extension View {
    /// The fast route: three taps, no confirmation, no reading.
    ///
    /// UNCONFIRMED BY DESIGN. A confirmation sheet is one more thing to find and
    /// tap while someone is taking the phone from you, and the undo window is
    /// what protects against a mis-tap instead. Three taps is rare enough not to
    /// happen by accident on a scrolling surface.
    public func emergencyWipeTripleTap(_ controller: EmergencyWipeController) -> some View {
        simultaneousGesture(
            TapGesture(count: 3).onEnded { controller.trigger() }
        )
        .accessibilityAction(named: EmergencyWipeCopy.settingsLabel) { controller.trigger() }
    }

    /// Puts the countdown above everything else on the surface.
    public func emergencyWipeBanner(_ controller: EmergencyWipeController) -> some View {
        overlay(alignment: .bottom) { EmergencyWipeBanner(controller: controller) }
    }
}
