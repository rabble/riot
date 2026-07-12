import SwiftUI

/// The rubber stamp landing on paper.
///
/// **One animation, two payoffs.** This is the *only* stamp effect in Riot. It
/// fires when an entry arrives from a peer ("that came from someone else, and
/// it just landed") and it fires when a permission is granted ("that decision
/// just became real"). Those are the same gesture — something outside you
/// committing to the page — so they get the same motion. Do not write a second
/// one; give this modifier a different `trigger` instead.
///
/// Pure SwiftUI. No UIKit here — these sources also compile into the macOS
/// target, and the *only* thing in the motion kit allowed to touch UIKit is
/// `Haptics`, behind `#if os(iOS)`.
/// Deliberately NOT generic over the trigger type. `keyframeAnimator`'s content
/// closure takes a `PlaceholderContentView<Self>`, so a generic `Self` would
/// drag a `Trigger.Type` metatype into an escaping, non-isolated closure — a
/// Swift 6 Sendable violation with no fix short of erasing the trigger. So the
/// trigger is erased here, once, and every call site stays clean.
public struct RiotStampSlam: ViewModifier {
    /// The spring named in the design spec: fast, and loose enough to overshoot
    /// like a stamp bouncing off the paper.
    public static var spring: Spring { Spring(response: 0.28, dampingRatio: 0.55) }

    @Environment(\.colorScheme) private var colorScheme

    private let trigger: AnyHashable
    private let flashes: Bool

    /// Bumped on appear and on every `trigger` change. `keyframeAnimator` runs
    /// its track once per change of this value, which is what makes the stamp
    /// re-fire rather than only playing once on first render.
    @State private var tick = 0

    public init(trigger: some Hashable, flashes: Bool = true) {
        self.trigger = AnyHashable(trigger)
        self.flashes = flashes
    }

    public func body(content: Content) -> some View {
        // Read on the main actor and captured by value. The keyframe closures
        // below are escaping and NOT main-actor-isolated, so touching `self`
        // inside them would both hop isolation to read `colorScheme` and drag
        // the generic `Trigger.Type` metatype across a Sendable boundary. A
        // `Color`, a `Bool` and a `Spring` are all Sendable; `self` is not.
        let pink = RiotTheme.pink(for: colorScheme)
        let flashes = self.flashes
        let spring = Self.spring

        return content
            .keyframeAnimator(initialValue: StampFrame.settled, trigger: tick) { view, frame in
                view
                    .overlay {
                        if flashes {
                            // A hard pink outline slapped over the content and
                            // fading out — the ink of the stamp, in the same
                            // flat, hard-bordered idiom as RiotCard / RiotBadge.
                            Rectangle()
                                .strokeBorder(pink, lineWidth: 3)
                                .opacity(frame.flash)
                                .allowsHitTesting(false)
                        }
                    }
                    .scaleEffect(frame.scale)
                    .rotationEffect(.degrees(frame.rotation))
            } keyframes: { _ in
                // scale: 1.35 (raised) → 0.94 (impact, squashed) → 1.0 (settled)
                KeyframeTrack(\.scale) {
                    LinearKeyframe(1.35, duration: 0.001)
                    SpringKeyframe(0.94, duration: 0.12, spring: spring)
                    SpringKeyframe(1.0, duration: 0.28, spring: spring)
                }
                // rotation: -3° (cocked) → 0° (square on the page)
                KeyframeTrack(\.rotation) {
                    LinearKeyframe(-3, duration: 0.001)
                    LinearKeyframe(-3, duration: 0.12)
                    SpringKeyframe(0, duration: 0.28, spring: spring)
                }
                // flash: the pink hits hard on impact, then bleeds away.
                KeyframeTrack(\.flash) {
                    LinearKeyframe(0.0, duration: 0.001)
                    LinearKeyframe(0.9, duration: 0.12)
                    LinearKeyframe(0.0, duration: 0.34)
                }
            }
            .onAppear { tick &+= 1 }
            .onChange(of: trigger) { _, _ in tick &+= 1 }
    }
}

/// The animatable state of one stamp, as three independent keyframe tracks.
///
/// Deliberately NOT nested inside `RiotStampSlam`: Swift does not allow a
/// static stored property inside a generic type, and `settled` wants to be one.
struct StampFrame {
    var scale: CGFloat
    var rotation: Double
    var flash: Double

    /// At rest the stamp is simply the content: full size, square, no ink.
    static let settled = StampFrame(scale: 1, rotation: 0, flash: 0)
}

public extension View {
    /// Slams this view down like a rubber stamp whenever `trigger` changes (and
    /// once when it first appears).
    ///
    /// Pass the identity of the thing being stamped — an entry id, a permission
    /// state — not a `Bool` you have to remember to reset.
    ///
    ///     EntryRow(entry).riotStampSlam(trigger: entry.id)          // it arrived
    ///     TrustSeal().riotStampSlam(trigger: app.isTrusted)         // it was granted
    func riotStampSlam(trigger: some Hashable, flashes: Bool = true) -> some View {
        modifier(RiotStampSlam(trigger: trigger, flashes: flashes))
    }
}
