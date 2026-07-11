import Network
import WebKit
import XCTest
@testable import RiotKit

/// The claim this file exists to test: a checklist item one person adds on
/// their phone actually shows up on the other person's phone.
///
/// Nothing here is a stand-in for the thing being proven. Both peers are real
/// `RiotProfileRepository` profiles over the real FFI, each with its own
/// storage, both in the same space, both with the checklist app installed and
/// trusted. Writes go through the same `appDataBridge(appID:)` the WebView
/// uses. The two peers are wired to each other over a real TCP socket on
/// loopback, carrying the real length-prefixed frames, driven by the app's own
/// `SyncCoordinator` on both ends. The last test loads the item into a real
/// WKWebView and reads it back out of the rendered DOM, so what is asserted is
/// what a person would see.
///
/// The one substitution — and it is only under the socket — is that the test
/// builds the two `NWConnection`s itself instead of calling
/// `LocalNetworkListener` / `LocalTCPFrameChannel.attempt`. Those two pin
/// `requiredInterfaceType = .wifi`, and loopback is not Wi-Fi, so in the
/// simulator they cannot connect to each other at all. Everything from the
/// frame codec upwards is the shipping code.
///
/// One thing this file does NOT reproduce, deliberately: exactly one peer here
/// calls `SyncCoordinator.start()`. It has to — the core's `ReconcileSession`
/// accepts a Hello only while it is idle, so a peer that has begun cannot also
/// answer one. `NearbyTransportController` starts the coordinator on BOTH peers
/// (`startLocalSession` and `finishRouteSelection` each run on both sides of a
/// pairing). Wired that way, these same two profiles go straight to `.failed`
/// and nothing replicates. That is a bug in the controller, and fixing it is
/// not this file's job.
@MainActor
final class AppSyncReplicationTests: XCTestCase {

    // MARK: - Peers

    /// One person's phone: their own profile, their own storage, their own copy
    /// of the checklist app.
    private struct Peer {
        let name: String
        let repository: RiotProfileRepository
        let appID: String
        let storageURL: URL
        let keyStore: WrappingKeyStore
    }

    /// Two people in the same space, each with the checklist installed and
    /// trusted. Alice creates the space; Bob joins it.
    ///
    /// Bob "joins" by opening a profile whose stored snapshot already names
    /// Alice's space — `RiotProfileRepository.open` calls `joinPublicSpace` for
    /// the persisted space, which is the same core call a real join performs.
    /// (There is no repository-level join API to call instead; the space
    /// travels out of band, by design.)
    private func openPair() throws -> (alice: Peer, bob: Peer) {
        let alice = try openPeer(name: "Alice", joining: nil)
        let space = try XCTUnwrap(alice.repository.currentSpace)
        let bob = try openPeer(name: "Bob", joining: space)
        XCTAssertEqual(
            alice.appID, bob.appID,
            "both peers must be running the same app — the id is content-derived"
        )
        return (alice, bob)
    }

