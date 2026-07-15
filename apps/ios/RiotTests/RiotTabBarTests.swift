import XCTest
@testable import RiotKit

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
}
