import XCTest

@testable import RiotKit

/// Journeys over the REAL substrate a shipped app runs on.
///
/// WHY THIS FILE EXISTS. On 2026-07-31 the 0.1.2 build shipped with every
/// interactive feature broken — reactions and replies both refused — while
/// 303 riot-ffi tests, 49 Swift test files, and five XCUITest suites were
/// green. They were green because each one replaces the parts that break:
///
///   * `open_local_profile()` (Rust tests) is IN-MEMORY, not SQLite.
///   * `RiotProfileRepository.open(...)` in the existing Swift tests passes no
///     `databasePath`, so its core is IN-MEMORY too — including the test named
///     "trust survives reopen".
///   * `ReactionUITestFixture` documents itself as "a bounded IN-MEMORY wire"
///     with a "stable IN-MEMORY wrapping key".
///
/// So the durable database, the persisted profile, and the reopen path — the
/// three things that actually failed — had no coverage at any level.
///
/// Every test here therefore uses a real on-disk SQLite database, writes
/// through the same repository calls the UI makes, and REOPENS from disk to
/// assert what survived. A journey that never reopens proves nothing about
/// persistence.
final class DurableJourneyTests: XCTestCase {
    /// One temp directory per test, holding both the profile snapshot and the
    /// SQLite database, so a reopen sees exactly what a relaunch would.
    private struct Workspace {
        let directory: URL
        let storage: ProtectedProfileStorage
        let databasePath: String
        let keyStore: WrappingKeyStore
    }

    private var created: [URL] = []

    override func tearDownWithError() throws {
        for url in created {
            try? FileManager.default.removeItem(at: url)
        }
        created = []
    }