    private func openPeer(name: String, joining space: RiotSpace?) throws -> Peer {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("app-sync-\(name)-\(UUID().uuidString).json")
        if let space { try seedSpace(space, at: url) }
        let keyStore = TestWrappingKeyStore()
        let repository = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: url),
            keyStore: keyStore,
            starterPacks: try starterPacks()
        )
        if space == nil { _ = try repository.createPublicSpace(title: "Berlin Mutual Aid") }
        let appID = try XCTUnwrap(repository.spaceApps().first).appIDHex
        // Trust is host-side and per profile: without it this profile hands out
        // no bridge, so each peer must make its own decision.
        try repository.trustApp(appID: appID)
        return Peer(name: name, repository: repository, appID: appID, storageURL: url, keyStore: keyStore)
    }

    /// Writes the on-disk snapshot a phone that has joined `space` would hold,
    /// so that opening it joins that space. Identity is left unset, so the open
    /// mints a fresh one — Bob is a different person from Alice.
    private func seedSpace(_ space: RiotSpace, at url: URL) throws {
        let snapshot: [String: Any] = [
            "space": ["namespaceID": space.namespaceID, "title": space.title],
            "alerts": [],
            "trustedAppIDs": [],
            "appDataBundles": [],
        ]
        try JSONSerialization.data(withJSONObject: snapshot).write(to: url, options: .atomic)
    }

    /// Repo root derived from this file at `apps/ios/RiotTests/…`, matching the
    /// other suites, so the frozen starter artifacts load without a bundle
    /// resource.
    private func starterPacks() throws -> [(manifest: Data, bundle: Data)] {
        let apps = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // RiotTests
            .deletingLastPathComponent() // ios
            .deletingLastPathComponent() // apps
            .deletingLastPathComponent() // repo root
            .appendingPathComponent("fixtures/apps")
        return [(
            manifest: try Data(contentsOf: apps.appendingPathComponent("checklist.manifest.cbor")),
            bundle: try Data(contentsOf: apps.appendingPathComponent("checklist.bundle.cbor"))
        )]
    }

    // MARK: - The claim

    /// The core claim: Alice adds an item, they sync, Bob has the item.
    func testItemAddedOnPeerAAppearsOnPeerB() async throws {
        let (alice, bob) = try openPair()
        let key = newItemKey()
        let added = try item(text: "Bring water to the corner", done: false, by: "Alice", at: 1)

        // The same call the page makes when someone types an item and taps Add.
        try XCTUnwrap(alice.repository.appDataBridge(appID: alice.appID)).put(key: key, valueJSON: added)
        XCTAssertNil(
            try bob.repository.appDataGet(appID: bob.appID, key: key),
            "Bob must not already have the item — otherwise this proves nothing"
        )

        try await sync(initiator: bob, responder: alice)

        XCTAssertEqual(
            try bob.repository.appDataGet(appID: bob.appID, key: key), added,
            "Bob did not receive the item Alice added"
        )
    }

    /// The item does not merely arrive in Bob's store — it renders on his
    /// screen. Bob's real app-runtime mount (the trust-gated bridge and
    /// resolver `AppRuntimeView` uses) is loaded into a real WKWebView and the
    /// item's text is read back out of the DOM the checklist built.
    func testReplicatedItemRendersInPeerBsChecklistWebView() async throws {
        let (alice, bob) = try openPair()
        let key = newItemKey()
        let text = "Bring water to the corner"
        try XCTUnwrap(alice.repository.appDataBridge(appID: alice.appID))
            .put(key: key, valueJSON: try item(text: text, done: false, by: "Alice", at: 1))

        try await sync(initiator: bob, responder: alice)

        let launch = try XCTUnwrap(
            AppRuntimeLaunch(repository: bob.repository, appIDHex: bob.appID),
            "Bob's trusted app must produce mount inputs"
        )
        let (webView, probe) = makeWebView(launch: launch)
        await loadEntryPoint(webView, probe, appID: launch.appIDHex)

        // The page renders on its own (`riot.watch` lists at load), so poll the
        // DOM rather than the store. Timer polls do not fire under XCTWaiter;
        // `callAsyncJavaScript` awaits, which yields the main thread.
        var labels: [String] = []
        for _ in 0..<60 where labels.isEmpty {
            labels = try await renderedItemLabels(webView)
        }
        XCTAssertEqual(labels, [text], "Alice's item is not on Bob's screen")

        let emptyHidden = try await callAsync(webView, "return document.getElementById('empty').hidden;")
        XCTAssertEqual(
            emptyHidden as? Bool, true,
            "the checklist still tells Bob there is nothing here"
        )
    }

    /// The other direction: Bob checks the item off, and Alice sees it checked.
    func testCheckingOffOnPeerBSyncsBackToPeerA() async throws {
        let (alice, bob) = try openPair()
        let key = newItemKey()
        try XCTUnwrap(alice.repository.appDataBridge(appID: alice.appID))
            .put(key: key, valueJSON: try item(text: "Bring water to the corner", done: false, by: "Alice", at: 1))
        try await sync(initiator: bob, responder: alice)

        // Willow orders same-path writes by timestamp, and the core stamps app
        // writes with whole seconds (`unix_seconds * 1_000_000`). Two writes
        // inside one second tie, and a tie is broken by payload digest — not by
        // who wrote last. A person checking an item off is always at least a
        // second after it was added, so wait for the second to turn rather than
        // race the clock and assert a coin flip.
        try await Task.sleep(for: .milliseconds(1_100))

        // Read-modify-write through Bob's own bridge — what the checkbox does.
        let onBob = try XCTUnwrap(try bob.repository.appDataGet(appID: bob.appID, key: key))
        var value = try XCTUnwrap(
            try JSONSerialization.jsonObject(with: Data(onBob.utf8)) as? [String: Any]
        )
        value["done"] = true
        value["updated_by"] = "Bob"
        try XCTUnwrap(bob.repository.appDataBridge(appID: bob.appID))
            .put(key: key, valueJSON: try json(value))

        try await sync(initiator: alice, responder: bob)

        let onAlice = try XCTUnwrap(
            try alice.repository.appDataGet(appID: alice.appID, key: key),
            "Alice lost the item entirely"
        )
        let seen = try XCTUnwrap(
            try JSONSerialization.jsonObject(with: Data(onAlice.utf8)) as? [String: Any]
        )
        XCTAssertEqual(seen["done"] as? Bool, true, "Alice does not see Bob's check-off")
        XCTAssertEqual(seen["updated_by"] as? String, "Bob")
    }

    /// The connection itself: the coordinator opens the protocol, previews what
    /// arrived, accepts it, and terminates cleanly on both ends.
    ///
    /// The preview count is asserted as it really is — 0 — and that is not a
    /// typo. `count` is the number of *alert* rows in the import, and app data
    /// is deliberately not a fake alert row, so an app-data-only sync previews
    /// as "0 new updates" even though it is about to import a real item. The
    /// data lands (every other test here proves that); the number the person is
    /// shown is wrong, which is a UI bug, not a sync bug.
    func testSyncReachesCaughtUpState() async throws {
        let (alice, bob) = try openPair()
        try XCTUnwrap(alice.repository.appDataBridge(appID: alice.appID))
            .put(key: newItemKey(), valueJSON: try item(text: "Cones", done: false, by: "Alice", at: 1))

        let states = try await sync(initiator: bob, responder: alice)

        XCTAssertEqual(
            states.initiator,
            [
                .gettingLatest(name: "Alice"),   // opened the protocol
                .preview(count: 0, name: "Alice"), // Alice's entries are here, pending the person's yes
                .gettingLatest(name: "Alice"),   // accepted; now offering his own
                .caughtUp,                        // done, session closed
            ],
            "the initiator did not run connect → preview → accept → done"
        )
        // The responder imports too, in the same exchange — it never opened the
        // protocol, so it publishes no `gettingLatest`, but Bob's own entries
        // (his trust marker, his copy of the app index) are entries Alice does
        // not have, so she previews and accepts them before completing. One
        // session, both directions.
        XCTAssertEqual(
            states.responder,
            [
                .preview(count: 0, name: "Bob"),
                .caughtUp,
            ],
            "the responder did not preview → accept → done"
        )
    }

    /// Both people edit the same item while apart, then sync. They must end up
    /// agreeing — that is the whole promise of a shared list.
    ///
    /// Who wins: Willow's recency order (timestamp, then payload digest, then
    /// payload length). Bob's edit is deliberately a second later, so his
    /// timestamp is strictly greater and Bob's value wins on BOTH phones. Had
    /// they landed inside the same second the digest would decide instead —
    /// arbitrary, but computed identically by both peers, so they would still
    /// converge. Convergence is the invariant; "latest wins" holds whenever the
    /// clock can tell the writes apart.
    func testConcurrentEditsToTheSameItemConverge() async throws {
        let (alice, bob) = try openPair()
        let key = newItemKey()
        try XCTUnwrap(alice.repository.appDataBridge(appID: alice.appID))
            .put(key: key, valueJSON: try item(text: "Bring water", done: false, by: "Alice", at: 1))
        try await sync(initiator: bob, responder: alice)

        // Apart: neither write can reach the other yet.
        let aliceEdit = try item(text: "Bring water — 6 crates", done: false, by: "Alice", at: 2)
        try XCTUnwrap(alice.repository.appDataBridge(appID: alice.appID)).put(key: key, valueJSON: aliceEdit)
        try await Task.sleep(for: .milliseconds(1_100))
        let bobEdit = try item(text: "Bring water — 6 crates", done: true, by: "Bob", at: 3)
        try XCTUnwrap(bob.repository.appDataBridge(appID: bob.appID)).put(key: key, valueJSON: bobEdit)

        try await sync(initiator: bob, responder: alice)

        let onAlice = try XCTUnwrap(try alice.repository.appDataGet(appID: alice.appID, key: key))
        let onBob = try XCTUnwrap(try bob.repository.appDataGet(appID: bob.appID, key: key))
        XCTAssertEqual(onAlice, onBob, "the two phones disagree about the same item")
        XCTAssertEqual(onBob, bobEdit, "the later edit did not win")
        XCTAssertNotEqual(onAlice, aliceEdit, "Alice kept her own stale edit")
    }

    // MARK: - Sync over a real socket

    /// One full nearby exchange between two open profiles: a real loopback TCP
    /// socket, the real frame codec, and the app's own `SyncCoordinator` on both
    /// ends. Returns the state each coordinator published, in order.
    ///
    /// Exactly one peer opens the protocol. The core's `ReconcileSession` only
    /// accepts a Hello while it is idle, so the responder must NOT begin —
    /// it answers. The single exchange still carries data BOTH ways: after the
    /// initiator imports, it offers its own summary and the responder requests
    /// what it is missing in the same session.
    @discardableResult
    private func sync(
        initiator: Peer,
        responder: Peer,
        timeout: TimeInterval = 30
    ) async throws -> (initiator: [NearbyConnectionState], responder: [NearbyConnectionState]) {
        let wire = DispatchQueue(label: "net.protest.riot.tests.wire")
        let (dialled, accepted) = try await connectedChannels(on: wire)

        let initiatorSide = try SyncPeerDriver(
            boundary: initiator.repository.openSyncBoundary(),
            channel: dialled,
            peerName: responder.name,
            wire: wire
        )
        let responderSide = try SyncPeerDriver(
            boundary: responder.repository.openSyncBoundary(),
            channel: accepted,
            peerName: initiator.name,
            wire: wire
        )
        defer {
            initiatorSide.stop()
            responderSide.stop()
        }

        wire.async { initiatorSide.start() }
        await fulfillment(of: [initiatorSide.done, responderSide.done], timeout: timeout)

        return (initiatorSide.states, responderSide.states)
    }

    /// A connected pair of the app's real `LocalTCPFrameChannel`s, over a real
    /// TCP socket on loopback. Both connections are delivered on `wire`, so each
    /// coordinator is only ever entered from that one queue.
    ///
    /// The sockets are built here rather than through `LocalNetworkListener` /
    /// `LocalTCPFrameChannel.attempt` because those demand
    /// `requiredInterfaceType = .wifi`; loopback is not Wi-Fi, and in the
    /// simulator there is no other way for two peers in one process to reach
    /// each other. The channel — framing, buffering, failure handling — is the
    /// shipping type.
    private func connectedChannels(
        on wire: DispatchQueue
    ) async throws -> (dialled: LocalTCPFrameChannel, accepted: LocalTCPFrameChannel) {
        let listener = try NWListener(using: .tcp, on: .any)
        let acceptedChannel = OneShot<LocalTCPFrameChannel>()
        let listening = OneShot<ListenerStart>()
        listener.newConnectionHandler = { connection in
            connection.start(queue: wire)
            acceptedChannel.resume(with: LocalTCPFrameChannel(connection: connection))
        }
        listener.stateUpdateHandler = { state in
            switch state {
            case .ready:
                listener.port.map { listening.resume(with: .ready($0)) }
            case .failed:
                listening.resume(with: .failed)
            default:
                break
            }
        }
        listener.start(queue: wire)
        guard case let .ready(port) = await listening.value() else {
            throw NearbyTransportError.notConnected
        }

        let connection = NWConnection(host: "127.0.0.1", port: port, using: .tcp)
        let dialledChannel = OneShot<LocalTCPFrameChannel>()
        connection.stateUpdateHandler = { state in
            if case .ready = state {
                dialledChannel.resume(with: LocalTCPFrameChannel(connection: connection))
            }
        }
        connection.start(queue: wire)

        let dialled = await dialledChannel.value()
        let accepted = await acceptedChannel.value()
        listener.cancel()
        return (dialled, accepted)
    }

    // MARK: - Bob's screen

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
        func webView(
            _ webView: WKWebView,
            didFailProvisionalNavigation navigation: WKNavigation!,
            withError error: Error
        ) {
            failure = "didFailProvisional: \(error)"
            done.fulfill()
        }
    }

    /// The same mount `AppRuntimeView` builds: the profile's trust-gated bridge
    /// and resolver, the injected `window.riot`, and the app scheme handler.
    private func makeWebView(launch: AppRuntimeLaunch) -> (WKWebView, NavProbe) {
        let bridge = AppBridgeController(bridge: launch.bridge)
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.userContentController.addUserScript(
            WKUserScript(source: RiotJS.source, injectionTime: .atDocumentStart, forMainFrameOnly: true)
        )
        configuration.userContentController.add(bridge, name: "riot")
        configuration.setURLSchemeHandler(
            AppSchemeHandler(resolver: launch.resolver),
            forURLScheme: AppSchemeHandler.scheme
        )
        let webView = WKWebView(frame: .zero, configuration: configuration)
        bridge.webView = webView
        let probe = NavProbe()
        webView.navigationDelegate = probe
        return (webView, probe)
    }

    private func loadEntryPoint(_ webView: WKWebView, _ probe: NavProbe, appID: String) async {
        webView.load(URLRequest(url: URL(string: "\(AppSchemeHandler.scheme)://\(appID)/index.html")!))
        await fulfillment(of: [probe.done], timeout: 30)
        XCTAssertTrue(probe.finished, "checklist page failed to load: \(probe.failure ?? "unknown")")
    }

    /// The visible text of every item the checklist actually rendered.
    private func renderedItemLabels(_ webView: WKWebView) async throws -> [String] {
        let json = try await callAsync(webView, """
            return JSON.stringify(
              Array.from(document.querySelectorAll('#items li label')).map((el) => el.textContent)
            );
        """) as? String
        guard let json,
              let labels = try JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String]
        else { return [] }
        return labels
    }

    private func callAsync(_ webView: WKWebView, _ body: String) async throws -> Any? {
        try await webView.callAsyncJavaScript(body, arguments: [:], contentWorld: .page)
    }

    // MARK: - Values

    /// A fresh item key. Key segments are `[a-z0-9-]` in the core, and the
    /// checklist's own `crypto.randomUUID()` is lowercase — Swift's is not.
    private func newItemKey() -> String {
        "items/\(UUID().uuidString.lowercased())"
    }

    /// One checklist row, shaped exactly as `fixtures/apps/checklist/app.js`
    /// writes it.
    private func item(text: String, done: Bool, by author: String, at updatedAt: Int) throws -> String {
        try json(["text": text, "done": done, "updated_by": author, "updated_at": updatedAt])
    }

    private func json(_ value: [String: Any]) throws -> String {
        String(
            decoding: try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]),
            as: UTF8.self
        )
    }
}

