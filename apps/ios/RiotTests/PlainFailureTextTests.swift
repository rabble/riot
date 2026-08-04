import XCTest
@testable import RiotKit

/// Every failure a person can see must be a sentence they can act on — never a
/// Rust enum. Before this, `errorMessage = String(describing: error)` put
/// `CorruptDatabase(message: "...")` and `AuthorityInvalid` straight into the
/// "Riot couldn't finish that" alert, at launch, on the one screen a stuck
/// person has. These tests pin the two properties that matter: every case is
/// mapped, and nothing that reaches a person carries the machine word.
@MainActor
final class PlainFailureTextTests: XCTestCase {
    /// Every `MobileError` the FFI can raise. If Rust gains a case, `plain(for:)`
    /// keeps compiling (the `default` catches it) — so this list is the guard
    /// that a new case gets its own sentence rather than silently falling back.
    private static let allMobileErrors: [MobileError] = [
        .Internal, .SessionFailed, .InvalidInput, .DraftNotFound, .ImportRejected,
        .StoreFull, .SessionLimit, .ObjectClosed, .PreviewConsumed, .PlanConsumed,
        .StalePreview, .EntropyUnavailable, .ClockUnavailable, .AppRejected,
        .NotSpaceOrganizer, .LegacyProfileCannotOrganize, .Database,
        .CommunityUnavailable,
    ]

    /// Words that mean something to whoever wrote the protocol and nothing to
    /// whoever is holding the phone. None may appear in a message a person reads.
    private static let jargon = [
        "namespace", "entry", "entries", "capability", "digest", "payload",
        "packet", "signature", "signed", "sign ", "hash", "cbor", "willow",
        "meadowcap", "tombstone", "ocap", "arbiter", "subspace", "preview",
        "plan", "session", "ffi", "sqlite", "wal", "error", "invalid", "null",
        "nil", "internal", "_", "(",
    ]

    func testEveryMobileErrorHasItsOwnSentence() {
        var seen = Set<String>()
        for error in Self.allMobileErrors {
            let text = PlainFailureText.plain(for: error)
            XCTAssertFalse(
                text.isEmpty,
                "\(error) produced no message"
            )
            XCTAssertNotEqual(
                text, PlainFailureText.unknown,
                "\(error) fell through to the generic message instead of being mapped"
            )
            seen.insert(text)
        }
        XCTAssertEqual(
            seen.count, Self.allMobileErrors.count,
            "two errors share a sentence — each refusal needs its own next step"
        )
    }

    func testNoMessageCarriesProtocolJargon() {
        let messages = Self.allMobileErrors.map { PlainFailureText.plain(for: $0) }
            + [PlainFailureText.unknown]
        for message in messages {
            let lowered = message.lowercased()
            for word in Self.jargon {
                XCTAssertFalse(
                    lowered.contains(word),
                    "\"\(message)\" contains the machine word \"\(word)\""
                )
            }
        }
    }

    func testEveryMessageEndsAsASentence() {
        for error in Self.allMobileErrors {
            let text = PlainFailureText.plain(for: error)
            XCTAssertTrue(
                text.hasSuffix("."),
                "\"\(text)\" is a fragment, not a sentence"
            )
            XCTAssertTrue(
                text.first.map { $0.isUppercase } ?? false,
                "\"\(text)\" does not start like a sentence"
            )
        }
    }

    /// A Swift-side error (repository, filesystem, decoding) has no case to map,
    /// so it takes the generic sentence — but it must still never print itself.
    func testAForeignErrorFallsBackWithoutPrintingItself() {
        struct AuthorityInvalid: Error {}
        let text = PlainFailureText.plain(for: AuthorityInvalid())
        XCTAssertEqual(text, PlainFailureText.unknown)
        XCTAssertFalse(text.contains("AuthorityInvalid"))
    }

    /// The raw string is not lost — it moves out of the person's way and into the
    /// place a developer looks. `technical(for:)` is what gets logged.
    func testTheRawErrorIsStillAvailableForTheLog() {
        XCTAssertEqual(
            PlainFailureText.technical(for: MobileError.StalePreview),
            String(describing: MobileError.StalePreview)
        )
    }

    // MARK: - The call sites that used to leak

    func testBootstrapFailureReachesTheAlertAsPlainWords() {
        let model = RiotAppModel()
        model.presentFailure(MobileError.Database)
        XCTAssertEqual(model.errorMessage, PlainFailureText.plain(for: MobileError.Database))
        XCTAssertEqual(
            model.errorTechnicalDetails,
            String(describing: MobileError.Database)
        )
    }

    func testDismissingClearsBothHalves() {
        let model = RiotAppModel()
        model.presentFailure(MobileError.Database)
        model.dismissError()
        XCTAssertNil(model.errorMessage)
        XCTAssertNil(model.errorTechnicalDetails)
    }

    /// The organizer refusals already had good copy and keep it — this pins that
    /// the new fallback did not flatten them back to the generic sentence.
    func testOrganizerRefusalsKeepTheirSpecificAdvice() {
        let member = RiotAppModel.approvalFailureMessage(MobileError.NotSpaceOrganizer)
        XCTAssertTrue(member.contains("organizer"))
        let legacy = RiotAppModel.approvalFailureMessage(MobileError.LegacyProfileCannotOrganize)
        XCTAssertTrue(legacy.contains("new profile"))
    }

    /// The old `default:` branch of `approvalFailureMessage` printed the enum.
    func testAnUnmappedApprovalFailureIsAlsoPlain() {
        let text = RiotAppModel.approvalFailureMessage(MobileError.Internal)
        XCTAssertFalse(text.contains("Internal"))
        XCTAssertEqual(text, PlainFailureText.plain(for: MobileError.Internal))
    }
}
