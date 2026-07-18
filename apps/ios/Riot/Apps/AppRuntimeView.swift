import SwiftUI
import WebKit

/// The trust-gated inputs a runtime host needs to mount one app in one space.
///
/// HARD CONTRACT (platform review): Rust deliberately does NOT trust-gate
/// `app_data_put/get/list` — the WebView host is the enforcement point. Both of
/// the repository accessors below return nil for an app that is not trusted in
/// the current profile, so failing this initializer is the single place that
/// refuses to mount an untrusted (or since-revoked) app. Constructing it is the
/// launch gate; nothing downstream re-checks trust.
struct AppRuntimeLaunch {
    let bridge: AppDataBridging
    let resolver: AppResourceResolver

    /// Lowercased hex host the scheme handler matches against, and the verified
    /// entry point loaded first — both taken from the resolver so the URL we
    /// build cannot disagree with what the handler will serve.
    var appIDHex: String { resolver.appIDHex }
    var entryPoint: String { resolver.entryPoint }

    /// Returns nil unless `appIDHex` names an installed app that is trusted in
    /// the current profile. A UI that offered "Open" a moment ago but had trust
    /// revoked in between resolves to nil here and must not mount the app.
    init?(repository: RiotProfileRepository, appIDHex: String) {
        guard let bridge = repository.appDataBridge(appID: appIDHex),
              let resolver = repository.appResolver(appID: appIDHex)
        else { return nil }
        self.bridge = bridge
        self.resolver = resolver
    }
}

/// Full-screen host for one trusted app in one space. Every navigation other
/// than `riot-app://` is refused (CSP does not block top-level navigation — the
/// navigation delegate is the load-bearing lock); browser state is
/// non-persistent; the page's only I/O is the riot bridge.
public struct AppRuntimeView: View {
    /// Posted by refresh sources outside the page (foregrounding, and sync
    /// accept) to re-run the page's `watch` callbacks.
    public static let dataChangedNotification = Notification.Name("RiotAppDataChanged")

    /// Posted when a running app's execution session is invalidated mid-use
    /// (trust revoked, namespace swapped, approval changed). The host closes the
    /// app to its named destination — "Return to Tools" (§4.7) — instead of
    /// leaving it looping against a dead session.
    public static let appInvalidatedNotification = Notification.Name("RiotAppInvalidated")

    /// Fire the invalidation route. Called from the bridge when a read/commit
    /// fails because the session is no longer valid.
    public static func postAppInvalidated() {
        NotificationCenter.default.post(name: appInvalidatedNotification, object: nil)
    }

    /// Tells every app mounted right now that the store changed underneath it.
    ///
    /// The one call a refresh source makes, so that the sources — foregrounding
    /// here, an accepted sync import in `NearbyTransportController` — cannot
    /// drift apart in how they announce it.
    public static func postDataChanged() {
        NotificationCenter.default.post(name: dataChangedNotification, object: nil)
    }

    private let repository: RiotProfileRepository
    private let appIDHex: String
    private let appName: String
    private let onClose: () -> Void

    public init(
        repository: RiotProfileRepository,
        appIDHex: String,
        appName: String,
        onClose: @escaping () -> Void
    ) {
        self.repository = repository
        self.appIDHex = appIDHex
        self.appName = appName
        self.onClose = onClose
    }

    public var body: some View {
        if let launch = AppRuntimeLaunch(repository: repository, appIDHex: appIDHex) {
            AppHostView(launch: launch, appName: appName, onClose: onClose)
        } else {
            // Trust was revoked between the Tools row rendering "Open" and this
            // view constructing. Per the HARD CONTRACT we must not mount an
            // untrusted app: render nothing and dismiss.
            Color.clear.onAppear(perform: onClose)
        }
    }
}

