import XCTest

@testable import RiotKit

private final class FakeSecretStore: DestructibleSecretStore {
    var key: Data? = Data(repeating: 7, count: 32)
    private(set) var destroyCount = 0
    private(set) var restoreCount = 0

    func loadOrCreateWrappingKey() throws -> Data { key ?? Data(repeating: 1, count: 32) }

    @discardableResult
    func destroyWrappingKey() throws -> Data? {
        destroyCount += 1
        defer { key = nil }
        return key
    }

    func restoreWrappingKey(_ key: Data) throws {
        restoreCount += 1
        self.key = key
    }
}

private final class FakeFiles: FileRemoving {
    var existing: Set<String>
    private(set) var removed: [String] = []
    init(existing: Set<String>) { self.existing = existing }
    func fileExists(atPath path: String) -> Bool { existing.contains(path) }
    func removeItem(atPath path: String) throws {
        removed.append(path)
        existing.remove(path)
    }
}

/// Runs scheduled work only when the test says so, so the countdown is asserted
/// deterministically instead of by sleeping.
private final class ManualScheduler: WipeScheduling {
    private struct Job {
        let id: Int
        let deadline: TimeInterval
        var work: (() -> Void)?
    }

    private var pending: [Job] = []
    private var now: TimeInterval = 0
    private var nextID = 0

    func schedule(after seconds: TimeInterval, _ work: @escaping () -> Void) -> WipeCancellable {
        nextID += 1
        let id = nextID
        pending.append(Job(id: id, deadline: now + seconds, work: work))
        return WipeCancellable { [weak self] in
            guard let self, let index = self.pending.firstIndex(where: { $0.id == id }) else {
                return
            }
            self.pending.remove(at: index)
        }
    }

    /// Runs the clock forward, firing each job at ITS deadline — including jobs
    /// scheduled by a job that just fired. The controller reschedules itself
    /// one second at a time, so a scheduler that only drained the initially
    /// pending set would deliver a single tick per call and never reach zero.
    func advance(by seconds: TimeInterval) {
        let target = now + seconds
        while let next = pending.filter({ $0.deadline <= target })
            .min(by: { $0.deadline < $1.deadline })
        {
            pending.removeAll { $0.id == next.id }
            now = next.deadline
            next.work?()
        }
        now = target
    }
}

@MainActor
final class EmergencyWipeControllerTests: XCTestCase {
    private let paths = WipePaths(
        databasePath: "/store/riot.db",
        profilePath: "/store/riot-profile.json",
        quarantineDirectory: "/store/quarantine")

    private func makeController(
        keys: FakeSecretStore = FakeSecretStore(),
        files: FakeFiles = FakeFiles(existing: ["/store/riot.db", "/store/quarantine"]),
        scheduler: ManualScheduler = ManualScheduler()
    ) -> EmergencyWipeController {
        EmergencyWipeController(
            wipe: EmergencyWipe(keyStore: keys, paths: paths, files: files),
            scheduler: scheduler)
    }

    /// Triggering must destroy the key AT ONCE — the countdown is only about
    /// file deletion, never about delaying the crypto-erase.
    func testTriggeringDestroysTheKeyImmediatelyAndStartsTheCountdown() throws {
        let keys = FakeSecretStore()
        let controller = makeController(keys: keys)

        controller.trigger()

        XCTAssertNil(keys.key, "the key is already gone while the countdown runs")
        XCTAssertEqual(controller.state, .counting(secondsRemaining: EmergencyWipeController.undoWindow))
    }

    /// The accidental triple-tap: undo inside the window puts the identity back.
    func testUndoWithinTheWindowRestoresTheIdentityAndDeletesNothing() throws {
        let keys = FakeSecretStore()
        let files = FakeFiles(existing: ["/store/riot.db", "/store/quarantine"])
        let controller = makeController(keys: keys, files: files)

        controller.trigger()
        controller.undo()

        XCTAssertEqual(keys.restoreCount, 1)
        XCTAssertNotNil(keys.key)
        XCTAssertTrue(files.removed.isEmpty)
        XCTAssertEqual(controller.state, .idle)
    }

    /// When the window elapses the files go and the state is terminal.
    func testTheWindowElapsingCommitsTheWipe() throws {
        let files = FakeFiles(existing: ["/store/riot.db", "/store/quarantine"])
        let scheduler = ManualScheduler()
        let controller = makeController(files: files, scheduler: scheduler)

        controller.trigger()
        scheduler.advance(by: TimeInterval(EmergencyWipeController.undoWindow))

        XCTAssertEqual(Set(files.removed), ["/store/riot.db", "/store/quarantine"])
        XCTAssertEqual(controller.state, .wiped)
    }

    /// Undo after the window is over must not resurrect anything.
    func testUndoAfterTheWindowIsIgnored() throws {
        let keys = FakeSecretStore()
        let scheduler = ManualScheduler()
        let controller = makeController(keys: keys, scheduler: scheduler)

        controller.trigger()
        scheduler.advance(by: TimeInterval(EmergencyWipeController.undoWindow))
        controller.undo()

        XCTAssertEqual(keys.restoreCount, 0)
        XCTAssertNil(keys.key)
        XCTAssertEqual(controller.state, .wiped)
    }

    /// The countdown is shown to the person, so it must actually tick down —
    /// a frozen number would misrepresent how long they have to undo.
    func testTheCountdownTicksTowardZero() throws {
        let scheduler = ManualScheduler()
        let controller = makeController(scheduler: scheduler)

        controller.trigger()
        scheduler.advance(by: 1)
        XCTAssertEqual(
            controller.state, .counting(secondsRemaining: EmergencyWipeController.undoWindow - 1))
        scheduler.advance(by: 1)
        XCTAssertEqual(
            controller.state, .counting(secondsRemaining: EmergencyWipeController.undoWindow - 2))
    }

    /// A second trigger during the countdown must not re-arm and strand the undo.
    func testTriggeringTwiceDoesNotRestartOrStrandTheUndo() throws {
        let keys = FakeSecretStore()
        let controller = makeController(keys: keys)

        controller.trigger()
        controller.trigger()
        controller.undo()

        XCTAssertEqual(keys.destroyCount, 1)
        XCTAssertEqual(keys.restoreCount, 1)
        XCTAssertNotNil(keys.key)
    }
}
