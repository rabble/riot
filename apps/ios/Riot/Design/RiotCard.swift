import SwiftUI

public struct RiotCard<Content: View>: View {
    @Environment(\.colorScheme) private var colorScheme
    private let content: Content

    public init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    public var body: some View {
        content
            .padding(20)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RiotTheme.paper2(for: colorScheme))
            .overlay(
                Rectangle().strokeBorder(RiotTheme.ink(for: colorScheme), lineWidth: 2)
            )
    }
}