/// The mounted-app chrome: the app's own themed surface, a subtle activity strip
/// naming who is active in it, and the sandboxed WebView. On iPhone it is hosted
/// by `MountedToolView` inside the Tools tab, which draws the "‹ Tools" back bar
/// and app name; the community header and tab bar stay on screen around it. On
/// macOS it lives in the split detail, where there is no back button, so it keeps
/// an explicit Close.
private struct AppHostView: View {
    let launch: AppRuntimeLaunch
    let appName: String
    let onClose: () -> Void

    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.colorScheme) private var colorScheme
    /// Who is active in THIS app, read from its own stored rows. Refreshed on
    /// mount, on a store-changed post, and on foreground.
    @State private var digest: AppActivityDigest = .empty

    var body: some View {
        VStack(spacing: 0) {
            #if os(macOS)
            HStack {
                Text(appName)
                    .font(.riot(.mono, size: 14, relativeTo: .body))
                    .textCase(.uppercase)
                Spacer()
                Button("Close", action: onClose)
                    .buttonStyle(.riotSecondary)
                    .accessibilityIdentifier("app-close")
            }
            .padding(12)
            #endif
            activityStrip
            AppWebView(launch: launch)
        }
        .background(RiotTheme.paper(for: colorScheme))
        .task { refreshDigest() }
        .onChange(of: scenePhase) { _, phase in
            if phase == .active { AppRuntimeView.postDataChanged() }
        }
        .onReceive(NotificationCenter.default.publisher(for: AppRuntimeView.dataChangedNotification)) { _ in
            refreshDigest()
        }
        .onReceive(NotificationCenter.default.publisher(for: AppRuntimeView.appInvalidatedNotification)) { _ in
            // §4.7: access was revoked/invalidated mid-use — return to Tools.
            onClose()
        }
    }

    /// A caption naming who is active in this app — the answer to "can I see the
    /// people while I'm in a tool?". Hidden when there is nothing to show, so an
    /// empty or single-author app never carries a bare strip.
    @ViewBuilder
    private var activityStrip: some View {
        if !digest.isEmpty {
            HStack(spacing: 6) {
                Image(systemName: "person.2.fill")
                    .font(.system(size: 11))
                    .accessibilityHidden(true)
                Text("Active: \(digest.caption())")
                    .font(.riot(.mono, size: 11, relativeTo: .caption2))
                    .lineLimit(1)
                Spacer(minLength: 0)
            }
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            .padding(.horizontal, 14)
            .padding(.vertical, 6)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RiotTheme.paper2(for: colorScheme))
            .overlay(alignment: .bottom) {
                Rectangle().fill(RiotTheme.line(for: colorScheme)).frame(height: 1)
            }
            .accessibilityElement(children: .combine)
            .accessibilityIdentifier("app-activity-strip")
        }
    }

    /// The top-level key collections the community tool suite stores its rows
    /// under. `list` requires a non-empty prefix (an empty one is rejected as
    /// `KeyEmpty`), so there is no single "list everything" call; the strip reads
    /// each collection an app might use and unions the rows. A tool that opened
    /// its own new collection would simply show no strip until its prefix is added
    /// here — a known, contained limit, never a crash.
    private static let collectionPrefixes = [
        "items", "messages", "posts", "photos",
        "proposals", "votes", "events", "rsvps", "pages",
    ]

    /// Read the app's rows through the same gated bridge the WebView uses and
    /// resolve their authors to a presence summary. A read failure (a torn-down
    /// or revoked session, or a collection this app never wrote) is skipped rather
    /// than clearing the digest — the invalidation path, not this, closes the app.
    private func refreshDigest() {
        var rows: [(key: String, valueJSON: String)] = []
        for prefix in Self.collectionPrefixes {
            if let some = try? launch.bridge.list(prefix: prefix) {
                rows.append(contentsOf: some)
            }
        }
        guard !rows.isEmpty else { return }
        digest = AppActivityDigest.from(rows: rows) { idHex in
            guard let profile = launch.bridge.profile(idHex: idHex) else { return nil }
            // The bridge returns this fallback for an id whose profile this
            // device has not synced yet; it is not a person to name.
            if profile.displayName == "member", profile.tag.isEmpty { return nil }
            return profile.displayName
        }
    }
}

/// A subtle "who is active in this app" summary, derived from the app's OWN
/// stored rows. Apps tag rows with an author id under a handful of conventional
/// keys (`author_id`, `updated_by_id`, …); this tallies one contribution per row
/// per distinct author WITHOUT knowing any single app's schema, so the same strip
/// works across the whole tool suite. It never guesses a name: an id is counted
/// only once `resolve` (the host profile lookup) returns a display name for it.
struct AppActivityDigest: Equatable {
    struct Contributor: Equatable {
        let name: String
        let count: Int
    }

    let contributors: [Contributor]

    static let empty = AppActivityDigest(contributors: [])
    var isEmpty: Bool { contributors.isEmpty }

