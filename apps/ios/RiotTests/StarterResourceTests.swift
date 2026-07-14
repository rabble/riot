import XCTest
@testable import RiotKit

/// Unit 0A contract: the Tools surface ships the whole eight-tool starter
/// catalog, not just Checklist, and a missing built-in fails loudly instead of
/// silently shrinking the list.
///
/// `RiotTests` is a hostless logic-test bundle (`TEST_HOST = ""`), so the
/// `RiotKit` test scheme does not build `Riot.app` and there is no app bundle to
/// inspect at test time. The `.cbor` artifacts are therefore registered in this
/// test bundle's own Resources phase (mirroring the same `PBXFileReference`s that
/// back the two Apple app targets), and this suite inspects
/// `Bundle(for:)`. The app targets carry the same pairs so the shipping products
/// bundle all eight; that registration is verified by the app build succeeding.
@MainActor
final class StarterResourceTests: XCTestCase {
    /// The canonical eight slugs, in `STARTER_CATALOG` order. The Rust
    /// `apps_starter.rs` demo-order test pins the matching display names.
    private static let expectedCatalog = [
        "checklist", "supply-board", "roll-call", "quick-poll",
        "chat", "dispatches", "wiki", "photo-wall",
    ]

    func testCatalogIsExactlyTheEightSlugsInDemoOrder() {
        XCTAssertEqual(RiotAppModel.starterCatalog, Self.expectedCatalog)
    }

    /// RED until every pair is registered in the RiotTests Resources phase.
    func testAllEightStarterPairsAreBundled() {
        let bundle = Bundle(for: Self.self)
        for slug in RiotAppModel.starterCatalog {
            XCTAssertNotNil(
                bundle.url(forResource: "\(slug).manifest", withExtension: "cbor"),
                "\(slug).manifest.cbor is not bundled into the test resources"
            )
            XCTAssertNotNil(
                bundle.url(forResource: "\(slug).bundle", withExtension: "cbor"),
                "\(slug).bundle.cbor is not bundled into the test resources"
            )
        }
    }

    /// The loader returns all eight pairs from the bundled resources. It reads
    /// the same `.cbor` artifacts the shipping app bundles (here via the test
    /// bundle, since `RiotTests` is hostless and `Bundle.main` is the xctest
    /// runner, not the app).
    func testLoadStarterPacksReturnsAllEightPairsFromBundle() throws {
        let bundle = Bundle(for: Self.self)
        let packs = try RiotAppModel.loadStarterPacks(resolve: { name in
            guard let url = bundle.url(forResource: name, withExtension: "cbor") else { return nil }
            return try? Data(contentsOf: url)
        })
        XCTAssertEqual(packs.count, 8)
        for pack in packs {
            XCTAssertFalse(pack.manifest.isEmpty)
            XCTAssertFalse(pack.bundle.isEmpty)
        }
    }

    /// A wholly unreadable catalog throws on the first slug — never returns a
    /// short list (the deleted `.compactMap` behaviour).
    func testMissingCatalogThrowsLoudly() {
        XCTAssertThrowsError(try RiotAppModel.loadStarterPacks(resolve: { _ in nil })) { error in
            guard let error = error as? StarterCatalogError else {
                return XCTFail("expected StarterCatalogError, got \(error)")
            }
            XCTAssertEqual(error.slug, "checklist")
            XCTAssertEqual(error.pack, .manifest)
        }
    }

    /// A partially-present catalog (Checklist only) still throws — a single
    /// missing tool is fatal to the load, not silently dropped.
    func testPartialCatalogThrowsOnFirstMissingTool() {
        let resolve: (String) -> Data? = { name in
            name.hasPrefix("checklist.") ? Data([0x01]) : nil
        }
        XCTAssertThrowsError(try RiotAppModel.loadStarterPacks(resolve: resolve)) { error in
            guard let error = error as? StarterCatalogError else {
                return XCTFail("expected StarterCatalogError, got \(error)")
            }
            XCTAssertEqual(error.slug, "supply-board")
        }
    }

    /// §4.7 recovery contract: a catalog failure surfaces a fixed error code and
    /// technical details, leaves the profile closed, and never opens with a
    /// quietly short Tools surface.
    func testCatalogFailureSurfacesRecoveryStateAndNeverOpensProfile() {
        let model = RiotAppModel()
        model.bootstrap(
            storageDirectory: FileManager.default.temporaryDirectory
                .appendingPathComponent("catalog-fail-\(UUID().uuidString)"),
            keyStore: FixedWrappingKeyStore(),
            starterPackResolver: { _ in nil }
        )

        XCTAssertEqual(model.starterCatalogFailure?.code, StarterCatalogFailure.catalogUnavailableCode)
        XCTAssertFalse(model.starterCatalogFailure?.technicalDetails.isEmpty ?? true)
        XCTAssertFalse(model.isProfileOpen)
        XCTAssertTrue(model.apps.isEmpty)
    }

    /// Retry with the catalog still broken stays failed and still never opens —
    /// the recovery action is safe and idempotent.
    func testRetryWithBrokenCatalogStaysFailed() {
        let model = RiotAppModel()
        model.bootstrap(
            storageDirectory: FileManager.default.temporaryDirectory
                .appendingPathComponent("catalog-retry-\(UUID().uuidString)"),
            keyStore: FixedWrappingKeyStore(),
            starterPackResolver: { _ in nil }
        )
        XCTAssertNotNil(model.starterCatalogFailure)

        model.retryStarterCatalog()

        XCTAssertEqual(model.starterCatalogFailure?.code, StarterCatalogFailure.catalogUnavailableCode)
        XCTAssertFalse(model.isProfileOpen)
    }
}

/// A fixed 32-byte wrapping key so sealed identities round-trip in tests
/// (duplicated per the project convention).
private final class FixedWrappingKeyStore: WrappingKeyStore {
    private var key: Data?
    func loadOrCreateWrappingKey() throws -> Data {
        if let key { return key }
        let created = Data(repeating: 0x5a, count: 32)
        key = created
        return created
    }
}
