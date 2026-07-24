import CryptoKit
import Foundation

public enum ReleaseFixtureError: Error, Equatable {
    case digestMismatch
    case malformedJSON
    case invalidRootKeys
    case invalidIdentifierKeys
    case invalidStateKeys
    case invalidSchemaVersion
    case invalidFixtureRevision
    case nonSyntheticKind
    case invalidFixedClock
    case invalidStateInventory
    case invalidIdentifier
    case mismatchedStateIdentifier
    case invalidNarrativeValue
    case prohibitedData
}

public struct ReleaseFixture: Sendable, Equatable, Decodable {
    public struct Identifiers: Sendable, Equatable, Decodable {
        public let communityId: String
        public let contributorId: String
        public let stateIds: [String: String]
    }

    public struct NarrativeState: Sendable, Equatable, Decodable {
        public let id: String
        public let surface: String
        public let headline: String
        public let supportingCopy: String
        public let communityId: String
        public let contributorId: String
        public let entryId: String
    }

    public let schemaVersion: Int
    public let fixtureRevision: String
    public let fixtureKind: String
    public let fixedClock: String
    public let identifiers: Identifiers
    public let narrativeStates: [NarrativeState]

    private static let pinnedDigest = "930a9c5aa06dea920b0502dbd72b6b2bf00d1b4cb9405b99e69e00d035640469"
    private static let rootKeys: Set<String> = [
        "schemaVersion", "fixtureRevision", "fixtureKind", "fixedClock",
        "identifiers", "narrativeStates",
    ]
    private static let identifierKeys: Set<String> = ["communityId", "contributorId", "stateIds"]
    private static let stateKeys: Set<String> = [
        "id", "surface", "headline", "supportingCopy",
        "communityId", "contributorId", "entryId",
    ]
    private static let expectedStates: [(id: String, surface: String, headline: String, copy: String)] = [
        ("community-newswire", "spaces-home", "Your community. Your newswire.", "Follow updates from a community you choose."),
        ("signed-publishing", "compose", "Publish signed updates from the field.", "Signatures show source and integrity, not whether a claim is true."),
        ("labels-and-signatures", "newswire", "Read signatures and community editorial labels.", "See signed source details. Community editorial labels are community signals, not independent factual verification."),
        ("community-tools", "apps-checklists", "Carry useful tools with the community.", "Open a shared checklist alongside community updates."),
        ("nearby-exchange", "nearby", "Exchange updates nearby.", "Experimental: exchange updates directly with a nearby device."),
        ("offline-copy", "offline-copy", "Keep a local copy available offline.", "Keep a local copy ready to read without a connection."),
    ]
    private static let prohibitedKeys: Set<String> = [
        "person", "personname", "name", "email", "phone", "location", "address",
        "latitude", "longitude", "coordinates", "notification", "devicetoken",
        "apnstoken", "fcmtoken", "private", "privatecommunity", "url", "hostname",
        "ipaddress", "npub", "nsec", "note",
    ]

    public static func decode(bytes: Data) throws -> ReleaseFixture {
        guard digest(bytes) == pinnedDigest else {
            throw ReleaseFixtureError.digestMismatch
        }
        return try validateSemantics(bytes: bytes)
    }

    static func validateSemantics(bytes: Data) throws -> ReleaseFixture {
        let raw: Any
        do {
            raw = try JSONSerialization.jsonObject(with: bytes)
        } catch {
            throw ReleaseFixtureError.malformedJSON
        }
        guard let root = raw as? [String: Any] else {
            throw ReleaseFixtureError.malformedJSON
        }
        try rejectProhibitedData(raw)
        guard Set(root.keys) == rootKeys else {
            throw ReleaseFixtureError.invalidRootKeys
        }
        guard let identifiersObject = root["identifiers"] as? [String: Any],
              Set(identifiersObject.keys) == identifierKeys else {
            throw ReleaseFixtureError.invalidIdentifierKeys
        }
        guard let statesObject = root["narrativeStates"] as? [[String: Any]] else {
            throw ReleaseFixtureError.malformedJSON
        }
        guard statesObject.allSatisfy({ Set($0.keys) == stateKeys }) else {
            throw ReleaseFixtureError.invalidStateKeys
        }
        let fixture: ReleaseFixture
        do {
            fixture = try JSONDecoder().decode(ReleaseFixture.self, from: bytes)
        } catch {
            throw ReleaseFixtureError.malformedJSON
        }
        try fixture.validate()
        return fixture
    }

