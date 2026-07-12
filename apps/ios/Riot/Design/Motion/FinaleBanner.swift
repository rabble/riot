import SwiftUI

/// The closing line of the demo, on screen instead of only in the air.
///
/// You say *"No internet. No servers. Just these phones."* while pointing at
/// two airplane-mode icons. The banner is the receipt for that claim: the room
/// gets to read it while it is still looking at data that just crossed between
/// two phones with every radio but Bluetooth turned off.
///
/// It is dismissible because it is the *end* of a story, not a permanent
/// chrome, and because nothing should ever sit over the board that the person
/// holding the phone cannot get rid of.
///
/// Pure SwiftUI — no UIKit; these sources also compile on macOS.
public struct FinaleBanner: View {
    /// The line, verbatim. Kept here so the script and the screen cannot drift.
    public static let message = "No internet. No servers. Just these phones."

    @Environment(\.colorScheme) private var colorScheme
    @Binding private var isPresented: Bool

    public init(isPresented: Binding<Bool>) {
        self._isPresented = isPresented
    }

    public var body: some View {
        if isPresented {
            HStack(alignment: .center, spacing: 14) {
                Text(Self.message)
                    .font(.riot(.poster, size: 20, relativeTo: .title3))
                    .textCase(.uppercase)
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)

                Button {
                    withAnimation(.easeOut(duration: 0.2)) { isPresented = false }
                } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 13, weight: .bold))
                        .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        .padding(8)
                        .overlay(
                            Rectangle().strokeBorder(RiotTheme.ink(for: colorScheme), lineWidth: 2)
                        )
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Dismiss")
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RiotTheme.paper2(for: colorScheme))
            .overlay(Rectangle().strokeBorder(RiotTheme.ink(for: colorScheme), lineWidth: 2))
            .transition(.move(edge: .bottom).combined(with: .opacity))
        }
    }
}
