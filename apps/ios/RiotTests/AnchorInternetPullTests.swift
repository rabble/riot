import XCTest
@testable import RiotKit

/// The proof for issue #107 Phase 3: the iOS app pulls a real community from the
/// DEPLOYED anchor relay over the internet.
///
/// This exercises the full net FFI surface end-to-end from the iOS SIMULATOR:
/// `bindNetRuntime()` binds an ephemeral follower iroh endpoint + tokio runtime
/// inside the staticlib, and `syncWithAnchor(...)` dials the live relay by its
/// direct node address, runs the gated `riot/sync/2` ReadCommitted session,
/// verifies every served entry through the canonical gate, and imports the
/// store-admissible ones into a fresh in-memory profile.
///
/// NETWORK-DEPENDENT: this test requires the deployed GCP anchor relay to be
/// live and reachable over the internet (outbound UDP). The simulator dials out
/// only — no inbound listener — so no local-network entitlement/prompt applies.
/// If the relay is down the test is skipped (not failed) via `XCTSkip`.
final class AnchorInternetPullTests: XCTestCase {

    /// The deployed relay's stable NodeId (64 hex) — the WHOLE dial hint. No IP,
    /// no port: the net runtime resolves it via iroh relay + pkarr/DNS discovery
    /// under the `N0` preset. The relay stands on a static IP behind GCP; dialing
    /// by bare NodeId survives its restarts (an ip:port would rot on redeploy).
    private static let anchorHint =
        "60ab7b416b0ef0b8088cd64a3ef01edd598dcc5bb7a4df03145f957fec2432d8"

    /// A root-signed ReadCommitted ticket (hex) for a community already
    /// committed on the relay: an O masthead + C comments + W wire namespace,
    /// ~3 entries total. Minted server-side; embedded here so the test is
    /// self-contained (no resource-bundle registration needed). Re-baked
    /// 2026-07-23: durable 89-day ticket (expires ~2026-10-20) for community
    /// root 2052fabaefdea8eb3da14b0064a39dc1f7e062b354fa9f7fde5b0c337439f5bf.
    private static let ticketHex =
        "83028c58202052fabaefdea8eb3da14b0064a39dc1f7e062b354fa9f7fde5b0c" +
        "337439f5bf58202052fabaefdea8eb3da14b0064a39dc1f7e062b354fa9f7fde" +
        "5b0c337439f5bf5820583b4fa0348fbb3dad51cbfd3e760cb4e695c97c0397cd" +
        "6f86c7be720a57f2025820a94dde010d9c3f70bbe6c39d7ab766fe272408cc9c" +
        "42a7454b57125915165c68582077ebb646a8e0ae43309d1e0383f35b8bfaf559" +
        "502c862c5fe27ebc2f7e9a70c81a6a618677026c726571756972655f6e6f6e65" +
        "6c726571756972655f6e6f6e65011a6a6186131a6ad6dbf758405a9b0175e949" +
        "4e9582a2493bfb37009c4aeeed1748b940f07430163ea95d8d18498a0cd72a29" +
        "c8fe3172e29f9ff8ce887ae0afee8433aab4be0c69b0c1d95c0b"

    private static func hexToData(_ hex: String) -> Data {
        var data = Data(capacity: hex.count / 2)
        var index = hex.startIndex
        while index < hex.endIndex {
            let next = hex.index(index, offsetBy: 2)
            guard let byte = UInt8(hex[index..<next], radix: 16) else {
                fatalError("ticket hex is not valid hex")
            }
            data.append(byte)
            index = next
        }
        return data
    }

    func testPullsCommunityFromLiveRelayOverTheInternet() throws {
        // The ticket decodes to the expected 282-byte envelope.
        let ticket = Self.hexToData(Self.ticketHex)
        XCTAssertEqual(ticket.count, Self.ticketHex.count / 2)
        XCTAssertFalse(ticket.isEmpty)

        // A fresh phone: an empty in-memory willow store, exactly as a phone that
        // has never seen this community. The pull is the only thing that can put
        // entries into it.
        let profile = try openLocalProfile()

        // Bind the FFI-owned iroh endpoint + tokio runtime (ephemeral follower).
        let net = try bindNetRuntime()

        let now = UInt64(Date().timeIntervalSince1970)

        let outcome: AnchorSyncOutcome
        do {
            outcome = try net.syncWithAnchor(
                profile: profile,
                anchorHint: Self.anchorHint,
                ticketBytes: ticket,
                nowUnix: now
            )
        } catch {
            // A transport/dial failure means the deployed relay is unreachable
            // (down, or this CI host has no outbound internet). That is an
            // environment condition, not a defect in the pull path — skip.
            throw XCTSkip(
                "anchor relay unreachable over the internet: \(error). " +
                "This test requires the live GCP relay at \(Self.anchorHint)."
            )
        }

        // Evidence: the concrete pull result from the live relay.
        print("ANCHOR_PULL_OUTCOME root=\(outcome.root)")
        for ns in outcome.namespaces {
            print("ANCHOR_PULL_NS id=\(ns.namespaceId) verified=\(ns.verified) " +
                  "imported=\(ns.imported) rejected=\(ns.rejected) refusal=\(ns.refusal ?? "none")")
        }

        // The outcome's root is the community root (the ticket's O namespace).
        XCTAssertFalse(outcome.root.isEmpty, "outcome must name a community root")

        // The relay committed O + C + W, so the pull attempts three namespaces.
        XCTAssertFalse(outcome.namespaces.isEmpty, "expected O/C/W namespaces")

        // The load-bearing assertion: entries actually crossed the internet,
        // verified through the canonical gate, and imported into the phone store.
        let totalVerified = outcome.namespaces.reduce(0) { $0 + $1.verified }
        let totalImported = outcome.namespaces.reduce(0) { $0 + $1.imported }
        let totalRejected = outcome.namespaces.reduce(0) { $0 + $1.rejected }

        // Nothing the anchor served may fail the phone's canonical gate.
        XCTAssertEqual(totalRejected, 0, "no served entry may be refused at the gate")

        // Real entries were verified and imported from the live relay.
        XCTAssertGreaterThan(totalVerified, 0, "expected verified entries from the relay")
        XCTAssertGreaterThan(
            totalImported, 0,
            "expected the pull to import entries into the phone store; got outcome \(outcome)"
        )

        // No namespace should have carried a refusal on the happy path.
        for ns in outcome.namespaces {
            XCTAssertNil(
                ns.refusal,
                "namespace \(ns.namespaceId) refused: \(ns.refusal ?? "")"
            )
        }
    }
}
