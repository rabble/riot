import SwiftUI

/// A pulse that says *someone else just touched this*.
///
/// When a peer's edit lands on your copy of a row, a hard ring pushes out from
/// behind that row and fades, and an attribution line fades in underneath it:
/// `checked by Ana · a3f91122`. The ring is drawn BEHIND the content so it
/// never covers the thing it is pointing at.
///
/// The ring is a `Rectangle` stroke rather than a circle on purpose: every
/// border in Riot is a hard flat rectangle (`RiotCard`, `RiotBadge`,
/// `RiotButtonStyle`), a circle would read as a different app, and a row is
/// wide — a circle inscribed in it barely reaches its edges, while a rectangle
/// ring genuinely pulses *out of the row*.
///
/// Pure SwiftUI — no UIKit; these sources also compile on macOS.
public struct SyncRipple<Content: View>: View {
    @Environment(\.colorScheme) private var colorScheme

    private let attribution: String?
    private let content: Content

    /// The ripple re-fires whenever this changes. It is bumped on appear and
    /// whenever `attribution` changes — which is precisely the moment a peer's
    /// edit landed on this row.
    @State private var tick = 0

    /// - Parameters:
    ///   - attribution: the line drawn under the content, already rendered.
    ///     Build it with ``RiotPerson/checkedBy`` — never assemble a name here.
    public init(attribution: String? = nil, @ViewBuilder content: () -> Content) {
        self.attribution = attribution
        self.content = content()
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            content
                .background(alignment: .center) { ring }

            if let attribution {
                Text(attribution)
                    .font(.riot(.mono, size: 11, relativeTo: .caption2))
                    .tracking(0.5)
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    .transition(.opacity)
                    .accessibilityLabel(attribution)
            }
        }
        .animation(.easeOut(duration: 0.35), value: attribution)
        .onAppear { tick &+= 1 }
        .onChange(of: attribution) { _, _ in tick &+= 1 }
    }

    /// Scales 0.6 → 1.4 while fading 0.5 → 0 over ~0.7s, then sits invisible at
    /// rest so nothing lingers on the row between syncs.
    private var ring: some View {
        Rectangle()
            .strokeBorder(RiotTheme.pink(for: colorScheme), lineWidth: 2)
            .keyframeAnimator(initialValue: RippleFrame.resting, trigger: tick) { view, frame in
                view
                    .scaleEffect(frame.scale)
                    .opacity(frame.opacity)
            } keyframes: { _ in
                KeyframeTrack(\.scale) {
                    LinearKeyframe(0.6, duration: 0.001)
                    LinearKeyframe(1.4, duration: 0.7)
                }
                KeyframeTrack(\.opacity) {
                    LinearKeyframe(0.5, duration: 0.001)
                    LinearKeyframe(0.0, duration: 0.7)
                }
            }
            .allowsHitTesting(false)
            .accessibilityHidden(true)
    }
}

/// The animatable state of the travelling ring.
///
/// Deliberately NOT nested inside `SyncRipple`: Swift does not allow a static
/// stored property inside a generic type, and `resting` wants to be one.
struct RippleFrame {
    var scale: CGFloat
    var opacity: Double

    /// Between pulses the ring is fully transparent — it exists only while it
    /// is travelling.
    static let resting = RippleFrame(scale: 0.6, opacity: 0)
}
