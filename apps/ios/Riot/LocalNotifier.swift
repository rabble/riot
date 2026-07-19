import Foundation

#if os(iOS)
import UserNotifications
#endif

// MARK: - Local, P2P-native new-content notifications
//
// Riot has no server and no push channel — there is nothing upstream to tell a
// device "you have new mail". The only truth about new content is what LOCAL
// events surface: an accepted nearby sync, or the app coming to the foreground.
// Both already announce themselves through `AppRuntimeView.dataChangedNotification`.
//
// This file turns that signal into a notification the way a local-first app must:
// it recomputes the per-community unread count from the SAME seen-cursor the
// what's-new surface uses (`NewswireUnread`), and decides — purely — whether to
// raise a system notification (app backgrounded), a subtle in-app banner (app
// foregrounded), or nothing at all. The decision is a pure function so it is
// testable on every platform without scheduling a real notification; the
// UserNotifications scheduling is a thin, iOS-guarded effect around it.

/// The scene state the notifier cares about, reduced to the only distinction the
/// decision needs: is the app in front of the reader (`active`) or not
/// (`background`). SwiftUI's `ScenePhase.inactive` maps to `background` — we only
/// raise a system alert when the app is genuinely not on screen.
public enum NotifierPhase: Equatable, Sendable {
    case active
    case background
}

/// Whether this device may raise a system notification. A deliberately small
/// mirror of `UNAuthorizationStatus` so the pure decision never imports
/// UserNotifications and stays testable everywhere.
public enum NotifierAuthorization: Equatable, Sendable {
    case authorized
    case denied
    case notDetermined

    /// Only an explicit grant lets a system alert through; `notDetermined` is
    /// treated as "not yet, ask first", never as permission.
    var canPostSystemNotifications: Bool { self == .authorized }
}

/// What the notifier should do about a community's current unread state. A pure
/// value the effect layer turns into a scheduled request, an in-app banner, or
/// nothing. `upTo` is the newest order key announced, so the caller can advance
/// its per-community de-spam cursor to exactly what it just told the reader about.
public enum NotificationDecision: Equatable, Sendable {
    /// Raise a system notification (app backgrounded and notifications allowed).
    case systemNotify(count: Int, upTo: UInt64)
    /// Surface a subtle in-app banner (app foregrounded — never a system alert).
    case inAppBanner(count: Int, upTo: UInt64)
    /// Nothing new to announce, or the app cannot notify.
    case nothing

    /// The single decision, pure over four inputs:
    /// - `unread`: the current per-device unread state (from `NewswireUnread`).
    /// - `lastNotifiedUpTo`: the newest order key already announced for this
    ///   community, or `nil` if never — the de-spam cursor.
    /// - `phase`: foreground vs background.
    /// - `authorization`: whether system alerts are permitted.
    ///
    /// De-spam is monotonic on the newest shown order key: a projection refresh
    /// that re-shows the same posts (or older ones) is silent, so the reader gets
    /// one notification per genuinely-new batch, not one per refresh.
    public static func decide(
        unread: NewswireUnread,
        lastNotifiedUpTo: UInt64?,
        phase: NotifierPhase,
        authorization: NotifierAuthorization
    ) -> NotificationDecision {
        // Nothing newer than the reader's own seen cursor → nothing to announce.
        guard unread.hasUnread, let latest = unread.latestTimestamp else { return .nothing }
        // Already announced up to (or past) this batch → stay silent.
        if let lastNotifiedUpTo, latest <= lastNotifiedUpTo { return .nothing }
        switch phase {
        case .active:
            // On screen: never hijack with a system alert — a subtle banner.
            return .inAppBanner(count: unread.count, upTo: latest)
        case .background:
            // Off screen: a system notification, but only if the reader allowed it.
            guard authorization.canPostSystemNotifications else { return .nothing }
            return .systemNotify(count: unread.count, upTo: latest)
        }
    }
}

/// The in-app banner published when new content lands while the app is
/// foregrounded — the foreground counterpart to a system notification. A plain
/// value the shell renders as a subtle, auto-dismissing toast.
public struct NewContentBanner: Equatable, Sendable, Identifiable {
    public let communityID: String
    public let communityName: String
    public let count: Int

    public var id: String { communityID }

    /// The one line both the banner and the system-notification body read from,
    /// so the phrasing can never drift between the two surfaces.
    public var text: String { NewContentBanner.summary(count: count, community: communityName) }

    public init(communityID: String, communityName: String, count: Int) {
        self.communityID = communityID
        self.communityName = communityName
        self.count = count
    }

    /// "3 new in Springfield" / "1 new in Springfield".
    public static func summary(count: Int, community: String) -> String {
        "\(count) new in \(community)"
    }
}

/// Schedules (or suppresses) real system notifications. Abstracted so the
/// notifier's effect layer is testable without the real notification center and
/// so the platform-specific UserNotifications code lives in exactly one place.
@MainActor
public protocol SystemNotificationScheduling: AnyObject {
    /// The current authorization, read fresh each evaluation (the reader may have
    /// changed it in Settings between syncs).
    func currentAuthorization() async -> NotifierAuthorization
    /// Prompt for authorization. Called at most once, and only when undetermined.
    func requestAuthorization() async -> NotifierAuthorization
    /// Post a local notification. Reusing `identifier` per community means a newer
    /// batch REPLACES the older pending one, so the count always reflects the
    /// latest — one entry per community, not a stack that grows per post.
    func schedule(identifier: String, title: String, body: String, communityID: String)
}

