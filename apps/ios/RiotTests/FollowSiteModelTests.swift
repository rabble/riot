import XCTest
@testable import RiotKit

/// Pure screening + row-projection tests for the follow-a-site flow. No device,
/// no FFI verify — those live in the core `follow_site` tests. This pins the
/// scheme/length screen and the honest row mapping (crucially: a transport-blocked
/// row offers NO refresh and carries NO fetch URL).
final class FollowSiteModelTests: XCTestCase {
    private let model = FollowSiteModel()

    func testScreenAcceptsASiteTicketAndTrims() throws {
        let ticket = try model.screen(ticket: "  riot://site/v1/abc123  ")
        XCTAssertEqual(ticket, "riot://site/v1/abc123")
    }

    func testScreenRejectsAForeignScheme() {
        XCTAssertThrowsError(try model.screen(ticket: "riot://newswire/join/v1/xyz")) { error in
            XCTAssertEqual(error as? FollowSiteError, .notASiteTicket)
        }
    }

    func testScreenRejectsAnOversizePayload() {
        let huge = "riot://site/v1/" + String(repeating: "a", count: 5000)
        XCTAssertThrowsError(try model.screen(ticket: huge)) { error in
            XCTAssertEqual(error as? FollowSiteError, .tooLong)
        }
    }

    func testHexBytesDecodesA32ByteRoot() {
        let hex = String(repeating: "0a", count: 32)
        XCTAssertEqual(FollowSiteModel.hexBytes(hex), Array(repeating: 0x0a, count: 32))
    }

    func testHexBytesRejectsWrongLengthOrNonHex() {
        XCTAssertNil(FollowSiteModel.hexBytes(String(repeating: "0a", count: 31)))
        XCTAssertNil(FollowSiteModel.hexBytes(String(repeating: "zz", count: 32)))
    }

    func testDisplayShowsRefreshForAnAvailableSiteWithURL() {
        let display = FollowedSiteDisplay(
            root: "r", title: "Bay Area IMC", stateLabel: "",
            transportBlocked: false, fetchURL: "https://mirror.example/site.bundle")
        XCTAssertTrue(display.canRefresh)
    }

    func testDisplayHoldsBackRefreshForATransportBlockedSite() {
        // Even if a URL were somehow present, a blocked row must not offer a fetch.
        let display = FollowedSiteDisplay(
            root: "r", title: "Hidden Service", stateLabel: "",
            transportBlocked: true, fetchURL: "https://leak.example/site.bundle")
        XCTAssertFalse(display.canRefresh)
    }

    func testStateLabelsAreHumanReadable() {
        XCTAssertEqual(FollowedSiteDisplay.label(forState: "available"), "Up to date")
        XCTAssertEqual(FollowedSiteDisplay.label(forState: "pending-first-sync"),
                       "Waiting for first sync")
        XCTAssertEqual(FollowedSiteDisplay.label(forState: "novel-token"), "novel-token")
    }
}
