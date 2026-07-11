import SwiftUI

public struct RiotTabItem: Identifiable, Equatable {
    public let destination: RiotDestination
    public let label: String
    public let systemImage: String
    public var id: RiotDestination { destination }
}

public struct RiotTabBar: View {
    @Environment(\.colorScheme) private var colorScheme
    @Binding private var selection: RiotDestination

    public static let items: [RiotTabItem] = RiotDestination.phoneTabs.map {
        RiotTabItem(destination: $0, label: $0.tabTitle, systemImage: $0.systemImage)
    }

    public init(selection: Binding<RiotDestination>) {
        self._selection = selection
    }

    public var body: some View {
        HStack(spacing: 0) {
            ForEach(Self.items) { item in
                Button {
                    selection = item.destination
                } label: {
                    tabLabel(for: item)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(item.label)
                .accessibilityAddTraits(item.destination == selection ? [.isButton, .isSelected] : .isButton)
            }
        }
        .padding(.top, 10)
        .padding(.bottom, 6)
        .background(RiotTheme.paper(for: colorScheme))
        .overlay(alignment: .top) {
            Rectangle().fill(RiotTheme.ink(for: colorScheme)).frame(height: 2)
        }
    }

    @ViewBuilder
    private func tabLabel(for item: RiotTabItem) -> some View {
        let isSelected = item.destination == selection
        VStack(spacing: 4) {
            Image(systemName: item.systemImage)
                .font(.system(size: 20, weight: .bold))
            Text(item.label)
                .font(.riot(.mono, size: 10, relativeTo: .caption2))
                .textCase(.uppercase)
                .tracking(0.5)
        }
        .foregroundStyle(isSelected ? RiotTheme.paper(for: colorScheme) : RiotTheme.ink(for: colorScheme))
        .frame(maxWidth: .infinity)
        .padding(.vertical, 6)
        .background {
            if isSelected {
                Rectangle()
                    .fill(RiotTheme.pink(for: colorScheme))
                    .rotationEffect(.degrees(-2))
                    .padding(.horizontal, 4)
            }
        }
    }
}
