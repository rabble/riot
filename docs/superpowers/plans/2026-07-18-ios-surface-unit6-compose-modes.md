# iOS Surface — Unit 6: Composer mode picker (Update/Alert/Request) + operational fields — Implementation Plan


**Plan-review gate: PENDING** (Feasibility + Scope + Completeness).
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Surface the operational compose modes that already work end-to-end via UniFFI. `PostUpdateView` carries `ComposerMode` (Update / Alert / Request) in its model but never draws the picker — and, worse, the model's validation already *requires* source-claim + expiry + coarse-location when the mode is not Update, while the view renders **no inputs for them**. So a user who could select Alert/Request today would be **permanently dead-disabled** on Post with nothing to satisfy (the gate-r1 blocker). This unit adds (1) the segmented Update/Alert/Request picker, (2) the three operational input fields shown **only** for Alert/Request, and (3) inline `model.validation` guidance so Post is never dead-disabled. Alert and Request stay **user-visibly distinct** (confirmed distinct at the core — see Ground truth).

**Architecture:** Pure-Swift, **view-only**. Every field the modes need (`mode`, `sourceClaims`, `coarseLocation`, `expiresAt`) is already a public `@Published` on `PostUpdateViewModel`, and `validation` + `operationalProfile` already consume them. The model is **not** touched — this unit only draws the inputs that bind to the existing model and renders the validation the model already computes. No new FFI, no `uniffi::Record`, no new file → **no pbxproj change**. Alert/Request route through the already-wired `create_newswire_post` overlay (`RiotProfileRepository.publishNewswirePost` → `createNewswirePost` → `NewswirePostInput`).

**Tech stack:** Swift 6 / SwiftUI (`Picker`, `DatePicker`), XCTest. Design: `docs/superpowers/specs/2026-07-18-ios-surface-built-capabilities-design.md` §6 (incl. gate-r1 correction) + §8 Unit 6 + §10.5 (Alert-vs-Request open question, resolved here).

**Shared-checkout:** this unit adds **no** Swift files → **no `project.pbxproj` edit** on either `apps/ios/Riot.xcodeproj` or `apps/macos/Riot.xcodeproj` (both already reference `PostUpdateView.swift`). It edits two already-registered files: `apps/ios/Riot/PostUpdateView.swift` and `apps/ios/RiotTests/PostUpdateTests.swift`. Still: pathspec commits, absolute `git`/`grep`, and claim those two files in COLLABORATION.md before editing (they are shared across sessions).

---

## Ground truth (verified)

- **`ComposerMode` enum (`PostUpdateView.swift:62-79`).** `Equatable, Sendable, CaseIterable` with three cases and everything the picker needs already on it:
  ```swift
  public enum ComposerMode: Equatable, Sendable, CaseIterable {
      case freeform            // .label == "Update"
      case operationalAlert    // .label == "Alert"
      case operationalRequest  // .label == "Request"
      public var requiresStricterFields: Bool { self != .freeform }
      public var label: String { … "Update" / "Alert" / "Request" }
  }
  ```
  No associated values ⇒ implicitly `Hashable` ⇒ usable as `ForEach(ComposerMode.allCases, id: \.self)` + `.tag(mode)`.
- **The model already owns every field the modes require — nothing to add (`PostUpdateView.swift:266-273`):**
  ```swift
  @Published public var mode: ComposerMode = .freeform          // :270
  @Published public var sourceClaims: [String] = []             // :271
  @Published public var coarseLocation: String = ""             // :272
  @Published public var expiresAt: Date?                        // :273  (starts nil)
  ```
