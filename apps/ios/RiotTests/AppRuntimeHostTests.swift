import Network
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
        try trustedRuntime(appID: appID).bridge
    }

    /// The profile too, for the tests that need to rename the person behind the
    /// bridge — the repository has no rename surface yet, but `ProfileSession`
    /// does, and the rename is exactly what this change exists to make work.
    private func trustedRuntime(
        appID: String
    ) throws -> (bridge: AppRuntimeDataBridge, profiles: ProfileSession) {
        // The bridge now runs on a gated AppExecutionSession (Unit 0C), which
        // only opens for a TRUSTED app — so install and trust a real app first.
        // The passed `appID` is the resolver's scheme host (page origin) and is
        // independent of the data app id, which is the real installed one.
        let profile = try openLocalProfile()
        _ = try profile.createPublicSpace(title: "Berlin Mutual Aid")
        let runtime = profile.appRuntime()
        let packs = try starterPacks()
        let record = try runtime.installApp(manifestBytes: packs[0].manifest, bundleBytes: packs[0].bundle)
        try runtime.trustApp(appId: record.appId)
        let execution = try profile.openAppExecution(appId: record.appId)
        let profiles = profile.profile()
        let bridge = AppRuntimeDataBridge(execution: execution, profiles: profiles)
        return (bridge, profiles)
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
        func whoami() -> BridgeProfile {
            BridgeProfile(idHex: String(repeating: "11", count: 32), displayName: "spy", tag: "11111111")
        }
        func profile(idHex: String) -> BridgeProfile? {
            BridgeProfile(idHex: idHex, displayName: "spy", tag: String(idHex.prefix(8)))
        }
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

// MARK: - Attribution: the app stores an id, never a name

extension AppRuntimeHostTests {
    /// The whole point of the change, driven through the real page: an item the
    /// checklist writes must carry the author's **id**, not a name snapshot.
    func testChecklistStoresTheAuthorIDAndNotANameSnapshot() async throws {
        let appID = String(repeating: "a", count: 64)
        let runtime = try trustedRuntime(appID: appID)
        let bridge = AppBridgeController(bridge: runtime.bridge)
        let (webView, probe) = makeWebView(resolver: try checklistResolver(appID: appID), bridge: bridge)
        await loadEntryPoint(webView, probe, appID: appID)

        try await addItem(webView, text: "bring water")

        let stored = try XCTUnwrap(runtime.bridge.list(prefix: "items").first?.valueJSON)
        let value = try XCTUnwrap(
            try JSONSerialization.jsonObject(with: Data(stored.utf8)) as? [String: Any]
        )
        let authorID = try XCTUnwrap(value["updated_by_id"] as? String, "no id stored: \(stored)")
        XCTAssertEqual(authorID, runtime.bridge.whoami().idHex)
        XCTAssertNil(
            value["updated_by"],
            "a name snapshot is exactly what must NOT be written any more: \(stored)"
        )
    }

    /// The payoff. Ana checks something off, THEN claims her name — and the row
    /// she already wrote says "Ana". Under a stored name snapshot this is
    /// impossible: the old name would stand forever, unrepairable.
    func testRenamingRepairsAttributionOnItemsAlreadyWritten() async throws {
        let appID = String(repeating: "a", count: 64)
        let runtime = try trustedRuntime(appID: appID)
        let bridge = AppBridgeController(bridge: runtime.bridge)
        let (webView, probe) = makeWebView(resolver: try checklistResolver(appID: appID), bridge: bridge)
        await loadEntryPoint(webView, probe, appID: appID)

        try await addItem(webView, text: "bring water")
        let tag = runtime.bridge.whoami().tag
        let before = try await eventuallyMeta(webView, equals: "member · \(tag)")
        XCTAssertEqual(before, "member · \(tag)", "unnamed author should render as the fallback pair")

        try runtime.profiles.setDisplayName(name: "Ana")
        bridge.notifyDataChanged()

        let after = try await eventuallyMeta(webView, equals: "Ana · \(tag)")
        XCTAssertEqual(after, "Ana · \(tag)", "the rename must repair the row that was already written")
    }

    /// Back-compat: an item written by the OLD code carries `updated_by`, a bare
    /// name with no id behind it. It must still draw — as-is, since there is
    /// nothing to resolve — and it must not take the page down.
    func testLegacyNameSnapshotRowsStillRender() async throws {
        let appID = String(repeating: "a", count: 64)
        let runtime = try trustedRuntime(appID: appID)
        let bridge = AppBridgeController(bridge: runtime.bridge)
        let (webView, probe) = makeWebView(resolver: try checklistResolver(appID: appID), bridge: bridge)
        await loadEntryPoint(webView, probe, appID: appID)

        // Exactly the shape the pre-id checklist wrote.
        try runtime.bridge.put(
            key: "items/legacy",
            valueJSON: #"{"text":"old item","done":false,"updated_by":"Ana · deadbeef","updated_at":1}"#
        )
        bridge.notifyDataChanged()

        let meta = try await eventuallyMeta(webView, equals: "Ana · deadbeef")
        XCTAssertEqual(meta, "Ana · deadbeef", "a legacy snapshot must render as stored")
        let rows = try await callAsync(webView, "return document.querySelectorAll('#items li').length;")
        XCTAssertEqual(rows as? Int, 1, "the legacy row must not crash the render")
    }

    /// Adds an item the way a person does: through the page's own form handler,
    /// so `stamp()` in `app.js` is what decides what gets stored.
    private func addItem(_ webView: WKWebView, text: String) async throws {
        let result = try await callAsync(webView, """
            document.getElementById('new-item').value = \(jsLiteral(text));
            document.getElementById('add-form')
              .dispatchEvent(new Event('submit', { cancelable: true, bubbles: true }));
            for (let i = 0; i < 100; i++) {
              const rows = await window.riot.list('items');
              if (rows.length > 0) { return 'added'; }
              await new Promise((r) => setTimeout(r, 20));
            }
            return 'never stored';
        """)
        XCTAssertEqual(result as? String, "added")
    }

    /// Polls the first row's attribution until it settles: the name is resolved
    /// asynchronously through `riot.profile()`, so the DOM lands a turn late.
    private func eventuallyMeta(_ webView: WKWebView, equals expected: String) async throws -> String {
        var seen = ""
        for _ in 0..<100 {
            seen = (try await callAsync(webView, """
                const meta = document.querySelector('#items li .meta');
                return meta ? meta.textContent : '';
            """)) as? String ?? ""
            if seen == expected { return seen }
            try await Task.sleep(nanoseconds: 20_000_000)
        }
        return seen
    }

    private func jsLiteral(_ value: String) -> String {
        let data = try? JSONSerialization.data(withJSONObject: [value])
        let array = data.flatMap { String(data: $0, encoding: .utf8) } ?? "[\"\"]"
        return String(array.dropFirst().dropLast())
    }
}

extension AppRuntimeHostTests {
    /// Form controls do not inherit `color` — WebKit gives a <button> the
    /// `buttontext` system color, which resolved to WHITE in this WebView and
    /// made the Add button (whose border is `currentColor`) invisible against
    /// the page while staying tappable, so XCUITest passed on a button no
    /// human could see. Pin the invariant: the button's foreground must match
    /// the page's, in whatever colour scheme is in force.
    func testAddButtonIsVisibleAgainstThePage() async throws {
        try await assertChecklistControlsAreLegible(in: .light)
        try await assertChecklistControlsAreLegible(in: .dark)
    }

    private func assertChecklistControlsAreLegible(
        in scheme: UIUserInterfaceStyle
    ) async throws {
        let appID = String(repeating: "a", count: 64)
        let resolver = try checklistResolver(appID: appID)
        let bridge = AppBridgeController(bridge: try trustedRuntimeBridge(appID: appID))
        let (webView, probe) = makeWebView(resolver: resolver, bridge: bridge)
        webView.overrideUserInterfaceStyle = scheme
        await loadEntryPoint(webView, probe, appID: appID)

        let report = try await callAsync(webView, """
        const style = (el) => getComputedStyle(el);
        const button = document.getElementById('add');
        const field = document.getElementById('new-item');
        return JSON.stringify({
          buttonColor: style(button).color,
          buttonBorder: style(button).borderTopColor,
          fieldColor: style(field).color,
          bodyColor: style(document.body).color,
          bodyBackground: style(document.body).backgroundColor,
        });
        """) as? String
        let seen = try XCTUnwrap(report)
        let values = try XCTUnwrap(
            try JSONSerialization.jsonObject(with: Data(seen.utf8)) as? [String: String]
        )
        let body = try XCTUnwrap(values["bodyColor"])
        XCTAssertEqual(values["buttonColor"], body, "\(scheme) button text vs page text: \(seen)")
        XCTAssertEqual(values["buttonBorder"], body, "\(scheme) button border vs page text: \(seen)")
        XCTAssertEqual(values["fieldColor"], body, "\(scheme) field text vs page text: \(seen)")
        // The page must paint its own background: an unpainted canvas leaves the
        // WebView's backing showing through (white in both schemes), which would
        // make dark-mode text white-on-white.
        XCTAssertNotEqual(
            values["bodyBackground"], body,
            "\(scheme) page background must contrast with its text: \(seen)"
        )
    }
}

// MARK: - Unit 0C: runtime containment & invalidation (SECURITY-CRITICAL)

extension AppRuntimeHostTests {
    /// The shared hostile-page fixture (scripts/apps/fixtures/hostile-egress.html)
    /// — the SAME attacker artifact the JS and Android suites use. It carries no
    /// CSP on purpose.
    private func hostileFixtureURL() -> URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // RiotTests
            .deletingLastPathComponent() // ios
            .deletingLastPathComponent() // apps
            .deletingLastPathComponent() // repo root
            .appendingPathComponent("scripts/apps/fixtures/hostile-egress.html")
    }

    /// A loopback TCP listener that counts inbound connections. It exists so the
    /// backstop test has a target that is ALWAYS reachable (127.0.0.1): a fetch to
    /// an unresolvable host would "fail" with or without the backstop and prove
    /// nothing. If any egress vector escapes, this sentinel sees a connection —
    /// so `connections == 0` is a real proof that FAILS the moment the backstop
    /// is removed.
    /// A lock-guarded connection counter, split out so the listener's
    /// `@Sendable` connection handler captures only THIS (Sendable by its lock
    /// invariant) and never the sentinel/`self` — which keeps the sentinel off
    /// the Sendable hook and avoids a listener→closure→self retain cycle.
    private final class ConnectionCounter: @unchecked Sendable {
        private let lock = NSLock()
        private var value = 0
        func increment() { lock.lock(); value += 1; lock.unlock() }
        var current: Int { lock.lock(); defer { lock.unlock() }; return value }
    }

    private final class LoopbackSentinel {
        private let listener: NWListener
        private let counter = ConnectionCounter()
        let port: UInt16

        init() throws {
            let parameters = NWParameters.tcp
            parameters.allowLocalEndpointReuse = true
            let listener = try NWListener(using: parameters)
            self.listener = listener
            let ready = DispatchSemaphore(value: 0)
            listener.stateUpdateHandler = { state in
                if case .ready = state { ready.signal() }
            }
            listener.newConnectionHandler = { [counter] connection in
                counter.increment()
                connection.cancel()
            }
            listener.start(queue: .global())
            _ = ready.wait(timeout: .now() + 5)
            guard let assigned = listener.port?.rawValue else {
                listener.cancel()
                throw NSError(domain: "LoopbackSentinel", code: 1)
            }
            self.port = assigned
        }

        var connections: Int { counter.current }
        func stop() { listener.cancel() }
    }

    /// A hostile page with its CSP stripped, loaded into a WebView configured
    /// exactly like the runtime's, must reach the loopback sentinel ZERO times
    /// through ANY resource-load egress vector. The block can only be the
    /// content-rule-list backstop — CSP is absent here.
    func testNetworkBackstopBlocksEveryEgressVectorWithCSPStripped() async throws {
        let sentinel = try LoopbackSentinel()
        defer { sentinel.stop() }

        guard let ruleList = await AppNetworkBackstop.compiledBlockAll() else {
            return XCTFail("backstop content rule list failed to compile — runtime fails closed")
        }
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        AppNetworkBackstop.harden(configuration)
        configuration.userContentController.add(ruleList)
        let webView = WKWebView(frame: .zero, configuration: configuration)
        let probe = NavProbe()
        webView.navigationDelegate = probe

        let fixture = try String(contentsOf: hostileFixtureURL(), encoding: .utf8)
        webView.loadHTMLString(fixture, baseURL: URL(string: "https://hostile.local/"))
        await fulfillment(of: [probe.done], timeout: 30)
        XCTAssertTrue(probe.finished, "hostile fixture failed to load: \(probe.failure ?? "unknown")")

        // Fire every vector at the sentinel. Skip the top-level-navigation vectors
        // (proven by the navigation lock test); they would navigate this harness
        // away. Everything else is a resource load the backstop must swallow.
        let target = "http://127.0.0.1:\(sentinel.port)/exfil"
        _ = try await callAsync(webView, """
            await window.__attemptEgress('\(target)', {
                skip: ['location-assign', 'location-replace', 'window-open']
            });
            return 'done';
        """)
        // Grace period for any escaping connection to actually land.
        try await Task.sleep(nanoseconds: 500_000_000)

        XCTAssertEqual(
            sentinel.connections, 0,
            "a hostile page with CSP stripped reached the network — the egress backstop failed"
        )

        // Direct fetch as a second, explicit signal that the page sees a block.
        let fetchResult = try await callAsync(webView, """
            try { await fetch('\(target)', { mode: 'no-cors' }); return 'REACHED'; }
            catch (e) { return 'blocked'; }
        """)
        XCTAssertEqual(fetchResult as? String, "blocked")
        XCTAssertEqual(sentinel.connections, 0, "fetch reached the sentinel after the block")
    }

    /// Counts every `evaluateJavaScript` the bridge makes into the page, so a
    /// teardown that failed to cancel the change observer would show up as a
    /// watcher re-run after teardown.
    private final class EvalSpyWebView: WKWebView {
        var evalCount = 0
        override func evaluateJavaScript(
            _ javaScriptString: String,
            completionHandler: ((Any?, Error?) -> Void)? = nil
        ) {
            evalCount += 1
            completionHandler?(nil, nil)
        }
    }

    /// On invalidation the runtime coordinator must cancel its data-changed
    /// observer and drop the WebView, so no watch callback fires after the UI is
    /// gone. A zombie session re-running watchers after teardown is exactly the
    /// leak this proves closed.
    func testRuntimeTeardownCancelsWatchersSoNoCallbackFiresAfterwards() async throws {
        let appID = String(repeating: "a", count: 64)
        let bridge = AppBridgeController(bridge: SpyDataBridge())
        let spy = EvalSpyWebView()
        bridge.webView = spy // `webView` is weak; `spy` (a local let) keeps it alive.
        let coordinator = AppRuntimeCoordinator(bridge: bridge, appIDHex: appID, entryPoint: "index.html")
        coordinator.observeDataChanges()

        // A data-changed post while live re-runs the watchers (one evaluate).
        AppRuntimeView.postDataChanged()
        for _ in 0..<20 where spy.evalCount == 0 { try await Task.sleep(nanoseconds: 25_000_000) }
        XCTAssertGreaterThanOrEqual(spy.evalCount, 1, "a live runtime must re-run watchers on data change")

        coordinator.tearDown(spy)
        XCTAssertNil(bridge.webView, "teardown must drop the WebView reference")
        let afterTeardown = spy.evalCount

        // A post AFTER teardown must reach no watcher.
        AppRuntimeView.postDataChanged()
        try await Task.sleep(nanoseconds: 200_000_000)
        XCTAssertEqual(
            spy.evalCount, afterTeardown,
            "a watch callback fired after teardown — the observer was not cancelled"
        )
    }
}

