import WebKit
import XCTest

@testable import RiotKit

final class AppBreadcrumbTests: XCTestCase {
    private final class EmptyBridge: AppDataBridging {
        func put(key: String, valueJSON: String) throws {}
        func get(key: String) throws -> String? { nil }
        func list(prefix: String) throws -> [(key: String, valueJSON: String)] { [] }
        func whoami() -> BridgeProfile {
            BridgeProfile(idHex: String(repeating: "1", count: 64), displayName: "Test", tag: "test")
        }
        func profile(idHex: String) -> BridgeProfile? { nil }
    }

    func testPageTitleAcceptsExactAppSuffix() {
        XCTAssertEqual(
            AppBreadcrumbTitle.page(from: "Meeting guide — Wiki", appName: "Wiki"),
            "Meeting guide"
        )
    }

    func testPageTitleRejectsRootMalformedAndUnsafeTitles() {
        XCTAssertNil(AppBreadcrumbTitle.page(from: "Wiki", appName: "Wiki"))
        XCTAssertNil(AppBreadcrumbTitle.page(from: "Meeting guide - Wiki", appName: "Wiki"))
        XCTAssertNil(AppBreadcrumbTitle.page(from: "Meeting\u{200B} guide — Wiki", appName: "Wiki"))
        XCTAssertNil(AppBreadcrumbTitle.page(from: "\nMeeting guide — Wiki", appName: "Wiki"))
        XCTAssertNil(AppBreadcrumbTitle.page(from: "Meeting guide — Wiki\u{2028}", appName: "Wiki"))
        XCTAssertNil(
            AppBreadcrumbTitle.page(
                from: String(repeating: "x", count: 513) + " — Wiki",
                appName: "Wiki"
            )
        )
        XCTAssertNil(
            AppBreadcrumbTitle.page(
                from: String(repeating: "x", count: 121) + " — Wiki",
                appName: "Wiki"
            )
        )
    }

    func testPageTitleNormalizesAndTrimsOrdinaryWhitespace() {
        XCTAssertEqual(
            AppBreadcrumbTitle.page(from: "  Cafe\u{301} notes — Wiki  ", appName: "Wiki"),
            "Café notes"
        )
    }

    func testResponsiveLabelsKeepMeaningAtBothWidths() {
        let labels = AppBreadcrumbLabels(
            community: "Wellington",
            app: "Wiki",
            page: "Meeting guide"
        )
        XCTAssertEqual(labels.full, ["Wellington", "Wiki", "Meeting guide"])
        XCTAssertEqual(labels.compact, ["🏘", "🧰", "📄"])
    }

    func testResponsiveLabelsOmitPageLevelAtAppRoot() {
        let labels = AppBreadcrumbLabels(community: "Wellington", app: "Wiki", page: nil)
        XCTAssertEqual(labels.full, ["Wellington", "Wiki"])
        XCTAssertEqual(labels.compact, ["🏘", "🧰"])
    }

    func testRoutePolicyClosesOnlyOnMacOS() {
        #if os(macOS)
        XCTAssertTrue(ToolRoutePolicy.closesMountedToolBeforeRoute)
        #else
        XCTAssertFalse(ToolRoutePolicy.closesMountedToolBeforeRoute)
        #endif
    }

    func testChooserSelectionDecisionsWhileToolIsMounted() {
        XCTAssertEqual(
            CommunityChooserSelectionDecision.decide(
                selectedID: "a",
                currentID: "a",
                mountedAppName: "Wiki"
            ),
            .dismissCurrent
        )
        XCTAssertEqual(
            CommunityChooserSelectionDecision.decide(
                selectedID: "b",
                currentID: "a",
                mountedAppName: "Wiki"
            ),
            .confirmSwitch
        )
        XCTAssertEqual(
            CommunityChooserSelectionDecision.decide(
                selectedID: "b",
                currentID: "a",
                mountedAppName: nil
            ),
            .switchImmediately
        )
    }

    func testConfirmedExitClosesBeforeSwitchAndStayDoesNothing() {
        var calls: [String] = []
        CommunityChooserConfirmation.switchCommunity.perform(
            closeTool: { calls.append("close") },
            switchCommunity: { calls.append("switch") }
        )
        XCTAssertEqual(calls, ["close", "switch"])

        calls.removeAll()
        CommunityChooserConfirmation.stay.perform(
            closeTool: { calls.append("close") },
            switchCommunity: { calls.append("switch") }
        )
        XCTAssertTrue(calls.isEmpty)
    }

    @MainActor
    func testRuntimeTeardownHandleRunsSynchronouslyOnce() {
        let handle = AppRuntimeTeardownHandle()
        var calls = 0
        handle.install(tearDown: { calls += 1 }, navigateRoot: {})

        handle.tearDownNow()
        XCTAssertEqual(calls, 1)
        handle.tearDownNow()
        XCTAssertEqual(calls, 1)
    }

    @MainActor
    func testCoordinatorPublishesAndCoalescesPageTitlesThenClearsAtRoot() async {
        let coordinator = AppRuntimeCoordinator(
            bridge: AppBridgeController(bridge: EmptyBridge()),
            appIDHex: String(repeating: "a", count: 64),
            entryPoint: "index.html",
            appName: "Wiki"
        )
        let webView = WKWebView()
        var published: [String?] = []
        let page = expectation(description: "nested page title")
        let root = expectation(description: "root title")
        coordinator.onPageTitleChanged = { title in
            published.append(title)
            if title == "Meeting guide" { page.fulfill() }
            if title == nil, published.contains(where: { $0 == "Meeting guide" }) { root.fulfill() }
        }
        coordinator.observePageTitles(in: webView)
        webView.loadHTMLString(
            """
            <title>Meeting guide — Wiki</title>
            <script>
              addEventListener("riot:navigate-root", () => { document.title = "Wiki"; });
            </script>
            """,
            baseURL: nil
        )

        await fulfillment(of: [page], timeout: 5)
        coordinator.navigateToAppRoot()
        await fulfillment(of: [root], timeout: 5)

        XCTAssertEqual(published.filter { $0 == "Meeting guide" }.count, 1)
        XCTAssertNil(published.last!)
        coordinator.tearDown(webView)
    }
}
