import SwiftUI

public struct RiotHeader<Trailing: View>: View {
    @Environment(\.colorScheme) private var colorScheme
    private let eyebrow: String?
    private let title: String
    private let trailing: Trailing

    public init(
        eyebrow: String? = nil,
        title: String,
        @ViewBuilder trailing: () -> Trailing = { EmptyView() }
    ) {
        self.eyebrow = eyebrow
        self.title = title
        self.trailing = trailing()
    }

    public var body: some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(alignment: .leading, spacing: 6) {
                if let eyebrow {
                    Text(eyebrow)
                        .font(.riot(.mono, size: 12, relativeTo: .caption))
                        .textCase(.uppercase)
                        .tracking(1)
                        .foregroundStyle(RiotTheme.pink(for: colorScheme))
                }
                // Clean editorial serif — no glitch drop-shadows, no forced
                // uppercase; the community's own name reads as itself.
                Text(title)
                    .font(.riotSerif(size: 30, relativeTo: .largeTitle))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
            trailing
                .padding(.top, 2)
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
        riotHeader(eyebrow: eyebrow, title) { EmptyView() }
    }

    func riotHeader<Trailing: View>(
        eyebrow: String? = nil,
        _ title: String,
        @ViewBuilder trailing: () -> Trailing
    ) -> some View {
        let content = safeAreaInset(edge: .top, spacing: 0) {
            RiotHeader(eyebrow: eyebrow, title: title, trailing: trailing)
        }
        // ToolbarPlacement.navigationBar exists only on iOS; there is no
        // navigation bar to hide on macOS, where RiotKit also compiles.
        #if os(iOS)
            return content.toolbar(.hidden, for: .navigationBar)
        #else
            return content
        #endif
    }
}