- **`validation` — the required-fields rule (`PostUpdateView.swift:316-329`).** Headline + body are always required; the three operational fields are required **only** when `mode.requiresStricterFields`, and the missing ones come back as human strings:
  ```swift
  public var validation: PostUpdateValidation {
      let hasHeadline = !headline.trimmed.isEmpty
      let hasBody = !body.trimmed.isEmpty
      guard hasHeadline, hasBody else { return .needsHeadlineAndBody }
      if mode.requiresStricterFields {
          var missing: [String] = []
          if trimmedSourceClaims.isEmpty { missing.append("a source claim") }
          if expiresAt == nil { missing.append("an expiry") }
          if coarseLocation.trimmed.isEmpty { missing.append("a coarse location") }
          if !missing.isEmpty { return .needsOperationalFields(missing) }
      }
      return .ready
  }
  ```
  Exact per-mode requirement: **Update** = headline + body only. **Alert** and **Request** = headline + body **+ a source claim + an expiry (`expiresAt != nil`) + a coarse location**. `PostUpdateValidation.needsOperationalFields([String])` is defined at `PostUpdateView.swift:86-93`.
- **`canPost` (`PostUpdateView.swift:332-340`)** = `validation.isReady && status == .editing`. So the picker's whole job on the Post button is: keep `validation` reachable to `.ready`. Today, with no inputs, Alert/Request can **never** reach `.ready` → dead-disable. That is the bug this unit closes.
- **`operationalProfile` — Alert and Request are DISTINCT (`PostUpdateView.swift:419-437`):**
  ```swift
  private var operationalProfile: NewswireOperationalProfile? {
      switch mode {
      case .freeform:           return nil
      case .operationalAlert:   return .alert(profile: NewswireAlertProfile(urgency: .immediate, severity: .severe, certainty: .observed, validFromUnixSeconds: nil))
      case .operationalRequest: return .request(profile: NewswireRequestProfile(kind: .need, neededByUnixSeconds: expiresAt.map { … }, contactInstructions: coarseLocation.trimmed))
      }
  }
  ```
  These map to **different core enum cases** — `crates/riot-core/src/newswire/model.rs:66-70` `OperationalProfileV1 { Alert(AlertProfileV1), Request(RequestProfileV1) }`, surfaced across FFI as `NewswireOperationalProfile { Alert { profile }, Request { profile } }` (`crates/riot-ffi/src/newswire_ffi.rs:80-82`). Alert carries urgency/severity/certainty; Request carries `kind` (`Need`/`Offer`) + `neededBy` + `contactInstructions`. **Distinctness answer: they are genuinely distinct operational post kinds — DO NOT collapse (resolves design §10.5).** The finer knobs (urgency/severity/certainty, Need-vs-Offer) stay **defaulted** per the model's own note (`PostUpdateView.swift:415-418`: "Full alert/request authoring … is a later refinement"); this unit ships only the three fields validation actually requires — exactly what unblocks Post.
- **The build-of the request + submit path (already wired, unchanged by this unit).** `post()` (`PostUpdateView.swift:383-409`) builds `PostUpdateRequest` from those fields (`sourceClaims`/`coarseLocation` gated on `mode.requiresStricterFields`, `expiresAtUnixSeconds` from `expiresAt`, `operationalProfile:` from the computed var above) → `publisher.publishNewswirePost(request)`. The seam lands in `RiotProfileRepository.publishNewswirePost` (`PostUpdateView.swift:163-177`) → `createNewswirePost(…)` (`Core/ProfileRepository.swift:1028-1052`) → `profile.createNewswirePost(input: NewswirePostInput(… operationalProfile: … sourceClaims: … expiresAtUnixSeconds: … coarseLocation: …))`. **`NewswirePostInput` carries the overlay fields** (operationalProfile, sourceClaims, expiresAtUnixSeconds, coarseLocation) — no new FFI needed.
- **The view today has NO operational inputs and NO picker.** `draftCard` (`PostUpdateView.swift:499-515`) renders exactly three controls: headline `TextField` (`post-headline`), body `TextField` (`post-body`), AI-assist `Toggle` (`post-ai-assist`). `reviewCard` (`:517-547`) shows the Post button `.disabled(!model.canPost)` with **no** inline explanation of *why* it is disabled — a silent dead-disable for Alert/Request. `body` (`:477-497`) stacks `draftCard`, `reviewCard`, optional `failureCard`.
- **Existing composer test drives the model directly (`RiotTests/PostUpdateTests.swift`).** `makeModel(...)` (`:57-69`) builds a `PostUpdateViewModel` with a `StubPublisher` (records `callCount` + `lastRequest`) and a `MemoryDraftStore`. `testOperationalProfileRequiresStricterFields` (`:225-256`) is the exact pattern to mirror: set `model.mode = .operationalAlert`, assert `case .needsOperationalFields` + `!model.canPost`, then set `model.sourceClaims`/`model.coarseLocation`/`model.expiresAt`, assert `.ready`, `post()`, assert `publisher.callCount == 1` and `publisher.lastRequest?.operationalProfile != nil`. **No FFI in these tests** — pure view-model.
- **No new file ⇒ no pbxproj change.** `PostUpdateView.swift` is already registered in both projects; this unit only edits it and its test. Confirmed: nothing to add to `apps/ios/Riot.xcodeproj/project.pbxproj` or `apps/macos/Riot.xcodeproj/project.pbxproj`.

