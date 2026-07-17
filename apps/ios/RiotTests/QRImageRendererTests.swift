import XCTest
import CoreGraphics
@testable import RiotKit

final class QRImageRendererTests: XCTestCase {
    func testRendersANonEmptyImageForAValidJoinLink() throws {
        let link = "riot://newswire/join/v1/00112233445566778899aabbccddeeff"
        let image = try XCTUnwrap(
            QRImageRenderer.makeQRCode(from: link),
            "a valid join link must render a QR raster"
        )
        XCTAssertGreaterThan(image.width, 0)
        XCTAssertGreaterThan(image.height, 0)
    }

    func testBlankInputRendersNothing() {
        XCTAssertNil(QRImageRenderer.makeQRCode(from: ""))
        XCTAssertNil(QRImageRenderer.makeQRCode(from: "   \n  "))
    }

    func testLongerPayloadIsAtLeastAsDense() throws {
        // A denser payload needs at least as many QR modules => not a smaller raster.
        let short = try XCTUnwrap(QRImageRenderer.makeQRCode(from: "riot://newswire/join/v1/aa"))
        let long = try XCTUnwrap(
            QRImageRenderer.makeQRCode(from: "riot://newswire/join/v1/" + String(repeating: "a", count: 200))
        )
        XCTAssertGreaterThanOrEqual(long.width, short.width)
    }
}
