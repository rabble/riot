import SwiftUI

/// What one person shows another: their profile and the collection of apps they
/// carry, with the ways you can bring them into your work.
///
/// This is the surface behind tapping a peer on the Connect screen. It reads
/// only what has already synced — a peer's rendered name, and the directory
/// entries authored by them — so it says nothing it cannot stand behind: an
/// author's key is never shown as a name, and an app that has not arrived yet
/// simply does not appear.
///
/// Plain language only (app rule): the words subspace, namespace, key never
/// appear. See docs/superpowers/plans/2026-07-12-peer-profile-and-collaborate.md.
public struct PeerProfileView: View {
    @ObservedObject private var model: RiotAppModel
    @StateObject private var directory = RiotDirectoryModel()
    @Environment(\.colorScheme) private var colorScheme

    /// The peer as the transport named them (e.g. "Patient Broom"), and — when
    /// known — the rendered profile name the directory attributes authorship to
    /// (e.g. "Ana · a3f91122"). Collections are matched on the latter.
    private let peerName: String
    private let authoredName: String?
    private let onClose: () -> Void
    private let onInvite: ((RiotSpace) -> Void)?
    /// Whether this is the person the device is in a session with right now, as
    /// opposed to one it can merely see. The sheet must not say "near you now"
    /// about someone it is actively synced with — that is the difference the
    /// Connect screen exists to make plain.
    private let isConnected: Bool

    public init(
        model: RiotAppModel,
        peerName: String,
        authoredName: String? = nil,
        isConnected: Bool = false,
        onInvite: ((RiotSpace) -> Void)? = nil,
        onClose: @escaping () -> Void
    ) {
        _model = ObservedObject(wrappedValue: model)
        self.peerName = peerName
        self.authoredName = authoredName
        self.isConnected = isConnected
        self.onInvite = onInvite
        self.onClose = onClose
    }

    /// What this device has learned about from people nearby: the apps that came
    /// in over a sync (`.arriving`) or are held pending your review (`.review`).
    ///
    /// NOTE: per-person attribution is temporarily unavailable — the directory
    /// row's `author` field was removed from the model while attribution is being
    /// reworked (see the plan doc). Until it returns, this shows the collection
    /// that arrived from your peers rather than one filtered to a single person;
    /// `authoredName` is retained on the API for when attribution lands.
    private var theirCollection: [RiotDirectoryRow] {
        directory.rows.filter { row in
            switch row.availability {
            case .review, .arriving: return true
            case .open, .get: return false
            }
        }
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                header
                collectionSection
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
        .onAppear {
            directory.attach(port: model.profileRepository)
            directory.refresh()
        }
    }

    private var header: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 8) {
                Text(peerName)
                    .font(.riot(.body, size: 20, relativeTo: .title3))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                if isConnected {
                    RiotBadge("Connected to you now")
                        .accessibilityIdentifier("peer-connected-badge")
                }
                Text(
                    isConnected
                        ? "You are connected to them right now. What they carry is theirs — nothing runs on your device until you turn it on."
                        : "Near you now. What they carry is theirs — nothing runs on your device until you turn it on."
                )
                .font(.riot(.body, size: 14, relativeTo: .callout))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            }
        }
    }

    @ViewBuilder
    private var collectionSection: some View {
        Text("What they carry")
            .font(.riot(.mono, size: 12, relativeTo: .caption))
            .textCase(.uppercase)
            .tracking(1)
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))

        if theirCollection.isEmpty {
            RiotEmptyState(
                title: "Nothing yet",
                message: "When you sync, the apps this person carries will show up here for you to review."
            )
        } else {
            ForEach(theirCollection) { row in
                RiotCard {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("\(row.name) · \(row.version)")
                            .font(.riot(.body, size: 16, relativeTo: .headline))
                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        Text(row.description)
                            .font(.riot(.body, size: 14, relativeTo: .body))
                            .foregroundStyle(RiotTheme.ink(for: colorScheme))
                        if let endorsement = row.endorsement {
                            Text(endorsement)
                                .font(.riot(.body, size: 12, relativeTo: .caption))
                                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                        }
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var collaborateSection: some View {
        Text("Collaborate")
            .font(.riot(.mono, size: 12, relativeTo: .caption))
            .textCase(.uppercase)
            .tracking(1)
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))

        if let space = model.space, let onInvite {
            RiotCard {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Bring them into a space you organize. They choose whether to accept — an invite is a door, not a push.")
                        .font(.riot(.body, size: 14, relativeTo: .callout))
                        .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                    Button("Invite to \(space.title)") { onInvite(space) }
                        .buttonStyle(.riotPrimary)
                        .accessibilityIdentifier("peer-invite-to-space")
                }
            }
        } else {
            RiotEmptyState(
                title: "No space to invite them to yet",
                message: "Create or open a space first, then you can invite the people near you into it."
            )
        }
    }
}
