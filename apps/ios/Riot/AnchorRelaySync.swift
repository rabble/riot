import Foundation
import OSLog
import RiotKit
import SwiftUI

/// The app's built-in "known relay + known community": the deployed GCP anchor
/// relay's stable NodeId and a root-signed ReadCommitted ticket for a community
/// already committed on that relay. Baked in so the phone can pull a real
/// community out of the box — no IP, no port, no manual paste.
///
/// The relay is dialed by its NodeId ALONE: iroh relay + pkarr/DNS discovery
/// resolves the address and NAT-traverses (the net runtime binds under the `N0`
/// preset). The NodeId is the stable address; no IP or port is ever baked or
/// pinned.
enum AnchorRelayDefaults {
    /// The deployed relay's stable NodeId (64 hex). This is the whole dial hint —
    /// `syncWithAnchor` resolves it via discovery. Never an IP:port.
    static let relayNodeId =
        "60ab7b416b0ef0b8088cd64a3ef01edd598dcc5bb7a4df03145f957fec2432d8"

    /// A root-signed ReadCommitted ticket (hex) for a community already committed
    /// on the relay: an O masthead + C comments + W wire namespace. Root-signed,
    /// so the phone's transport-floor gate verifies it before any packet.
    /// Re-baked 2026-07-23: durable community (89-day ticket, expires ~2026-10-20);
    /// community root 2052fabaefdea8eb3da14b0064a39dc1f7e062b354fa9f7fde5b0c337439f5bf.
    static let communityTicketHex =
        "83028c58202052fabaefdea8eb3da14b0064a39dc1f7e062b354fa9f7fde5b0c" +
        "337439f5bf58202052fabaefdea8eb3da14b0064a39dc1f7e062b354fa9f7fde" +
        "5b0c337439f5bf5820583b4fa0348fbb3dad51cbfd3e760cb4e695c97c0397cd" +
        "6f86c7be720a57f2025820a94dde010d9c3f70bbe6c39d7ab766fe272408cc9c" +
        "42a7454b57125915165c68582077ebb646a8e0ae43309d1e0383f35b8bfaf559" +
        "502c862c5fe27ebc2f7e9a70c81a6a618677026c726571756972655f6e6f6e65" +
        "6c726571756972655f6e6f6e65011a6a6186131a6ad6dbf758405a9b0175e949" +
        "4e9582a2493bfb37009c4aeeed1748b940f07430163ea95d8d18498a0cd72a29" +
        "c8fe3172e29f9ff8ce887ae0afee8433aab4be0c69b0c1d95c0b"

    /// Decode the baked ticket hex to bytes.
    static var communityTicket: Data { data(fromHex: communityTicketHex) }

    static func data(fromHex hex: String) -> Data {
        var data = Data(capacity: hex.count / 2)
        var index = hex.startIndex
        while index < hex.endIndex {
            let next = hex.index(index, offsetBy: 2)
            guard let byte = UInt8(hex[index..<next], radix: 16) else { return Data() }
            data.append(byte)
            index = next
        }
        return data
    }
}

/// Drives the real, user-triggered anchor-relay pull from the running app.
///
/// This is the shipping counterpart of the `AnchorInternetPullTests` proof: it
/// binds the FFI-owned net runtime (ephemeral iroh endpoint under the `N0`
/// preset — relay + discovery on), dials the baked relay BY NODEID over the
/// internet, runs the gated `riot/sync/2` ReadCommitted pull, and imports the
/// verified O/C/W entries into a willow store. Progress and the outcome are
/// published for the UI and logged via `os_log` so a device/simulator run leaves
/// a machine-checkable trail ("dialed relay … imported N entries").
@MainActor
final class AnchorRelaySyncModel: ObservableObject {
    enum Phase: Equatable {
        case idle
        case syncing
        case done(imported: Int, verified: Int)
        case failed(String)
    }

    @Published private(set) var phase: Phase = .idle
    @Published private(set) var statusLine: String = "Not connected"

    private let logger = Logger(subsystem: "net.protest.riot", category: "anchor-relay")
    /// Guards the auto-start so a Home re-appearance never re-fires the network.
    private var hasAutoStarted = false

    var isSyncing: Bool { phase == .syncing }

    /// Fire the pull exactly once per model lifetime — the "on app launch" path.
    func autoStartIfNeeded() {
        guard !hasAutoStarted else { return }
        hasAutoStarted = true
        syncFromDefaultRelay()
    }