// MARK: - Unit 0C: end-to-end containment through the REAL bridge

extension AppRuntimeHostTests {
    /// A trusted repository with the checklist starter installed, trusted, and
    /// its real app id — the setup the running app actually uses.
    private func trustedRepositoryWithApp() throws -> (RiotProfileRepository, String) {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("app-runtime-revoke-\(UUID().uuidString).json")
        let repository = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url),
            keyStore: FixedWrappingKeyStore(),
            starterPacks: try starterPacks()
        )
        _ = try repository.createPublicSpace(title: "Berlin Mutual Aid")
        let appID = try repository.spaceApps()[0].appIDHex
        try repository.trustApp(appID: appID)
        return (repository, appID)
    }

    /// THE load-bearing test: drive the ACTUAL bridge the running app is handed
    /// (`appDataBridge`), revoke trust through the repository, and prove the next
    /// read AND commit through that same live bridge fail — i.e. the Rust
    /// generation gate reaches the page path, not just the Rust unit test.
    func testRevokingTrustFailsTheNextReadAndCommitThroughTheLiveBridge() throws {
        let (repository, appID) = try trustedRepositoryWithApp()
        let bridge = try XCTUnwrap(repository.appDataBridge(appID: appID))

        try bridge.put(key: "note", valueJSON: "\"hi\"")
        XCTAssertEqual(try bridge.get(key: "note"), "\"hi\"")
        XCTAssertTrue(bridge.isSessionValid())

        // Revoke through the repository — the SAME live bridge must now fail.
        try repository.untrustApp(appID: appID)

        XCTAssertThrowsError(
            try bridge.get(key: "note"),
            "a revoked app must not read through the live bridge it already holds"
        )
        XCTAssertThrowsError(
            try bridge.put(key: "note2", valueJSON: "\"x\""),
            "a revoked app must not commit through the live bridge"
        )
        XCTAssertFalse(
            bridge.isSessionValid(),
            "a revoked session reports invalid so the host routes to Return to Tools"
        )

        // And the blocked write never landed: a fresh bridge after re-approval
        // does not see note2.
        try repository.trustApp(appID: appID)
        let fresh = try XCTUnwrap(repository.appDataBridge(appID: appID))
        XCTAssertNil(try fresh.get(key: "note2"), "the revoked commit must not have reached the store")
    }

    /// §4.7: a bridge failure caused by an INVALIDATED session routes to Return to
    /// Tools (onInvalidated fires, fixed copy), while an ordinary per-op rejection
    /// (a malformed key on a still-valid session) does NOT — it stays inline.
    func testInvalidatedSessionRoutesToReturnToToolsButAPerOpRejectionDoesNot() throws {
        let (repository, appID) = try trustedRepositoryWithApp()
        let bridge = try XCTUnwrap(repository.appDataBridge(appID: appID))
        let controller = AppBridgeController(bridge: bridge)
        var invalidatedFired = false
        controller.onInvalidated = { invalidatedFired = true }

        // A malformed key on a VALID session: rejected, but NOT an invalidation.
        _ = controller.handleForTesting(body: [
            "id": 1, "op": "put", "key": "../escape", "value": "\"x\"",
        ])
        XCTAssertFalse(invalidatedFired, "a per-op rejection must not route to Return to Tools")
        XCTAssertTrue(bridge.isSessionValid())

        // Now revoke; the same op class is an invalidation and MUST route.
        try repository.untrustApp(appID: appID)
        _ = controller.handleForTesting(body: ["id": 2, "op": "get", "key": "note"])
        XCTAssertTrue(
            invalidatedFired,
            "a read that fails because the session was revoked must route to Return to Tools"
        )
    }
}
