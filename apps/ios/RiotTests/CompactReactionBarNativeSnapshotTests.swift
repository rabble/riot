import SwiftUI
import XCTest
#if os(iOS)
import UIKit
#endif

@testable import RiotKit

@MainActor
final class ReactionFixtureSyncPumpTests: XCTestCase {
    func testEnvironmentRequiresTheExactFixtureAndAValidUUID() {
        let runID = UUID()
        XCTAssertEqual(ReactionUITestEnvironment.resolve([:]), .inactive)
        XCTAssertEqual(
            ReactionUITestEnvironment.resolve([
                "RIOT_UI_TEST_FIXTURE": "reactions-joined",
            ]),
            .invalid
        )
        XCTAssertEqual(
            ReactionUITestEnvironment.resolve([
                "RIOT_UI_TEST_FIXTURE": "reactions-joined",
                "RIOT_UI_TEST_RUN_ID": "not-a-uuid",
            ]),
            .invalid
        )
        XCTAssertEqual(
            ReactionUITestEnvironment.resolve([
                "RIOT_UI_TEST_FIXTURE": "reactions-joined",
                "RIOT_UI_TEST_RUN_ID": runID.uuidString,
                "RIOT_UI_TEST_REACTION_DELAY_MS": "750",
                "RIOT_UI_TEST_WINDOW_WIDTH": "900",
            ]),
            .valid(ReactionUITestConfiguration(
                runID: runID,
                reactionMode: .success,
                reactionDelayMilliseconds: 750,
                windowWidth: 900
            ))
        )
    }

    func testClosedFailureModesMapToTypedPublicCopyBeforeAnyWrite() {
        XCTAssertNil(ReactionUITestReactionMode.success.preCommitFailure)
        XCTAssertNil(ReactionUITestReactionMode.stall.preCommitFailure)
        XCTAssertEqual(
            ReactionUITestReactionMode.retryable.preCommitFailure,
            ReactionFailure(
                kind: .retryablePersistence,
                publicCode: "reaction_persistence",
                message: "Couldn’t save your reaction. Try again."
            )
        )
        XCTAssertEqual(
            ReactionUITestReactionMode.authority.preCommitFailure?.kind,
            .authorityOrInput
        )
        XCTAssertEqual(
            ReactionUITestReactionMode.capacity.preCommitFailure?.kind,
            .capacity
        )
        XCTAssertEqual(
            ReactionUITestReactionMode.clock.preCommitFailure?.kind,
            .clock
        )
    }

    func testRealJoinedReaderProjectsTheAuthorsPost() throws {
        let runID = UUID()
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("riot-reaction-pair-\(runID.uuidString)", isDirectory: true)
        addTeardownBlock {
            try? FileManager.default.removeItem(at: directory)
        }

        let pair = try RiotProfileRepository.makeReactionUITestPair(
            baseDirectory: directory,
            keyStore: UIAutomationWrappingKeyStore(runID: runID)
        )

        XCTAssertEqual(try pair.reader.activeCommunity()?.relationship, .member)
        let projection = try pair.reader.projectNewswire(
            spaceDescriptorEntryID: pair.descriptorEntryID
        )
        XCTAssertTrue(projection.openWire.contains { $0.entryId == pair.postEntryID })
    }

    func testOnlyAuthorBeginsAndAOneFrameExchangeTerminates() throws {
        let author = FixtureBoundary(
            beginOutcomes: [.sendMore(terminal: true)],
            outboundFrames: [Data("author".utf8)],
            receiveOutcomes: [.done]
        )
        let reader = FixtureBoundary(
            beginOutcomes: [.failed],
            outboundFrames: [],
            receiveOutcomes: [.done]
        )

        let transfers = try ReactionFixtureSyncPump().run(author: author, reader: reader)

        XCTAssertEqual(transfers, 1)
        XCTAssertEqual(author.beginCalls, 1)
        XCTAssertEqual(reader.beginCalls, 0)
        XCTAssertEqual(reader.receivedFrames, [Data("author".utf8)])
        XCTAssertEqual(author.closeCalls, 1)
        XCTAssertEqual(reader.closeCalls, 1)
    }

    func testPreviewIsAcceptedBeforeTheExchangeContinues() throws {
        let author = FixtureBoundary(
            beginOutcomes: [.sendMore()],
            outboundFrames: [Data("author".utf8)],
            receiveOutcomes: [.done]
        )
        let reader = FixtureBoundary(
            beginOutcomes: [],
            outboundFrames: [Data("reader".utf8)],
            receiveOutcomes: [.readyToPreview(count: 1)],
            acceptOutcomes: [.sendMore(terminal: true)]
        )

        let transfers = try ReactionFixtureSyncPump().run(author: author, reader: reader)

        XCTAssertEqual(transfers, 2)
        XCTAssertEqual(reader.acceptCalls, 1)
        XCTAssertEqual(author.receivedFrames, [Data("reader".utf8)])
    }

