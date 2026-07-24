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
        let accent = RiotTheme.accent(for: colorScheme)
        let onAccent = RiotTheme.onAccent(for: colorScheme)
        let isPrimary = emphasis == .primary

        // Primary: one filled accent pill. Secondary: a quiet ghost — a hairline
        // outline that warms to the accent on press, never a heavy black box.
        let fill: Color = isPrimary ? accent : Color.clear
        let foreground: Color =
            isPrimary
                ? onAccent
                : (configuration.isPressed ? accent : ink)
        let border: Color = isPrimary ? .clear : (configuration.isPressed ? accent : RiotTheme.line(for: colorScheme))

        return configuration.label
            .font(.riot(.body, size: 15, relativeTo: .callout))
            .fontWeight(.semibold)
            .padding(.horizontal, 18)
            .padding(.vertical, 11)
            .foregroundStyle(foreground)
            .background(fill)
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .strokeBorder(border, lineWidth: 1)
            )
            .opacity(configuration.isPressed && isPrimary ? 0.86 : 1)
    }
}

public extension ButtonStyle where Self == RiotButtonStyle {
    static var riotPrimary: RiotButtonStyle { RiotButtonStyle(.primary) }
    static var riotSecondary: RiotButtonStyle { RiotButtonStyle(.secondary) }
}