    private func validate() throws {
        guard schemaVersion == 1 else { throw ReleaseFixtureError.invalidSchemaVersion }
        guard fixtureRevision == "riot-1.0-synthetic-v1" else {
            throw ReleaseFixtureError.invalidFixtureRevision
        }
        guard fixtureKind == "synthetic" else { throw ReleaseFixtureError.nonSyntheticKind }
        guard fixedClock == "2026-07-24T12:00:00.000Z" else {
            throw ReleaseFixtureError.invalidFixedClock
        }
        guard narrativeStates.map(\.id) == Self.expectedStates.map(\.id),
              identifiers.stateIds.keys.count == Self.expectedStates.count,
              Set(identifiers.stateIds.keys) == Set(Self.expectedStates.map(\.id)) else {
            throw ReleaseFixtureError.invalidStateInventory
        }

        try Self.requireIdentifier(
            identifiers.communityId,
            label: "riot-release-fixture:v1:community"
        )
        try Self.requireIdentifier(
            identifiers.contributorId,
            label: "riot-release-fixture:v1:contributor"
        )
        for (index, expected) in Self.expectedStates.enumerated() {
            guard let stateIdentifier = identifiers.stateIds[expected.id] else {
                throw ReleaseFixtureError.invalidStateInventory
            }
            try Self.requireIdentifier(
                stateIdentifier,
                label: "riot-release-fixture:v1:\(expected.id)"
            )
            let state = narrativeStates[index]
            guard state.surface == expected.surface,
                  state.headline == expected.headline,
                  state.supportingCopy == expected.copy else {
                throw ReleaseFixtureError.invalidNarrativeValue
            }
            guard state.communityId == identifiers.communityId,
                  state.contributorId == identifiers.contributorId,
                  state.entryId == stateIdentifier else {
                throw ReleaseFixtureError.mismatchedStateIdentifier
            }
        }
    }

    private static func requireIdentifier(_ identifier: String, label: String) throws {
        guard identifier.range(of: "^[0-9a-f]{64}$", options: .regularExpression) != nil,
              identifier == digest(Data(label.utf8)) else {
            throw ReleaseFixtureError.invalidIdentifier
        }
    }

    private static func rejectProhibitedData(_ value: Any) throws {
        if let object = value as? [String: Any] {
            for (key, child) in object {
                guard !prohibitedKeys.contains(key.lowercased()) else {
                    throw ReleaseFixtureError.prohibitedData
                }
                try rejectProhibitedData(child)
            }
        } else if let array = value as? [Any] {
            for child in array {
                try rejectProhibitedData(child)
            }
        } else if let string = value as? String, isProhibitedValue(string) {
            throw ReleaseFixtureError.prohibitedData
        }
    }

    private static func isProhibitedValue(_ value: String) -> Bool {
        let lower = value.lowercased()
        if lower.contains("person: ana") ||
            lower.contains("ana@example.com") ||
            lower.contains("+64 21 555 0100") ||
            lower.contains("123 main street") ||
            lower.contains("37.7749,-122.4194") ||
            lower.contains("exponentpushtoken[fixture]") ||
            lower.contains("private community") ||
            lower.contains("https://") ||
            lower.contains("http://") ||
            lower.contains("riot.protest.net") ||
            lower.contains("203.0.113.1") {
            return true
        }
        return lower.hasPrefix("npub1") || lower.hasPrefix("nsec1") || lower.hasPrefix("note1")
    }

    private static func digest(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }
}