---

## Task 1: Mode picker — draw it + bind `model.mode`

**Files:** Modify `apps/ios/Riot/PostUpdateView.swift`; Test `apps/ios/RiotTests/PostUpdateTests.swift`

- [ ] **Step 1: Failing test.** The picker exposure is verified through model state + the mode's label contract (the view binds `$model.mode`; the label copy is pinned so it can't regress to mechanism):
```swift
// MARK: - Mode picker

@MainActor
func testModeSelectionSwitchesTheModel() {
    let model = makeModel()
    XCTAssertEqual(model.mode, .freeform, "the composer opens in Update mode")

    model.mode = .operationalAlert
    XCTAssertTrue(model.mode.requiresStricterFields)

    model.mode = .operationalRequest
    XCTAssertTrue(model.mode.requiresStricterFields)

    model.mode = .freeform
    XCTAssertFalse(model.mode.requiresStricterFields, "Update pulls in no extra fields")
}

@MainActor
func testModeLabelsAreOutcomeLanguageNotMechanism() {
    XCTAssertEqual(ComposerMode.freeform.label, "Update")
    XCTAssertEqual(ComposerMode.operationalAlert.label, "Alert")
    XCTAssertEqual(ComposerMode.operationalRequest.label, "Request")
    XCTAssertEqual(ComposerMode.allCases.count, 3, "the picker offers exactly Update/Alert/Request")
}
```

- [ ] **Step 2: Run → FAIL** only if a label/case regresses; if these pass immediately (contract already holds) that is expected — they lock the picker's data model before the view work. Run:
  `/usr/bin/xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -only-testing:RiotTests/PostUpdateTests`

- [ ] **Step 3: Implement — add a `modeCard` and place it first in `body`.** Insert a new card private var and add it at the top of the `VStack` in `body` (`PostUpdateView.swift:479`, before `draftCard`):
```swift
// in body's VStack(spacing: 16) { … } — first child:
modeCard
draftCard
// … reviewCard, failureCard unchanged
```
```swift
private var modeCard: some View {
    RiotCard {
        VStack(alignment: .leading, spacing: 10) {
            eyebrow("What kind of post")
            Picker("Post kind", selection: $model.mode) {
                ForEach(ComposerMode.allCases, id: \.self) { mode in
                    Text(mode.label).tag(mode)
                }
            }
            .pickerStyle(.segmented)
            .accessibilityIdentifier("post-mode-picker")
        }
    }
}
```
`eyebrow(_:)` and `RiotCard` already exist in this file (`:558-564`, used by `draftCard`). `ForEach(…, id: \.self)` is valid because `ComposerMode` is `Hashable` (no associated values).

- [ ] **Step 4: Run → PASS.** **Step 5: Commit** `apps/ios/Riot/PostUpdateView.swift` + `apps/ios/RiotTests/PostUpdateTests.swift` (pathspec).

---

## Task 2: Operational input fields — shown ONLY for Alert/Request, bound to the model

**Files:** Modify `apps/ios/Riot/PostUpdateView.swift`; Test `apps/ios/RiotTests/PostUpdateTests.swift`

