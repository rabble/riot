import SwiftUI

public enum RiotTheme {
    public static func paper(for scheme: ColorScheme) -> Color {
        scheme == .dark ? hex(0x131209) : hex(0xEAE6DA)
    }

    public static func paper2(for scheme: ColorScheme) -> Color {
        scheme == .dark ? hex(0x1C1A10) : hex(0xE1DCCB)
    }

    public static func ink(for scheme: ColorScheme) -> Color {
        scheme == .dark ? hex(0xEFE9D8) : hex(0x17160F)
    }

    public static func inkSoft(for scheme: ColorScheme) -> Color {
        scheme == .dark ? hex(0xBEB69E) : hex(0x4A473B)
    }

    public static func blue(for scheme: ColorScheme) -> Color {
        scheme == .dark ? hex(0x6D84FF) : hex(0x22399F)
    }

    public static func pink(for scheme: ColorScheme) -> Color {
        scheme == .dark ? hex(0xFF5F9E) : hex(0xD1216E)
    }

    public static func line(for scheme: ColorScheme) -> Color {
        ink(for: scheme).opacity(scheme == .dark ? 0.16 : 0.18)
    }

    public static func lineStrong(for scheme: ColorScheme) -> Color {
        ink(for: scheme).opacity(scheme == .dark ? 0.36 : 0.4)
    }

    /// The card surface — a soft, near-white sheet that floats a touch lighter than
    /// the warm paper ground in light, and a warm charcoal in dark. Paired with a
    /// hairline `line(for:)` border and a rounded corner, this is what turns the old
    /// hard-bordered box into the reference's calm paper card.
    public static func card(for scheme: ColorScheme) -> Color {
        scheme == .dark ? hex(0x201E16) : hex(0xFCFAF4)
    }

    /// The single accent for a primary action — a grounded, civic green. One filled
    /// pill per card wears this; everything else stays quiet. Slightly brighter in
    /// dark so the pill reads without glare.
    public static func accent(for scheme: ColorScheme) -> Color {
        scheme == .dark ? hex(0x34A06E) : hex(0x1E6B4F)
    }

    /// Text/gliphs that sit on top of the accent pill — always the light paper tone,
    /// in either scheme, so the label stays legible on green.
    public static func onAccent(for _: ColorScheme) -> Color {
        hex(0xF6F2E9)
    }

    /// A stable, key-derived disc colour for a person's initials avatar. Not
    /// decoration: two people who both claim "Ana" get different discs because the
    /// colour is a pure function of their key, so the eye can tell them apart the
    /// same way the tag does. Deterministic across runs (a summed-scalar hash, never
    /// `String.hashValue`, which is seeded per launch).
    public static func avatarColor(forKey key: String) -> Color {
        let palette: [UInt32] = [0xC8791F, 0x1E6B4F, 0x2B6CB0, 0x7A4F9E, 0xB0522E]
        let sum = key.unicodeScalars.reduce(0) { $0 &+ Int($1.value) }
        return hex(palette[sum % palette.count])
    }

    private static func hex(_ value: UInt32) -> Color {
        Color(
            red: Double((value >> 16) & 0xFF) / 255,
            green: Double((value >> 8) & 0xFF) / 255,
            blue: Double(value & 0xFF) / 255
        )
    }
}

public enum RiotFontRole {
    case poster
    case body
    case mono
    case monoBold

    var postScriptName: String {
        switch self {
        case .poster: return "Anton-Regular"
        case .body: return "WorkSans-Regular"
        case .mono: return "SpaceMono-Regular"
        case .monoBold: return "SpaceMono-Bold"
        }
    }
}

public extension Font {
    static func riot(_ role: RiotFontRole, size: CGFloat, relativeTo textStyle: Font.TextStyle = .body) -> Font {
        .custom(role.postScriptName, size: size, relativeTo: textStyle)
    }

    /// The editorial serif — used only for headings a person reads AS writing
    /// (a report headline, a community's name), never for chrome or data. "Iowan
    /// Old Style" ships on both iOS and macOS; `.custom` falls back to the system
    /// serif if a platform lacks it, and scales with Dynamic Type via `relativeTo`.
    static func riotSerif(size: CGFloat, relativeTo textStyle: Font.TextStyle = .body) -> Font {
        .custom("Iowan Old Style", size: size, relativeTo: textStyle)
    }
}