    /// The active people, most active first, capped so the strip stays a caption
    /// ("Ana, Priya, Sam +2"). Names only — presence, not analytics.
    func caption(limit: Int = 4) -> String {
        let shown = contributors.prefix(limit).map(\.name)
        let overflow = contributors.count - shown.count
        let list = shown.joined(separator: ", ")
        return overflow > 0 ? "\(list) +\(overflow)" : list
    }

    /// Build a digest from an app's stored rows. `resolve` maps an id-hex to a
    /// display name (nil = unknown, so it is not shown). One contribution is
    /// counted per row per author; same-named authors collapse into one entry so
    /// the caption never repeats a name.
    static func from(
        rows: [(key: String, valueJSON: String)],
        resolve: (String) -> String?
    ) -> AppActivityDigest {
        var counts: [String: Int] = [:]
        var order: [String] = []
        for row in rows {
            for id in authorIDs(inValueJSON: row.valueJSON) {
                guard let name = resolve(id) else { continue }
                if counts[name] == nil { order.append(name) }
                counts[name, default: 0] += 1
            }
        }
        let seen = Dictionary(uniqueKeysWithValues: order.enumerated().map { ($1, $0) })
        let contributors = order
            .map { Contributor(name: $0, count: counts[$0] ?? 0) }
            .sorted { lhs, rhs in
                lhs.count != rhs.count
                    ? lhs.count > rhs.count
                    : (seen[lhs.name] ?? 0) < (seen[rhs.name] ?? 0)
            }
        return AppActivityDigest(contributors: contributors)
    }

    /// The author ids in one row's JSON value: any top-level string field whose
    /// KEY reads like authorship and whose VALUE is a 32-byte subspace id-hex.
    private static func authorIDs(inValueJSON json: String) -> [String] {
        guard let data = json.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return [] }
        return object.compactMap { key, value in
            guard looksLikeAuthorKey(key),
                  let string = value as? String,
                  isSubspaceID(string)
            else { return nil }
            return string
        }
    }

    private static func looksLikeAuthorKey(_ key: String) -> Bool {
        let lowered = key.lowercased()
        return lowered.contains("author") || lowered.contains("_by_id")
            || lowered == "by_id" || lowered.contains("owner")
    }

    private static func isSubspaceID(_ value: String) -> Bool {
        value.count == 64 && value.allSatisfy { $0.isHexDigit && !$0.isUppercase }
    }
}

/// Bridges the trusted launch inputs into a configured `WKWebView`. Internal
/// (not private) so the navigation lock and change-notification wiring on its
/// coordinator can be unit-tested directly.
///
/// The WebView contract is identical on both platforms — non-persistent browser
/// state, the injected `riot` bridge, the app's scheme handler, and the
/// navigation lock. Only the representable protocol differs (UIKit on iOS,
/// AppKit on macOS), so both entry points funnel through `makeWebView` and
/// there is exactly one copy of the security-relevant configuration.
struct AppWebView {
    let launch: AppRuntimeLaunch

    /// `@MainActor` is explicit here: it was previously inferred from the
    /// `UIViewRepresentable` conformance, which this struct no longer declares
    /// directly (the conformances moved to the per-platform extensions below).
    @MainActor
    func makeCoordinator() -> AppRuntimeCoordinator {
        AppRuntimeCoordinator(
            bridge: AppBridgeController(bridge: launch.bridge),
            appIDHex: launch.appIDHex,
            entryPoint: launch.entryPoint
        )
    }

    @MainActor
    fileprivate func makeWebView(coordinator: AppRuntimeCoordinator) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.userContentController.addUserScript(
            WKUserScript(source: RiotJS.source, injectionTime: .atDocumentStart, forMainFrameOnly: true)
        )
        configuration.userContentController.add(coordinator.bridge, name: "riot")
        configuration.setURLSchemeHandler(
            AppSchemeHandler(resolver: launch.resolver),
            forURLScheme: AppSchemeHandler.scheme
        )
        // Independent, CSP-agnostic egress hardening applied at CONFIG time,
        // before the WebView exists (WebRTC preference is read at creation).
        AppNetworkBackstop.harden(configuration)
        let webView = WKWebView(frame: .zero, configuration: configuration)
        webView.navigationDelegate = coordinator
        webView.uiDelegate = coordinator
        coordinator.bridge.webView = webView
        // When a bridge call fails because the session was invalidated (§4.7),
        // close the app to Tools rather than showing a generic per-op error.
        coordinator.bridge.onInvalidated = { AppRuntimeView.postAppInvalidated() }
        coordinator.observeDataChanges()
        // Fail closed: the entry point is NOT loaded until the block-all content
        // rule list has been compiled and attached. If it cannot be applied, no
        // app page ever runs in this WebView. This is what makes the network
        // backstop independent of the page's (strippable) CSP.
        coordinator.applyEgressBackstopThenLoad()
        return webView
    }
}

