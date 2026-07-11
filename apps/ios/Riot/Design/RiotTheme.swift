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
}
