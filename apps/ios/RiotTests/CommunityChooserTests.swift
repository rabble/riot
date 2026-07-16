import XCTest

@testable import RiotKit

/// Unit 3B — the "Your communities" chooser and switch. The pure models
/// (plain-language rows, returning-opens-last-available, Command-K) are proven
/// standalone; a single-community integration test proves the registry seam and
/// the app-model wiring over the real FFI.
final class CommunityChooserTests: XCTestCase {

    // MARK: - Plain-language rows

    func testRelationshipsRenderInPlainLanguageNotTechnicalTerms() {
        XCTAssertEqual(CommunityRelationship.organizer.plainLabel, "Organizer")
        XCTAssertEqual(CommunityRelationship.member.plainLabel, "Member")
        XCTAssertEqual(CommunityRelationship.publicReader.plainLabel, "Public reader")
    }

    func testRecentActivityAndSyncFreshnessAreHumanPhrasesNotTimestamps() {
        let now = Date(timeIntervalSince1970: 1_000_000)
        func at(_ secondsAgo: TimeInterval) -> UInt64 { UInt64(now.timeIntervalSince1970 - secondsAgo) }

        XCTAssertEqual(CommunityRelativeTime.recentActivity(nil, now: now), "No activity yet")
        XCTAssertEqual(CommunityRelativeTime.syncFreshness(nil, now: now), "Not synced yet")
        XCTAssertEqual(CommunityRelativeTime.recentActivity(at(10), now: now), "Active just now")
        XCTAssertEqual(CommunityRelativeTime.recentActivity(at(60), now: now), "Active 1 minute ago")
        XCTAssertEqual(CommunityRelativeTime.recentActivity(at(120), now: now), "Active 2 minutes ago")
        XCTAssertEqual(CommunityRelativeTime.recentActivity(at(3_600), now: now), "Active 1 hour ago")
        XCTAssertEqual(CommunityRelativeTime.syncFreshness(at(7_200), now: now), "Synced 2 hours ago")
        XCTAssertEqual(CommunityRelativeTime.recentActivity(at(86_400), now: now), "Active 1 day ago")
        XCTAssertEqual(CommunityRelativeTime.syncFreshness(at(172_800), now: now), "Synced 2 days ago")
    }

    func testAChooserRowLeadsWithNameAndRelationshipNeverTheNamespaceID() {
        let now = Date(timeIntervalSince1970: 1_000_000)
        let core = CommunityRow(
            namespaceId: String(repeating: "a", count: 64),
            title: "Queers of Aotearoa",
            relationship: .member,
            descriptorEntryId: "desc-1",
            recentActivityUnixSeconds: UInt64(now.timeIntervalSince1970 - 3_600),
            syncFreshnessUnixSeconds: nil,
            archived: false,
            quarantined: false,
            available: true
        )
        let row = CommunityChooserRow.from(core, now: now)

        XCTAssertEqual(row.name, "Queers of Aotearoa")
        XCTAssertEqual(row.relationshipLabel, "Member")
        XCTAssertEqual(row.recentActivity, "Active 1 hour ago")
        XCTAssertEqual(row.syncFreshness, "Not synced yet")
        XCTAssertTrue(row.available)
        // No visible field carries the raw namespace id — it is a11y-only.
        for visible in [row.name, row.relationshipLabel, row.recentActivity, row.syncFreshness] {
            XCTAssertFalse(visible.contains(core.namespaceId), "a technical id leaked into \(visible)")
        }
        XCTAssertTrue(row.accessibilityID.contains(core.namespaceId), "the a11y handle may carry the id")
    }

    // MARK: - Returning opens the last available community directly

    private func core(
        _ ns: String,
        title: String = "C",
        available: Bool = true,
        archived: Bool = false,
        quarantined: Bool = false
    ) -> CommunityRow {
        CommunityRow(
            namespaceId: ns,
            title: title,
            relationship: .organizer,
            descriptorEntryId: nil,
            recentActivityUnixSeconds: nil,
            syncFreshnessUnixSeconds: nil,
            archived: archived,
            quarantined: quarantined,
            available: available
        )
    }

    func testReturningOpensTheLastAvailableCommunityDirectly() {
        let active = core("ns-a", title: "A", available: true)
        let outcome = CommunityReturnOutcome.decide(active: active, all: [active, core("ns-b")])
        XCTAssertEqual(outcome, .openCommunity(namespaceID: "ns-a"))
    }