#if os(macOS)
extension AppWebView: NSViewRepresentable {
    func makeNSView(context: Context) -> WKWebView {
        makeWebView(coordinator: context.coordinator)
    }

    func updateNSView(_ webView: WKWebView, context: Context) {}

    /// When SwiftUI removes the hosted app (Close, a community switch, navigating
    /// away), tear the runtime down so no watch callback fires after the UI is
    /// gone and no zombie session keeps reading.
    static func dismantleNSView(_ webView: WKWebView, coordinator: AppRuntimeCoordinator) {
        coordinator.tearDown(webView)
    }
}
#else
extension AppWebView: UIViewRepresentable {
    func makeUIView(context: Context) -> WKWebView {
        makeWebView(coordinator: context.coordinator)
    }

    func updateUIView(_ webView: WKWebView, context: Context) {}

    /// See `dismantleNSView` — the same teardown on the UIKit side.
    static func dismantleUIView(_ webView: WKWebView, coordinator: AppRuntimeCoordinator) {
        coordinator.tearDown(webView)
    }
}
#endif

/// Owns the bridge, the navigation lock, and the change-notification observer
/// for one hosted app.
@MainActor
final class AppRuntimeCoordinator: NSObject, WKNavigationDelegate, WKUIDelegate {
    let bridge: AppBridgeController
    private let appIDHex: String
    private let entryPoint: String
    /// Mutated only on the main actor; read once from the nonisolated `deinit`,
    /// where `NotificationCenter.removeObserver` is itself thread-safe.
    private nonisolated(unsafe) var observer: NSObjectProtocol?
    /// Set once the runtime has been torn down. After this, no data-changed post
    /// re-runs the page's watchers — a callback that fired after teardown would
    /// be reading for a UI that is already gone.
    private var tornDown = false

    init(bridge: AppBridgeController, appIDHex: String, entryPoint: String) {
        self.bridge = bridge
        self.appIDHex = appIDHex
        self.entryPoint = entryPoint
    }

    /// The initial load target: the verified entry point under the app's own
    /// scheme host. Same-origin sub-resource loads (`app.js`, `style.css`) share
    /// this scheme and so pass the navigation lock below.
    var entryURL: URL? {
        URL(string: "\(AppSchemeHandler.scheme)://\(appIDHex)/\(entryPoint)")
    }

    /// Compile and attach the block-all egress content rule list, THEN load the
    /// entry point. Fail closed: if the backstop cannot be applied the page is
    /// never loaded, so no app code runs without the network wall in place.
    func applyEgressBackstopThenLoad() {
        Task { @MainActor [weak self] in
            guard let self, !self.tornDown else { return }
            guard let webView = self.bridge.webView else { return }
            guard let ruleList = await AppNetworkBackstop.compiledBlockAll() else {
                // Compilation failed: do not load the app. Report closed.
                return
            }
            guard !self.tornDown else { return }
            webView.configuration.userContentController.add(ruleList)
            if let url = self.entryURL {
                webView.load(URLRequest(url: url))
            }
        }
    }

    /// Subscribes to `AppRuntimeView.dataChangedNotification` and re-runs the
    /// page's watchers when it fires. Posts arrive on `.main`; the block hops to
    /// the main actor to touch the WebView. A post that arrives after teardown is
    /// ignored — the observer is removed on teardown, and this guard is the belt
    /// to that suspenders.
    func observeDataChanges() {
        observer = NotificationCenter.default.addObserver(
            forName: AppRuntimeView.dataChangedNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, !self.tornDown else { return }
                self.bridge.notifyDataChanged()
            }
        }
    }

    /// Tear the runtime down on invalidation (Close, revoke, community switch,
    /// navigate away). Removes the change observer so no watcher re-runs, drops
    /// the WebView reference so `notifyDataChanged` becomes a no-op, and halts the
    /// page by loading `about:blank` (local, no network). Idempotent.
    func tearDown(_ webView: WKWebView) {
        guard !tornDown else { return }
        tornDown = true
        if let observer {
            NotificationCenter.default.removeObserver(observer)
            self.observer = nil
        }
        // Destroy the Rust execution session so any in-flight or later bridge
        // call fails closed — the session cannot outlive the UI.
        bridge.teardown()
        bridge.webView = nil
        webView.stopLoading()
        webView.loadHTMLString("", baseURL: nil)
    }

    /// The security-load-bearing navigation lock: CSP does not constrain
    /// top-level navigation, so this refuses every load whose scheme is not
    /// `riot-app`. A trusted page cannot navigate the host frame to the network,
    /// `about:`, or a `javascript:` URL.
    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        let allowed = navigationAction.request.url?.scheme == AppSchemeHandler.scheme
        decisionHandler(allowed ? .allow : .cancel)
    }

    /// Refuses `window.open` and any other request for a secondary WebView.
    /// Returning nil is WebKit's contract for "do not create one", so a trusted
    /// page cannot spawn an unmanaged frame outside the navigation lock above.
    /// Made explicit rather than resting on the absence of a UI delegate.
    func webView(
        _ webView: WKWebView,
        createWebViewWith configuration: WKWebViewConfiguration,
        for navigationAction: WKNavigationAction,
        windowFeatures: WKWindowFeatures
    ) -> WKWebView? {
        nil
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }
}