- [ ] **Step 1: Failing test.** The fields appear/hide with the mode, and setting them through the model reaches `.ready` (the view's bindings write these same properties). The appear/hide contract is asserted via a view-model-visible flag the view reads:
```swift
// MARK: - Operational fields visibility

@MainActor
func testOperationalFieldsAreHiddenForUpdateAndShownForAlertAndRequest() {
    let model = makeModel()
    XCTAssertFalse(model.mode.requiresStricterFields, "Update: no operational fields")

    model.mode = .operationalAlert
    XCTAssertTrue(model.mode.requiresStricterFields, "Alert: operational fields shown")

    model.mode = .operationalRequest
    XCTAssertTrue(model.mode.requiresStricterFields, "Request: operational fields shown")
}

@MainActor
func testOperationalFieldBindingsFeedValidationAndTheSignedWrite() {
    let publisher = StubPublisher()
    let model = makeModel(publisher: publisher)
    model.headline = "Tear gas at the south barricade"
    model.body = "Move north; medics are staging by the fountain."
    model.mode = .operationalAlert

    // Fields empty → not ready.
    guard case .needsOperationalFields = model.validation else {
        return XCTFail("empty operational fields must not validate")
    }

    // The three inputs the view binds.
    model.sourceClaims = ["Saw it myself"]     // source-claim field
    model.coarseLocation = "South barricade"   // coarse-location field
    model.expiresAt = Date(timeIntervalSince1970: 1_720_003_600)  // expiry picker

    XCTAssertEqual(model.validation, .ready)
    model.post()
    XCTAssertEqual(publisher.lastRequest?.sourceClaims, ["Saw it myself"])
    XCTAssertEqual(publisher.lastRequest?.coarseLocation, "South barricade")
    XCTAssertEqual(publisher.lastRequest?.expiresAtUnixSeconds, 1_720_003_600)
}
```

- [ ] **Step 2: Run → FAIL** if any binding property is mis-typed; otherwise these lock the contract the view must honour. Run the same `-only-testing:RiotTests/PostUpdateTests`.

- [ ] **Step 3: Implement — an `operationalCard`, rendered only when `model.mode.requiresStricterFields`.** Insert it between `draftCard` and `reviewCard` in `body`:
```swift
draftCard
if model.mode.requiresStricterFields { operationalCard }
reviewCard
```
```swift
// A single-source-claim binding onto the model's [String] (finer multi-source
// authoring is a later refinement; validation needs one non-empty claim).
private var sourceClaimBinding: Binding<String> {
    Binding(
        get: { model.sourceClaims.first ?? "" },
        set: { model.sourceClaims = $0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? [] : [$0] }
    )
}

// The expiry starts unset (model.expiresAt == nil) so Alert/Request are honestly
// incomplete until the person sets one. A toggle reveals the picker; turning it
// off clears the expiry back to nil (validation fails again — no silent default).
private var hasExpiryBinding: Binding<Bool> {
    Binding(
        get: { model.expiresAt != nil },
        set: { model.expiresAt = $0 ? (model.expiresAt ?? now()) : nil }
    )
}

private func now() -> Date { Date() }

private var operationalCard: some View {
    RiotCard {
        VStack(alignment: .leading, spacing: 14) {
            eyebrow(model.mode == .operationalAlert ? "Alert details" : "Request details")
            TextField("Source (how you know)", text: sourceClaimBinding, axis: .vertical)
                .font(.riot(.body, size: 15, relativeTo: .body))
                .accessibilityIdentifier("post-source-claim")
            TextField("Coarse location (area, not a precise point)", text: $model.coarseLocation)
                .font(.riot(.body, size: 15, relativeTo: .body))
                .accessibilityIdentifier("post-coarse-location")
            Toggle("Set an expiry", isOn: hasExpiryBinding)
                .tint(RiotTheme.pink(for: colorScheme))
                .accessibilityIdentifier("post-expiry-toggle")
            if let _ = model.expiresAt {
                DatePicker(
                    "Expires",
                    selection: Binding(get: { model.expiresAt ?? now() }, set: { model.expiresAt = $0 }),
                    displayedComponents: [.date, .hourAndMinute]
                )
                .accessibilityIdentifier("post-expiry-picker")
            }
        }
    }
}
```
Notes: `coarseLocation` binds directly to `$model.coarseLocation` (already a `String`). `sourceClaims`/`expiresAt` need the small computed bindings above because the model types are `[String]` / `Date?` and the model is intentionally **not** changed. `RiotTheme.pink(for:)` + `colorScheme` are already used by `draftCard` (`:511`, `:468`).

- [ ] **Step 4: Run → PASS.** **Step 5: Commit** both files (pathspec).

---

## Task 3: Inline validation messaging — Post never dead-disabled (THE gate-r1 blocker)

**Files:** Modify `apps/ios/Riot/PostUpdateView.swift`; Test `apps/ios/RiotTests/PostUpdateTests.swift`

- [ ] **Step 1: Failing test (RED→GREEN).** The KEY assertion of the unit: selecting Alert with empty fields disables Post **and surfaces guidance naming the missing fields**, and filling them enables Post. Add a small view-model-visible guidance string so the copy is testable without SwiftUI:
```swift
// MARK: - Post is never dead-disabled (gate-r1 blocker)

@MainActor
func testAlertWithEmptyFieldsDisablesPostButShowsActionableGuidance() {
    let model = makeModel()
    model.headline = "Headline"
    model.body = "Body"
    // Update mode: ready, no guidance.
    XCTAssertTrue(model.canPost)
    XCTAssertNil(model.validationGuidance)

    // Alert with nothing supplied: disabled, but NOT silently — guidance lists what's missing.
    model.mode = .operationalAlert
    XCTAssertFalse(model.canPost)
    let guidance = try? XCTUnwrap(model.validationGuidance)
    XCTAssertTrue(guidance?.contains("source") ?? false)
    XCTAssertTrue(guidance?.contains("expiry") ?? false)
    XCTAssertTrue(guidance?.contains("location") ?? false)
}

@MainActor
func testSupplyingOperationalFieldsEnablesPostAndClearsGuidance() {
    let model = makeModel()
    model.headline = "Headline"
    model.body = "Body"
    model.mode = .operationalRequest
    XCTAssertFalse(model.canPost)

    model.sourceClaims = ["A neighbour told me"]
    model.coarseLocation = "North gate"
    model.expiresAt = Date(timeIntervalSince1970: 1_720_003_600)

    XCTAssertTrue(model.canPost, "Alert/Request must never strand Post once its fields are supplied")
    XCTAssertNil(model.validationGuidance, "no guidance once ready")
}
```

- [ ] **Step 2: Run → FAIL** (`validationGuidance` undefined). Run `-only-testing:RiotTests/PostUpdateTests`.

- [ ] **Step 3: Implement.**
  - Add a **read-only computed** on `PostUpdateViewModel` that renders the existing `validation` as guidance copy (this is presentation of already-computed state, not new business logic — it stays in the view model so it is unit-testable). Place next to `validation`/`canPost` (~`PostUpdateView.swift:340`):
```swift
/// Plain-language guidance for why Post is disabled, or nil when ready. So the
/// composer explains what's still needed instead of a silent dead-disable —
/// the exact stranding an operational mode would otherwise cause.
public var validationGuidance: String? {
    switch validation {
    case .ready:
        return nil
    case .needsHeadlineAndBody:
        return "Add a headline and body to post."
    case let .needsOperationalFields(missing):
        // missing is already human: "a source claim", "an expiry", "a coarse location".
        return "To post \(mode.label.lowercased()), add \(missing.joined(separator: ", "))."
    }
}
```
  - Render it in `reviewCard`, just above the Post button (`PostUpdateView.swift:539`, the `else` branch that draws the button when not `.posted`):
```swift
} else {
    if let guidance = model.validationGuidance {
        Text(guidance)
            .font(.riot(.body, size: 13, relativeTo: .footnote))
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            .accessibilityIdentifier("post-validation-guidance")
    }
    Button(PostUpdateViewModel.primaryActionTitle, action: model.post)
        .buttonStyle(.riotPrimary)
        .accessibilityIdentifier("post-update")
        .disabled(!model.canPost)
}
```
  This guarantees: whenever `canPost` is false in the editing state, a person sees *why* and *what to add* — Post is disabled but never dead (no path to enable). `needsOperationalFields`'s human strings ("a source claim", "an expiry", "a coarse location") satisfy the `contains("source"/"expiry"/"location")` assertions verbatim.

- [ ] **Step 4: Run → PASS.** **Step 5: Commit** both files (pathspec).

---

## Task 4: Alert vs Request produce distinct operational posts (design §10.5 resolved)

**Files:** Test `apps/ios/RiotTests/PostUpdateTests.swift` (no source change — asserts existing distinct mapping)

- [ ] **Step 1: Failing test.** Prove the two modes emit **different** core overlay cases end-to-end through the signed write:
```swift
// MARK: - Alert vs Request are distinct outcomes (design §10.5)

@MainActor
private func postWith(mode: ComposerMode) -> NewswireOperationalProfile? {
    let publisher = StubPublisher()
    let model = makeModel(publisher: publisher)
    model.headline = "H"; model.body = "B"; model.mode = mode
    model.sourceClaims = ["Saw it"]; model.coarseLocation = "The plaza"
    model.expiresAt = Date(timeIntervalSince1970: 1_720_003_600)
    XCTAssertTrue(model.canPost)
    model.post()
    return publisher.lastRequest?.operationalProfile
}

@MainActor
func testAlertAndRequestProduceDistinctOperationalProfiles() {
    let alert = postWith(mode: .operationalAlert)
    let request = postWith(mode: .operationalRequest)

    guard case .alert = alert else { return XCTFail("Alert mode must emit an .alert overlay") }
    guard case .request = request else { return XCTFail("Request mode must emit a .request overlay") }
    XCTAssertNotEqual(alert, request, "Alert and Request must not collapse to the same core post")

    // Update stays freeform — no overlay at all.
    XCTAssertNil(postWith(mode: .freeform))  // note: .freeform is postable with just H/B; fields ignored
}
```
  > Adjust the `.freeform` line: for `.freeform` the operational fields are irrelevant, so build that case with only headline+body. Keep the two distinct-case asserts as the core of the test.

- [ ] **Step 2: Run → FAIL** only if the mapping is broken; it should **PASS** immediately, confirming the pre-existing `operationalProfile` mapping already yields distinct `.alert`/`.request` cases. This test **locks** that distinctness so a future refactor can't silently collapse the two modes. Run `-only-testing:RiotTests/PostUpdateTests`.

- [ ] **Step 3: Implement — none.** The distinctness is already correct in `operationalProfile` (`PostUpdateView.swift:419-437`); this task is a regression lock, not a change. **Distinctness decision recorded:** Alert ⇒ `NewswireOperationalProfile.alert` (urgency/severity/certainty); Request ⇒ `.request` (kind Need/Offer, neededBy, contact). They are **distinct** — do NOT collapse (design §10.5 open question resolved).

- [ ] **Step 4: Run → PASS.** **Step 5: Commit** the test (pathspec).

---

## Task 5: Build both platforms + full composer suite

- [ ] iOS: `/usr/bin/xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -only-testing:RiotTests/PostUpdateTests` — all new + existing `PostUpdateTests` green.
- [ ] iOS app + macOS app **BUILD SUCCEEDED** — the picker/`DatePicker`/`Toggle` are cross-platform SwiftUI (no `#if os(iOS)` needed; `.pickerStyle(.segmented)` and `DatePicker` compile on macOS). Confirm the macOS `RiotKit` build:
  `/usr/bin/xcodebuild build -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS` (or the repo's macOS scheme name).
- [ ] Confirm **no pbxproj change** was made: `/usr/bin/git status --porcelain apps/ios/Riot.xcodeproj/project.pbxproj apps/macos/Riot.xcodeproj/project.pbxproj` prints nothing.
- [ ] Commit any fixups (pathspec).

---

## Self-Review

- **§6 requirement → task mapping:**
  - "Add a segmented Update/Alert/Request control at the composer top" → **Task 1** (`modeCard`, `post-mode-picker`, bound to `$model.mode`).
  - "also adds the operational inputs — a source-claim field, an expiry picker, and a coarse-location field — shown only when Alert/Request is selected" → **Task 2** (`operationalCard`, gated on `model.mode.requiresStricterFields`; `post-source-claim`/`post-coarse-location`/`post-expiry-picker`).
  - "plus inline `model.validation` messaging (‘add a source and expiry to post an alert’)" and "Update mode is unchanged (no extra fields)" → **Task 3** (`validationGuidance`, `post-validation-guidance`; Update path shows no operational card and no operational guidance).
  - "Alert vs Request must be user-visibly distinct … If they resolve to the same operational post kind at the core, collapse … Confirm the distinct outcome" → **Task 4** + Ground truth: **CONFIRMED DISTINCT** (`OperationalProfileV1::Alert` vs `::Request`, different associated payloads) → **do NOT collapse**; §10.5 open question resolved.
  - "Alert/Request route through `create_newswire_post` with the operational overlay" → already wired (`post()` → `publishNewswirePost` → `createNewswirePost` → `NewswirePostInput.operationalProfile`); no FFI added (§8 Unit 6 "No").
- **THE key assertion — "Alert/Request no longer strands Post" (gate-r1 blocker):** `testAlertWithEmptyFieldsDisablesPostButShowsActionableGuidance` (disabled + guidance naming source/expiry/location) → `testSupplyingOperationalFieldsEnablesPostAndClearsGuidance` (enabled once supplied). Before this unit, Alert/Request could reach `.needsOperationalFields` but the view rendered **no inputs** to satisfy it → permanent dead-disable. Task 2 supplies the inputs; Task 3 supplies the explanation. Blocker closed.
- **Placeholder scan:** all Swift is real and bindable to existing public model members (`mode`/`sourceClaims`/`coarseLocation`/`expiresAt`), which already exist and are already consumed by `validation`/`operationalProfile`. The only non-source-quoted sketch is the macOS scheme *name* in Task 5 (`RiotKit-macOS`) — the implementer confirms the exact scheme from `xcodebuild -list -project apps/macos/Riot.xcodeproj`. No TODO/`fatalError`/stub left.
- **Scope discipline (no gold-plating):** urgency/severity/certainty and Need-vs-Offer authoring are **out of scope** (kept at the model's existing defaults per `PostUpdateView.swift:415-418`); this unit ships only the three fields `validation` actually requires — the minimum that unblocks Post. Multi-source authoring is a single-claim binding for now (noted inline).
- **Model untouched:** the `PostUpdateViewModel` gains **one read-only presentation computed** (`validationGuidance`) and **no** stored state or behaviour change — validation, `canPost`, `operationalProfile`, and `post()` are unchanged. All existing `PostUpdateTests` (freeform validation, one-signed-write, draft persistence, failure copy) stay green.
- **Type consistency:** `ComposerMode`/`PostUpdateValidation`/`NewswireOperationalProfile`/`NewswireAlertProfile`/`NewswireRequestProfile` used exactly as defined in `PostUpdateView.swift` + the generated bindings. `ForEach(ComposerMode.allCases, id: \.self)` relies on the enum's implicit `Hashable` (no associated values) — verified.
- **No new file ⇒ no pbxproj edit ⇒ no cross-project serialization risk.** Edits are confined to `PostUpdateView.swift` + `PostUpdateTests.swift`, both already registered in both projects. Task 5 asserts the pbxproj files stay clean.
- **Dependency order:** T1 (picker) → T2 (fields the picker reveals) → T3 (guidance over T1+T2 state) → T4 (distinctness lock) → T5 (build). All within one pure-Swift unit.
