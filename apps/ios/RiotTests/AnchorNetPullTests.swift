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
    /// The relay's stable NodeId (64 hex) — the WHOLE dial hint. No IP, no port.
    /// `syncWithAnchor` resolves it via iroh relay + pkarr/DNS discovery (the net
    /// runtime binds under the `N0` preset). This is the same NodeId-only dial the
    /// shipping iOS/macOS default path uses; a non-zero import proves discovery
    /// resolved the bare NodeId and the data crossed the internet.
    private static let anchorHint =
        "60ab7b416b0ef0b8088cd64a3ef01edd598dcc5bb7a4df03145f957fec2432d8"

    /// A root-signed `ReadCommitted` ticket (hex) for a community committed on
    /// the live relay — ~3 entries across the O/C/W namespaces. Re-baked
    /// 2026-07-23: durable 89-day ticket (expires ~2026-10-20) for community
    /// root 2052fabaefdea8eb3da14b0064a39dc1f7e062b354fa9f7fde5b0c337439f5bf.
    private static let ticketHex =
        "83028c58202052fabaefdea8eb3da14b0064a39dc1f7e062b354fa9f7fde5b0c337439f5bf58202052fabaefdea8eb3da14b0064a39dc1f7e062b354fa9f7fde5b0c337439f5bf5820583b4fa0348fbb3dad51cbfd3e760cb4e695c97c0397cd6f86c7be720a57f2025820a94dde010d9c3f70bbe6c39d7ab766fe272408cc9c42a7454b57125915165c68582077ebb646a8e0ae43309d1e0383f35b8bfaf559502c862c5fe27ebc2f7e9a70c81a6a618677026c726571756972655f6e6f6e656c726571756972655f6e6f6e65011a6a6186131a6ad6dbf758405a9b0175e9494e9582a2493bfb37009c4aeeed1748b940f07430163ea95d8d18498a0cd72a29c8fe3172e29f9ff8ce887ae0afee8433aab4be0c69b0c1d95c0b"

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