/// The independent, CSP-agnostic network egress backstop for hosted apps
/// (Unit 0C — SECURITY-CRITICAL).
///
/// The WKWebView navigation delegate (`decidePolicyFor navigationAction`) only
/// sees FRAME navigations. It never sees `fetch`, `XMLHttpRequest`, `WebSocket`,
/// `EventSource`, `sendBeacon`, remote `<img>/<script>/<link>/<iframe>`
/// subresources, CSS `url()`, DNS-prefetch/preconnect, or a favicon request.
/// Before Unit 0C those were blocked ONLY by the strict CSP the scheme handler
/// serves — and an attacker who controls the page can strip that CSP. So the CSP
/// cannot be the containment.
///
/// This is the containment: a compiled `WKContentRuleList` that BLOCKS every URL
/// and then re-permits only the app's own `riot-app` scheme. Content rule lists
/// are enforced at WebKit's network layer, completely independent of page
/// content, so egress is denied even with the page's CSP removed. `AppWebView`
/// fails closed — it never loads an app page until this list is attached.
///
/// LIMIT (see the 0C report): a content rule list governs the URL-loading
/// system. `RTCPeerConnection` (WebRTC) does NOT flow through it, so a content
/// rule list cannot block STUN/TURN egress. `harden` disables WebRTC via the
/// only lever WKWebView exposes; where that lever is unavailable WebRTC is a
/// documented residual risk to threat-model, not something CSP or the rule list
/// closes.
enum AppNetworkBackstop {
    static let identifier = "riot-app-egress-block-v1"

    /// Block every URL; then lift the block for the app's own scheme so its
    /// bundle (served by the custom scheme handler) still loads. `url-filter` is
    /// a regex over the full URL string.
    static let ruleListJSON = """
    [
      { "trigger": { "url-filter": ".*" }, "action": { "type": "block" } },
      { "trigger": { "url-filter": "^riot-app://" }, "action": { "type": "ignore-previous-rules" } }
    ]
    """

    /// Compile (or reuse the store-cached) block-all rule list. Returns nil on
    /// failure so the caller can fail closed. WebKit caches by identifier, so the
    /// compile cost is paid once per process.
    @MainActor
    static func compiledBlockAll() async -> WKContentRuleList? {
        guard let store = WKContentRuleListStore.default() else { return nil }
        return await withCheckedContinuation { continuation in
            store.compileContentRuleList(
                forIdentifier: identifier,
                encodedContentRuleList: ruleListJSON
            ) { list, _ in
                continuation.resume(returning: list)
            }
        }
    }

    /// Config-time hardening applied before the WebView is created. Closes the
    /// non-URL egress channels a content rule list cannot: WebRTC. Best-effort —
    /// the WebRTC preference is private, so this is defensive and its absence is a
    /// documented residual, never a silent assumption of safety.
    @MainActor
    static func harden(_ configuration: WKWebViewConfiguration) {
        // WKWebpagePreferences / WKPreferences do not expose a public switch for
        // WebRTC. `peerConnectionEnabled` is the historical private key; guard the
        // KVC so an OS that renamed or removed it cannot crash the host.
        let preferences = configuration.preferences
        if preferences.responds(to: NSSelectorFromString("peerConnectionEnabled")) {
            preferences.setValue(false, forKey: "peerConnectionEnabled")
        }
    }
}
