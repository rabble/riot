# Riot Prototype Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a first native prototype that can create a packet, author a structured alert, render it locally, export/import a portable archive, and preserve signer/provenance metadata.

**Architecture:** Start with a small Swift app using a local packet store abstraction that can later be backed by Willow. The first prototype should use JSON files for speed, but every storage path and object should mirror the planned Willow mapping so the real Willow store can replace it without changing product flows.

**Tech Stack:** Swift, SwiftUI, local file storage, WebKit sandboxed preview, CryptoKit/Ed25519-compatible signer once selected, XCTest.

---

## Scope Check

This plan is intentionally a prototype plan, not the full Riot architecture. It does not implement real Willow encoding, Meadowcap, Drop Format, WTP, or a local LLM. It builds the product skeleton those pieces will plug into.

## Planned File Structure

- `RiotApp/` - future native app source root.
- `RiotApp/PacketRuntime/PacketManifest.swift` - packet metadata model.
- `RiotApp/PacketRuntime/PacketObject.swift` - typed packet object model.
- `RiotApp/PacketRuntime/PacketStore.swift` - local JSON-backed store.
- `RiotApp/Authoring/AlertDraft.swift` - alert authoring draft and validation.
- `RiotApp/Authoring/AlertComposerModel.swift` - create/edit alert flow.
- `RiotApp/Rendering/PacketSiteRenderer.swift` - builds local HTML from packet objects.
- `RiotApp/Exchange/PacketArchive.swift` - import/export archive format.
- `RiotApp/Trust/SignerIdentity.swift` - local signer metadata.
- `RiotAppTests/` - focused unit tests for model, store, render, and archive behavior.

## Task 1: Create Package Skeleton

**Files:**
- Create: `Package.swift`
- Create: `RiotApp/PacketRuntime/PacketManifest.swift`
- Create: `RiotApp/PacketRuntime/PacketObject.swift`
- Create: `RiotAppTests/PacketRuntimeTests.swift`

- [ ] **Step 1: Create Swift package manifest**

Create `Package.swift`:

```swift
// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "riot",
    platforms: [
        .iOS(.v17),
        .macOS(.v14)
    ],
    products: [
        .library(name: "RiotApp", targets: ["RiotApp"])
    ],
    targets: [
        .target(name: "RiotApp", path: "RiotApp"),
        .testTarget(name: "RiotAppTests", dependencies: ["RiotApp"], path: "RiotAppTests")
    ]
)
```

- [ ] **Step 2: Add packet models**

Create `RiotApp/PacketRuntime/PacketManifest.swift`:

```swift
import Foundation

public struct PacketManifest: Codable, Equatable, Sendable {
    public let id: String
    public let title: String
    public let summary: String
    public let namespaceID: String
    public let createdAt: Date
    public let updatedAt: Date

    public init(id: String, title: String, summary: String, namespaceID: String, createdAt: Date, updatedAt: Date) {
        self.id = id
        self.title = title
        self.summary = summary
        self.namespaceID = namespaceID
        self.createdAt = createdAt
        self.updatedAt = updatedAt
    }
}
```

Create `RiotApp/PacketRuntime/PacketObject.swift`:

```swift
import Foundation

public enum PacketObjectKind: String, Codable, Equatable, Sendable {
    case alert
    case resourceLocation
    case checklist
    case correction
}

public struct PacketObject: Codable, Equatable, Sendable {
    public let id: String
    public let kind: PacketObjectKind
    public let title: String
    public let body: String
    public let createdAt: Date
    public let expiresAt: Date?
    public let signerID: String
    public let sourceNote: String
    public let aiAssisted: Bool

    public init(
        id: String,
        kind: PacketObjectKind,
        title: String,
        body: String,
        createdAt: Date,
        expiresAt: Date?,
        signerID: String,
        sourceNote: String,
        aiAssisted: Bool
    ) {
        self.id = id
        self.kind = kind
        self.title = title
        self.body = body
        self.createdAt = createdAt
        self.expiresAt = expiresAt
        self.signerID = signerID
        self.sourceNote = sourceNote
        self.aiAssisted = aiAssisted
    }
}
```

- [ ] **Step 3: Add model tests**

Create `RiotAppTests/PacketRuntimeTests.swift`:

```swift
import XCTest
@testable import RiotApp

final class PacketRuntimeTests: XCTestCase {
    func testPacketObjectRoundTripsThroughJSON() throws {
        let object = PacketObject(
            id: "alert-1",
            kind: .alert,
            title: "Bridge closed",
            body: "Use the north route.",
            createdAt: Date(timeIntervalSince1970: 1_000),
            expiresAt: Date(timeIntervalSince1970: 2_000),
            signerID: "signer-local",
            sourceNote: "Confirmed by field team",
            aiAssisted: true
        )

        let data = try JSONEncoder().encode(object)
        let decoded = try JSONDecoder().decode(PacketObject.self, from: data)

        XCTAssertEqual(decoded, object)
    }
}
```

- [ ] **Step 4: Run tests**

Run: `swift test`

Expected: test suite passes with `PacketRuntimeTests.testPacketObjectRoundTripsThroughJSON`.

- [ ] **Step 5: Commit**

```bash
git add Package.swift RiotApp RiotAppTests
git commit -m "chore: create riot package skeleton"
```

## Task 2: JSON Packet Store

**Files:**
- Create: `RiotApp/PacketRuntime/PacketStore.swift`
- Test: `RiotAppTests/PacketStoreTests.swift`

- [ ] **Step 1: Write failing store test**

Create `RiotAppTests/PacketStoreTests.swift`:

```swift
import XCTest
@testable import RiotApp

final class PacketStoreTests: XCTestCase {
    func testSaveAndLoadPacketObjects() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        let store = PacketStore(root: root)
        let object = PacketObject(
            id: "alert-1",
            kind: .alert,
            title: "Water available",
            body: "Bring containers.",
            createdAt: Date(timeIntervalSince1970: 100),
            expiresAt: nil,
            signerID: "local",
            sourceNote: "Volunteer table",
            aiAssisted: false
        )

        try store.save(object, packetID: "disaster")
        let objects = try store.loadObjects(packetID: "disaster")

        XCTAssertEqual(objects, [object])
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `swift test --filter PacketStoreTests`

Expected: FAIL because `PacketStore` is not defined.

- [ ] **Step 3: Implement store**

Create `RiotApp/PacketRuntime/PacketStore.swift`:

```swift
import Foundation

public final class PacketStore {
    private let root: URL
    private let encoder: JSONEncoder
    private let decoder: JSONDecoder

    public init(root: URL) {
        self.root = root
        self.encoder = JSONEncoder()
        self.encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        self.decoder = JSONDecoder()
    }

    public func save(_ object: PacketObject, packetID: String) throws {
        let directory = root.appendingPathComponent(packetID, isDirectory: true).appendingPathComponent("objects", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let url = directory.appendingPathComponent("\(object.id).json")
        let data = try encoder.encode(object)
        try data.write(to: url, options: [.atomic])
    }

    public func loadObjects(packetID: String) throws -> [PacketObject] {
        let directory = root.appendingPathComponent(packetID, isDirectory: true).appendingPathComponent("objects", isDirectory: true)
        guard FileManager.default.fileExists(atPath: directory.path) else { return [] }
        let urls = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "json" }
            .sorted { $0.lastPathComponent < $1.lastPathComponent }
        return try urls.map { url in
            let data = try Data(contentsOf: url)
            return try decoder.decode(PacketObject.self, from: data)
        }
    }
}
```

- [ ] **Step 4: Run test to verify pass**

Run: `swift test --filter PacketStoreTests`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add RiotApp/PacketRuntime/PacketStore.swift RiotAppTests/PacketStoreTests.swift
git commit -m "feat: add JSON packet store"
```

## Task 3: Alert Authoring Validation

**Files:**
- Create: `RiotApp/Authoring/AlertDraft.swift`
- Test: `RiotAppTests/AlertDraftTests.swift`

- [ ] **Step 1: Write failing validation tests**

Create `RiotAppTests/AlertDraftTests.swift`:

```swift
import XCTest
@testable import RiotApp

final class AlertDraftTests: XCTestCase {
    func testOperationalAlertRequiresSourceAndExpiry() {
        let draft = AlertDraft(
            title: "Clinic moved",
            body: "Go to the school gym.",
            sourceNote: "",
            expiresAt: nil,
            aiAssisted: true
        )

        XCTAssertEqual(draft.validationErrors, [.missingSource, .missingExpiry])
    }

    func testValidDraftCreatesPacketObject() throws {
        let expiry = Date(timeIntervalSince1970: 2_000)
        let draft = AlertDraft(
            title: "Clinic moved",
            body: "Go to the school gym.",
            sourceNote: "Confirmed by medic team",
            expiresAt: expiry,
            aiAssisted: true
        )

        let object = try draft.makeObject(id: "alert-1", signerID: "local", now: Date(timeIntervalSince1970: 1_000))

        XCTAssertEqual(object.kind, .alert)
        XCTAssertEqual(object.expiresAt, expiry)
        XCTAssertEqual(object.sourceNote, "Confirmed by medic team")
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `swift test --filter AlertDraftTests`

Expected: FAIL because `AlertDraft` is not defined.

- [ ] **Step 3: Implement alert draft**

Create `RiotApp/Authoring/AlertDraft.swift`:

```swift
import Foundation

public enum AlertDraftValidationError: String, Error, Equatable, Sendable {
    case missingTitle
    case missingBody
    case missingSource
    case missingExpiry
}

public struct AlertDraft: Equatable, Sendable {
    public var title: String
    public var body: String
    public var sourceNote: String
    public var expiresAt: Date?
    public var aiAssisted: Bool

    public init(title: String, body: String, sourceNote: String, expiresAt: Date?, aiAssisted: Bool) {
        self.title = title
        self.body = body
        self.sourceNote = sourceNote
        self.expiresAt = expiresAt
        self.aiAssisted = aiAssisted
    }

    public var validationErrors: [AlertDraftValidationError] {
        var errors: [AlertDraftValidationError] = []
        if title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { errors.append(.missingTitle) }
        if body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { errors.append(.missingBody) }
        if sourceNote.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { errors.append(.missingSource) }
        if expiresAt == nil { errors.append(.missingExpiry) }
        return errors
    }

    public func makeObject(id: String, signerID: String, now: Date) throws -> PacketObject {
        if let first = validationErrors.first {
            throw first
        }
        return PacketObject(
            id: id,
            kind: .alert,
            title: title.trimmingCharacters(in: .whitespacesAndNewlines),
            body: body.trimmingCharacters(in: .whitespacesAndNewlines),
            createdAt: now,
            expiresAt: expiresAt,
            signerID: signerID,
            sourceNote: sourceNote.trimmingCharacters(in: .whitespacesAndNewlines),
            aiAssisted: aiAssisted
        )
    }
}
```

- [ ] **Step 4: Run test to verify pass**

Run: `swift test --filter AlertDraftTests`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add RiotApp/Authoring/AlertDraft.swift RiotAppTests/AlertDraftTests.swift
git commit -m "feat: validate alert drafts"
```

## Task 4: Local HTML Renderer

**Files:**
- Create: `RiotApp/Rendering/PacketSiteRenderer.swift`
- Test: `RiotAppTests/PacketSiteRendererTests.swift`

- [ ] **Step 1: Write failing render test**

Create `RiotAppTests/PacketSiteRendererTests.swift`:

```swift
import XCTest
@testable import RiotApp

final class PacketSiteRendererTests: XCTestCase {
    func testRendererEscapesAlertContent() {
        let renderer = PacketSiteRenderer()
        let object = PacketObject(
            id: "alert-1",
            kind: .alert,
            title: "<Clinic>",
            body: "Go to A & B.",
            createdAt: Date(timeIntervalSince1970: 100),
            expiresAt: nil,
            signerID: "local",
            sourceNote: "field",
            aiAssisted: false
        )

        let html = renderer.render(objects: [object], packetTitle: "Disaster")

        XCTAssertTrue(html.contains("&lt;Clinic&gt;"))
        XCTAssertTrue(html.contains("Go to A &amp; B."))
        XCTAssertFalse(html.contains("<Clinic>"))
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `swift test --filter PacketSiteRendererTests`

Expected: FAIL because `PacketSiteRenderer` is not defined.

- [ ] **Step 3: Implement renderer**

Create `RiotApp/Rendering/PacketSiteRenderer.swift`:

```swift
import Foundation

public struct PacketSiteRenderer: Sendable {
    public init() {}

