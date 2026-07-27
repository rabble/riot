import SwiftUI
import WebKit

/// Parses the deliberately small title contract shared by native chrome and a
/// mounted app: `Page — App`. App-controlled text is display-only and bounded
/// before normalization so it can never become a navigation authority or an
/// unbounded allocation in the host.
enum AppBreadcrumbTitle {
    static func page(from rawTitle: String?, appName: String) -> String? {
        guard let rawTitle, rawTitle.utf16.count <= 512 else { return nil }
        guard rawTitle.unicodeScalars.allSatisfy({ scalar in
            switch scalar.properties.generalCategory {
            case .control, .format, .lineSeparator, .paragraphSeparator:
                return false
            default:
                return true
            }
        }) else { return nil }

        let normalized = rawTitle.precomposedStringWithCanonicalMapping
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let normalizedAppName = appName.precomposedStringWithCanonicalMapping
        let suffix = " — \(normalizedAppName)"
        guard normalized.hasSuffix(suffix) else { return nil }
        let page = String(normalized.dropLast(suffix.count))
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard !page.isEmpty, page.count <= 120, !page.contains("›") else { return nil }
        return page
    }
}

/// The semantic levels used by both full-text and compact breadcrumb variants.
struct AppBreadcrumbLabels: Equatable {
    let community: String
    let app: String
    let page: String?

    init(community: String, app: String, page: String?) {
        self.community = Self.safeLevel(community)
        self.app = Self.safeLevel(app)
        self.page = page.map(Self.safeLevel)
    }

    var full: [String] { [community, app, page].compactMap { $0 } }
    var compact: [String] { page == nil ? ["🏘", "🧰"] : ["🏘", "🧰", "📄"] }
    var isAppRootActionAvailable: Bool { page != nil }
    var communityAccessibilityLabel: String {
        "Choose community, current community: \(community)"
    }
    var appAccessibilityLabel: String {
        isAppRootActionAvailable ? "Return to \(app) home" : app
    }
    var pageAccessibilityLabel: String? {
        page.map { "Current page: \($0)" }
    }

    private static func safeLevel(_ value: String) -> String {
        value.replacingOccurrences(of: "›", with: "·")
    }
}

/// Synchronous control plane for one mounted WebView. Shell transitions call
/// `tearDownNow()` before changing community state; the breadcrumb uses the
/// same mount-scoped handle to ask the page to return to its own root.
@MainActor
public final class AppRuntimeTeardownHandle {
    private var tearDownAction: (() -> Void)?
    private var navigateRootAction: (() -> Void)?

    public init() {}

    func install(tearDown: @escaping () -> Void, navigateRoot: @escaping () -> Void) {
        tearDownAction = tearDown
        navigateRootAction = navigateRoot
    }

    func remove() {
        tearDownAction = nil
        navigateRootAction = nil
    }

    public func tearDownNow() {
        let action = tearDownAction
        remove()
        action?()
    }

    func navigateToRoot() {
        navigateRootAction?()
    }
}

