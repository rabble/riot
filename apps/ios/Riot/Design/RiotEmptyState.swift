import SwiftUI

public struct RiotEmptyState: View {
    @Environment(\.colorScheme) private var colorScheme
    private let title: String
    private let message: String

    public init(title: String, message: String) {
        self.title = title
        self.message = message
    }

    public var body: some View {
        VStack(spacing: 14) {
            Text(title)
                .font(.riot(.poster, size: 26, relativeTo: .title))
                .textCase(.uppercase)
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                .multilineTextAlignment(.center)
            Text(message)
                .font(.riot(.body, size: 15, relativeTo: .body))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                .multilineTextAlignment(.center)
        }
        .padding(32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