// MARK: - One peer's coordinator

/// One end of a live sync: the app's own `SyncCoordinator` over a real channel,
/// plus the states it published. Every call into the coordinator happens on the
/// `wire` queue — the same queue Network.framework delivers this peer's frames
/// on — so the coordinator is never entered from two threads at once.
private final class SyncPeerDriver: @unchecked Sendable {
    /// Fulfilled once the session reaches a terminal state, however it ends.
    let done = XCTestExpectation(description: "sync terminal state")

    private let coordinator: SyncCoordinator
    private let lock = NSLock()
    private var observed: [NearbyConnectionState] = []

    init(
        boundary: MobileSyncSessionBoundary,
        channel: FrameChannel,
        peerName: String,
        wire: DispatchQueue
    ) throws {
        let connection = NearbyConnection(base: channel, baseRoute: .localNetwork, localAttempt: { nil })
        connection.confirmPairing()
        try connection.activate()
        let coordinator = SyncCoordinator(session: boundary, connection: connection, friendlyName: peerName)
        self.coordinator = coordinator
        done.assertForOverFulfill = false

        coordinator.onStateChanged = { [weak self] state in
            guard let self else { return }
            self.lock.lock()
            self.observed.append(state)
            self.lock.unlock()
            switch state {
            case .preview:
                // The person tapping "Add these updates". Queued rather than run
                // inside the state callback so it lands after the frame that
                // produced the preview has been fully handled.
                wire.async { self.coordinator.addPreviewedContent() }
            case .caughtUp, .alreadyCurrent, .failed:
                self.done.fulfill()
            default:
                break
            }
        }
    }

