import XCTest
@testable import RiotKit

/// Live proof that the `net`-feature FFI surface can pull a community from the
/// DEPLOYED anchor relay over the internet, from a macOS test process.
///
/// This dials a REAL relay running on GCP (not a loopback fixture): it binds the
/// FFI-owned iroh endpoint (`bindNetRuntime`), then drives the gated
/// ReadCommitted pull (`MobileNetRuntime.syncWithAnchor`) against a root-signed
/// ticket for a community already committed on that relay, and asserts entries
/// landed in the phone store through the canonical preview→plan→commit boundary.
///
/// The `net` surface only exists when the staticlib + generated bindings are
/// built with `--features net` / `RIOT_FFI_NET_BINDINGS=1`. If those types are
/// missing the whole target won't compile — which is itself the coupling proof.
///
/// Because it reaches the public internet it is not a hermetic unit test. It
/// runs by default (this is a proof target); set `RIOT_SKIP_LIVE_ANCHOR=1` in
/// the test process environment to opt out and keep a run fully offline.
final class AnchorNetPullTests: XCTestCase {
    /// Direct node hint for the live relay: `<id_hex>@<ip:port>`.
    private static let anchorHint =
        "60ab7b416b0ef0b8088cd64a3ef01edd598dcc5bb7a4df03145f957fec2432d8@136.65.192.159:38472"

    /// A root-signed `ReadCommitted` ticket (hex) for a community committed on
    /// the live relay — ~3 entries across the O/C/W namespaces.
    private static let ticketHex =
        "83028c582031724287c743287652d99b9cb6178aff8f19153fde1a89399c91316974acfc87582031724287c743287652d99b9cb6178aff8f19153fde1a89399c91316974acfc875820d62b536e8b2ca4a44733723a868d65239c97283077ed3077470511d1a37a9d9658204269b5846a1f58095c9a0c6fd83f8977af25b0c182fc723aa4d745bd7e09c9385820aa6fdeaa645a644cf42c316e49fadd823cb473e1cd831f94a67d9f803031ef6b1a6a616a57026c726571756972655f6e6f6e656c726571756972655f6e6f6e65011a6a6169f31a6a6178675840de7b68ef985bc3109669dd8dc8c64b3a090629db25e2261e753514e97a6b2fda9b97cee2a9562baa3483480ae139632ff8b7ef0b62a102879740cde450394600"

    private static func decodeHex(_ hex: String) -> Data {
        var data = Data(capacity: hex.count / 2)
        var index = hex.startIndex
        while index < hex.endIndex {
            let next = hex.index(index, offsetBy: 2)
            data.append(UInt8(hex[index..<next], radix: 16)!)
            index = next
        }
        return data
    }

    func testPullCommunityFromLiveAnchorRelayOverTheInternet() throws {
        try XCTSkipIf(
            ProcessInfo.processInfo.environment["RIOT_SKIP_LIVE_ANCHOR"] == "1",
            "RIOT_SKIP_LIVE_ANCHOR=1 set — skipping the live GCP relay dial."
        )

        let ticket = Self.decodeHex(Self.ticketHex)
        XCTAssertEqual(ticket.count, Self.ticketHex.count / 2, "ticket hex must decode cleanly")

        // A fresh, empty phone store: any entry it ends up holding came off the
        // wire from the relay, nothing was pre-seeded.
        let profile = try openLocalProfile()

        // Bind the FFI-owned iroh endpoint + tokio runtime (ephemeral follower).
        let runtime = try bindNetRuntime()

        let now = UInt64(Date().timeIntervalSince1970)
        let outcome = try runtime.syncWithAnchor(
            profile: profile,
            anchorHint: Self.anchorHint,
            ticketBytes: ticket,
            nowUnix: now
        )

        let imported = outcome.namespaces.reduce(0) { $0 + $1.imported }
        let verified = outcome.namespaces.reduce(0) { $0 + $1.verified }
        let rejected = outcome.namespaces.reduce(0) { $0 + $1.rejected }

        print("LIVE ANCHOR PULL root=\(outcome.root)")
        for ns in outcome.namespaces {
            print(
                "  ns=\(ns.namespaceId) verified=\(ns.verified) imported=\(ns.imported)"
                    + " rejected=\(ns.rejected) refusal=\(ns.refusal ?? "none")"
            )
        }
        print("LIVE ANCHOR PULL totals verified=\(verified) imported=\(imported) rejected=\(rejected)")

        // The proof: the relay served this community and entries verified through
        // the canonical gate and committed into the store.
        XCTAssertGreaterThan(verified, 0, "the live relay served nothing that verified")
        XCTAssertGreaterThan(imported, 0, "no entries imported from the live relay")
        XCTAssertEqual(rejected, 0, "the live relay served something that failed the gate")

        // Best-effort store readback. `listCurrentEntries()` enumerates the
        // *currently selected* community; a bare anchor pull imports into the
        // store without selecting/activating a community, so this throws
        // `InvalidInput` (the same "nothing is current" path DemoMode hiding
        // exercises). That is expected and is NOT the proof — the proof is the
        // verified/imported counts above. We surface the readback for the log
        // without letting its (expected) throw fail the live-pull assertion.
        if let entries = try? profile.listCurrentEntries() {
            print("LIVE ANCHOR PULL store now surfaces \(entries.count) current entries")
        } else {
            print("LIVE ANCHOR PULL store readback deferred (no current community selected)")
        }
    }
}
