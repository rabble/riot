import XCTest
@testable import RiotKit

/// Discover front door — the #1 missing user goal: find a community you don't
/// already have a link for.
///
/// `CommunityDirectory` is the seam the Discover surface reads through. The
/// filtering (search + category) and the seed source are pure and provable here
/// without any FFI, exactly like `JoinReferenceModel`. The live signed-directory
/// feed is the follow-up; today the source is a clearly-marked seed.
@MainActor
final class CommunityDirectoryTests: XCTestCase {
    // MARK: - Seed source

    /// The seed source returns a handful of realistic activist communities,
    /// mirroring the prototype (River City Wire, Eastside Tenant Union, Harbor
    /// Mutual Aid, …), and every one is flagged as a seed so no fake data is ever
    /// presented as a real live community.
    func testSeedSourceReturnsRealisticCommunitiesAllMarkedSeed() {
        let source = SeededCommunityDirectory()
        let all = source.discoverableCommunities()
        XCTAssertGreaterThanOrEqual(all.count, 5, "the seed mirrors the prototype's set")
        XCTAssertTrue(all.allSatisfy(\.isSeed), "no seed row may masquerade as a live community")
        XCTAssertTrue(all.contains { $0.name == "River City Wire" })
        XCTAssertTrue(all.contains { $0.name == "Eastside Tenant Union" })
        XCTAssertTrue(all.contains { $0.name == "Harbor Mutual Aid" })
        // Every row leads with plain language a person can act on.
        XCTAssertTrue(all.allSatisfy { !$0.about.isEmpty && !$0.stewardName.isEmpty })
    }

    /// Both membership shapes exist in the seed, so the surface can show the
    /// open-vs-invite tag honestly.
    func testSeedCoversOpenAndInvite() {
        let all = SeededCommunityDirectory().discoverableCommunities()
        XCTAssertTrue(all.contains { $0.isOpen })
        XCTAssertTrue(all.contains { !$0.isOpen })
    }

    // MARK: - Search filtering (pure)

    func testSearchMatchesNameCaseInsensitively() {
        let all = SeededCommunityDirectory().discoverableCommunities()
        let hits = CommunityDiscoveryModel.filter(all, search: "eastside", category: nil)
        XCTAssertEqual(hits.count, 1)
        XCTAssertEqual(hits.first?.name, "Eastside Tenant Union")
    }

    func testSearchMatchesAboutAndSteward() {
        let all = SeededCommunityDirectory().discoverableCommunities()
        // "eviction" appears in a description, not a name.
        XCTAssertFalse(CommunityDiscoveryModel.filter(all, search: "eviction", category: nil).isEmpty)
        // A steward's name is searchable too.
        let steward = all.first!.stewardName
        XCTAssertFalse(CommunityDiscoveryModel.filter(all, search: steward, category: nil).isEmpty)
    }

    func testBlankSearchReturnsEverything() {
        let all = SeededCommunityDirectory().discoverableCommunities()
        XCTAssertEqual(CommunityDiscoveryModel.filter(all, search: "   ", category: nil).count, all.count)
        XCTAssertEqual(CommunityDiscoveryModel.filter(all, search: "", category: nil).count, all.count)
    }

    func testSearchWithNoMatchesReturnsEmpty() {
        let all = SeededCommunityDirectory().discoverableCommunities()
        XCTAssertTrue(CommunityDiscoveryModel.filter(all, search: "zzzznomatch", category: nil).isEmpty)
    }

    // MARK: - Category filtering (pure)

    func testCategoryFilterRestrictsToThatCategory() {
        let all = SeededCommunityDirectory().discoverableCommunities()
        for category in DiscoverCategory.allCases {
            let hits = CommunityDiscoveryModel.filter(all, search: "", category: category)
            XCTAssertTrue(hits.allSatisfy { $0.category == category },
                          "\(category) filter must only return that category")
        }
    }

    func testSearchAndCategoryCompose() {
        let all = SeededCommunityDirectory().discoverableCommunities()
        // A category that has a member, intersected with a search that also matches it.
        guard let sample = all.first else { return XCTFail("seed empty") }
        let hits = CommunityDiscoveryModel.filter(
            all, search: sample.name, category: sample.category)
        XCTAssertTrue(hits.contains { $0.id == sample.id })
        // The same search under a different category must not return it.
        let otherCategory = DiscoverCategory.allCases.first { $0 != sample.category }!
        let miss = CommunityDiscoveryModel.filter(
            all, search: sample.name, category: otherCategory)
        XCTAssertFalse(miss.contains { $0.id == sample.id })
    }

    // MARK: - View model

    /// The model publishes filtered results and re-derives them when the search
    /// text or the selected category changes.
    func testModelPublishesFilteredResults() {
        let model = CommunityDiscoveryModel(source: SeededCommunityDirectory())
        model.refresh()
        let total = model.results.count
        XCTAssertGreaterThan(total, 0)

        model.searchText = "harbor"
        XCTAssertEqual(model.results.count, 1)
        XCTAssertEqual(model.results.first?.name, "Harbor Mutual Aid")

        model.searchText = ""
        XCTAssertEqual(model.results.count, total)

        model.selectedCategory = model.results.first?.category
        XCTAssertTrue(model.results.allSatisfy { $0.category == model.selectedCategory })

        // Toggling the category off restores the full set.
        model.selectedCategory = nil
        XCTAssertEqual(model.results.count, total)
    }

    /// Every seed row's category maps to a labelled, glyphed chip — the four the
    /// prototype names (Events / Help & recs / Info / Guides).
    func testEveryCategoryHasLabelAndGlyph() {
        XCTAssertEqual(DiscoverCategory.allCases.count, 4)
        for category in DiscoverCategory.allCases {
            XCTAssertFalse(category.label.isEmpty)
            XCTAssertFalse(category.systemImage.isEmpty)
        }
    }

    // MARK: - Join routing

    /// A seed carries no real join reference (it is not a live community), so the
    /// join routing falls back to the honest paste/QR path rather than fabricating
    /// a joinable coordinate. A row that DID carry a reference would route through
    /// the existing commit-join path — this proves the branch the seed takes.
    func testSeedJoinRoutesToPasteQRFallback() {
        let seed = SeededCommunityDirectory().discoverableCommunities().first!
        XCTAssertNil(seed.joinReference,
                     "a seed is not a live community; it must not carry a joinable coordinate")
        switch CommunityJoinRoute.route(for: seed) {
        case .pasteOrScan:
            break // expected
        case .commitReference:
            XCTFail("a seed with no reference must not claim a joinable coordinate")
        }
    }

    /// A discoverable community that DOES carry a reference (the shape a live feed
    /// will produce) routes into the existing commit-join flow.
    func testCommunityWithReferenceRoutesToCommitJoin() {
        let live = DiscoverableCommunity(
            id: "live-1",
            name: "Live Wire",
            about: "A real one from the feed",
            category: .info,
            stewardName: "Someone",
            peopleCount: 10,
            activityHint: "active",
            isOpen: true,
            whatsNew: [],
            isSeed: false,
            joinReference: "riot://newswire/join/v1/deadbeef"
        )
        switch CommunityJoinRoute.route(for: live) {
        case .commitReference(let ref):
            XCTAssertEqual(ref, "riot://newswire/join/v1/deadbeef")
        case .pasteOrScan:
            XCTFail("a community carrying a reference must route into commit-join")
        }
    }
}