    public func render(objects: [PacketObject], packetTitle: String) -> String {
        let cards = objects.map { object in
            """
            <article class="card">
              <p class="kind">\(escape(object.kind.rawValue))</p>
              <h2>\(escape(object.title))</h2>
              <p>\(escape(object.body))</p>
              <footer>Signed by \(escape(object.signerID)) · Source: \(escape(object.sourceNote))</footer>
            </article>
            """
        }.joined(separator: "\n")

        return """
        <!doctype html>
        <html>
        <head>
          <meta charset="utf-8">
          <meta name="viewport" content="width=device-width, initial-scale=1">
          <title>\(escape(packetTitle))</title>
          <style>
            body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; margin: 24px; background: #f7f4ed; color: #161616; }
            .card { border: 1px solid #222; padding: 16px; margin: 0 0 16px; background: #fff; }
            .kind { text-transform: uppercase; font-size: 12px; letter-spacing: 0.08em; }
            footer { font-size: 12px; color: #555; }
          </style>
        </head>
        <body>
          <h1>\(escape(packetTitle))</h1>
          \(cards)
        </body>
        </html>
        """
    }

    private func escape(_ input: String) -> String {
        input
            .replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
            .replacingOccurrences(of: "\"", with: "&quot;")
            .replacingOccurrences(of: "'", with: "&#39;")
    }
}
```

- [ ] **Step 4: Run test to verify pass**

Run: `swift test --filter PacketSiteRendererTests`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add RiotApp/Rendering/PacketSiteRenderer.swift RiotAppTests/PacketSiteRendererTests.swift
git commit -m "feat: render packet objects as local HTML"
```

## Task 5: Archive Export and Import

**Files:**
- Create: `RiotApp/Exchange/PacketArchive.swift`
- Test: `RiotAppTests/PacketArchiveTests.swift`

- [ ] **Step 1: Write failing archive test**

Create `RiotAppTests/PacketArchiveTests.swift`:

```swift
import XCTest
@testable import RiotApp

final class PacketArchiveTests: XCTestCase {
    func testArchiveRoundTripPreservesManifestAndObjects() throws {
        let manifest = PacketManifest(
            id: "disaster",
            title: "Disaster Packet",
            summary: "Local response info",
            namespaceID: "namespace-1",
            createdAt: Date(timeIntervalSince1970: 100),
            updatedAt: Date(timeIntervalSince1970: 200)
        )
        let object = PacketObject(
            id: "alert-1",
            kind: .alert,
            title: "Road closed",
            body: "Use 4th.",
            createdAt: Date(timeIntervalSince1970: 150),
            expiresAt: nil,
            signerID: "local",
            sourceNote: "field",
            aiAssisted: false
        )

        let archive = PacketArchive(manifest: manifest, objects: [object])
        let data = try archive.encode()
        let decoded = try PacketArchive.decode(data)

        XCTAssertEqual(decoded.manifest, manifest)
        XCTAssertEqual(decoded.objects, [object])
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `swift test --filter PacketArchiveTests`

Expected: FAIL because `PacketArchive` is not defined.

- [ ] **Step 3: Implement archive**

Create `RiotApp/Exchange/PacketArchive.swift`:

```swift
import Foundation

public struct PacketArchive: Codable, Equatable, Sendable {
    public let manifest: PacketManifest
    public let objects: [PacketObject]

    public init(manifest: PacketManifest, objects: [PacketObject]) {
        self.manifest = manifest
        self.objects = objects
    }

    public func encode() throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        return try encoder.encode(self)
    }

    public static func decode(_ data: Data) throws -> PacketArchive {
        try JSONDecoder().decode(PacketArchive.self, from: data)
    }
}
```

- [ ] **Step 4: Run test to verify pass**

Run: `swift test --filter PacketArchiveTests`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add RiotApp/Exchange/PacketArchive.swift RiotAppTests/PacketArchiveTests.swift
git commit -m "feat: add packet archive round trip"
```

## Self-Review

Spec coverage:

- Packet creation is covered by Tasks 1 and 2.
- Structured alert authoring is covered by Task 3.
- Local rendering is covered by Task 4.
- Portable import/export scaffold is covered by Task 5.
- Real Willow, Meadowcap, Drop Format, WTP, and LLM integration are intentionally out of scope for this prototype plan and documented as future replacements/extensions.

Placeholder scan:

- The plan contains no placeholder language or unspecified test steps.

Type consistency:

- `PacketObject`, `PacketManifest`, `PacketStore`, `AlertDraft`, `PacketSiteRenderer`, and `PacketArchive` names are consistent across tasks.
