import SwiftUI

/// What one person shows another when you open them: who they are, how you're
/// related right now (near you / connected), and the one thing you can actually
/// do — bring them into a community you organize.
///
/// It deliberately does NOT lead with (or show) the list of tools a phone
/// carries: that is a protocol detail, not a person, and it was never even
/// attributed to this individual. A person is identity + relationship + a way
/// to connect.
///
/// Plain language only (app rule): the words subspace, namespace, key never
/// appear. See docs/superpowers/plans/2026-07-12-peer-profile-and-collaborate.md.
public struct PeerProfileView: View {
    @ObservedObject private var model: RiotAppModel
    @Environment(\.colorScheme) private var colorScheme

    /// The peer as the transport named them (e.g. "Patient Broom").
    private let peerName: String
    private let onClose: () -> Void
    private let onInvite: ((RiotSpace) -> Void)?
    /// Whether this is the person the device is in a session with right now, as
    /// opposed to one it can merely see.
    private let isConnected: Bool

    public init(
        model: RiotAppModel,
        peerName: String,
        // Retained for call-site compatibility; the profile no longer renders a
        // per-person carried-tools list, so attribution is unused here.
        authoredName: String? = nil,
        isConnected: Bool = false,
        onInvite: ((RiotSpace) -> Void)? = nil,
        onClose: @escaping () -> Void
    ) {
        _model = ObservedObject(wrappedValue: model)
        self.peerName = peerName
        self.isConnected = isConnected
        self.onInvite = onInvite
        self.onClose = onClose
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                identityHeader
                collaborateSection
            }
            .padding(20)
        }
        .riotHeader(eyebrow: "Person", peerName)
        .overlay(alignment: .topTrailing) {
            Button("Close", action: onClose)
                .buttonStyle(.riotSecondary)
                .padding(12)
                .accessibilityIdentifier("peer-profile-close")
        }
    }

    private var initials: String {
        let letters = peerName.split(separator: " ").prefix(2).compactMap(\.first)
        return letters.isEmpty ? "?" : String(letters).uppercased()
    }

    private var identityHeader: some View {
        RiotCard {
            HStack(alignment: .top, spacing: 13) {
                ZStack {
                    Circle().fill(RiotTheme.avatarColor(forKey: peerName))
                    Text(initials)
                        .font(.system(size: 17, weight: .bold))
                        .foregroundStyle(.white)
                }
                .frame(width: 46, height: 46)

                VStack(alignment: .leading, spacing: 5) {
                    Text(peerName)
                        .font(.riotSerif(size: 22, relativeTo: .title2))
                        .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    if isConnected {
                        RiotBadge("Connected to you now")
                            .accessibilityIdentifier("peer-connected-badge")
                    } else {
                        Text("Near you now")
                            .font(.riot(.mono, size: 11, relativeTo: .caption))
                            .textCase(.uppercase)
                            .tracking(1)
                            .foregroundStyle(RiotTheme.accent(for: colorScheme))
                    }
                    Text(isConnected
                        ? "You’re synced with them right now."
                        : "A phone near you. You don’t know each other yet — bring them into a community to start.")
                        .font(.riot(.body, size: 13, relativeTo: .footnote))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
            }
        }
    }

    @ViewBuilder
    private var collaborateSection: some View {
        switch PeerCollaboration(space: model.space, canInvite: onInvite != nil) {
        case let .invite(space):
            RiotCard {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Bring them in")
                        .font(.riot(.body, size: 16, relativeTo: .headline))
                        .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    Text("Invite them into a community you organize. They choose whether to accept — an invite is a door, not a push.")
                        .font(.riot(.body, size: 14, relativeTo: .callout))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    Button("Invite to \(space.title)") { onInvite?(space) }
                        .buttonStyle(.riotPrimary)
                        .accessibilityIdentifier("peer-invite-to-space")
                }
            }
        case let .nothingToOffer(state):
            RiotEmptyState(title: state.title, message: state.message)
        }
    }
}

/// What the Collaborate section can honestly offer about this person.
///
/// The distinction this type exists to keep straight: a peer with no invite
/// route is NOT always a peer with nowhere to go. When there is no space at all,
/// there is genuinely nothing to invite them to. But when a space exists and the
/// invite route is simply absent, this is a **synced identity** — someone already
/// carrying this space's latest, not a phone in range — and telling them "no
/// space to invite them to" is a lie about their own space. Those two are
/// different sentences, which is why the decision is a value the tests can read
/// rather than a chain of `if let`s inside a view body.
public enum PeerCollaboration: Equatable, Sendable {
    /// A space of ours, and a route to bring them into it.
    case invite(RiotSpace)
    /// No invite to offer — for one of two quite different reasons.
    case nothingToOffer(EmptyState)

    public enum EmptyState: Equatable, Sendable {
        /// There is no space yet, so there is nothing to invite anyone to.
        case noSpace
        /// A space exists and they are already part of its network.
        case alreadyInNetwork

        public var title: String {
            switch self {
            case .noSpace: "No community to invite them to yet"
            case .alreadyInNetwork: "Already in your network"
            }
        }

        public var message: String {
            switch self {
            case .noSpace:
                "Create or open a community first, then you can invite the people near you into it."
            case .alreadyInNetwork:
                "You’ve synced with this person — they’re carrying your community’s latest."
            }
        }
    }

    public init(space: RiotSpace?, canInvite: Bool) {
        switch (space, canInvite) {
        case let (.some(space), true):
            self = .invite(space)
        case (.none, _):
            self = .nothingToOffer(.noSpace)
        case (.some, false):
            self = .nothingToOffer(.alreadyInNetwork)
        }
    }

    /// The empty state to draw, or nil when there is an invite to offer instead.
    public var emptyState: EmptyState? {
        switch self {
        case .invite: nil
        case let .nothingToOffer(state): state
        }
    }
}
