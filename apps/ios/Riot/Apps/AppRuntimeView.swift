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
    /// Posted by refresh sources outside the page (foregrounding, and — where a
    /// host wires it — sync completion) to re-run the page's `watch` callbacks.
    public static let dataChangedNotification = Notification.Name("RiotAppDataChanged")

    private let repository: RiotProfileRepository
    private let appIDHex: String
    private let appName: String
    private let onClose: () -> Void

    @Environment(\.scenePhase) private var scenePhase

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
        Group {
            if let launch = AppRuntimeLaunch(repository: repository, appIDHex: appIDHex) {
                VStack(spacing: 0) {
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
                    AppWebView(launch: launch)
                }
                .onChange(of: scenePhase) { _, phase in
                    if phase == .active {
                        NotificationCenter.default.post(name: Self.dataChangedNotification, object: nil)
                    }
                }
            } else {
                // Trust was revoked between the Tools row rendering "Open" and
                // this view constructing. Per the HARD CONTRACT we must not
                // mount an untrusted app: render nothing and dismiss.
                Color.clear.onAppear(perform: onClose)
            }
        }
    }
}

/// Bridges the trusted launch inputs into a configured `WKWebView`. Internal
/// (not private) so the navigation lock and change-notification wiring on its
/// coordinator can be unit-tested directly.
struct AppWebView: UIViewRepresentable {
    let launch: AppRuntimeLaunch

    func makeCoordinator() -> AppRuntimeCoordinator {
        AppRuntimeCoordinator(
            bridge: AppBridgeController(bridge: launch.bridge),
            appIDHex: launch.appIDHex,
            entryPoint: launch.entryPoint
        )
    }

    func makeUIView(context: Context) -> WKWebView {
        let coordinator = context.coordinator
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
        let webView = WKWebView(frame: .zero, configuration: configuration)
        webView.navigationDelegate = coordinator
        webView.uiDelegate = coordinator
        coordinator.bridge.webView = webView
        coordinator.observeDataChanges()
        if let url = coordinator.entryURL {
            webView.load(URLRequest(url: url))
        }
        return webView
    }

    func updateUIView(_ webView: WKWebView, context: Context) {}
}

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

    /// Subscribes to `AppRuntimeView.dataChangedNotification` and re-runs the
    /// page's watchers when it fires. Posts arrive on `.main`; the block hops to
    /// the main actor to touch the WebView.
    func observeDataChanges() {
        observer = NotificationCenter.default.addObserver(
            forName: AppRuntimeView.dataChangedNotification,
            object: nil,
            queue: .main
        ) { [weak bridge] _ in
            MainActor.assumeIsolated { bridge?.notifyDataChanged() }
        }
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