    private func makeWorkspace(_ label: String) throws -> Workspace {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("journey-\(label)-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        created.append(directory)
        return Workspace(
            directory: directory,
            storage: try ProtectedProfileStorage(
                fileURL: directory.appendingPathComponent("riot-profile.json")
            ),
            databasePath: directory.appendingPathComponent("riot.db").path,
            keyStore: StableTestWrappingKeyStore()
        )
    }

    /// Opens the repository the way the app does at launch: persisted snapshot,
    /// wrapping key, and a DURABLE database.
    private func open(_ workspace: Workspace) throws -> RiotProfileRepository {
        try RiotProfileRepository.open(
            storage: workspace.storage,
            keyStore: workspace.keyStore,
            databasePath: workspace.databasePath
        )
    }

    // MARK: - Journey 1: anything written survives a relaunch

    /// The relay registry is part of the same durable profile the app opens at
    /// launch. The built-in relay is only a first-run seed; a runtime relay must
    /// take precedence, survive the reopen, and be the input to the registered
    /// pull path without reconstructing a NodeId or ticket in Swift.
    func testARuntimeRelaySurvivesAProfileReopenAndDrivesTheNextPull() throws {
        let workspace = try makeWorkspace("relay-survives")
        let customRelay = RelayRecord(
            nodeId: String(repeating: "11", count: 32),
            ticketBytes: Data([0xa1, 0xb2, 0xc3]),
            lastAnsweredUnixSeconds: nil
        )

        do {
            let first = try open(workspace)
            let seeded = try XCTUnwrap(first.durableProfile.relayForNextPull())
            XCTAssertEqual(seeded.nodeId, AnchorRelayDefaults.relayNodeId)
            XCTAssertEqual(seeded.ticketBytes, AnchorRelayDefaults.communityTicket)
            try first.durableProfile.addRelay(relay: customRelay)
        }

        let second = try open(workspace)
        let relays = try second.durableProfile.listRelays()
        XCTAssertEqual(relays.first, customRelay)
        XCTAssertEqual(relays.count, 2, "the seeded relay remains available behind the runtime relay")
        XCTAssertEqual(try second.durableProfile.relayForNextPull(), customRelay)

        let runtime = try bindNetRuntime()
        XCTAssertThrowsError(
            try runtime.syncWithNextRelay(profile: second.durableProfile, nowUnix: 1)
        ) { error in
            XCTAssertEqual(error as? AnchorSyncError, .TicketMalformed)
        }
    }

    /// The most basic promise the app makes, and it was never tested end to end
    /// against a real database: post an update, close the app, open it again,
    /// and the update is still there.
    func testAPostSurvivesReopeningTheProfileFromDisk() throws {
        let workspace = try makeWorkspace("post-survives")

        let community: NewswireSignedRecord
        let post: NewswireSignedRecord
        do {
            let first = try open(workspace)
            community = try first.createNewswireCommunity(
                name: "River City Wire",
                summary: "Community newswire.",
                editorialRoster: []
            )
            post = try first.createNewswirePost(
                spaceDescriptorEntryID: community.entryId,
                headline: "Free breakfast at the corner church",
                body: "Every morning this week.",
                language: "en"
            )
            try first.persistCommunities()
        }

        // A relaunch: brand-new repository over the same directory.
        let second = try open(workspace)
        XCTAssertNil(
            second.recovery,
            "reopening a profile this build just wrote must not trigger recovery"
        )
        let projection = try second.projectNewswire(spaceDescriptorEntryID: community.entryId)
        XCTAssertTrue(
            projection.openWire.contains { $0.entryId == post.entryId },
            "a post written before the reopen must still be on the wire after it"
        )
    }

    // MARK: - Journey 2: reply, then prove it is still there after a reopen

    /// The failure a person reported: "That reply was not accepted." Asserts the
    /// whole path — write the reply, project it under its parent, reopen from
    /// disk, and find it still attached.
    func testAReplySurvivesReopeningTheProfileFromDisk() throws {
        let workspace = try makeWorkspace("reply-survives")

        let community: NewswireSignedRecord
        let post: NewswireSignedRecord
        let reply: NewswireSignedRecord
        do {
            let first = try open(workspace)
            community = try first.createNewswireCommunity(
                name: "River City Wire",
                summary: "Community newswire.",
                editorialRoster: []
            )
            post = try first.createNewswirePost(
                spaceDescriptorEntryID: community.entryId,
                headline: "Rent strike meeting Thursday",
                body: "7pm at the hall.",
                language: "en"
            )
            reply = try first.createNewswireComment(
                spaceDescriptorEntryID: community.entryId,
                parentEntryID: post.entryId,
                body: "I can bring chairs.",
                language: "en"
            )
            try first.persistCommunities()
        }

        let second = try open(workspace)
        XCTAssertNil(second.recovery, "a reply must not cost the profile its next open")
        let projection = try second.projectNewswire(spaceDescriptorEntryID: community.entryId)
        XCTAssertTrue(
            projection.openWire.contains { $0.entryId == post.entryId },
            "the replied-to post must survive the reopen"
        )
        XCTAssertTrue(
            projection.comments.contains {
                $0.entryId == reply.entryId && $0.parentEntryId == post.entryId
            },
            "a reply written before the reopen must still hang under its parent after it"
        )
    }

    // MARK: - Journey 3: react, then prove the tally survives a reopen

    /// The other reported failure: "Reactions aren't available for this post."
    func testAReactionSurvivesReopeningTheProfileFromDisk() throws {
        let workspace = try makeWorkspace("reaction-survives")

        let community: NewswireSignedRecord
        let post: NewswireSignedRecord
        do {
            let first = try open(workspace)
            community = try first.createNewswireCommunity(
                name: "River City Wire",
                summary: "Community newswire.",
                editorialRoster: []
            )
            post = try first.createNewswirePost(
                spaceDescriptorEntryID: community.entryId,
                headline: "Bike lane reopens on 5th",
                body: "As of this morning.",
                language: "en"
            )
            _ = try first.toggleNewswireReaction(
                spaceDescriptorEntryID: community.entryId,
                parentEntryID: post.entryId,
                kind: "solidarity",
                active: true
            )
            try first.persistCommunities()
        }

        let second = try open(workspace)
        XCTAssertNil(second.recovery, "a reaction must not cost the profile its next open")
        let projection = try second.projectNewswire(spaceDescriptorEntryID: community.entryId)
        let reacted = projection.openWire.first { $0.entryId == post.entryId }
        XCTAssertNotNil(reacted, "the reacted-to post must survive the reopen")
        XCTAssertTrue(
            (reacted?.reactions.contains { $0.count > 0 }) ?? false,
            "a reaction written before the reopen must still be tallied after it"
        )
    }

    // MARK: - Journey 4: an upgrade must not discard the profile

    /// THE BUG THAT IS ACTIVELY DESTROYING DATA. `quarantine/recovery.log` on a
    /// real machine shows `profile-open / InvalidInput` on 25 July (app 0.1.1)
    /// and again on 31 July (app 0.1.2) — each one moving the person's identity
    /// and community aside and minting a NEW author, which is why their writes
    /// were then refused in a community they thought they had joined.
    ///
    /// A profile this build wrote must reopen cleanly. That is the weakest form
    /// of the guarantee and it is the one to hold first; a fixture written by an
    /// actual older release belongs here too, once one is checked in.
    func testReopeningAProfileNeverQuarantinesItOrChangesIdentity() throws {
        let workspace = try makeWorkspace("upgrade")

        let identityBefore: String
        do {
            let first = try open(workspace)
            _ = try first.createNewswireCommunity(
                name: "River City Wire",
                summary: "Community newswire.",
                editorialRoster: []
            )
            try first.persistCommunities()
            identityBefore = try first.me().id
        }

        let second = try open(workspace)
        XCTAssertNil(
            second.recovery,
            """
            reopening a profile must not quarantine it. When this fires, the \
            person silently loses their identity and community and every write \
            into that community is refused afterwards.
            """
        )
        XCTAssertEqual(
            try second.me().id,
            identityBefore,
            "the author identity must be the same person across a reopen"
        )
        XCTAssertFalse(
            try second.listCommunities().isEmpty,
            "a community held before the reopen must still be held after it"
        )
    }
}

/// A wrapping key that is stable for the lifetime of one test workspace, so a
/// reopen unseals the identity the first open sealed — exactly what the real
/// keychain is supposed to provide, and what a fresh key would break.
private final class StableTestWrappingKeyStore: WrappingKeyStore {
    private var key: Data?

    func loadOrCreateWrappingKey() throws -> Data {
        if let key { return key }
        let created = Data(repeating: 0x5a, count: 32)
        key = created
        return created
    }
}
