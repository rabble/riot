import XCTest

@testable import RiotKit

/// WU-006 Task 8a — the §9.3 mandatory seizure disclosure gate for owned-site
/// (masthead) creation. Minting an owned masthead puts the site's owner key on
/// this phone; a captor who seizes the unlocked device gains that same power —
/// impersonation of the site plus revocation of the real editors, not merely
/// the loss of a key. These tests pin that the disclosure copy says so, and
/// that the mint gate stays closed until the disclosure is acknowledged.
final class OwnedSiteCreationTests: XCTestCase {
    // MARK: - §9.3 disclosure copy

    func testDisclosureBodyNamesImpersonationAsASeizureConsequence() {
        let body = SiteSeizureDisclosure.body.lowercased()
        XCTAssertTrue(
            body.contains("impersonat"),
            "the disclosure must name impersonation of the site as a seizure consequence"
        )
    }

    func testDisclosureBodyNamesEditorRevocationAsASeizureConsequence() {
        let body = SiteSeizureDisclosure.body.lowercased()
        XCTAssertTrue(
            body.contains("revoke"),
            "the disclosure must name revocation as a seizure consequence"
        )
        XCTAssertTrue(
            body.contains("editor"),
            "the disclosure must name the real editors as who gets revoked"
        )
    }

    func testDisclosureBodyDoesNotReduceTheRiskToKeyLossAlone() {
        let body = SiteSeizureDisclosure.body.lowercased()
        // The copy must go further than "you could lose a key" — it must name a
        // full takeover of the site, which is the actual §9.3 risk.
        XCTAssertTrue(
            body.contains("take over") || body.contains("takeover"),
            "the disclosure must describe a takeover of the site, not just key loss"
        )
    }

    func testDisclosureHasATitleAndAnAcknowledgementLabel() {
        XCTAssertFalse(SiteSeizureDisclosure.title.isEmpty)
        XCTAssertFalse(SiteSeizureDisclosure.acknowledgementLabel.isEmpty)
    }

    // MARK: - Gate

    func testGateCanMintIsFalseBeforeAcknowledgement() {
        let gate = OwnedSiteCreationGate()
        XCTAssertFalse(gate.disclosureAcknowledged)
        XCTAssertFalse(gate.canMint)
    }

    func testGateCanMintIsTrueOnlyAfterAcknowledgement() {
        var gate = OwnedSiteCreationGate()
        gate.disclosureAcknowledged = true
        XCTAssertTrue(gate.canMint)
    }

    func testGateCanMintReturnsFalseAgainIfAcknowledgementIsRevoked() {
        var gate = OwnedSiteCreationGate(disclosureAcknowledged: true)
        XCTAssertTrue(gate.canMint)
        gate.disclosureAcknowledged = false
        XCTAssertFalse(gate.canMint)
    }
}
