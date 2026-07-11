import WebKit
import XCTest
@testable import RiotKit

@MainActor
final class AppRuntimeHostTests: XCTestCase {

    // MARK: Fixtures

    private func sampleBundle() -> DecodedAppBundle {
        DecodedAppBundle(
            entryPoint: "index.html",
            resources: [
                AppResource(path: "app.js", contentType: "text/javascript", bytes: Data("riot.watch();".utf8)),
                AppResource(path: "index.html", contentType: "text/html", bytes: Data("<!doctype html>".utf8)),
            ]
        )
    }

    /// Builds a resolver directly from the three committed checklist fixture
    /// files (repo root derived from this file's path), matching the content
    /// types the Rust packer assigns.
    private func checklistResolver(appID: String) throws -> AppResourceResolver {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // RiotTests
            .deletingLastPathComponent() // ios
            .deletingLastPathComponent() // apps
            .deletingLastPathComponent() // repo root
        let dir = root.appendingPathComponent("fixtures/apps/checklist")
        let resources = [
            AppResource(
                path: "index.html", contentType: "text/html",
                bytes: try Data(contentsOf: dir.appendingPathComponent("index.html"))
            ),
            AppResource(
                path: "style.css", contentType: "text/css",
                bytes: try Data(contentsOf: dir.appendingPathComponent("style.css"))
            ),
            AppResource(
                path: "app.js", contentType: "text/javascript",
                bytes: try Data(contentsOf: dir.appendingPathComponent("app.js"))
            ),
        ]
        return AppResourceResolver(
            appIDHex: appID,
            bundle: DecodedAppBundle(entryPoint: "index.html", resources: resources)
        )
    }

    private func trustedRuntimeBridge(appID: String) throws -> AppRuntimeDataBridge {
        let profile = try openLocalProfile()
        _ = try profile.createPublicSpace(title: "Berlin Mutual Aid")
        return AppRuntimeDataBridge(session: profile.appRuntime(), appIDHex: appID)
    }

