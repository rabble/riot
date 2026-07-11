import XCTest
@testable import RiotKit

final class RiotTabBarTests: XCTestCase {
    func testItemsMatchPhoneTabsInOrder() {
        XCTAssertEqual(RiotTabBar.items.map(\.destination), RiotDestination.phoneTabs)
        XCTAssertEqual(RiotTabBar.items.map(\.label), RiotDestination.phoneTabs.map(\.tabTitle))
        XCTAssertEqual(RiotTabBar.items.map(\.systemImage), RiotDestination.phoneTabs.map(\.systemImage))
    }
}