    /// Bind the net runtime, dial the baked relay by NodeId, pull the baked
    /// community, and surface the result. Runs off the main actor for the network
    /// leg; publishes back on the main actor.
    func syncFromDefaultRelay() {
        guard !isSyncing else { return }
        phase = .syncing
        statusLine = "Dialing relay \(shortId(AnchorRelayDefaults.relayNodeId)) by NodeId…"
        logger.log("anchor-relay: dialing relay \(AnchorRelayDefaults.relayNodeId, privacy: .public) by NodeId (no ip:port)")

        let nodeId = AnchorRelayDefaults.relayNodeId
        let ticket = AnchorRelayDefaults.communityTicket
        let now = UInt64(Date().timeIntervalSince1970)

        Task.detached { [logger] in
            do {
                // A fresh in-memory profile: an empty willow store, exactly like a
                // phone that has never seen this community. The pull is the only
                // thing that can put entries into it — so a non-zero import is
                // honest proof the data crossed the internet from the relay.
                let profile = try openLocalProfile()
                let net = try bindNetRuntime()
                // Dial by BARE NodeId — discovery resolves the address.
                let outcome = try net.syncWithAnchor(
                    profile: profile,
                    anchorHint: nodeId,
                    ticketBytes: ticket,
                    nowUnix: now
                )
                let imported = outcome.namespaces.reduce(0) { $0 + Int($1.imported) }
                let verified = outcome.namespaces.reduce(0) { $0 + Int($1.verified) }
                let rejected = outcome.namespaces.reduce(0) { $0 + Int($1.rejected) }
                logger.log("anchor-relay: dialed relay \(nodeId, privacy: .public), imported \(imported, privacy: .public) entries (verified \(verified, privacy: .public), rejected \(rejected, privacy: .public)) for community root \(outcome.root, privacy: .public)")
                for ns in outcome.namespaces {
                    logger.log("anchor-relay: namespace \(ns.namespaceId, privacy: .public) verified=\(ns.verified, privacy: .public) imported=\(ns.imported, privacy: .public) rejected=\(ns.rejected, privacy: .public) refusal=\(ns.refusal ?? "none", privacy: .public)")
                }
                await MainActor.run {
                    self.phase = .done(imported: imported, verified: verified)
                    self.statusLine =
                        "Connected. Imported \(imported) entries from the relay (verified \(verified))."
                }
            } catch {
                logger.error("anchor-relay: pull failed: \(error.localizedDescription, privacy: .public)")
                await MainActor.run {
                    self.phase = .failed(error.localizedDescription)
                    self.statusLine = "Relay unreachable: \(error.localizedDescription)"
                }
            }
        }
    }

    private func shortId(_ id: String) -> String {
        guard id.count > 12 else { return id }
        return "\(id.prefix(6))…\(id.suffix(6))"
    }
}

/// The visible in-app entry point: a card with a "Sync from the relay" button and
/// a live status line. Tapping it dials the deployed relay by NodeId and pulls
/// the built-in community over the internet — the real user path for
/// "leave the room, still sync".
public struct AnchorRelaySyncCard: View {
    @Environment(\.colorScheme) private var colorScheme
    @StateObject private var model = AnchorRelaySyncModel()
    private let autoStart: Bool

    public init(autoStart: Bool = false) {
        self.autoStart = autoStart
    }

    public var body: some View {
        RiotCard {
            VStack(alignment: .leading, spacing: 12) {
                Text("SYNC FROM THE RELAY")
                    .font(.riot(.mono, size: 12, relativeTo: .caption))
                    .tracking(1)
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
                Text("Pull a live community over the internet")
                    .font(.riot(.body, size: 20, relativeTo: .title3))
                    .foregroundStyle(RiotTheme.ink(for: colorScheme))
                    .accessibilityAddTraits(.isHeader)
                Text("Connect to the built-in anchor relay by its NodeId and pull a community that is already published there. No IP, no account.")
                    .font(.riot(.body, size: 15, relativeTo: .callout))
                    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))

                Button(model.isSyncing ? "Syncing…" : "Sync from the relay") {
                    model.syncFromDefaultRelay()
                }
                .buttonStyle(.riotSecondary)
                .disabled(model.isSyncing)
                .accessibilityIdentifier("anchor-relay-sync")

                HStack(spacing: 8) {
                    if model.isSyncing { ProgressView().controlSize(.small) }
                    Text(model.statusLine)
                        .font(.riot(.mono, size: 13, relativeTo: .footnote))
                        .foregroundStyle(statusColor)
                        .accessibilityIdentifier("anchor-relay-status")
                }
            }
        }
        .task {
            if autoStart { model.autoStartIfNeeded() }
        }
    }

    private var statusColor: Color {
        switch model.phase {
        case .done: return .green
        case .failed: return .red
        default: return RiotTheme.inkSoft(for: colorScheme)
        }
    }
}