    /// Records the outcome of a single navigation so tests can wait for the
    /// WebContent process to finish loading (a cold start can take several
    /// seconds) before polling page JS.
    private final class NavProbe: NSObject, WKNavigationDelegate {
        var finished = false
        var failure: String?
        let done = XCTestExpectation(description: "nav")
        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            finished = true
            done.fulfill()
        }
        func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
            failure = "didFail: \(error)"
            done.fulfill()
        }
        func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
            failure = "didFailProvisional: \(error)"
            done.fulfill()
        }
    }

    private func makeWebView(resolver: AppResourceResolver, bridge: AppBridgeController) -> (WKWebView, NavProbe) {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.userContentController.addUserScript(
            WKUserScript(source: RiotJS.source, injectionTime: .atDocumentStart, forMainFrameOnly: true)
        )
        configuration.userContentController.add(bridge, name: "riot")
        configuration.setURLSchemeHandler(
            AppSchemeHandler(resolver: resolver),
            forURLScheme: AppSchemeHandler.scheme
        )
        let webView = WKWebView(frame: .zero, configuration: configuration)
        bridge.webView = webView
        let probe = NavProbe()
        webView.navigationDelegate = probe
        return (webView, probe)
    }

    /// Loads the entry point and awaits navigation completion, so the
    /// WebContent cold start (several seconds on first launch) is absorbed
    /// before the test drives page JS. `window.riot` is injected at document
    /// start, so it is present once this returns.
    private func loadEntryPoint(_ webView: WKWebView, _ probe: NavProbe, appID: String) async {
        webView.load(URLRequest(url: URL(string: "\(AppSchemeHandler.scheme)://\(appID)/index.html")!))
        await fulfillment(of: [probe.done], timeout: 30)
        XCTAssertTrue(probe.finished, "checklist page failed to load: \(probe.failure ?? "unknown")")
    }

    /// Runs an async JS function body in the page world and returns its
    /// resolved value. Unlike `evaluateJavaScript`, this awaits the bridge's
    /// promises, so a `window.riot` round-trip resolves before it returns.
    private func callAsync(_ webView: WKWebView, _ body: String) async throws -> Any? {
        try await webView.callAsyncJavaScript(body, arguments: [:], contentWorld: .page)
    }

    // MARK: Codec

    func testCodecDecodesTheCanonicalEncodingItProduces() throws {
        let encoded = try AppBundleCodec.encode(sampleBundle())
        XCTAssertEqual(try AppBundleCodec.decode(encoded), sampleBundle())
    }

    func testCodecRejectsTrailingBytes() throws {
        let encoded = try AppBundleCodec.encode(sampleBundle()) + Data([0])
        XCTAssertThrowsError(try AppBundleCodec.decode(encoded))
    }

    func testCodecRejectsMislabeledOuterKey() throws {
        var encoded = try AppBundleCodec.encode(sampleBundle())
        // Byte 0 is map(2); byte 1 is the outer key 0. Relabel it to key 2 so
        // strict key checking must reject before any resource is read.
        encoded[1] = 0x02
        XCTAssertThrowsError(try AppBundleCodec.decode(encoded))
    }

    func testCodecRejectsOversizedResourceCountWithoutAllocating() {
        // map(2), key 0, "a" entry point, key 1, array claiming 2^32 items.
        let forged = Data([
            0xA2, 0x00, 0x61, 0x61, 0x01,
            0x9A, 0xFF, 0xFF, 0xFF, 0xFF,
        ])
        XCTAssertThrowsError(try AppBundleCodec.decode(forged))
    }

    func testCodecRejectsEntryPointNotAmongResources() {
        let bundle = DecodedAppBundle(entryPoint: "missing.html", resources: sampleBundle().resources)
        XCTAssertThrowsError(try AppBundleCodec.encode(bundle))
    }

    // MARK: Resolver

    func testResolverServesExactMatchesOnly() {
        let resolver = AppResourceResolver(appIDHex: String(repeating: "a", count: 64), bundle: sampleBundle())
        XCTAssertEqual(resolver.entryPoint, "index.html")
        XCTAssertEqual(resolver.resolve(path: "index.html")?.contentType, "text/html")
        XCTAssertNil(resolver.resolve(path: "../escape"))
        XCTAssertNil(resolver.resolve(path: "missing.js"))
        XCTAssertNil(resolver.resolve(path: ""))
    }

    // MARK: Scheme handler

    func testSchemeHandlerServesEntryPointWithStrictCSP() throws {
        let appID = String(repeating: "a", count: 64)
        let handler = AppSchemeHandler(resolver: AppResourceResolver(appIDHex: appID, bundle: sampleBundle()))
        let response = try handler.response(for: URL(string: "riot-app://\(appID)/index.html")!)
        XCTAssertEqual(response.response.statusCode, 200)
        XCTAssertEqual(
            response.response.value(forHTTPHeaderField: "Content-Security-Policy"),
            AppSchemeHandler.csp
        )
        XCTAssertEqual(response.response.value(forHTTPHeaderField: "Content-Type"), "text/html")
        XCTAssertFalse(response.bytes.isEmpty)
    }

    func testSchemeHandlerRefusesUnknownPathsForeignAppsAndBadURLs() throws {
        let appID = String(repeating: "a", count: 64)
        let handler = AppSchemeHandler(resolver: AppResourceResolver(appIDHex: appID, bundle: sampleBundle()))
        XCTAssertThrowsError(try handler.response(for: URL(string: "riot-app://\(appID)/missing.js")!))
        let foreign = String(repeating: "b", count: 64)
        XCTAssertThrowsError(try handler.response(for: URL(string: "riot-app://\(foreign)/index.html")!))
        XCTAssertThrowsError(try handler.response(for: URL(string: "https://example.com/index.html")!))
    }

    // MARK: Bridge round-trip through the real FFI

    func testChecklistPageBootsAndRoundTripsAnItemThroughTheBridge() async throws {
        let appID = String(repeating: "a", count: 64)
        let dataBridge = try trustedRuntimeBridge(appID: appID)
        let bridge = AppBridgeController(bridge: dataBridge)
        let (webView, probe) = makeWebView(resolver: try checklistResolver(appID: appID), bridge: bridge)
        await loadEntryPoint(webView, probe, appID: appID)

        let ready = try await callAsync(webView, "return window.riot ? 'ready' : 'missing';")
        XCTAssertEqual(ready as? String, "ready")

        let stored = try await callAsync(webView, """
            await window.riot.put('items/test-item', {text: 'water', done: false, updated_by: '', updated_at: 1});
            return 'stored';
        """)
        XCTAssertEqual(stored as? String, "stored")

        let persisted = try dataBridge.get(key: "items/test-item")
        XCTAssertNotNil(persisted)
        XCTAssertTrue(persisted!.contains("water"))
    }

    func testHostileFetchAndOutOfScopeKeysFail() async throws {
        let appID = String(repeating: "a", count: 64)
        let bridge = AppBridgeController(bridge: try trustedRuntimeBridge(appID: appID))
        let (webView, probe) = makeWebView(resolver: try checklistResolver(appID: appID), bridge: bridge)
        await loadEntryPoint(webView, probe, appID: appID)

        // CSP: network fetch must be blocked inside the page.
        let fetchResult = try await callAsync(webView, """
            try { await fetch('https://example.com'); return 'FETCHED'; }
            catch (e) { return 'blocked'; }
        """)
        XCTAssertEqual(fetchResult as? String, "blocked")

        // Rust-side scoping: a traversal-shaped key must reject, not write.
        let putResult = try await callAsync(webView, """
            try { await window.riot.put('../escape', {x: 1}); return 'WROTE'; }
            catch (e) { return 'rejected'; }
        """)
        XCTAssertEqual(putResult as? String, "rejected")
    }

    // MARK: Bridge message validation

    func testBridgeRejectsMalformedAndOversizedMessages() throws {
        let bridge = AppBridgeController(bridge: try trustedRuntimeBridge(appID: String(repeating: "a", count: 64)))
        XCTAssertFalse(bridge.handleForTesting(body: "not a dictionary"))
        XCTAssertFalse(bridge.handleForTesting(body: ["op": "get"])) // missing id
        XCTAssertFalse(bridge.handleForTesting(body: [
            "id": 1, "op": "put", "key": "items/x",
            "value": String(repeating: "a", count: 300_000),
        ])) // oversized
    }

    // MARK: - AppRuntimeView host: navigation lock

    /// A `WKNavigationAction` with no public initializer, subclassed so the
    /// navigation-policy delegate can be driven with an arbitrary URL. Only the
    /// `request` the delegate inspects is overridden.
    private final class StubNavigationAction: WKNavigationAction {
        private let stubbedRequest: URLRequest
        init(url: URL) {
            self.stubbedRequest = URLRequest(url: url)
            super.init()
        }
        override var request: URLRequest { stubbedRequest }
    }

    private final class SpyDataBridge: AppDataBridging {
        func put(key: String, valueJSON: String) throws {}
        func get(key: String) throws -> String? { nil }
        func list(prefix: String) throws -> [(key: String, valueJSON: String)] { [] }
        func displayName() -> String { "spy" }
    }

    private func navigationDecision(
        _ coordinator: AppRuntimeCoordinator,
        for urlString: String
    ) -> WKNavigationActionPolicy {
        var decision: WKNavigationActionPolicy?
        coordinator.webView(
            WKWebView(),
            decidePolicyFor: StubNavigationAction(url: URL(string: urlString)!)
        ) { decision = $0 }
        return decision ?? .cancel
    }

    func testNavigationLockAllowsOnlyRiotAppScheme() {
        let appID = String(repeating: "a", count: 64)
        let coordinator = AppRuntimeCoordinator(
            bridge: AppBridgeController(bridge: SpyDataBridge()),
            appIDHex: appID,
            entryPoint: "index.html"
        )

        XCTAssertEqual(navigationDecision(coordinator, for: "riot-app://\(appID)/index.html"), .allow)
        for hostile in ["https://example.com", "http://example.com", "about:blank", "javascript:alert(1)"] {
            XCTAssertEqual(
                navigationDecision(coordinator, for: hostile), .cancel,
                "navigation lock must cancel \(hostile)"
            )
        }
    }

    // MARK: - AppRuntimeView host: trust gate

    /// Repo root derived from this file at `apps/ios/RiotTests/…` (four levels
    /// up), matching `AppRepositoryTests`, so the frozen starter artifacts load.
    private func repoRoot() -> URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // RiotTests
            .deletingLastPathComponent() // ios
            .deletingLastPathComponent() // apps
            .deletingLastPathComponent() // repo root
    }

    private func starterPacks() throws -> [(manifest: Data, bundle: Data)] {
        let apps = repoRoot().appendingPathComponent("fixtures/apps")
        return [(
            manifest: try Data(contentsOf: apps.appendingPathComponent("checklist.manifest.cbor")),
            bundle: try Data(contentsOf: apps.appendingPathComponent("checklist.bundle.cbor"))
        )]
    }

    private final class FixedWrappingKeyStore: WrappingKeyStore {
        private var key: Data?
        func loadOrCreateWrappingKey() throws -> Data {
            if let key { return key }
            let created = Data(repeating: 0x5a, count: 32)
            key = created
            return created
        }
    }

    private func trustedRepository() throws -> (RiotProfileRepository, String) {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("app-runtime-view-\(UUID().uuidString).json")
        let repository = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url),
            keyStore: FixedWrappingKeyStore(),
            starterPacks: try starterPacks()
        )
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")
        return (repository, try repository.spaceApps()[0].appIDHex)
    }

    /// The launch decision — the host-side trust gate the platform depends on —
    /// refuses to produce mount inputs for an untrusted (or unknown) app and
    /// only succeeds once the app is trusted. `AppRuntimeView` renders no
    /// WebView and calls `onClose` in the nil case.
    func testAppRuntimeLaunchIsGatedOnTrust() throws {
        let (repository, appID) = try trustedRepository()

        XCTAssertNil(
            AppRuntimeLaunch(repository: repository, appIDHex: appID),
            "untrusted app must not produce launch inputs"
        )
        XCTAssertNil(
            AppRuntimeLaunch(repository: repository, appIDHex: String(repeating: "e", count: 64)),
            "unknown app id must not produce launch inputs"
        )

        try repository.trustApp(appID: appID)
        let launch = try XCTUnwrap(AppRuntimeLaunch(repository: repository, appIDHex: appID))
        XCTAssertEqual(launch.appIDHex, appID.lowercased())
        XCTAssertEqual(launch.entryPoint, "index.html")
    }

    // MARK: - AppRuntimeView host: change notification plumbing

    /// Posting `AppRuntimeView.dataChangedNotification` drives the coordinator's
    /// observer, which calls `bridge.notifyDataChanged()` — observed here by
    /// replacing the page's `__riotDataChanged` with a counter and reading it
    /// back through the bridge's own JS round-trip. (Timer polls don't fire
    /// under XCTWaiter; `callAsyncJavaScript` awaits, so each poll yields the
    /// main thread for the `.main`-queued observer and the JS to run.)
    func testDataChangedNotificationRerunsPageWatchers() async throws {
        let appID = String(repeating: "a", count: 64)
        let bridge = AppBridgeController(bridge: SpyDataBridge())
        let (webView, probe) = makeWebView(resolver: try checklistResolver(appID: appID), bridge: bridge)
        await loadEntryPoint(webView, probe, appID: appID)

        _ = try await callAsync(webView, """
            window.__dc = 0;
            window.__riotDataChanged = function () { window.__dc += 1; };
            return 'ok';
        """)

        let coordinator = AppRuntimeCoordinator(bridge: bridge, appIDHex: appID, entryPoint: "index.html")
        coordinator.observeDataChanges()
        NotificationCenter.default.post(name: AppRuntimeView.dataChangedNotification, object: nil)

        var observed = 0
        for _ in 0..<50 {
            observed = (try await callAsync(webView, "return window.__dc || 0;")) as? Int ?? 0
            if observed >= 1 { break }
        }
        XCTAssertGreaterThanOrEqual(observed, 1, "notification did not re-run page watchers")
    }
}
