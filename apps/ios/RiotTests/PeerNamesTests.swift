import XCTest
@testable import RiotKit

final class PeerNamesTests: XCTestCase {
    func testTheSameNonceAlwaysGivesTheSameName() {
        // A peer must keep its name for as long as you can see it, or you can't
        // say "the copper heron is yours?" across a room.
        XCTAssertEqual(PeerNames.name(sessionNonce: 12345), PeerNames.name(sessionNonce: 12345))
    }

    func testAPeerNameIsNeverShapedLikeAPerson() {
        // A person renders as "Ana · a3f9" — a claimed name bound to a key tag.
        // A phone we have not met must never wear that shape, or the one
        // distinction that matters collapses.
        for nonce in stride(from: UInt64(0), to: 5_000, by: 7) {
            let name = PeerNames.name(sessionNonce: nonce)
            XCTAssertFalse(name.contains("·"), "a peer handle must never carry a key tag: \(name)")
            XCTAssertEqual(name.split(separator: " ").count, 2, "expected two words, got \(name)")
        }
    }

    func testTheNameSpaceIsActuallyReachable() {
        // The regression this class exists for: the old generator drew both
        // indices from the same nonce linearly (`n % 4`, `(n * 2) % 4`), so half
        // its combinations could never occur. Sweep a lot of nonces and demand
        // we actually reach nearly all of the 128 × 128 space.
        var seen = Set<String>()
        for nonce in 0..<UInt64(200_000) {
            seen.insert(PeerNames.name(sessionNonce: nonce))
        }
        // Coupon collector, not wishful thinking: with N=16,384 buckets and
        // d draws, the expected number still unseen is N·e^(−d/N). At d=200k
        // that is 16384·e^(−12.2) ≈ 0.08 — so essentially full coverage, and a
        // threshold of 16,300 is comfortable rather than lucky. (At 60k draws
        // ~420 would still be missing, which is healthy behaviour, not a bug —
        // an earlier version of this test asserted 16,000 there and failed the
        // *test*, not the code.)
        XCTAssertGreaterThan(
            seen.count, 16_300,
            "only \(seen.count) of 16,384 names are reachable — the mixer is correlating the halves"
        )
    }

    func testBothHalvesVaryIndependently() {
        // Pin the specific failure mode: neither the adjective nor the noun may
        // be stuck on a subset while the other moves.
        var adjectives = Set<Substring>()
        var nouns = Set<Substring>()
        for nonce in 0..<UInt64(20_000) {
            let parts = PeerNames.name(sessionNonce: nonce).split(separator: " ")
            adjectives.insert(parts[0])
            nouns.insert(parts[1])
        }
        XCTAssertEqual(adjectives.count, PeerNames.adjectives.count, "some adjectives are unreachable")
        XCTAssertEqual(nouns.count, PeerNames.nouns.count, "some nouns are unreachable")
    }

    func testCollisionsAreRareEnoughToBeFunnyRatherThanConfusing() {
        // These are handles, not identities — a collision is a moment of comedy,
        // not a security failure. But it should be rare in a real room.
        var seen = Set<String>()
        var collisions = 0
        for nonce in stride(from: UInt64(1), to: 51, by: 1) {
            let name = PeerNames.name(sessionNonce: nonce &* 0x9E37_79B9)
            if !seen.insert(name).inserted { collisions += 1 }
        }
        XCTAssertLessThanOrEqual(collisions, 2, "a room of 50 phones should rarely double up")
    }

    func testTheOldEntryPointStillWorksAndIsFixed() {
        // Every call site goes through FriendlyNameGenerator; it must now inherit
        // the real name space rather than the sixteen-name one.
        var seen = Set<String>()
        for nonce in 0..<UInt64(5_000) {
            seen.insert(FriendlyNameGenerator.name(sessionNonce: nonce))
        }
        XCTAssertGreaterThan(seen.count, 1_000, "FriendlyNameGenerator is still on the tiny list")
    }
}