#if os(macOS)
/// Compact native location chrome for a mounted tool. The full hierarchy is
/// preferred as one indivisible candidate; when it does not fit, every level
/// switches to an emoji together so the hierarchy never becomes a mixture of
/// clipped and unexplained labels.
private struct AppBreadcrumbView: View {
    let labels: AppBreadcrumbLabels
    let onCommunity: () -> Void
    let onAppRoot: () -> Void

    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        ViewThatFits(in: .horizontal) {
            row(compact: false)
                .fixedSize(horizontal: true, vertical: false)
            row(compact: true)
                .fixedSize(horizontal: true, vertical: false)
        }
        .frame(maxWidth: .infinity, minHeight: 36, maxHeight: 36, alignment: .leading)
        .padding(.horizontal, 12)
        .background(RiotTheme.paper2(for: colorScheme))
        .overlay(alignment: .bottom) {
            Rectangle().fill(RiotTheme.line(for: colorScheme)).frame(height: 1)
        }
    }

    @ViewBuilder
    private func row(compact: Bool) -> some View {
        HStack(spacing: 7) {
            crumbButton(
                compact ? "🏘" : labels.community,
                accessibilityLabel: labels.communityAccessibilityLabel,
                help: labels.community,
                identifier: "breadcrumb-community",
                action: onCommunity
            )
            separator
            if labels.isAppRootActionAvailable {
                crumbButton(
                    compact ? "🧰" : labels.app,
                    accessibilityLabel: labels.appAccessibilityLabel,
                    help: labels.app,
                    identifier: "breadcrumb-app",
                    action: onAppRoot
                )
            } else {
                currentLevel(
                    compact ? "🧰" : labels.app,
                    accessibilityLabel: labels.appAccessibilityLabel,
                    help: labels.app,
                    identifier: "breadcrumb-app"
                )
            }
            if let page = labels.page {
                separator
                currentLevel(
                    compact ? "📄" : page,
                    accessibilityLabel: labels.pageAccessibilityLabel ?? page,
                    help: page,
                    identifier: "breadcrumb-page"
                )
            }
        }
        .font(.riot(.mono, size: 12, relativeTo: .caption))
        .foregroundStyle(RiotTheme.ink(for: colorScheme))
    }

    private func crumbButton(
        _ title: String,
        accessibilityLabel: String,
        help: String,
        identifier: String,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Text(title)
                .lineLimit(1)
                .padding(.horizontal, 4)
                .frame(minHeight: 36)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .foregroundStyle(RiotTheme.pink(for: colorScheme))
        .accessibilityLabel(accessibilityLabel)
        .accessibilityIdentifier(identifier)
        .help(help)
    }

    private func currentLevel(
        _ title: String,
        accessibilityLabel: String,
        help: String,
        identifier: String
    ) -> some View {
        Text(title)
            .lineLimit(1)
            .fontWeight(.semibold)
            .accessibilityLabel(accessibilityLabel)
            .accessibilityIdentifier(identifier)
            .accessibilityAddTraits(.isHeader)
            .help(help)
    }

    private var separator: some View {
        Text("›")
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            .accessibilityHidden(true)
    }
}
#endif

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
    private let communityName: String
    private let onOpenCommunity: () -> Void
    private let teardownHandle: AppRuntimeTeardownHandle
    private let onClose: () -> Void

    public init(
        repository: RiotProfileRepository,
        appIDHex: String,
        appName: String,
        communityName: String,
        teardownHandle: AppRuntimeTeardownHandle,
        onOpenCommunity: @escaping () -> Void,
        onClose: @escaping () -> Void
    ) {
        self.repository = repository
        self.appIDHex = appIDHex
        self.appName = appName
        self.communityName = communityName
        self.teardownHandle = teardownHandle
        self.onOpenCommunity = onOpenCommunity
        self.onClose = onClose
    }

    public var body: some View {
        if let launch = AppRuntimeLaunch(repository: repository, appIDHex: appIDHex) {
            AppHostView(
                launch: launch,
                appName: appName,
                communityName: communityName,
                teardownHandle: teardownHandle,
                onOpenCommunity: onOpenCommunity,
                onClose: onClose
            )
        } else {
            // Trust was revoked between the Tools row rendering "Open" and this
            // view constructing. Per the HARD CONTRACT we must not mount an
            // untrusted app: render nothing and dismiss.
            Color.clear.onAppear(perform: onClose)
        }
    }
}

