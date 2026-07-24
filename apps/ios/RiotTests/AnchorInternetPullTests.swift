import XCTest
@testable import RiotKit

final class AnchorRelayFailureTests: XCTestCase {
    func testExpiredTicketIsNotReportedAsAnOfflineRelay() {
        let message = AnchorRelayFailure.message(
            for: AnchorSyncError.DialRefused(reason: "ExpiredTicket")
        )

        XCTAssertEqual(
            message,
            "Riot’s built-in community link expired. Update Riot to get a fresh link; nothing on your device changed."
        )
        XCTAssertFalse(message.localizedCaseInsensitiveContains("offline"))
        XCTAssertFalse(message.localizedCaseInsensitiveContains("no internet"))
    }

    func testTransportFailureKeepsTheReachabilityMessage() {
        XCTAssertEqual(
            AnchorRelayFailure.message(
                for: AnchorSyncError.Transport(reason: "connection timed out")
            ),
            AnchorRelayFailure.relayUnreachable
        )
    }

    func testNonExpiredDialRefusalReportsAnInvalidBuiltInLink() {
        XCTAssertEqual(
            AnchorRelayFailure.message(
                for: AnchorSyncError.DialRefused(reason: "InvalidTicket")
            ),
            "Riot’s built-in community link is no longer valid. Update Riot to get a fresh link; nothing on your device changed."
        )
    }

    func testNamespaceExpiredTicketRefusalIsNotReportedAsSuccessfulSync() {
        XCTAssertEqual(
            AnchorRelayFailure.message(
                forRefusal: "Some(ExpiredTicket { expires_at: 1, observed_at: 2 })"
            ),
            "Riot’s built-in community link expired. Update Riot to get a fresh link; Riot did not open or switch communities."
        )
    }

    func testNamespaceAuthorityRefusalReportsAnInvalidBuiltInLink() {
        XCTAssertEqual(
            AnchorRelayFailure.message(
                forRefusal: "Some(ManifestMismatch { expected_digest: [], observed_digest: [] })"
            ),
            "Riot’s built-in community link is no longer valid. Update Riot to get a fresh link; Riot did not open or switch communities."
        )
    }

    func testNamespaceBusyRefusalReportsAReachableButBusyRelay() {
        XCTAssertEqual(
            AnchorRelayFailure.message(
                forRefusal: "Some(Busy { limit_id: SyncSessions, retry_after_seconds: 1 })"
            ),
            "Riot reached the relay, but it is busy just now. Try again shortly; Riot did not open or switch communities."
        )
    }
}

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

    func testPullsCommunityFromLiveRelayOverTheInternet() throws {
        // Exercise the exact relay and ticket the shipping app uses. A second
        // copied test ticket could stay healthy while the real app default rots.
        let ticket = AnchorRelayDefaults.communityTicket
        XCTAssertEqual(ticket.count, AnchorRelayDefaults.communityTicketHex.count / 2)
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
                anchorHint: AnchorRelayDefaults.relayNodeId,
                ticketBytes: ticket,
                nowUnix: now
            )
        } catch let error as AnchorSyncError {
            guard case .Transport = error else {
                throw error
            }
            // A transport/dial failure means the deployed relay is unreachable
            // (down, or this CI host has no outbound internet). That is an
            // environment condition. Ticket and authority failures are product
            // defects and MUST fail this test instead of being mislabeled.
            throw XCTSkip(
                "anchor relay unreachable over the internet: \(error). " +
                "This test requires the live GCP relay at \(AnchorRelayDefaults.relayNodeId)."
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
