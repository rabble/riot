import SwiftUI

public struct RiotHeader: View {
    @Environment(\.colorScheme) private var colorScheme
    private let eyebrow: String?
    private let title: String

    public init(eyebrow: String? = nil, title: String) {
        self.eyebrow = eyebrow
        self.title = title
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let eyebrow {
                Text(eyebrow)
                    .font(.riot(.mono, size: 12, relativeTo: .caption))
                    .textCase(.uppercase)
                    .tracking(1)
                    .foregroundStyle(RiotTheme.pink(for: colorScheme))
            }
            Text(title)
                .font(.riot(.poster, size: 34, relativeTo: .largeTitle))
                .textCase(.uppercase)
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                .shadow(color: RiotTheme.blue(for: colorScheme), radius: 0, x: 2, y: 2)
                .shadow(color: RiotTheme.pink(for: colorScheme), radius: 0, x: -2, y: -2)
        }
        .padding(.horizontal, 20)
        .padding(.top, 20)
        .padding(.bottom, 14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RiotTheme.paper(for: colorScheme))
    }
}

public extension View {
    func riotHeader(eyebrow: String? = nil, _ title: String) -> some View {
        self
            .toolbar(.hidden, for: .navigationBar)
            .safeAreaInset(edge: .top, spacing: 0) {
                RiotHeader(eyebrow: eyebrow, title: title)
            }
    }
}