    func testAnUnavailableLastCommunityOpensTheChooserWithItsRecordPreserved() {
        let active = core("ns-a", title: "Fire Watch", available: false)
        let outcome = CommunityReturnOutcome.decide(active: active, all: [active])
        XCTAssertEqual(outcome, .unavailable(CommunityUnavailable(name: "Fire Watch")))
    }

    func testNoActiveButHeldCommunitiesShowsTheChooser() {
        let outcome = CommunityReturnOutcome.decide(active: nil, all: [core("ns-a"), core("ns-b")])
        XCTAssertEqual(outcome, .chooser)
    }

    func testNoHeldCommunityIsTheNoCommunityState() {
        XCTAssertEqual(CommunityReturnOutcome.decide(active: nil, all: []), .noCommunity)
        // Only archived communities held → still nothing to open directly.
        XCTAssertEqual(
            CommunityReturnOutcome.decide(active: nil, all: [core("ns-a", archived: true)]),
            .noCommunity
        )
    }

    // MARK: - Command-K

    func testCommunitySelectionIsFocusedWithCommandK() {
        XCTAssertEqual(CommunitySelectionShortcut.keyEquivalent, "k")
    }

    // MARK: - Registry seam + app-model wiring (single community, real FFI)

    @MainActor
    func testCreatingACommunityListsItInTheChooserWithPlainLanguage() throws {
        let directory = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let model = RiotAppModel()
        model.bootstrap(storageDirectory: directory, keyStore: TestWrappingKeyStore(), starterPacks: [])

        model.createSpace(title: "Riverside Tenants Union")

        XCTAssertEqual(model.communities.count, 1, "the created community appears in the chooser")
        let row = try XCTUnwrap(model.communities.first)
        XCTAssertEqual(row.name, "Riverside Tenants Union")
        XCTAssertEqual(row.relationshipLabel, "Organizer", "the creator is the organizer")
        XCTAssertTrue(row.available)
        XCTAssertFalse(row.archived)
    }

