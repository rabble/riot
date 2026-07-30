import CryptoKit
import XCTest
@testable import RiotKit

final class ReleaseFixtureTests: XCTestCase {
    private static let expectedDigest = "930a9c5aa06dea920b0502dbd72b6b2bf00d1b4cb9405b99e69e00d035640469"

    func testCanonicalFixtureAndSemanticContract() throws {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "riot-1.0-synthetic", withExtension: "json"))
        let bytes = try Data(contentsOf: url)
        let digest = SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined()
        XCTAssertEqual(digest, Self.expectedDigest)

        let fixture = try ReleaseFixture.decode(bytes: bytes)
        XCTAssertEqual(fixture.schemaVersion, 1)
        XCTAssertEqual(fixture.fixtureRevision, "riot-1.0-synthetic-v1")
        XCTAssertEqual(fixture.fixtureKind, "synthetic")
        XCTAssertEqual(fixture.fixedClock, "2026-07-24T12:00:00.000Z")
        XCTAssertEqual(
            fixture.narrativeStates.map(\.id),
            ["community-newswire", "signed-publishing", "labels-and-signatures", "community-tools", "nearby-exchange", "offline-copy"]
        )
        XCTAssertEqual(
            fixture.narrativeStates.map(\.surface),
            ["spaces-home", "compose", "newswire", "apps-checklists", "nearby", "offline-copy"]
        )
        XCTAssertEqual(
            fixture.narrativeStates.map(\.headline),
            [
                "Your community. Your newswire.",
                "Publish signed updates from the field.",
                "Read signatures and community editorial labels.",
                "Carry useful tools with the community.",
                "Exchange updates nearby.",
                "Keep a local copy available offline.",
            ]
        )
        XCTAssertTrue(fixture.narrativeStates[4].supportingCopy.hasPrefix("Experimental:"))

        var changed = bytes
        changed.append(0x20)
        XCTAssertThrowsError(try ReleaseFixture.decode(bytes: changed)) {
            XCTAssertEqual($0 as? ReleaseFixtureError, .digestMismatch)
        }
    }

    func testStructureIdentifiersAndTypedFailuresOnMacOS() throws {
        let root = try mutableRoot()
        XCTAssertEqual(
            Set(root.keys),
            ["schemaVersion", "fixtureRevision", "fixtureKind", "fixedClock", "identifiers", "narrativeStates"]
        )
        let identifiers = root["identifiers"] as! [String: Any]
        XCTAssertEqual(Set(identifiers.keys), ["communityId", "contributorId", "stateIds"])
        let states = root["narrativeStates"] as! [[String: Any]]
        for state in states {
            XCTAssertEqual(
                Set(state.keys),
                ["id", "surface", "headline", "supportingCopy", "communityId", "contributorId", "entryId"]
            )
        }

        let fixture = try ReleaseFixture.decode(bytes: fixtureBytes())
        XCTAssertEqual(fixture.identifiers.communityId, digest("riot-release-fixture:v1:community"))
        XCTAssertEqual(fixture.identifiers.contributorId, digest("riot-release-fixture:v1:contributor"))
        for id in fixture.narrativeStates.map(\.id) {
            let identifier = try XCTUnwrap(fixture.identifiers.stateIds[id])
            XCTAssertEqual(identifier, digest("riot-release-fixture:v1:\(id)"))
            XCTAssertNotNil(identifier.range(of: "^[0-9a-f]{64}$", options: .regularExpression))
        }

        try assertMutation(.invalidRootKeys) { $0["unknown"] = true }
        try assertMutation(.invalidIdentifierKeys) {
            var identifiers = $0["identifiers"] as! [String: Any]
            identifiers["unknown"] = true
            $0["identifiers"] = identifiers
        }
        try assertMutation(.invalidStateKeys) {
            var states = $0["narrativeStates"] as! [[String: Any]]
            states[0]["unknown"] = true
            $0["narrativeStates"] = states
        }
        try assertMutation(.invalidFixedClock) { $0["fixedClock"] = "2026-07-24T12:00:01.000Z" }
        try assertMutation(.nonSyntheticKind) { $0["fixtureKind"] = "real" }
        try assertMutation(.invalidIdentifier) {
            var identifiers = $0["identifiers"] as! [String: Any]
            identifiers["communityId"] = "short"
            $0["identifiers"] = identifiers
        }
        try assertMutation(.mismatchedStateIdentifier) {
            var states = $0["narrativeStates"] as! [[String: Any]]
            states[0]["entryId"] = String(repeating: "0", count: 64)
            $0["narrativeStates"] = states
        }
        try assertMutation(.invalidStateInventory) {
            var states = $0["narrativeStates"] as! [[String: Any]]
            states.removeLast()
            $0["narrativeStates"] = states
        }
        try assertMutation(.invalidStateInventory) {
            var states = $0["narrativeStates"] as! [[String: Any]]
            states.append(states[0])
            $0["narrativeStates"] = states
        }
        try assertMutation(.invalidStateInventory) {
            var states = $0["narrativeStates"] as! [[String: Any]]
            states.swapAt(0, 1)
            $0["narrativeStates"] = states
        }
    }

    func testMutationsFailClosedOnMacOS() throws {
        let keys = [
            "person", "personName", "name", "email", "phone", "location", "address",
            "latitude", "longitude", "coordinates", "notification", "deviceToken",
            "apnsToken", "fcmToken", "private", "privateCommunity", "url", "hostname",
            "ipAddress", "npub", "nsec", "note",
        ]
        for key in keys {
            var root = try mutableRoot()
            var states = root["narrativeStates"] as! [[String: Any]]
            states[0][key] = "synthetic-test-value"
            root["narrativeStates"] = states
            XCTAssertEqual(try semanticError(root), .prohibitedData)
        }

        let values = [
            "Person: Ana", "ana@example.com", "+64 21 555 0100", "123 Main Street",
            "37.7749,-122.4194", "ExponentPushToken[fixture]", "private community",
            "https://riot.protest.net", "riot.protest.net", "203.0.113.1",
            "npub1t985dmat80n6xlrnhsjzzrlhfkcmmemul47n3mz9lws70lrxs0pqwzdyaw",
            "nsec1tu92893lv55urd4almqhfnrv48ls2uwas5hxg6eashq9jhnt45ts9en3zd",
            "note1m99r7nwc0wdrkzldrqan96gklg5usqspq7z9696j6unf0ljnpxjspqfw99",
        ]
        for value in values {
            var root = try mutableRoot()
            var states = root["narrativeStates"] as! [[String: Any]]
            states[0]["supportingCopy"] = value
            root["narrativeStates"] = states
            XCTAssertEqual(try semanticError(root), .prohibitedData)
        }
    }

    private func mutableRoot() throws -> [String: Any] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: fixtureBytes()) as? [String: Any])
    }

    private func fixtureBytes() throws -> Data {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "riot-1.0-synthetic", withExtension: "json"))
        return try Data(contentsOf: url)
    }

    private func assertMutation(
        _ expected: ReleaseFixtureError,
        mutate: (inout [String: Any]) -> Void
    ) throws {
        var root = try mutableRoot()
        mutate(&root)
        XCTAssertEqual(try semanticError(root), expected)
    }

    private func digest(_ label: String) -> String {
        SHA256.hash(data: Data(label.utf8)).map { String(format: "%02x", $0) }.joined()
    }

    private func semanticError(_ root: [String: Any]) throws -> ReleaseFixtureError {
        do {
            _ = try ReleaseFixture.validateSemantics(
                bytes: JSONSerialization.data(withJSONObject: root, options: [.sortedKeys])
            )
            XCTFail("Expected semantic validation to fail")
            return .malformedJSON
        } catch let error as ReleaseFixtureError {
            return error
        }
    }
}
