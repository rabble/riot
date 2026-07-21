import XCTest
@testable import RiotKit

/// The in-app "Using Riot" field manual (offline-guides design, Guide 2).
/// These tests pin the guide to the design's information architecture and —
/// more importantly — to the CURRENT visible UI: every label a step names must
/// be the exact string the app draws today. When a label changes, the compile
/// or a test here breaks before a stale instruction ships.
@MainActor
final class UsingRiotGuideTests: XCTestCase {

    // MARK: Information architecture

    /// The section order is the design doc's Guide 2 IA. Order matters: a
    /// person under pressure scans top-to-bottom from setup to recovery.
    func testSectionsFollowTheDesignInformationArchitectureInOrder() {
        XCTAssertEqual(
            UsingRiotGuide.sections.map(\.title),
            [
                "Start here",
                "Create or join a community",
                "Manage your communities",
                "Post and read updates",
                "Exchange nearby",
                "Share a community",
                "Use community tools",
                "Privacy and safety",
                "Troubleshooting",
                "Platform notes",
                "What is not available yet",
            ]
        )
    }

    /// Section ids are stable and unique — they are navigation anchors.
    func testSectionIDsAreUnique() {
        let ids = UsingRiotGuide.sections.map(\.id)
        XCTAssertEqual(ids.count, Set(ids).count)
    }

    // MARK: Task shape (design "Instruction format")

    /// Every task has a goal, at least one step, an expected result, and one
    /// recovery path. No half-written instruction ships.
    func testEveryTaskHasStepsAnExpectedResultAndARecovery() {
        for section in UsingRiotGuide.sections {
            for task in section.tasks {
                XCTAssertFalse(task.goal.isEmpty, "\(section.id): empty goal")
                XCTAssertFalse(task.steps.isEmpty, "\(task.goal): no steps")
                for step in task.steps {
                    XCTAssertFalse(step.isEmpty, "\(task.goal): empty step")
                }
                XCTAssertFalse(
                    task.expectedResult.isEmpty,
                    "\(task.goal): no expected result"
                )
                XCTAssertFalse(task.recovery.isEmpty, "\(task.goal): no recovery")
            }
        }
    }

    /// Every task states whether it works offline or needs a permission or
    /// connection — the design's "Works offline" contract.
    func testEveryTaskDeclaresConnectivity() {
        for section in UsingRiotGuide.sections {
            for task in section.tasks {
                switch task.connectivity {
                case .worksOffline:
                    break
                case let .needsPermission(what):
                    XCTAssertFalse(what.isEmpty, "\(task.goal): empty permission")
                }
            }
        }
    }

    // MARK: Labels match the current UI exactly (editorial rule 7)

    /// Steps must name what the screen actually says. Each entry pairs a
    /// canonical UI string with the requirement that some step contains it.
    func testInstructionsUseTheExactVisibleLabels() {
        let allSteps = UsingRiotGuide.sections
            .flatMap(\.tasks)
            .flatMap(\.steps)
        let required = [
            "Get started",
            "Join with a link or QR",
            "Create a community",
            "Join this community",
            "Your communities",
            "Community settings",
            "Leave this community",
            "Share this community",
            PostUpdateViewModel.primaryActionTitle,   // "Post an update"
            "Find nearby devices",
            NearbyStrings.stopLabel,                  // "Stop"
            "Not now",
            "Open Settings",
            RiotDestination.home.title,
            RiotDestination.tools.title,
            RiotDestination.people.title,
            RiotDestination.nearby.title,
        ]
        for label in required {
            XCTAssertTrue(
                allSteps.contains { $0.contains(label) },
                "no step names the visible label \"\(label)\""
            )
        }
    }

    /// The post-flow instruction quotes the app's real pending-exchange copy,
    /// so "posted" is never oversold as "delivered".
    func testPostingExplainsLocalSuccessAndPendingExchange() {
        let section = UsingRiotGuide.sections.first { $0.id == "post-and-read" }
        XCTAssertNotNil(section)
        let text = (section?.tasks.flatMap { [$0.expectedResult] + $0.steps } ?? [])
            .joined(separator: " ")
        XCTAssertTrue(
            text.contains(PostedUpdate.pendingExchangeStatus),
            "posting task must quote the exact pending-exchange status"
        )
    }

    // MARK: Honesty boundaries (design editorial rules 4/5, privacy section)

    /// The privacy section keeps the non-negotiable boundaries: public content
    /// is plaintext, pseudonymity is not anonymity, and no recall guarantee.
    func testPrivacySectionKeepsTheHonestyBoundaries() {
        guard let section = UsingRiotGuide.sections.first(where: { $0.id == "privacy" })
        else { return XCTFail("privacy section missing") }
        let text = (section.notes + section.tasks.flatMap(\.steps)).joined(separator: " ")
        XCTAssertTrue(text.localizedCaseInsensitiveContains("plaintext"))
        XCTAssertTrue(text.localizedCaseInsensitiveContains("not anonymity"))
        XCTAssertTrue(text.localizedCaseInsensitiveContains("cannot"))
    }

    /// "What is not available yet" names the current gaps and never shrinks to
    /// nothing while those gaps exist.
    func testNotAvailableYetNamesTheCurrentGaps() {
        guard let section = UsingRiotGuide.sections.last else {
            return XCTFail("no sections")
        }
        XCTAssertEqual(section.id, "not-yet")
        let text = section.notes.joined(separator: " ")
        XCTAssertTrue(text.localizedCaseInsensitiveContains("encrypted private groups"))
        XCTAssertTrue(text.localizedCaseInsensitiveContains("internet"))
        XCTAssertTrue(text.localizedCaseInsensitiveContains("prototype"))
    }

    /// Every section carries either notes or tasks — no empty shells.
    func testNoSectionIsEmpty() {
        for section in UsingRiotGuide.sections {
            XCTAssertTrue(
                !section.notes.isEmpty || !section.tasks.isEmpty,
                "\(section.id) has no content"
            )
        }
    }

    // MARK: Entry point

    /// The entry label is the design's canonical cross-platform name.
    func testEntryLabelIsUsingRiot() {
        XCTAssertEqual(UsingRiotGuide.entryLabel, "Using Riot")
    }

    /// The guide shows which build its instructions were checked against
    /// (design: "The rendered guide shows the tested app version and checked
    /// date"), so a person on an older build knows the labels may differ.
    func testGuideCarriesACheckedDate() {
        XCTAssertFalse(UsingRiotGuide.checkedDate.isEmpty)
    }
}
