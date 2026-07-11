import SwiftUI

public struct RiotBadge: View {
    @Environment(\.colorScheme) private var colorScheme
    private let text: String
    private let stamped: Bool

    public init(_ text: String, stamped: Bool = false) {
        self.text = text
        self.stamped = stamped
    }

    public var body: some View {
        Text(text)
            .font(.riot(.mono, size: 12, relativeTo: .caption))
            .textCase(.uppercase)
            .tracking(1)
            .multilineTextAlignment(.leading)
            .foregroundStyle(RiotTheme.ink(for: colorScheme))
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .overlay(Rectangle().strokeBorder(RiotTheme.ink(for: colorScheme), lineWidth: 2))
            .rotationEffect(stamped ? .degrees(-2) : .zero)
    }
}
