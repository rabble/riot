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
    /// Direct node hint for the live relay: `<id_hex>@<ip:port>`.
    private static let anchorHint =
        "60ab7b416b0ef0b8088cd64a3ef01edd598dcc5bb7a4df03145f957fec2432d8@136.65.192.159:38472"

    /// Root-signed `ReadCommitted` ticket (hex) for a community on the relay.
    private static let ticketHex =
        "83028c582031724287c743287652d99b9cb6178aff8f19153fde1a89399c91316974acfc87582031724287c743287652d99b9cb6178aff8f19153fde1a89399c91316974acfc875820d62b536e8b2ca4a44733723a868d65239c97283077ed3077470511d1a37a9d9658204269b5846a1f58095c9a0c6fd83f8977af25b0c182fc723aa4d745bd7e09c9385820aa6fdeaa645a644cf42c316e49fadd823cb473e1cd831f94a67d9f803031ef6b1a6a616a57026c726571756972655f6e6f6e656c726571756972655f6e6f6e65011a6a6169f31a6a6178675840de7b68ef985bc3109669dd8dc8c64b3a090629db25e2261e753514e97a6b2fda9b97cee2a9562baa3483480ae139632ff8b7ef0b62a102879740cde450394600"

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