    func testEndlessExchangeStopsBeforeTheSixtyFifthTransfer() {
        let author = FixtureBoundary.echoing()
        let reader = FixtureBoundary.echoing()

        XCTAssertThrowsError(
            try ReactionFixtureSyncPump(maxTransfers: 64).run(author: author, reader: reader)
        ) { error in
            XCTAssertEqual(
                error as? ReactionUITestFixtureError,
                .transferLimitExceeded(limit: 64)
            )
        }
        XCTAssertEqual(author.beginCalls, 1)
        XCTAssertEqual(reader.beginCalls, 0)
        XCTAssertEqual(author.receivedFrames.count + reader.receivedFrames.count, 64)
        XCTAssertEqual(author.closeCalls, 1)
        XCTAssertEqual(reader.closeCalls, 1)
    }
}

#if os(iOS)
@MainActor
final class CompactReactionBarNativeSnapshotTests: XCTestCase {
    func testNormalDynamicTypeAtExact320PointWidth() {
        render(
            category: .large,
            attachmentName: "reaction-ios-320-normal"
        )
    }

    func testAccessibilityDynamicTypeAtExact320PointWidth() {
        render(
            category: .accessibilityExtraExtraExtraLarge,
            attachmentName: "reaction-ios-320-accessibility"
        )
    }

    private func render(
        category: ContentSizeCategory,
        attachmentName: String
    ) {
        let controls = ReactionKind.allCases.map {
            ReactionControlPresentation(
                kind: $0,
                count: $0 == .support ? 1 : 0,
                selected: $0 == .support,
                pending: $0 == .important,
                failed: $0 == .grief
            )
        }
        let root = CompactReactionBar(
            rowToken: "native-snapshot",
            controls: controls,
            failureMessage: "Couldn’t save your reaction. Try again.",
            showsLegend: true,
            onToggle: { _ in },
            onDismissLegend: {}
        )
        .environment(\.sizeCategory, category)
        .padding(16)
        .frame(width: 320, height: 568, alignment: .topLeading)
        .background(Color.white)

        let host = UIHostingController(rootView: root)
        host.view.frame = CGRect(x: 0, y: 0, width: 320, height: 568)
        host.view.backgroundColor = .white
        let window = UIWindow(frame: host.view.bounds)
        window.backgroundColor = .white
        window.rootViewController = host
        window.makeKeyAndVisible()
        defer {
            window.isHidden = true
        }

        host.view.setNeedsLayout()
        host.view.layoutIfNeeded()
        XCTAssertEqual(host.view.bounds.width, 320, accuracy: 0.01)

        let renderer = UIGraphicsImageRenderer(bounds: host.view.bounds)
        let image = renderer.image { context in
            host.view.layer.render(in: context.cgContext)
        }
        XCTAssertEqual(image.size.width, 320, accuracy: 0.01)
        guard let pixelData = image.cgImage?.dataProvider?.data as Data? else {
            return XCTFail("native render must expose pixel bytes")
        }
        XCTAssertGreaterThan(
            Set(pixelData).count,
            8,
            "native render must contain real controls, not a blank surface"
        )
        guard let png = image.pngData() else {
            return XCTFail("native SwiftUI render must produce PNG bytes")
        }
        let attachment = XCTAttachment(data: png, uniformTypeIdentifier: "public.png")
        attachment.name = attachmentName
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
#endif

private final class FixtureBoundary: MobileSyncSessionBoundary {
    private var beginOutcomes: [NearbySyncOutcome]
    private var outboundFrames: [Data]
    private var receiveOutcomes: [NearbySyncOutcome]
    private var acceptOutcomes: [NearbySyncOutcome]

    private(set) var beginCalls = 0
    private(set) var acceptCalls = 0
    private(set) var closeCalls = 0
    private(set) var receivedFrames: [Data] = []

    init(
        beginOutcomes: [NearbySyncOutcome],
        outboundFrames: [Data],
        receiveOutcomes: [NearbySyncOutcome],
        acceptOutcomes: [NearbySyncOutcome] = []
    ) {
        self.beginOutcomes = beginOutcomes
        self.outboundFrames = outboundFrames
        self.receiveOutcomes = receiveOutcomes
        self.acceptOutcomes = acceptOutcomes
    }

    static func echoing() -> FixtureBoundary {
        FixtureBoundary(
            beginOutcomes: [.sendMore()],
            outboundFrames: Array(repeating: Data("frame".utf8), count: 65),
            receiveOutcomes: Array(repeating: .sendMore(), count: 65)
        )
    }

    func begin() throws -> NearbySyncOutcome {
        beginCalls += 1
        return beginOutcomes.removeFirst()
    }

    func nextOutbound() throws -> Data? {
        outboundFrames.isEmpty ? nil : outboundFrames.removeFirst()
    }

    func receive(_ frame: Data) throws -> NearbySyncOutcome {
        receivedFrames.append(frame)
        return receiveOutcomes.removeFirst()
    }

    func acceptImport() throws -> NearbySyncOutcome {
        acceptCalls += 1
        return acceptOutcomes.removeFirst()
    }

    func rejectImport() throws -> NearbySyncOutcome { .failed }

    func close() throws {
        closeCalls += 1
    }
}
