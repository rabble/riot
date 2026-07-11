import XCTest
import SwiftUI
@testable import RiotKit

final class RiotThemeTests: XCTestCase {
    func testPaperColorMatchesMarketingSiteTokens() {
        assertColor(RiotTheme.paper(for: .light), hex: 0xEAE6DA)
        assertColor(RiotTheme.paper(for: .dark), hex: 0x131209)
    }

    func testInkColorMatchesMarketingSiteTokens() {
        assertColor(RiotTheme.ink(for: .light), hex: 0x17160F)
        assertColor(RiotTheme.ink(for: .dark), hex: 0xEFE9D8)
    }

    func testAccentColorsMatchMarketingSiteTokens() {
        assertColor(RiotTheme.blue(for: .light), hex: 0x22399F)
        assertColor(RiotTheme.blue(for: .dark), hex: 0x6D84FF)
        assertColor(RiotTheme.pink(for: .light), hex: 0xD1216E)
        assertColor(RiotTheme.pink(for: .dark), hex: 0xFF5F9E)
    }

    func testFontRolesMapToExpectedPostScriptNames() {
        XCTAssertEqual(RiotFontRole.poster.postScriptName, "Anton-Regular")
        XCTAssertEqual(RiotFontRole.body.postScriptName, "WorkSans-Regular")
        XCTAssertEqual(RiotFontRole.mono.postScriptName, "SpaceMono-Regular")
        XCTAssertEqual(RiotFontRole.monoBold.postScriptName, "SpaceMono-Bold")
    }

    private func assertColor(_ color: Color, hex: UInt32, file: StaticString = #filePath, line: UInt = #line) {
        let resolved = UIColor(color)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        resolved.getRed(&r, green: &g, blue: &b, alpha: &a)
        let expectedR = CGFloat((hex >> 16) & 0xFF) / 255
        let expectedG = CGFloat((hex >> 8) & 0xFF) / 255
        let expectedB = CGFloat(hex & 0xFF) / 255
        XCTAssertEqual(r, expectedR, accuracy: 0.01, "red", file: file, line: line)
        XCTAssertEqual(g, expectedG, accuracy: 0.01, "green", file: file, line: line)
        XCTAssertEqual(b, expectedB, accuracy: 0.01, "blue", file: file, line: line)
    }
}
