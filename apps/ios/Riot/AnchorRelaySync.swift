import Foundation
import RiotKit
import SwiftUI

/// The visible "leave the room, still connected" surface. It doesn't lead with
/// wires and NodeIds — it leads with a community you can walk into. Tapping
/// Connect pulls the built-in community over the internet INTO the durable
/// profile (via ``RiotAppModel/syncFromRelay()``), so on success it appears in
/// "Your communities" and Open takes you into it. The three placements (Home,
/// Transport, Onboarding) all read the SAME app-model state, so they reflect one
/// shared pull rather than three isolated ones.
public struct AnchorRelaySyncCard: View {
    @Environment(\.colorScheme) private var colorScheme
    @ObservedObject var model: RiotAppModel
    @State private var didAutoStart = false
    private let autoStart: Bool

    public init(model: RiotAppModel, autoStart: Bool = false) {
        self.model = model
        self.autoStart = autoStart
    }

    public var body: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 12) {
                if let result = model.relaySyncResult {
                    successState(result)
                } else {
                    idleState
                }
            }
        }
        .task {
            guard autoStart, !didAutoStart,
                  model.relaySyncResult == nil, !model.isRelaySyncing else { return }
            didAutoStart = true
            await model.syncFromRelay()
        }
    }

    // MARK: Idle / connecting

    @ViewBuilder private var idleState: some View {
        Text("STAY CONNECTED")
            .font(.riot(.mono, size: 12, relativeTo: .caption))
            .tracking(1)
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        Text("Find a community over the internet")
            .font(.riot(.body, size: 20, relativeTo: .title3))
            .foregroundStyle(RiotTheme.ink(for: colorScheme))
            .accessibilityAddTraits(.isHeader)
        Text("Pull a live community that's already published, and it becomes yours to read and carry — even when there's no one nearby. No IP, no account.")
            .font(.riot(.body, size: 15, relativeTo: .callout))
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))

        Button(model.isRelaySyncing ? "Connecting…" : "Connect to a community") {
            Task { await model.syncFromRelay() }
        }
        .buttonStyle(.riotPrimary)
        .disabled(model.isRelaySyncing)
        .accessibilityIdentifier("anchor-relay-sync")

        if model.isRelaySyncing {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text("Reaching the community…")
                    .font(.riot(.body, size: 14, relativeTo: .footnote))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            }
            .accessibilityIdentifier("anchor-relay-status")
        }

        if let error = model.relaySyncError {
            Text(error)
                .font(.riot(.body, size: 14, relativeTo: .footnote))
                .foregroundStyle(.red)
                .accessibilityIdentifier("anchor-relay-error")
        }
    }

    // MARK: Success — lead with the community, offer the way in

    @ViewBuilder private func successState(_ result: RelaySyncResult) -> some View {
        Text(result.isWalkInReady ? "YOU'RE IN" : "SYNCED")
            .font(.riot(.mono, size: 12, relativeTo: .caption))
            .tracking(1)
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        Text(result.communityName)
            .font(.riot(.body, size: 22, relativeTo: .title2))
            .foregroundStyle(RiotTheme.ink(for: colorScheme))
            .accessibilityAddTraits(.isHeader)
            .accessibilityIdentifier("anchor-relay-community-name")

        Text(summaryLine(result))
            .font(.riot(.body, size: 15, relativeTo: .callout))
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            .accessibilityIdentifier("anchor-relay-status")

        if let namespaceID = result.namespaceID, result.isWalkInReady {
            Button("Open \(result.communityName)") {
                model.openSyncedCommunity(namespaceID: namespaceID)
            }
            .buttonStyle(.riotPrimary)
            .accessibilityIdentifier("anchor-relay-open-community")
        } else {
            Text("It's saved on this device — it'll open here once there's a wire to walk into.")
                .font(.riot(.body, size: 14, relativeTo: .footnote))
                .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
        }

        Button(model.isRelaySyncing ? "Syncing…" : "Sync again") {
            Task { await model.syncFromRelay() }
        }
        .buttonStyle(.riotSecondary)
        .disabled(model.isRelaySyncing)
        .accessibilityIdentifier("anchor-relay-resync")
    }

    /// "Synced just now · 4 posts · 3 people" — reassurance you're current with
    /// these people, then the life of the place.
    private func summaryLine(_ result: RelaySyncResult) -> String {
        var parts: [String] = []
        if let namespaceID = result.namespaceID,
           let synced = model.lastSyncedText(for: namespaceID) {
            parts.append(synced)
        } else {
            parts.append("Synced just now")
        }
        if result.postCount > 0 {
            parts.append("\(result.postCount) \(result.postCount == 1 ? "post" : "posts")")
        }
        if result.peopleCount > 0 {
            parts.append("\(result.peopleCount) \(result.peopleCount == 1 ? "person" : "people")")
        }
        return parts.joined(separator: " · ")
    }
}
