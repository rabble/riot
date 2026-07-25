import Foundation
import OSLog
import SwiftUI

/// Cancels scheduled countdown work.
public struct WipeCancellable {
    private let onCancel: () -> Void
    public init(_ onCancel: @escaping () -> Void) { self.onCancel = onCancel }
    public func cancel() { onCancel() }
}

/// The countdown's clock, injectable so tests assert the state machine
/// deterministically instead of sleeping through a real five seconds.
public protocol WipeScheduling {
    func schedule(after seconds: TimeInterval, _ work: @escaping () -> Void) -> WipeCancellable
}

/// Production clock: the main queue, since the countdown drives UI.
public struct MainQueueWipeScheduler: WipeScheduling {
    public init() {}

    public func schedule(after seconds: TimeInterval, _ work: @escaping () -> Void)
        -> WipeCancellable
    {
        let item = DispatchWorkItem(block: work)
        DispatchQueue.main.asyncAfter(deadline: .now() + seconds, execute: item)
        return WipeCancellable { item.cancel() }
    }
}

/// Drives the emergency wipe's undo window and exposes it to the UI.
///
/// The engine (`EmergencyWipe`) is deliberately timer-free — it knows how to
/// arm, undo, and commit, but not when. This controller owns the *when*, which
/// keeps both halves testable: the engine against fake stores, the countdown
/// against a fake clock.
///
/// Note what `trigger()` does NOT do: it does not wait. The identity key is
/// destroyed the instant it is called. The countdown only defers deleting
/// files, so a person who mis-tapped gets their profile back while an
/// adversary who force-quits the app still gets unrecoverable data.
@MainActor
public final class EmergencyWipeController: ObservableObject {
    public enum State: Equatable {
        case idle
        /// Key already destroyed; files still on disk until this reaches zero.
        case counting(secondsRemaining: Int)
        case wiped
    }

    /// How long the person has to take it back. Short enough to be useless to
    /// someone who has taken the phone, long enough to catch a mis-tap.
    public static let undoWindow = 5

    private static let logger = Logger(subsystem: "net.protest.riot", category: "emergency-wipe")

    @Published public private(set) var state: State = .idle

    private let wipe: EmergencyWipe
    private let scheduler: WipeScheduling
    private var tick: WipeCancellable?

    public init(wipe: EmergencyWipe, scheduler: WipeScheduling = MainQueueWipeScheduler()) {
        self.wipe = wipe
        self.scheduler = scheduler
    }

    /// Destroys the identity key now and starts the undo countdown.
    ///
    /// A repeat trigger while counting is ignored rather than re-arming: a
    /// second arm could strand the retained key and turn a recoverable mis-tap
    /// into a permanent wipe.
    public func trigger() {
        guard case .idle = state else { return }
        do {
            try wipe.arm()
        } catch {
            Self.logger.error("emergency wipe could not arm: \(String(describing: error))")
            return
        }
        state = .counting(secondsRemaining: Self.undoWindow)
        scheduleTick()
    }

    /// Takes it back, if the window has not closed.
    public func undo() {
        guard case .counting = state else { return }
        tick?.cancel()
        tick = nil
        do {
            try wipe.undo()
            state = .idle
        } catch {
            // The key could not be put back — the identity is gone for real, so
            // say so rather than showing a restored profile that cannot sign.
            Self.logger.error("emergency wipe undo failed: \(String(describing: error))")
            state = .wiped
        }
    }

    private func scheduleTick() {
        tick = scheduler.schedule(after: 1) { [weak self] in
            guard let self else { return }
            MainActor.assumeIsolated {
                guard case let .counting(remaining) = self.state else { return }
                let next = remaining - 1
                if next <= 0 {
                    self.commit()
                } else {
                    self.state = .counting(secondsRemaining: next)
                    self.scheduleTick()
                }
            }
        }
    }

    private func commit() {
        tick?.cancel()
        tick = nil
        do {
            let removed = try wipe.commit()
            Self.logger.notice("emergency wipe removed \(removed.count) paths")
        } catch {
            // Report wiped regardless: the key is destroyed, so whatever is
            // still on disk is unreadable. Claiming otherwise would be worse.
            Self.logger.error("emergency wipe commit incomplete: \(String(describing: error))")
        }
        state = .wiped
    }
}