/// The mounted-app chrome: the app's own themed surface, a subtle activity strip
/// naming who is active in it, and the sandboxed WebView. On iPhone this is a
/// PUSH under the Tools tab, so the "‹ Tools" back button and app-name title come
/// from the enclosing `NavigationStack` and the community header + tab bar stay
/// on screen. On macOS it lives in the split detail and uses a compact native
/// community › app › page breadcrumb for location and escape routes.
private struct AppHostView: View {
    let launch: AppRuntimeLaunch
    let appName: String
    let communityName: String
    let teardownHandle: AppRuntimeTeardownHandle
    let onOpenCommunity: () -> Void
    let onClose: () -> Void

    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.colorScheme) private var colorScheme
    /// Who is active in THIS app, read from its own stored rows. Refreshed on
    /// mount, on a store-changed post, and on foreground.
    @State private var digest: AppActivityDigest = .empty
    @State private var pageTitle: String?

    var body: some View {
        VStack(spacing: 0) {
            #if os(macOS)
            AppBreadcrumbView(
                labels: AppBreadcrumbLabels(
                    community: communityName,
                    app: appName,
                    page: pageTitle
                ),
                onCommunity: onOpenCommunity,
                onAppRoot: { teardownHandle.navigateToRoot() }
            )
            #endif
            activityStrip
            AppWebView(
                launch: launch,
                appName: appName,
                teardownHandle: teardownHandle,
                onPageTitleChanged: { pageTitle = $0 }
            )
        }
        .background(RiotTheme.paper(for: colorScheme))
        #if os(iOS)
        .navigationTitle(appName)
        .navigationBarTitleDisplayMode(.inline)
        #endif
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

    /// Read the app's rows through the same gated bridge the WebView uses and
    /// resolve their authors to a presence summary. A read failure (a torn-down
    /// or revoked session) leaves the last digest in place rather than clearing
    /// it — the invalidation path, not this, closes the app.
    private func refreshDigest() {
        guard let rows = try? launch.bridge.list(prefix: "") else { return }
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
    let appName: String
    let teardownHandle: AppRuntimeTeardownHandle
    let onPageTitleChanged: (String?) -> Void

    @MainActor
    init(
        launch: AppRuntimeLaunch,
        appName: String,
        teardownHandle: AppRuntimeTeardownHandle,
        onPageTitleChanged: @escaping (String?) -> Void
    ) {
        self.launch = launch
        self.appName = appName
        self.teardownHandle = teardownHandle
        self.onPageTitleChanged = onPageTitleChanged
    }

    /// `@MainActor` is explicit here: it was previously inferred from the
    /// `UIViewRepresentable` conformance, which this struct no longer declares
    /// directly (the conformances moved to the per-platform extensions below).
    @MainActor
    func makeCoordinator() -> AppRuntimeCoordinator {
        AppRuntimeCoordinator(
            bridge: AppBridgeController(bridge: launch.bridge),
            appIDHex: launch.appIDHex,
            entryPoint: launch.entryPoint,
            appName: appName,
            onPageTitleChanged: onPageTitleChanged,
            teardownHandle: teardownHandle
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
        coordinator.observePageTitles(in: webView)
        teardownHandle.install(
            tearDown: { [weak coordinator, weak webView] in
                guard let coordinator, let webView else { return }
                coordinator.tearDown(webView)
            },
            navigateRoot: { [weak coordinator] in coordinator?.navigateToAppRoot() }
        )
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

    func updateNSView(_ webView: WKWebView, context: Context) {
        context.coordinator.onPageTitleChanged = onPageTitleChanged
    }

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

    func updateUIView(_ webView: WKWebView, context: Context) {
        context.coordinator.onPageTitleChanged = onPageTitleChanged
    }

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
    static let navigateRootScript = """
    (() => { window.dispatchEvent(new Event("riot:navigate-root")); return document.title; })()
    """

    let bridge: AppBridgeController
    private let appIDHex: String
    private let entryPoint: String
    private let appName: String
    private weak var observedTitleWebView: WKWebView?
    private weak var teardownHandle: AppRuntimeTeardownHandle?
    private var titleObservation: NSKeyValueObservation?
    private var lastPublishedPageTitle: String?
    private var hasPublishedPageTitle = false
    private var rootNavigationGeneration = 0
    var onPageTitleChanged: ((String?) -> Void)?
    /// Mutated only on the main actor; read once from the nonisolated `deinit`,
    /// where `NotificationCenter.removeObserver` is itself thread-safe.
    private nonisolated(unsafe) var observer: NSObjectProtocol?
    /// Set once the runtime has been torn down. After this, no data-changed post
    /// re-runs the page's watchers — a callback that fired after teardown would
    /// be reading for a UI that is already gone.
    private var tornDown = false

    init(
        bridge: AppBridgeController,
        appIDHex: String,
        entryPoint: String,
        appName: String = "",
        onPageTitleChanged: ((String?) -> Void)? = nil,
        teardownHandle: AppRuntimeTeardownHandle? = nil
    ) {
        self.bridge = bridge
        self.appIDHex = appIDHex
        self.entryPoint = entryPoint
        self.appName = appName
        self.onPageTitleChanged = onPageTitleChanged
        self.teardownHandle = teardownHandle
    }

    func observePageTitles(in webView: WKWebView) {
        observedTitleWebView = webView
        titleObservation?.invalidate()
        titleObservation = webView.observe(\.title, options: [.initial, .new]) { [weak self] webView, _ in
            Task { @MainActor [weak self, weak webView] in
                guard let self, let webView, !self.tornDown else { return }
                self.publishPageTitle(from: webView.title)
            }
        }
    }

    func navigateToAppRoot() {
        guard !tornDown, let webView = observedTitleWebView else { return }
        rootNavigationGeneration &+= 1
        let generation = rootNavigationGeneration
        webView.evaluateJavaScript(Self.navigateRootScript) { [weak self, weak webView] result, error in
            Task { @MainActor [weak self, weak webView] in
                guard let self, !self.tornDown, generation == self.rootNavigationGeneration else { return }
                guard error == nil else { return }
                let returnedTitle = result as? String
                self.publishPageTitle(from: returnedTitle ?? webView?.title)
            }
        }
    }

    private func publishPageTitle(from rawTitle: String?) {
        let pageTitle = AppBreadcrumbTitle.page(from: rawTitle, appName: appName)
        guard !hasPublishedPageTitle || pageTitle != lastPublishedPageTitle else { return }
        hasPublishedPageTitle = true
        lastPublishedPageTitle = pageTitle
        onPageTitleChanged?(pageTitle)
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
        rootNavigationGeneration &+= 1
        titleObservation?.invalidate()
        titleObservation = nil
        observedTitleWebView = nil
        onPageTitleChanged = nil
        teardownHandle?.remove()
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
        decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void
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
        titleObservation?.invalidate()
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
