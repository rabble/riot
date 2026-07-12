import SwiftUI
import XCTest

#if os(iOS)
    import UIKit
#else
    import AppKit
#endif

@testable import RiotKit

/// The motion kit: it has to draw, in both schemes, without falling over, and
/// it has to never put a raw key on screen where a person's name belongs.
@MainActor
final class MotionTests: XCTestCase {
    private let ana = RiotPerson(
        id: "a3f91122334455660000000000000000000000000000000000000000000000ff",
        displayName: "Ana",
        tag: "a3f91122"
    )

    private let marcus = RiotPerson(
        id: "bb0011223344556600000000000000000000000000000000000000000000ee00",
        displayName: "Marcus",
        tag: "bb001122"
    )

    // MARK: - Every component draws, light and dark

    func testStampSlamRendersInBothSchemes() {
        for scheme in ColorScheme.allCases {
            render(
                RiotCard { Text("Court support needed Thursday") }
                    .riotStampSlam(trigger: "entry-1"),
                scheme: scheme
            )
        }
    }

    func testSyncRippleRendersInBothSchemes() {
        for scheme in ColorScheme.allCases {
            render(
                SyncRipple(attribution: ana.checkedBy) {
                    Text("Bring folding chairs")
                },
                scheme: scheme
            )
        }
    }

    func testSyncRippleRendersWithoutAnAttribution() {
        for scheme in ColorScheme.allCases {
            render(SyncRipple { Text("Bring folding chairs") }, scheme: scheme)
        }
    }

    func testRadarPairingViewRendersInBothSchemes() {
        for scheme in ColorScheme.allCases {
            render(RadarPairingView(peers: []), scheme: scheme)
            render(RadarPairingView(peers: [ana]), scheme: scheme)
            render(RadarPairingView(peers: [ana, marcus]), scheme: scheme)
        }
    }

    func testFinaleBannerRendersInBothSchemes() {
        for scheme in ColorScheme.allCases {
            render(FinaleBanner(isPresented: .constant(true)), scheme: scheme)
            render(FinaleBanner(isPresented: .constant(false)), scheme: scheme)
        }
    }

    // MARK: - Haptics

    /// The point of this test is not that the phone buzzed — it is that the
    /// call COMPILES and RETURNS on whatever platform the suite is running on.
    /// On macOS these are no-op stubs, and a stub that crashed or that forced a
    /// caller into `#if os(iOS)` would defeat the entire design.
    func testHapticsAreCallableAndReturn() {
        Haptics.trustThunk()
        Haptics.syncComplete()
        Haptics.arrival()
    }

    // MARK: - The radar never shows a raw key

    func testRadarWithNoPeersIsSearchingAndNeverAnError() {
        XCTAssertEqual(
            RadarPairingView.state(for: []),
            .searching("Looking for people nearby…")
        )
        XCTAssertEqual(RadarPairingView.searchingMessage, "Looking for people nearby…")
    }

    func testRadarShowsThePeersRenderedDisplayName() {
        XCTAssertEqual(
            RadarPairingView.state(for: [ana]),
            .peers(["Ana · a3f91122"])
        )
    }

    func testRadarNeverPrintsARawKey() {
        guard case let .peers(labels) = RadarPairingView.state(for: [ana, marcus]) else {
            return XCTFail("two peers should not read as searching")
        }
        XCTAssertEqual(labels, ["Ana · a3f91122", "Marcus · bb001122"])

        for (label, peer) in zip(labels, [ana, marcus]) {
            XCTAssertFalse(
                label.contains(peer.id),
                "the radar drew the raw key id: \(label)"
            )
            XCTAssertTrue(label.hasPrefix(peer.displayName + " · "))
            XCTAssertTrue(label.hasSuffix(peer.tag))
        }
    }

    // MARK: - A name is never rendered bare

    func testARenderedNameAlwaysCarriesItsKeyTag() {
        XCTAssertEqual(ana.rendered, "Ana · a3f91122")
        XCTAssertNotEqual(ana.rendered, ana.displayName, "a bare name is never shown")
    }

    func testTwoPeopleClaimingTheSameNameStillRenderDifferently() {
        let impostor = RiotPerson(id: String(repeating: "cd", count: 32), displayName: "Ana", tag: "cdcdcdcd")
        XCTAssertNotEqual(ana.rendered, impostor.rendered)
    }

    func testRippleAttributionReadsAsTheDemoScriptSaysIt() {
        XCTAssertEqual(ana.checkedBy, "checked by Ana · a3f91122")
    }

    // MARK: - Hosting

    /// Hosts the view and forces a real layout + draw pass, so a body that
    /// would trap at runtime traps here instead of on stage.
    private func render(
        _ view: some View,
        scheme: ColorScheme,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        let bounds = CGRect(x: 0, y: 0, width: 390, height: 844)
        let rooted = view
            .environment(\.colorScheme, scheme)
            .frame(width: bounds.width, height: bounds.height)

        #if os(iOS)
            let controller = UIHostingController(rootView: rooted)
            guard let root = controller.view else {
                return XCTFail("hosting controller had no view", file: file, line: line)
            }
            root.frame = bounds
            root.setNeedsLayout()
            root.layoutIfNeeded()
            let renderer = UIGraphicsImageRenderer(bounds: bounds)
            _ = renderer.image { context in
                root.layer.render(in: context.cgContext)
            }
        #else
            let controller = NSHostingController(rootView: rooted)
            let root = controller.view
            root.frame = bounds
            root.layoutSubtreeIfNeeded()
            if let rep = root.bitmapImageRepForCachingDisplay(in: bounds) {
                root.cacheDisplay(in: bounds, to: rep)
            }
        #endif
    }
}
