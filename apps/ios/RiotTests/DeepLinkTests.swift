import XCTest
@testable import RiotKit

/// "Open in Riot" deep-link + signature verify (web = reach, app = truth).
///
/// The public web newswire renders signed posts and emits deep links —
/// `riot://open?namespace=<ns>` (home/masthead) and
/// `riot://open?namespace=<ns>&entry=<entry_id>` (per-post) — plus the existing
/// `riot://newswire/join/v1/...` share reference. This suite pins two things:
///
///  1. **Parsing** — both `open` shapes and the `join` shape are recognised, and
///     junk is refused, so the router never acts on a link it did not understand.
///
///  2. **The anti-forgery boundary** — a post is reported `verified` ONLY when the
///     device independently HOLDS it as a signed record for a community it
///     follows. That record is in the store only because it passed core's
///     Ed25519-verifying import path (`inspect_news_record`), so a mirror cannot
///     make the app show "verified" for a post it never synced. The final test
///     drives the REAL `MobileProfile`/`riot-core` linked into `RiotKit`: a
///     genuine signed post verifies; a forged entry id the mirror could invent
///     resolves to `postNotHeld`, never `verified`.
@MainActor
final class DeepLinkTests: XCTestCase {

    // MARK: - Parsing: both link shapes + rejection

    func testParsesPerPostOpenLink() {
        let url = URL(string: "riot://open?namespace=ab12&entry=cd34")!
        XCTAssertEqual(RiotDeepLink.parse(url), .openSpace(namespace: "ab12", entry: "cd34"))
    }

    func testParsesHomeOpenLinkWithoutEntry() {
        let url = URL(string: "riot://open?namespace=ab12")!
        XCTAssertEqual(RiotDeepLink.parse(url), .openSpace(namespace: "ab12", entry: nil))
    }

    func testParsesJoinReferenceAndPreservesTheWholeCanonicalString() {
        // The join codec (`decodeShareReference`) consumes the FULL canonical
        // string, so the router must hand it back verbatim, not re-split it.
        let canonical = "riot://newswire/join/v1/AAAA.BBBB.CCCC"
        XCTAssertEqual(RiotDeepLink.parse(URL(string: canonical)!),
                       .joinReference(encoded: canonical))
    }

    func testRejectsNonRiotScheme() {
        XCTAssertNil(RiotDeepLink.parse(URL(string: "https://example.org/open?namespace=ab12")!))
    }

    func testRejectsOpenLinkWithoutANamespace() {
        XCTAssertNil(RiotDeepLink.parse(URL(string: "riot://open?entry=cd34")!))
    }

    func testRejectsUnknownRiotHost() {
        XCTAssertNil(RiotDeepLink.parse(URL(string: "riot://frobnicate?x=1")!))
    }

    // MARK: - Verify outcome: pure resolver branches

    func testNotFollowingWhenTheDeviceDoesNotHoldTheCommunity() {
        let out = RiotDeepLinkResolver.resolveOpen(
            namespace: "ns", entry: "e1", followsNamespace: false, heldEntryIDs: [])
        XCTAssertEqual(out, .notFollowing(namespace: "ns", entry: "e1"))
    }

    func testOpenedHomeWhenFollowingAndTheLinkNamesNoEntry() {
        let out = RiotDeepLinkResolver.resolveOpen(
            namespace: "ns", entry: nil, followsNamespace: true, heldEntryIDs: [])
        XCTAssertEqual(out, .openedHome(namespace: "ns"))
    }

    func testVerifiedWhenFollowingAndThePostIsHeld() {
        // Entry id casing must not matter — the web emits lowercase hex, the
        // projection likewise, but a link could arrive upper-cased.
        let out = RiotDeepLinkResolver.resolveOpen(
            namespace: "ns", entry: "E1", followsNamespace: true,
            heldEntryIDs: ["e1"], headlineForEntry: { _ in "Report" })
        XCTAssertEqual(out, .verified(namespace: "ns", entry: "E1", headline: "Report"))
    }

    func testPostNotHeldWhenFollowingButTheEntryIsAbsent() {
        let out = RiotDeepLinkResolver.resolveOpen(
            namespace: "ns", entry: "e1", followsNamespace: true, heldEntryIDs: ["other"])
        XCTAssertEqual(out, .postNotHeld(namespace: "ns", entry: "e1"))
    }

    // MARK: - End-to-end against REAL signed records (the anti-forgery boundary)

    func testVerifiesAGenuineSignedPostAndRefusesAForgedEntryID() throws {
        let profile = try openLocalProfile()
        let space = try profile.createNewswireSpace(input: NewswireSpaceInput(
            name: "Riverside", summary: "Community newswire.", languages: ["en"],
            geographicTags: [], topicTags: [], editorialRoster: []))
        let post = try profile.createNewswirePost(input: NewswirePostInput(
            spaceDescriptorEntryId: space.entryId, headline: "Bridge blocked", body: "Body.",
            language: "en", eventTimeUnixSeconds: nil, expiresAtUnixSeconds: nil,
            coarseLocation: nil, sourceClaims: [], operationalProfile: nil, aiAssisted: false))

        let projection = try profile.projectNewswireSpace(spaceDescriptorEntryId: space.entryId)
        let held = Set(
            (projection.openWire + projection.frontPage + projection.earlier)
                .map { $0.entryId.lowercased() })

        // The genuine, signature-verified post → verified.
        let genuine = RiotDeepLinkResolver.resolveOpen(
            namespace: "ns", entry: post.entryId, followsNamespace: true, heldEntryIDs: held)
        guard case .verified = genuine else {
            return XCTFail("a genuine signed post must verify, got \(genuine)")
        }

        // A forged entry id the mirror could invent → never verified.
        let forgedEntry = String(repeating: "ff", count: 32)
        let forged = RiotDeepLinkResolver.resolveOpen(
            namespace: "ns", entry: forgedEntry, followsNamespace: true, heldEntryIDs: held)
        XCTAssertEqual(forged, .postNotHeld(namespace: "ns", entry: forgedEntry))
    }
}