/// Turns the local "store changed" signal into new-content notifications. Owns
/// the per-community de-spam cursor and the published foreground banner; delegates
/// the pure choice to `NotificationDecision.decide` and the actual scheduling to
/// an injected `SystemNotificationScheduling`.
@MainActor
public final class LocalNotifier: ObservableObject {
    /// The current foreground banner, if any. The shell observes this and shows a
    /// subtle toast; `nil` means nothing to show.
    @Published public private(set) var banner: NewContentBanner?

    private let scheduler: SystemNotificationScheduling
    /// Per community (keyed by its stable namespace id), the newest order key we
    /// have already announced. In-memory: de-spam within a run of the app, which
    /// is the window in which a projection can refresh and re-show the same posts.
    private var lastNotifiedUpTo: [String: UInt64] = [:]

    public init(scheduler: SystemNotificationScheduling) {
        self.scheduler = scheduler
    }

    /// The production notifier: the real UserNotifications center on iOS, an inert
    /// scheduler elsewhere (macOS builds the same SwiftUI but does not surface
    /// these notifications).
    public static func makeDefault() -> LocalNotifier {
        #if os(iOS)
        return LocalNotifier(scheduler: UserNotificationScheduler())
        #else
        return LocalNotifier(scheduler: InertNotificationScheduler())
        #endif
    }

    /// Ask for notification permission once, and only if the reader has not yet
    /// decided. Called at a relevant first moment (the community shell appearing —
    /// i.e. the reader already has a community), never aggressively on cold launch.
    /// A denied reader is left alone.
    public func requestAuthorizationIfNeeded() async {
        guard await scheduler.currentAuthorization() == .notDetermined else { return }
        _ = await scheduler.requestAuthorization()
    }

    /// Evaluate one community's fresh unread state and act on the decision. Safe to
    /// call on every `dataChangedNotification`: de-spam keeps repeat calls for the
    /// same batch silent. An empty community id (a community with no identity yet)
    /// is inert.
    public func evaluate(
        communityID: String,
        communityName: String,
        unread: NewswireUnread,
        phase: NotifierPhase
    ) async {
        guard !communityID.isEmpty else { return }
        let authorization = await scheduler.currentAuthorization()
        let decision = NotificationDecision.decide(
            unread: unread,
            lastNotifiedUpTo: lastNotifiedUpTo[communityID],
            phase: phase,
            authorization: authorization
        )
        switch decision {
        case .nothing:
            break
        case let .systemNotify(count, upTo):
            lastNotifiedUpTo[communityID] = upTo
            scheduler.schedule(
                identifier: "riot.newcontent.\(communityID)",
                title: communityName,
                body: NewContentBanner.summary(count: count, community: communityName),
                communityID: communityID
            )
        case let .inAppBanner(count, upTo):
            lastNotifiedUpTo[communityID] = upTo
            banner = NewContentBanner(communityID: communityID, communityName: communityName, count: count)
        }
    }

    /// Dismiss the foreground banner (the toast auto-dismisses; this is also the
    /// hook a tap or a route change uses).
    public func dismissBanner() { banner = nil }
}

#if os(iOS)
/// The production scheduler: the real UserNotifications center. Requires no
/// Info.plist usage string (unlike camera) — only an authorization request.
@MainActor
final class UserNotificationScheduler: SystemNotificationScheduling {
    func currentAuthorization() async -> NotifierAuthorization {
        let settings = await UNUserNotificationCenter.current().notificationSettings()
        switch settings.authorizationStatus {
        case .authorized, .provisional, .ephemeral:
            return .authorized
        case .denied:
            return .denied
        default:
            return .notDetermined
        }
    }

    func requestAuthorization() async -> NotifierAuthorization {
        let granted = (try? await UNUserNotificationCenter.current()
            .requestAuthorization(options: [.alert, .badge, .sound])) ?? false
        return granted ? .authorized : .denied
    }

    func schedule(identifier: String, title: String, body: String, communityID: String) {
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        // Groups this community's notifications together, and carries the id so a
        // future tap handler can deep-link to that community's Home.
        content.threadIdentifier = communityID
        content.userInfo = ["communityID": communityID]
        // nil trigger = deliver now; the reused identifier collapses to one entry.
        let request = UNNotificationRequest(identifier: identifier, content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request)
    }
}
#else
/// The non-iOS scheduler: does nothing and reports no permission, so the shared
/// SwiftUI compiles and runs on macOS without surfacing these notifications.
@MainActor
final class InertNotificationScheduler: SystemNotificationScheduling {
    func currentAuthorization() async -> NotifierAuthorization { .denied }
    func requestAuthorization() async -> NotifierAuthorization { .denied }
    func schedule(identifier: String, title: String, body: String, communityID: String) {}
}
#endif
