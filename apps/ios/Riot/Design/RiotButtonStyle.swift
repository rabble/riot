import SwiftUI

public enum RiotButtonEmphasis {
    case primary
    case secondary
}

public struct RiotButtonStyle: ButtonStyle {
    @Environment(\.colorScheme) private var colorScheme
    private let emphasis: RiotButtonEmphasis

    public init(_ emphasis: RiotButtonEmphasis = .primary) {
        self.emphasis = emphasis
    }

    public func makeBody(configuration: Configuration) -> some View {
        let ink = RiotTheme.ink(for: colorScheme)
        let paper = RiotTheme.paper(for: colorScheme)
        let pink = RiotTheme.pink(for: colorScheme)
        let isPrimary = emphasis == .primary
        let fill: Color = configuration.isPressed ? pink : (isPrimary ? ink : Color.clear)
        let foreground: Color = (isPrimary || configuration.isPressed) ? paper : ink
        let border: Color = configuration.isPressed ? pink : ink

        return configuration.label
            .font(.riot(.mono, size: 13, relativeTo: .footnote))
            .textCase(.uppercase)
            .tracking(1)
            .padding(.horizontal, 22)
            .padding(.vertical, 14)
            .foregroundStyle(foreground)
            .background(fill)
            .overlay(Rectangle().strokeBorder(border, lineWidth: 2))
    }
}

public extension ButtonStyle where Self == RiotButtonStyle {
    static var riotPrimary: RiotButtonStyle { RiotButtonStyle(.primary) }
    static var riotSecondary: RiotButtonStyle { RiotButtonStyle(.secondary) }
}
