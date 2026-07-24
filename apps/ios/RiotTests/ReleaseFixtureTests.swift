import CryptoKit
import XCTest
@testable import RiotKit

final class ReleaseFixtureTests: XCTestCase {
    private static let expectedDigest = "930a9c5aa06dea920b0502dbd72b6b2bf00d1b4cb9405b99e69e00d035640469"
    private static let stateIDs = [
        "community-newswire",
        "signed-publishing",
        "labels-and-signatures",
        "community-tools",
        "nearby-exchange",
        "offline-copy",
    ]
    private static let surfaces = [
        "spaces-home",
        "compose",
        "newswire",
        "apps-checklists",
        "nearby",
        "offline-copy",
    ]
    private static let headlines = [
        "Your community. Your newswire.",
        "Publish signed updates from the field.",
        "Read signatures and community editorial labels.",
        "Carry useful tools with the community.",
        "Exchange updates nearby.",
        "Keep a local copy available offline.",
    ]

    func testCanonicalFixtureIsPinnedAndExact() throws {
        let bytes = try fixtureBytes()
        XCTAssertEqual(sha256(bytes), Self.expectedDigest)

        let fixture = try ReleaseFixture.decode(bytes: bytes)
        XCTAssertEqual(fixture.schemaVersion, 1)
        XCTAssertEqual(fixture.fixtureRevision, "riot-1.0-synthetic-v1")
        XCTAssertEqual(fixture.fixtureKind, "synthetic")
        XCTAssertEqual(fixture.fixedClock, "2026-07-24T12:00:00.000Z")
        XCTAssertEqual(fixture.narrativeStates.map(\.id), Self.stateIDs)
        XCTAssertEqual(fixture.narrativeStates.map(\.surface), Self.surfaces)
        XCTAssertEqual(fixture.narrativeStates.map(\.headline), Self.headlines)
        XCTAssertTrue(fixture.narrativeStates[4].supportingCopy.hasPrefix("Experimental:"))
    }

    func testKeySetsAndDeterministicIdentifiersAreExact() throws {
        let bytes = try fixtureBytes()
        let root = try XCTUnwrap(JSONSerialization.jsonObject(with: bytes) as? [String: Any])
        XCTAssertEqual(Set(root.keys), ["schemaVersion", "fixtureRevision", "fixtureKind", "fixedClock", "identifiers", "narrativeStates"])

        let identifiers = try XCTUnwrap(root["identifiers"] as? [String: Any])
        XCTAssertEqual(Set(identifiers.keys), ["communityId", "contributorId", "stateIds"])
        let states = try XCTUnwrap(root["narrativeStates"] as? [[String: Any]])
        for state in states {
            XCTAssertEqual(Set(state.keys), ["id", "surface", "headline", "supportingCopy", "communityId", "contributorId", "entryId"])
        }

        let fixture = try ReleaseFixture.decode(bytes: bytes)
        let expected = [
            fixture.identifiers.communityId: "riot-release-fixture:v1:community",
            fixture.identifiers.contributorId: "riot-release-fixture:v1:contributor",
        ]
        for (identifier, label) in expected {
            XCTAssertEqual(identifier, sha256(Data(label.utf8)))
            XCTAssertNotNil(identifier.range(of: "^[0-9a-f]{64}$", options: .regularExpression))
        }
        for id in Self.stateIDs {
            let identifier = try XCTUnwrap(fixture.identifiers.stateIds[id])
            XCTAssertEqual(identifier, sha256(Data("riot-release-fixture:v1:\(id)".utf8)))
            XCTAssertNotNil(identifier.range(of: "^[0-9a-f]{64}$", options: .regularExpression))
        }
    }

