import XCTest
import SwiftUI
@testable import RiotKit

@MainActor
final class RiotTabBarTests: XCTestCase {
    /// The iPhone tab bar is exactly the four community routes, in canonical
    /// order, each with its own label and icon (selection is never by color
    /// alone — §4.6).
    func testItemsAreTheFourRoutesInCanonicalOrder() {
        XCTAssertEqual(
            RiotTabBar.items.map(\.destination),
            [.home, .tools, .people, .nearby]
        )
        XCTAssertEqual(RiotTabBar.items.map(\.label), ["Home", "Tools", "People", "Nearby"])
        XCTAssertEqual(
            RiotTabBar.items.map(\.systemImage),
            RiotDestination.phoneTabs.map(\.systemImage)
        )
    }

    func testItemsTrackPhoneTabs() {
        XCTAssertEqual(RiotTabBar.items.map(\.destination), RiotDestination.phoneTabs)
        XCTAssertEqual(RiotTabBar.items.map(\.label), RiotDestination.phoneTabs.map(\.tabTitle))
    }

    func testTabBarUsesTwoRowsOnlyAtAccessibilitySizes() {
        XCTAssertEqual(RiotTabBar.layout(for: .xxxLarge), .horizontal)
        XCTAssertEqual(RiotTabBar.layout(for: .accessibility1), .accessibilityGrid)
        XCTAssertEqual(RiotTabBar.layout(for: .accessibility5), .accessibilityGrid)
    }

    /// The unread badge shows nothing at zero, the exact count through nine, and
    /// caps at "9+" so a large count never widens the tab.
    func testBadgeTextGatesZeroCountsAndNine() {
        XCTAssertNil(RiotTabBar.badgeText(forCount: 0))
        XCTAssertNil(RiotTabBar.badgeText(forCount: -3))
        XCTAssertEqual(RiotTabBar.badgeText(forCount: 1), "1")
        XCTAssertEqual(RiotTabBar.badgeText(forCount: 9), "9")
        XCTAssertEqual(RiotTabBar.badgeText(forCount: 10), "9+")
        XCTAssertEqual(RiotTabBar.badgeText(forCount: 250), "9+")
    }

    func testTabAccessibilityLabelIncludesTheActualUnreadCount() {
        XCTAssertEqual(
            RiotTabBar.accessibilityLabel(for: RiotTabBar.items[0], unreadCount: 0),
            "Home"
        )
        XCTAssertEqual(
            RiotTabBar.accessibilityLabel(for: RiotTabBar.items[0], unreadCount: 12),
            "Home, 12 unread"
        )
    }
}
