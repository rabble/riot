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
    private let unreadBadges: [RiotDestination: Int]

    public static let items: [RiotTabItem] = RiotDestination.phoneTabs.map {
        RiotTabItem(destination: $0, label: $0.tabTitle, systemImage: $0.systemImage)
    }

    /// The text a tab's unread badge shows, or `nil` when there is nothing to
    /// announce. Zero and negative counts are inert; counts above 9 cap at "9+" so
    /// the badge never widens the tab. Pure so the mapping is unit-tested directly.
    public static func badgeText(forCount count: Int) -> String? {
        guard count > 0 else { return nil }
        return count > 9 ? "9+" : String(count)
    }

    public init(
        selection: Binding<RiotDestination>,
        unreadBadges: [RiotDestination: Int] = [:]
    ) {
        self._selection = selection
        self.unreadBadges = unreadBadges
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
                .overlay(alignment: .topTrailing) {
                    if let badge = Self.badgeText(forCount: unreadBadges[item.destination] ?? 0) {
                        unreadBadge(badge)
                    }
                }
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

    /// The unread count badge that rides the top-trailing corner of a tab icon —
    /// the "what's new" cue for a community route the reader is not currently on.
    private func unreadBadge(_ text: String) -> some View {
        Text(text)
            .font(.riot(.mono, size: 9, relativeTo: .caption2))
            .foregroundStyle(RiotTheme.paper(for: colorScheme))
            .padding(.horizontal, 4)
            .padding(.vertical, 1)
            .background(Capsule().fill(RiotTheme.pink(for: colorScheme)))
            .offset(x: 10, y: -6)
            .accessibilityIdentifier("tab-unread-\(text)")
    }
}
