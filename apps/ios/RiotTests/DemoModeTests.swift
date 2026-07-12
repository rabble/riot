import XCTest
@testable import RiotKit

/// Demo mode, end to end: the seeded bundle really is a resource in the built
/// bundle, and loading it really does surface the seeded space through Rust.
///
/// **The first test is the important one.** A shipping data file only reaches a
/// device if it is in a target's Resources build phase, and the Riot app target's
/// Debug configuration does not define `DEBUG` — so a `#if DEBUG` source-tree
/// fallback compiles away to nothing and the file is silently, invisibly absent
/// at runtime. Nothing catches that but a test that asks the *bundle* for the
/// resource by exactly the name and extension the loading code asks for.
final class DemoModeTests: XCTestCase {
    /// The bundle these tests are built into. Its Resources phase carries the
    /// same fixture the app target's does, so a resource-phase mistake fails
    /// here rather than on stage.
    private var testBundle: Bundle { Bundle(for: type(of: self)) }

    // MARK: - The resource is really there

    func testTheDemoFixtureIsPresentInTheBundleResources() throws {
        let url = try XCTUnwrap(
            testBundle.url(
                forResource: DemoFixture.resourceName,
                withExtension: DemoFixture.resourceExtension
            ),
            "demo-space.riot-evidence is missing from the Resources build phase"
        )
        let bytes = try Data(contentsOf: url)
        XCTAssertGreaterThan(bytes.count, 1_000, "the fixture is the real signed bundle")
    }

    func testDemoFixtureBytesLoadThroughTheSameAccessorTheViewUses() throws {
        let bytes = try XCTUnwrap(
            DemoFixture.bytes(in: testBundle),
            "DemoFixture must find the resource by the name the app asks for"
        )
        XCTAssertFalse(bytes.isEmpty)
    }

    func testDemoFixtureReturnsNilWhenTheResourceIsAbsent() {
        // A bundle with no such resource: the loader reports absence rather than
        // trapping, and the surface says the one sentence it is allowed to say.
        XCTAssertNil(DemoFixture.bytes(in: Bundle(for: XCTestCase.self)))
    }

    // MARK: - Loading it surfaces the seeded space

    func testLoadingTheDemoFixtureSurfacesTheSeededSpace() throws {
        let bytes = try XCTUnwrap(DemoFixture.bytes(in: testBundle))
        let profile = try openLocalProfile()
        let loader = DemoProfileLoader(profile: profile)

        let space = try loader.loadDemoSpace(bytes: bytes)

        XCTAssertEqual(space.title, "Riverside Tenants Union")
        XCTAssertEqual(space.namespaceID.count, 64)

        // The six seeded alerts are live, through the ordinary import — this is
        // Rust's board, not a fixture the test hand-built.
        let entries = try profile.listCurrentEntries()
        XCTAssertEqual(entries.count, 6)
        XCTAssertTrue(entries.allSatisfy { $0.namespaceId == space.namespaceID })

        // And the members read as people, not as `member-<hex>`.
        let names = try profile.profile().displayNames().map(\.rendered)
        for member in ["Ana", "Marcus", "Priya", "Dee"] {
            XCTAssertTrue(
                names.contains { $0.hasPrefix("\(member) · ") },
                "\(member) should resolve to a rendered display name; got \(names)"
            )
        }
    }

    func testHidingStopsListingTheDemoSpace() throws {
        let bytes = try XCTUnwrap(DemoFixture.bytes(in: testBundle))
        let profile = try openLocalProfile()
        let loader = DemoProfileLoader(profile: profile)

        _ = try loader.loadDemoSpace(bytes: bytes)
        XCTAssertEqual(try profile.listCurrentEntries().count, 6)

        try loader.hideDemoSpace()

        // No space lists the demo namespace any more, so nothing reaches its
        // entries. They are still in the store — Willow is append-only and hiding
        // is not deleting — but they are unreachable, which is what hiding means.
        XCTAssertThrowsError(try profile.listCurrentEntries())
    }

    func testACorruptBundleIsRefusedWithoutLoadingAnything() throws {
        var bytes = try XCTUnwrap(DemoFixture.bytes(in: testBundle))
        bytes[bytes.count / 2] ^= 0xff
        let profile = try openLocalProfile()
        let loader = DemoProfileLoader(profile: profile)

        XCTAssertThrowsError(try loader.loadDemoSpace(bytes: bytes))

        // Nothing half-imported: the import is transactional, so the profile is
        // exactly where it was.
        _ = try profile.createPublicSpace(title: "Fresh")
        XCTAssertTrue(try profile.listCurrentEntries().isEmpty)
    }
}