    func testPublicDigestAndSemanticFailuresAreTyped() throws {
        let bytes = try fixtureBytes()
        var changedBytes = bytes
        changedBytes.append(0x20)
        XCTAssertThrowsError(try ReleaseFixture.decode(bytes: changedBytes)) {
            XCTAssertEqual($0 as? ReleaseFixtureError, .digestMismatch)
        }

        XCTAssertEqual(try assertMutation(["unknown": true], at: []), .invalidRootKeys)
        XCTAssertEqual(try assertMutation(["unknown": true], at: ["identifiers"]), .invalidIdentifierKeys)
        XCTAssertEqual(try assertMutation(["unknown": true], at: ["narrativeStates", 0]), .invalidStateKeys)
        try assertReplacement("2026-07-24T12:00:01.000Z", at: ["fixedClock"], error: .invalidFixedClock)
        try assertReplacement("real", at: ["fixtureKind"], error: .nonSyntheticKind)
        try assertReplacement("short", at: ["identifiers", "communityId"], error: .invalidIdentifier)
        try assertReplacement(String(repeating: "0", count: 64), at: ["narrativeStates", 0, "entryId"], error: .mismatchedStateIdentifier)

        var root = try mutableRoot()
        var states = root["narrativeStates"] as! [[String: Any]]
        states.removeLast()
        root["narrativeStates"] = states
        try assertSemantic(root, .invalidStateInventory)

        root = try mutableRoot()
        states = root["narrativeStates"] as! [[String: Any]]
        states.append(states[0])
        root["narrativeStates"] = states
        try assertSemantic(root, .invalidStateInventory)

        root = try mutableRoot()
        states = root["narrativeStates"] as! [[String: Any]]
        states.swapAt(0, 1)
        root["narrativeStates"] = states
        try assertSemantic(root, .invalidStateInventory)
    }

    func testEveryProhibitedKeyAndValueFailsClosed() throws {
        let prohibitedKeys = [
            "person", "personName", "name", "email", "phone", "location", "address",
            "latitude", "longitude", "coordinates", "notification", "deviceToken",
            "apnsToken", "fcmToken", "private", "privateCommunity", "url", "hostname",
            "ipAddress", "npub", "nsec", "note",
        ]
        for key in prohibitedKeys {
            XCTAssertEqual(
                try assertMutation([key: "synthetic-test-value"], at: ["narrativeStates", 0]),
                .prohibitedData
            )
        }

        let prohibitedValues = [
            "Person: Ana", "ana@example.com", "+64 21 555 0100", "123 Main Street",
            "37.7749,-122.4194", "ExponentPushToken[fixture]", "private community",
            "https://riot.protest.net", "riot.protest.net", "203.0.113.1",
            "npub1t985dmat80n6xlrnhsjzzrlhfkcmmemul47n3mz9lws70lrxs0pqwzdyaw",
            "nsec1tu92893lv55urd4almqhfnrv48ls2uwas5hxg6eashq9jhnt45ts9en3zd",
            "note1m99r7nwc0wdrkzldrqan96gklg5usqspq7z9696j6unf0ljnpxjspqfw99",
        ]
        for value in prohibitedValues {
            try assertReplacement(value, at: ["narrativeStates", 0, "supportingCopy"], error: .prohibitedData)
        }
    }

    private func fixtureBytes() throws -> Data {
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "riot-1.0-synthetic", withExtension: "json"))
        return try Data(contentsOf: url)
    }

    private func mutableRoot() throws -> [String: Any] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: fixtureBytes()) as? [String: Any])
    }

    @discardableResult
    private func assertMutation(_ values: [String: Any], at path: [Any]) throws -> ReleaseFixtureError {
        var root = try mutableRoot()
        if path.isEmpty {
            root.merge(values) { _, new in new }
        } else if path.count == 1 {
            var object = root[path[0] as! String] as! [String: Any]
            object.merge(values) { _, new in new }
            root[path[0] as! String] = object
        } else {
            var states = root[path[0] as! String] as! [[String: Any]]
            states[path[1] as! Int].merge(values) { _, new in new }
            root[path[0] as! String] = states
        }
        return try semanticError(root)
    }

    private func assertReplacement(_ value: Any, at path: [Any], error: ReleaseFixtureError) throws {
        var root = try mutableRoot()
        if path.count == 1 {
            root[path[0] as! String] = value
        } else if path.count == 2 {
            var object = root[path[0] as! String] as! [String: Any]
            object[path[1] as! String] = value
            root[path[0] as! String] = object
        } else {
            var states = root[path[0] as! String] as! [[String: Any]]
            states[path[1] as! Int][path[2] as! String] = value
            root[path[0] as! String] = states
        }
        try assertSemantic(root, error)
    }

    private func semanticError(_ root: [String: Any]) throws -> ReleaseFixtureError {
        let bytes = try JSONSerialization.data(withJSONObject: root, options: [.sortedKeys])
        do {
            _ = try ReleaseFixture.validateSemantics(bytes: bytes)
            XCTFail("Expected semantic validation to fail")
            return .malformedJSON
        } catch let error as ReleaseFixtureError {
            return error
        }
    }

    private func assertSemantic(_ root: [String: Any], _ expected: ReleaseFixtureError) throws {
        XCTAssertEqual(try semanticError(root), expected)
    }

    private func sha256(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }
}
