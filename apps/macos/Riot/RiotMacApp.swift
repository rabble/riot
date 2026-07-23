import AppKit
import RiotKit
import SwiftUI

@main
struct RiotMacApp: App {
    @StateObject private var model = RiotAppModel()

    var body: some Scene {
        WindowGroup {
            ConferenceShellView(model: model)
                .task { model.bootstrap() }
                // Riot's identity is the warm cream/newsprint zine look — a
                // light-first design. Lock the appearance so the brand stays
                // coherent instead of inverting to a muddy dark paper in the
                // system's dark mode.
                .preferredColorScheme(.light)
        }
        .defaultSize(width: 480, height: 860)
        .commands {
            // A developer affordance (macOS-only, `net`-feature): dial the
            // deployed anchor relay over the internet and import a community,
            // then report the outcome. Same path the RiotKitTests-macOS live
            // proof exercises — `bindNetRuntime()` + `syncWithAnchor(...)`.
            CommandMenu("Debug") {
                Button("Pull Community From Live Anchor Relay") {
                    AnchorRelayDebugPull.run()
                }
                .keyboardShortcut("P", modifiers: [.command, .shift])
            }
        }
    }
}

/// The user-triggerable twin of `AnchorNetPullTests`: binds the FFI-owned iroh
/// endpoint and pulls the committed community from the DEPLOYED relay over the
/// internet, then surfaces the verified/imported counts in an alert. Kept in
/// the macOS app target only — it depends on the `net`-feature FFI surface.
enum AnchorRelayDebugPull {
    /// The deployed relay's stable NodeId (64 hex) — the WHOLE dial hint. No IP,
    /// no port. The FFI net runtime binds under the `N0` preset (iroh relay +
    /// pkarr/DNS discovery on), so `syncWithAnchor` resolves this NodeId to a live
    /// address and NAT-traverses. This mirrors the iOS default
    /// (`AnchorRelayDefaults.relayNodeId`) so macOS and iOS dial the same way.
    private static let anchorHint =
        "60ab7b416b0ef0b8088cd64a3ef01edd598dcc5bb7a4df03145f957fec2432d8"

    /// Root-signed `ReadCommitted` ticket (hex) for a community on the relay.
    /// Re-baked 2026-07-23: durable 89-day ticket (expires ~2026-10-20) for
    /// community root 2052fabaefdea8eb3da14b0064a39dc1f7e062b354fa9f7fde5b0c337439f5bf.
    private static let ticketHex =
        "83028c58202052fabaefdea8eb3da14b0064a39dc1f7e062b354fa9f7fde5b0c337439f5bf58202052fabaefdea8eb3da14b0064a39dc1f7e062b354fa9f7fde5b0c337439f5bf5820583b4fa0348fbb3dad51cbfd3e760cb4e695c97c0397cd6f86c7be720a57f2025820a94dde010d9c3f70bbe6c39d7ab766fe272408cc9c42a7454b57125915165c68582077ebb646a8e0ae43309d1e0383f35b8bfaf559502c862c5fe27ebc2f7e9a70c81a6a618677026c726571756972655f6e6f6e656c726571756972655f6e6f6e65011a6a6186131a6ad6dbf758405a9b0175e9494e9582a2493bfb37009c4aeeed1748b940f07430163ea95d8d18498a0cd72a29c8fe3172e29f9ff8ce887ae0afee8433aab4be0c69b0c1d95c0b"

    private static func decodeHex(_ hex: String) -> Data {
        var data = Data(capacity: hex.count / 2)
        var index = hex.startIndex
        while index < hex.endIndex {
            let next = hex.index(index, offsetBy: 2)
            data.append(UInt8(hex[index..<next], radix: 16) ?? 0)
            index = next
        }
        return data
    }

    /// Kick the pull off the main thread (the FFI `block_on`s the dial), then
    /// present the result on the main thread.
    static func run() {
        let ticket = decodeHex(ticketHex)
        let now = UInt64(Date().timeIntervalSince1970)
        Task.detached(priority: .userInitiated) {
            let title: String
            let message: String
            do {
                let profile = try openLocalProfile()
                let runtime = try bindNetRuntime()
                let outcome = try runtime.syncWithAnchor(
                    profile: profile,
                    anchorHint: anchorHint,
                    ticketBytes: ticket,
                    nowUnix: now
                )
                let verified = outcome.namespaces.reduce(0) { $0 + $1.verified }
                let imported = outcome.namespaces.reduce(0) { $0 + $1.imported }
                let rejected = outcome.namespaces.reduce(0) { $0 + $1.rejected }
                title = "Pulled community from live relay"
                message = """
                    root \(outcome.root.prefix(16))…
                    namespaces: \(outcome.namespaces.count)
                    verified: \(verified)   imported: \(imported)   rejected: \(rejected)
                    """
            } catch {
                title = "Anchor pull failed"
                message = "\(error)"
            }
            await MainActor.run {
                let alert = NSAlert()
                alert.messageText = title
                alert.informativeText = message
                alert.runModal()
            }
        }
    }
}