    @MainActor
    func testTheRegistryReportsTheActiveCommunityForReturningDirectly() throws {
        let directory = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let repository = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: directory.appendingPathComponent("profile.json")),
            keyStore: TestWrappingKeyStore(),
            databasePath: directory.appendingPathComponent("riot.db").path
        )
        _ = try repository.createBackingSpace(name: "Uganda")

        let all = try repository.listCommunities()
        XCTAssertEqual(all.count, 1)
        let active = try XCTUnwrap(try repository.activeCommunity())
        XCTAssertEqual(active.title, "Uganda")
        XCTAssertTrue(active.available)
        // Returning opens it directly.
        XCTAssertEqual(
            CommunityReturnOutcome.decide(active: active, all: all),
            .openCommunity(namespaceID: active.namespaceId)
        )

        // Re-selecting the active community (a cached switch) is idempotent.
        let switched = try repository.switchToCommunity(namespaceID: active.namespaceId)
        XCTAssertEqual(switched.namespaceId, active.namespaceId)
    }

    @MainActor
    func testTheChooserPresentationTogglesWithoutChangingCommunities() throws {
        let directory = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let model = RiotAppModel()
        model.bootstrap(storageDirectory: directory, keyStore: TestWrappingKeyStore(), starterPacks: [])
        model.createSpace(title: "Germany")

        XCTAssertFalse(model.isCommunityChooserPresented)
        model.openCommunityChooser()
        XCTAssertTrue(model.isCommunityChooserPresented)
        model.dismissCommunityChooser()
        XCTAssertFalse(model.isCommunityChooserPresented)
        // Still the same community — opening the chooser changes nothing.
        XCTAssertEqual(model.community?.name, "Germany")
    }

    // MARK: - Performance: a cached community switch is under 300 ms (sim-relative)

    /// The <300 ms cached-switch gate (nav design), measured with `measure` on the
    /// iPhone 17 Pro simulator, OS 26.2 — sim-relative; physical-device timing is
    /// assumed-not-proven (§8.3 honesty rule). "Cached" means both communities are
    /// already held in the registry, so a switch is an author unseal + reproject,
    /// not a fresh join or sync. Uses the raw FFI profile to hold two communities
    /// (a member community via the core multi-community join).
    @MainActor
    func testACachedCommunitySwitchIsUnder300msSimRelative() throws {
        let dir = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: dir) }
        let key = Data(repeating: 0x42, count: 32)
        let profile = try openLocalProfileWithDatabase(
            dbPath: dir.appendingPathComponent("riot.db").path
        )
        let a = try profile.createPublicSpace(title: "Community A")
        // A second namespace, minted by a throwaway profile, joined as a member.
        let other = try openLocalProfile()
        let b = try other.createPublicSpace(title: "Community B")
        _ = try profile.joinPublicSpace(space: b)
        try profile.persistCommunities(wrappingKey: key)

        // Both communities are now cached in the registry. Measure a round-trip
        // switch and assert the per-switch time is under the 300 ms gate.
        let iterations = 10
        let start = Date()
        for _ in 0..<iterations {
            _ = try profile.switchCommunity(namespaceId: a.namespaceId, wrappingKey: key)
            _ = try profile.switchCommunity(namespaceId: b.namespaceId, wrappingKey: key)
        }
        let perSwitch = Date().timeIntervalSince(start) / Double(iterations * 2)
        XCTAssertLessThan(
            perSwitch, 0.300,
            "a cached community switch must be under 300 ms (sim-relative); was \(perSwitch)s"
        )

        // Also record the standard XCTClockMetric baseline, matching 2A's harness.
        measure(metrics: [XCTClockMetric()]) {
            _ = try? profile.switchCommunity(namespaceId: a.namespaceId, wrappingKey: key)
            _ = try? profile.switchCommunity(namespaceId: b.namespaceId, wrappingKey: key)
        }
    }

    // MARK: - Newswire descriptor survives a relaunch/switch

    /// Regression: `newswireDescriptorEntryID` was only ever captured inside
    /// `createCommunity`, so a community reached after a relaunch, a switch, or a
    /// join had a nil descriptor. Home then projected an empty descriptor id and
    /// degraded to the "updates unavailable" state — the newswire was effectively
    /// dead for every community except one just created in the current session.
    /// `reload()` (run by both `bootstrap` on restore and `switchCommunity`) must
    /// re-derive it from the persisted `CommunityRow.descriptorEntryId`.
    @MainActor
    func testARestoredCommunityReDerivesItsNewswireDescriptorSoHomeProjectsTheWire() throws {
        let directory = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let keyStore = TestWrappingKeyStore()

        // Session 1: found a community. This signs an immutable newswire
        // SpaceDescriptorV1 and captures its entry id.
        let founding = RiotAppModel()
        founding.bootstrap(storageDirectory: directory, keyStore: keyStore, starterPacks: [])
        founding.createCommunity(CommunityCreationRequest(name: "Fire Watch"))
        let signed = try XCTUnwrap(
            founding.community?.newswireDescriptorEntryID,
            "create signs and captures the descriptor"
        )
        XCTAssertFalse(signed.isEmpty)
        let namespaceID = try XCTUnwrap(founding.community?.namespaceID)

        // Session 2: a fresh model opens the SAME profile — a relaunch. It never
        // calls createCommunity, so opening the community must re-derive the
        // descriptor from the persisted CommunityRow instead of leaving it nil.
        // (A single-community device has an empty chooser list by design, so the
        // community is reached via the active community / an explicit switch.)
        let relaunched = RiotAppModel()
        relaunched.bootstrap(storageDirectory: directory, keyStore: keyStore, starterPacks: [])
        let autoOpened = relaunched.community != nil
        if relaunched.community == nil {
            relaunched.switchCommunity(namespaceID: namespaceID)
        }
        let rows = try relaunched.profileRepository?.listCommunities() ?? []
        let rowDesc = rows.first { $0.namespaceId == namespaceID }?.descriptorEntryId
        XCTAssertEqual(
            relaunched.community?.newswireDescriptorEntryID, signed,
            "DIAG autoOpened=\(autoOpened) rows=\(rows.count) rowDescriptor=\(rowDesc ?? "nil") activeNs=\(relaunched.community?.namespaceID ?? "nil")"
        )
    }

    // MARK: - Helpers

    private static func temporaryProfileDirectory() throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("riot-chooser-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }
}

private final class TestWrappingKeyStore: WrappingKeyStore {
    private var key = Data(repeating: 0x42, count: 32)
    func loadOrCreateWrappingKey() throws -> Data { key }
}