    func start() { coordinator.start() }
    func stop() { coordinator.stop() }

    var states: [NearbyConnectionState] {
        lock.lock()
        defer { lock.unlock() }
        return observed
    }
}

private enum ListenerStart: Sendable {
    case ready(NWEndpoint.Port)
    case failed
}

/// A value produced once by a Network.framework callback and awaited elsewhere.
/// Resuming a continuation twice traps, and state handlers do fire more than
/// once, so every resume goes through here.
private final class OneShot<Value: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private var stored: Value?
    private var waiter: ((Value) -> Void)?

    func resume(with value: Value) {
        lock.lock()
        guard stored == nil else { return lock.unlock() }
        stored = value
        let waiter = self.waiter
        self.waiter = nil
        lock.unlock()
        waiter?(value)
    }

    func value() async -> Value {
        await withCheckedContinuation { continuation in
            lock.lock()
            if let stored {
                lock.unlock()
                return continuation.resume(returning: stored)
            }
            waiter = { continuation.resume(returning: $0) }
            lock.unlock()
        }
    }
}

/// Duplicated per the project convention (each suite keeps its own): a fixed
/// 32-byte key so sealed identities round-trip.
private final class TestWrappingKeyStore: WrappingKeyStore {
    private var key: Data?

    func loadOrCreateWrappingKey() throws -> Data {
        if let key { return key }
        let created = Data(repeating: 0x5a, count: 32)
        key = created
        return created
    }
}

