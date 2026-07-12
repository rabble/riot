import SwiftUI

/// One person, as a screen is allowed to draw them.
///
/// The three fields are exactly the FFI's `WhoAmI { id, displayName, tag }`.
/// Core has ALREADY sanitized `displayName` — no separator, no bidi, no control
/// characters — which is the whole reason `displayName + " · " + tag` is a safe
/// thing to concatenate here. This type does not re-sanitize, re-split, or
/// invent any other shape; it applies core's single sanctioned rendering
/// (`render_display_name`) and nothing else.
public struct RiotPerson: Identifiable, Equatable, Hashable, Sendable {
    /// The stable key id, lowercase hex. Identity only. **Never drawn.**
    public let id: String
    /// The name this person claims. Never shown bare — see ``rendered``.
    public let displayName: String
    /// The few hex characters tied to their actual key, from core.
    public let tag: String

    public init(id: String, displayName: String, tag: String) {
        self.id = id
        self.displayName = displayName
        self.tag = tag
    }

    /// The ONE way a person is ever shown: `Ana · a3f91122`.
    ///
    /// A self-claimed name is never rendered bare. Two people can both claim
    /// "Ana"; their tags differ, and nothing merges them. The suffix is not
    /// decoration — it is the honest admission that the name alone proves
    /// nothing.
    public var rendered: String { "\(displayName) · \(tag)" }

    /// The attribution line for a row a peer just changed, ready to hand to
    /// ``SyncRipple``: `checked by Ana · a3f91122`.
    public var checkedBy: String { "checked by \(rendered)" }
}

/// What the radar has to say, given who it can currently see.
///
/// `RadarPairingView` is a *drawing of this value* — the view branches on it,
/// so asserting on it is asserting on what the audience reads off the screen.
public enum RadarPairingState: Equatable {
    /// Nobody yet. **This is not an error and must never be dressed as one.**
    /// Bluetooth genuinely takes seconds; a red state here would teach the room
    /// that the demo broke when it is simply still looking.
    case searching(String)
    /// Rendered labels, in radar order. Only ever ``RiotPerson/rendered`` —
    /// a raw id must never reach this array.
    case peers([String])
}

/// The pairing radar: concentric rings, a sweeping arc, and a labeled dot for
/// each person found nearby.
///
/// It **takes peers as a parameter**. It does not talk to the transport layer,
/// start a scan, or own a discovery session — the caller does that and hands
/// down a list, which is what makes this view a pure, snapshot-testable drawing
/// and what keeps the transport free to change underneath it.
///
/// Pure SwiftUI — no UIKit; these sources also compile on macOS.
public struct RadarPairingView: View {
    /// Never "no devices found", never "failed". It is still listening.
    public static let searchingMessage = "Looking for people nearby…"

    /// The pure reading of the radar. The body below renders exactly this.
    public static func state(for peers: [RiotPerson]) -> RadarPairingState {
        peers.isEmpty ? .searching(searchingMessage) : .peers(peers.map(\.rendered))
    }

    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private let peers: [RiotPerson]
    private let diameter: CGFloat

    @State private var sweep: Double = 0

    public init(peers: [RiotPerson], diameter: CGFloat = 260) {
        self.peers = peers
        self.diameter = diameter
    }

    public var body: some View {
        VStack(spacing: 20) {
            ZStack {
                rings
                sweepArc
                dots
            }
            .frame(width: diameter, height: diameter)

            caption
        }
        .padding(24)
        .onAppear(perform: startSweeping)
    }

    // MARK: - Rings

    private var rings: some View {
        ZStack {
            ForEach([1.0, 0.68, 0.36], id: \.self) { fraction in
                Circle()
                    .strokeBorder(RiotTheme.lineStrong(for: colorScheme), lineWidth: 2)
                    .frame(width: diameter * fraction, height: diameter * fraction)
            }
            // The device itself, dead centre.
            Rectangle()
                .fill(RiotTheme.ink(for: colorScheme))
                .frame(width: 10, height: 10)
                .rotationEffect(.degrees(45))
        }
        .accessibilityHidden(true)
    }

    /// A rotating angular gradient, masked to the dish — the sweep of a scan.
    private var sweepArc: some View {
        let pink = RiotTheme.pink(for: colorScheme)
        return Circle()
            .fill(
                AngularGradient(
                    gradient: Gradient(stops: [
                        .init(color: pink.opacity(0), location: 0.0),
                        .init(color: pink.opacity(0.35), location: 0.18),
                        .init(color: pink.opacity(0), location: 0.25),
                        .init(color: pink.opacity(0), location: 1.0),
                    ]),
                    center: .center
                )
            )
            .rotationEffect(.degrees(sweep))
            .frame(width: diameter, height: diameter)
            .allowsHitTesting(false)
            .accessibilityHidden(true)
    }

    private func startSweeping() {
        // Someone who has asked the system to calm motion down should not be
        // handed a permanently spinning arc.
        guard !reduceMotion else { return }
        withAnimation(.linear(duration: 2.4).repeatForever(autoreverses: false)) {
            sweep = 360
        }
    }

    // MARK: - Peers

    /// Each discovered peer pops in with the one stamp — same motion as an entry
    /// landing on the board, because it is the same event: someone else arrived.
    private var dots: some View {
        ZStack {
            ForEach(Array(peers.enumerated()), id: \.element.id) { index, peer in
                peerDot(peer)
                    .offset(offset(for: index, of: peers.count))
                    .riotStampSlam(trigger: peer.id, flashes: false)
            }
        }
    }

    private func peerDot(_ peer: RiotPerson) -> some View {
        VStack(spacing: 5) {
            Rectangle()
                .fill(RiotTheme.pink(for: colorScheme))
                .frame(width: 14, height: 14)
                .overlay(Rectangle().strokeBorder(RiotTheme.ink(for: colorScheme), lineWidth: 2))

            // Rendered, always. `peer.id` is never drawn.
            Text(peer.rendered)
                .font(.riot(.mono, size: 11, relativeTo: .caption2))
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
                .padding(.horizontal, 6)
                .padding(.vertical, 3)
                .background(RiotTheme.paper2(for: colorScheme))
                .overlay(Rectangle().strokeBorder(RiotTheme.ink(for: colorScheme), lineWidth: 2))
                .fixedSize()
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(peer.rendered)
    }

    /// Peers are placed around a ring, evenly, starting at the top. Radar
    /// position is decorative — nothing here claims to be a real bearing or a
    /// real distance, and it must not, because it is not measuring either.
    private func offset(for index: Int, of count: Int) -> CGSize {
        guard count > 0 else { return .zero }
        let radius = diameter * 0.34
        let angle = (Double(index) / Double(count)) * 2 * .pi - .pi / 2
        return CGSize(width: radius * cos(angle), height: radius * sin(angle))
    }

    // MARK: - Caption

    @ViewBuilder
    private var caption: some View {
        switch Self.state(for: peers) {
        case let .searching(message):
            Text(message)
                .font(.riot(.mono, size: 13, relativeTo: .footnote))
                .tracking(1)
                .textCase(.uppercase)
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                .multilineTextAlignment(.center)
                .accessibilityLabel(message)

        case let .peers(labels):
            Text(labels.count == 1 ? "1 person nearby" : "\(labels.count) people nearby")
                .font(.riot(.mono, size: 13, relativeTo: .footnote))
                .tracking(1)
                .textCase(.uppercase)
                .foregroundStyle(RiotTheme.ink(for: colorScheme))
        }
    }
}
