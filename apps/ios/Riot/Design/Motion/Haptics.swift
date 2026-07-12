import SwiftUI

#if os(iOS)
    // THE ONE FILE in the motion kit that may import UIKit — and only behind
    // this guard. Every other file under Design/Motion is pure SwiftUI, because
    // these same sources compile into the macOS target (apps/macos) and a single
    // unguarded `import UIKit` anywhere would break that build.
    import UIKit
#endif

/// The three taps in the hand.
///
/// On macOS every one of these is a no-op. That is deliberate and it is the
/// whole point of this type: **call sites are identical on both platforms.**
/// No caller writes `#if os(iOS)` around a haptic. A Mac has nothing to buzz,
/// so the request is simply answered with silence rather than pushed back onto
/// every screen that wants to make something feel real.
public enum Haptics {
    /// A decision landing. Paired with the stamp on "Let everyone here use
    /// this" — the one solid thunk that says the permission became real.
    @MainActor
    public static func trustThunk() {
        #if os(iOS)
            let generator = UIImpactFeedbackGenerator(style: .heavy)
            generator.prepare()
            generator.impactOccurred()
        #endif
    }

    /// A sync finishing. The peer's data is all here.
    @MainActor
    public static func syncComplete() {
        #if os(iOS)
            let generator = UINotificationFeedbackGenerator()
            generator.prepare()
            generator.notificationOccurred(.success)
        #endif
    }

    /// One entry arriving from someone else. Light on purpose — six of these in
    /// a row during the sync finale must feel like rain, not like an alarm.
    @MainActor
    public static func arrival() {
        #if os(iOS)
            let generator = UIImpactFeedbackGenerator(style: .light)
            generator.prepare()
            generator.impactOccurred()
        #endif
    }
}
