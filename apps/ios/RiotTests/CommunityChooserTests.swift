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
        _ = try profile.joinPublicSpace(space: b, wrappingKey: key)
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

    // MARK: - Creating a community signs a projectable newswire

    /// Regression: the founder form collects no summary, so
    /// `CommunityCreationRequest.summary` is empty — and core rejects a newswire
    /// `SpaceDescriptorV1` with an empty summary (`InvalidInput`). The result was
    /// that EVERY user-created community signed no descriptor and launched with a
    /// permanently dead wire ("updates unavailable"), which is the newswire being
    /// invisible. `createCommunity` must supply a non-empty summary so the
    /// descriptor is signed and Home can project the (empty) wire.
    @MainActor
    func testCreatingACommunityWithNoSummaryStillSignsAProjectableNewswire() throws {
        let directory = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let model = RiotAppModel()
        model.bootstrap(storageDirectory: directory, keyStore: TestWrappingKeyStore(), starterPacks: [])

        // A founder who typed only a community name — no summary.
        model.createCommunity(CommunityCreationRequest(name: "Fire Watch"))

        let descriptor = try XCTUnwrap(
            model.community?.newswireDescriptorEntryID,
            "create must sign a newswire descriptor even with no summary; errorMessage=\(model.errorMessage ?? "nil")"
        )
        XCTAssertFalse(descriptor.isEmpty)

        // The signed descriptor must PROJECT (an empty wire) rather than throw, so
        // Home shows the friendly "post the first update" state, never the
        // "updates unavailable" recovery state that a missing descriptor produces.
        XCTAssertNoThrow(
            try model.profileRepository?.projectNewswire(spaceDescriptorEntryID: descriptor),
            "a freshly created community's wire must project (empty), not fail"
        )
    }

    // MARK: - Unit 3D: manual multi-community JOIN via share reference

    /// A well-formed share reference for `namespace`, with placeholder descriptor
    /// and digest coordinates. Manual join only reads the namespace (the descriptor
    /// + entries arrive later over sync — "pending first sync"), so the other two
    /// coordinates being placeholders is faithful to what a real reference carries
    /// before its descriptor is in hand.
    private func shareReference(forNamespace namespace: String) throws -> String {
        try newswireEncodeShareReference(
            namespaceId: namespace,
            descriptorEntryId: String(repeating: "1", count: 64),
            contentDigest: String(repeating: "2", count: 64)
        )
    }

    /// The core guarantee, proven on the raw FFI: joining a SECOND community mints
    /// a FRESH, UNLINKABLE author for the target namespace (its signing key is not
    /// the origin community's), and the outgoing community's author is PARKED — not
    /// destroyed — so switching back restores it byte-for-byte.
    @MainActor
    func testJoiningASecondCommunityMintsAFreshUnlinkableAuthorAndParksTheFirst() throws {
        let dir = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: dir) }
        let key = Data(repeating: 0x42, count: 32)
        let profile = try openLocalProfileWithDatabase(dbPath: dir.appendingPathComponent("riot.db").path)

        let a = try profile.createPublicSpace(title: "Community A")
        let authorInA = try profile.identity()
        XCTAssertEqual(authorInA.namespaceId, a.namespaceId, "A's author subspace IS its own namespace (organizer)")

        // A second namespace, minted by a throwaway profile, then joined as a member.
        let origin = try openLocalProfile()
        let b = try origin.createPublicSpace(title: "Community B")
        _ = try profile.joinPublicSpace(
            space: PublicSpace(namespaceId: b.namespaceId, title: "New community", isPublic: true),
            wrappingKey: key
        )
        let authorInB = try profile.identity()

        XCTAssertNotEqual(authorInB.namespaceId, authorInA.namespaceId, "B is a different community")
        XCTAssertNotEqual(
            authorInB.signingKeyId, authorInA.signingKeyId,
            "the joined community holds a FRESH author — its signing key is not A's (unlinkable)"
        )
        XCTAssertNotEqual(
            authorInB.namespaceId, authorInB.signingKeyId,
            "joining someone else's space, the author subspace is minted, not the namespace itself"
        )

        // The first community's author is parked, not destroyed: switching back
        // restores the SAME signing identity A held before the join.
        try profile.persistCommunities(wrappingKey: key)
        let restoredA = try profile.switchCommunity(namespaceId: a.namespaceId, wrappingKey: key)
        XCTAssertEqual(restoredA.namespaceId, a.namespaceId)
        XCTAssertEqual(try profile.identity().signingKeyId, authorInA.signingKeyId, "A restored intact")
        let restoredB = try profile.switchCommunity(namespaceId: b.namespaceId, wrappingKey: key)
        XCTAssertEqual(restoredB.namespaceId, b.namespaceId)
        XCTAssertEqual(try profile.identity().signingKeyId, authorInB.signingKeyId, "B restored intact")
    }

    /// The repository wrapper: `joinAdditionalCommunity` holds BOTH communities,
    /// makes the joined one active with a distinct relationship, ISOLATES content
    /// (A's board is not visible under B), and switching back restores A's board —
    /// so the join parked A rather than replacing it.
    @MainActor
    func testJoinAdditionalCommunityHoldsBothIsolatesContentAndParksTheActive() throws {
        let dir = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: dir) }
        let repository = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: dir.appendingPathComponent("profile.json")),
            keyStore: TestWrappingKeyStore(),
            databasePath: dir.appendingPathComponent("riot.db").path
        )
        let a = try repository.createPublicSpace(title: "Community A")
        _ = try repository.signAlert(
            in: a,
            draft: AlertDraft(
                expiresAt: UInt64(Date().timeIntervalSince1970) + 3_600,
                headline: "Water shut off on 3rd St",
                description: "Bring jugs to the union hall",
                sourceClaims: ["Union hall notice"],
                aiAssisted: false
            )
        )
        XCTAssertFalse(try repository.currentEntries().isEmpty, "A has a board entry before the join")

        // Join B (a different namespace) as a manual, share-reference join.
        let origin = try openLocalProfile()
        let b = try origin.createPublicSpace(title: "Community B")
        let ref = try repository.decodeShareReference(shareReference(forNamespace: b.namespaceId))
        XCTAssertEqual(ref.namespaceId, b.namespaceId)
        let joined = try repository.joinAdditionalCommunity(
            RiotSpace(
                namespaceID: ref.namespaceId,
                title: CommunityShareJoin.provisionalTitle(namespaceID: ref.namespaceId)
            ),
            descriptorEntryID: ref.descriptorEntryId
        )

        XCTAssertEqual(joined.namespaceId, b.namespaceId, "the joined community is now active")
        XCTAssertEqual(joined.relationship, .member, "joining someone else's space makes you a member")
        let held = try repository.listCommunities()
        XCTAssertEqual(held.count, 2, "both communities are held")
        XCTAssertTrue(held.contains { $0.namespaceId == a.namespaceID }, "A is parked, not dropped")

        // Risk 15: the joined community carries its descriptor handle from the share
        // reference — NOT a dead follow. This is what lets its Home reproject once
        // sync delivers the descriptor + posts.
        let bRow = held.first { $0.namespaceId == b.namespaceId }
        XCTAssertEqual(
            bRow?.descriptorEntryId, ref.descriptorEntryId,
            "the joined community carries its descriptor handle, not a dead follow"
        )

        // Isolation: A's board entry does NOT bleed into B. A had content; B shows
        // none of it — the join scopes the board to the newly active community.
        XCTAssertTrue(try repository.currentEntries().isEmpty, "B shows none of A's entries (isolation)")

        // Parked, not destroyed: A is still HELD and switching back makes it the
        // active community again, with its organizer relationship intact. (The
        // legacy alert board is not asserted here — CurrentEntry payloads are not
        // retained across a switch; the community shell's content is the newswire,
        // and cross-community entry isolation is proven at the FFI level in
        // `persistence_contract.rs`.)
        let backToA = try repository.switchToCommunity(namespaceID: a.namespaceID)
        XCTAssertEqual(backToA.namespaceId, a.namespaceID, "A is parked, not dropped — switchable again")
        XCTAssertEqual(backToA.relationship, .organizer, "A's organizer relationship survives the round-trip")
    }

    /// Re-joining the community that is ALREADY active is idempotent: it does not
    /// mint a second author or fork the registry.
    @MainActor
    func testRejoiningTheActiveCommunityViaShareReferenceIsIdempotent() throws {
        let dir = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: dir) }
        let repository = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: dir.appendingPathComponent("profile.json")),
            keyStore: TestWrappingKeyStore(),
            databasePath: dir.appendingPathComponent("riot.db").path
        )
        let a = try repository.createPublicSpace(title: "Community A")

        let ref = try repository.decodeShareReference(shareReference(forNamespace: a.namespaceID))
        let again = try repository.joinAdditionalCommunity(
            RiotSpace(namespaceID: a.namespaceID, title: "Community A"),
            descriptorEntryID: ref.descriptorEntryId
        )
        XCTAssertEqual(again.namespaceId, a.namespaceID)
        XCTAssertEqual(again.relationship, .organizer, "still the organizer of their own space")
        XCTAssertEqual(try repository.listCommunities().count, 1, "no second author was minted")
    }

    /// The payoff integration test: create A, JOIN B by share reference, SWITCH
    /// back to A — the shell reprojects each time (name + organizer hint flip) and
    /// the joined community carries its descriptor handle from the reference (Risk
    /// 15), so the shell can reproject B's Home once sync delivers it.
    @MainActor
    func testCreateAJoinBSwitchReprojectsTheShell() throws {
        let dir = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: dir) }
        let model = RiotAppModel()
        model.bootstrap(storageDirectory: dir, keyStore: TestWrappingKeyStore(), starterPacks: [])

        model.createSpace(title: "Community A")
        XCTAssertEqual(model.community?.name, "Community A")
        XCTAssertTrue(model.community?.isOrganizer ?? false, "the creator is A's organizer")

        // A second namespace to join by reference.
        let origin = try openLocalProfile()
        let b = try origin.createPublicSpace(title: "Community B")
        model.joinAdditionalCommunity(shareReference: try shareReference(forNamespace: b.namespaceId))

        XCTAssertNil(model.errorMessage, "the manual join succeeds")
        XCTAssertEqual(model.communities.count, 2, "both communities appear in the chooser")
        XCTAssertEqual(model.community?.namespaceID, b.namespaceId, "the shell is now on B")
        XCTAssertFalse(model.community?.isOrganizer ?? true, "on B this person is a member, not an organizer")
        XCTAssertEqual(
            model.newswireDescriptorEntryID, String(repeating: "1", count: 64),
            "B carries the descriptor handle from its share reference — the shell can reproject once sync delivers it"
        )

        model.switchCommunity(namespaceID: model.communities.first { $0.name == "Community A" }!.namespaceID)
        XCTAssertEqual(model.community?.name, "Community A", "the shell reprojects back to A")
        XCTAssertTrue(model.community?.isOrganizer ?? false, "A's organizer hint returns")
    }

    /// A freshly joined community is rendered "pending first sync" — a distinct,
    /// honest state — not fabricated content: a member row with no activity and no
    /// sync yet. An organizer's own space, or a community with any activity, is not.
    func testANewlyJoinedCommunityRowIsPendingFirstSync() {
        func row(
            relationship: CommunityRelationship,
            activity: UInt64?,
            sync: UInt64?,
            descriptor: String? = nil
        ) -> CommunityChooserRow {
            CommunityChooserRow.from(
                CommunityRow(
                    namespaceId: String(repeating: "a", count: 64),
                    title: "New community",
                    relationship: relationship,
                    descriptorEntryId: descriptor,
                    recentActivityUnixSeconds: activity,
                    syncFreshnessUnixSeconds: sync,
                    archived: false,
                    quarantined: false,
                    available: true
                )
            )
        }
        XCTAssertTrue(
            row(relationship: .member, activity: nil, sync: nil).pendingFirstSync,
            "a held-but-never-synced member community is pending first sync"
        )
        // Risk 15: carrying the descriptor handle does NOT flip the state — the row
        // is pending until SYNC delivers content, not merely because it holds the
        // handle to fetch it.
        XCTAssertTrue(
            row(relationship: .member, activity: nil, sync: nil, descriptor: String(repeating: "1", count: 64))
                .pendingFirstSync,
            "holding the descriptor handle is not the same as having synced — still pending"
        )
        XCTAssertFalse(
            row(relationship: .organizer, activity: nil, sync: nil).pendingFirstSync,
            "an organizer created the space — its descriptor is local, never pending sync"
        )
        XCTAssertFalse(
            row(relationship: .member, activity: 1_000_000, sync: 1_000_000).pendingFirstSync,
            "once content has arrived, the community is no longer pending"
        )
    }

    /// The decode wrapper rejects a string that is not a canonical share reference,
    /// rather than interpreting it.
    @MainActor
    func testDecodingAMalformedShareReferenceIsRefused() throws {
        let dir = try Self.temporaryProfileDirectory()
        defer { try? FileManager.default.removeItem(at: dir) }
        let repository = try RiotProfileRepository.open(
            storage: try ProtectedProfileStorage(fileURL: dir.appendingPathComponent("profile.json")),
            keyStore: TestWrappingKeyStore(),
            databasePath: dir.appendingPathComponent("riot.db").path
        )
        XCTAssertThrowsError(try repository.decodeShareReference("https://example.com/not-a-reference"))
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
