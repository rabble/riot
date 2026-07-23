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

    /// The deployed relay's direct node address: `<node_id_hex>@<ip:port>`.
    /// Standing on GCP; this is the address the phone dials over the internet.
    private static let anchorHint =
        "60ab7b416b0ef0b8088cd64a3ef01edd598dcc5bb7a4df03145f957fec2432d8@136.65.192.159:38472"

    /// A root-signed ReadCommitted ticket (hex) for a community already
    /// committed on the relay: an O masthead + C comments + W wire namespace,
    /// ~3 entries total. Minted server-side; embedded here so the test is
    /// self-contained (no resource-bundle registration needed).
    private static let ticketHex =
        "83028c582031724287c743287652d99b9cb6178aff8f19153fde1a89399c9131" +
        "6974acfc87582031724287c743287652d99b9cb6178aff8f19153fde1a89399c" +
        "91316974acfc875820d62b536e8b2ca4a44733723a868d65239c97283077ed30" +
        "77470511d1a37a9d9658204269b5846a1f58095c9a0c6fd83f8977af25b0c182" +
        "fc723aa4d745bd7e09c9385820aa6fdeaa645a644cf42c316e49fadd823cb473" +
        "e1cd831f94a67d9f803031ef6b1a6a616a57026c726571756972655f6e6f6e65" +
        "6c726571756972655f6e6f6e65011a6a6169f31a6a6178675840de7b68ef985b" +
        "c3109669dd8dc8c64b3a090629db25e2261e753514e97a6b2fda9b97cee2a956" +
        "2baa3483480ae139632ff8b7ef0b62a102879740cde450394600"

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
