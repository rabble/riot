# iOS Surface — Unit 4b: editor un-gate (Swift consumer of `newswire_is_editor`) — Implementation Plan


**Plan-review gate: PASSED (Feasibility + Scope + Completeness, 2026-07-18).
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Replace the session-only editorial-visibility hack with the descriptor-authenticated FFI predicate, so `EditorialActionSheet` controls appear for a real editor of *any* active community — **joined or created** — and the display gate is provably identical to the core authority gate. A community whose descriptor has not yet synced shows an honest "controls appear after this community's first sync" note instead of silently nothing. Delete the two coexisting roster-authority sources so only one remains.

**Architecture:** `NewswireSurfaceModel.canOfferEditorialControls` stops consulting the local `EditorialAuthority.isRecognizedEditor(myKeyHex:roster:)` static (which never talks to core and treats an empty roster as founder-true — *diverging from core*, design §4 CORRECTED) and instead reads a cached `isEditor` computed once in `load()` from the new FFI predicate `newswireIsEditor(spaceDescriptorEntryID:, subjectID:)` — the SAME roster authority the core enforces at admission (Unit 4a's shared `is_editorial_authority`). The predicate returns `false` (never an error) for an unknown / not-yet-synced descriptor, so the model renders a one-line pending note instead of a bare empty view. The model gains an injectable `authority: NewswireEditorAuthorityChecking` seam (mirrors the existing `NewswireEditorialActing` seam) so it stays unit-testable and `RiotProfileRepository` supplies the live implementation. The dead `CommunityContext.editorialRoster` field + its create-time population and the `EditorialAuthority` enum are removed — one roster-authority source, not two.

**Tech stack:** Swift 6 / SwiftUI, XCTest, real UniFFI (`MobileProfile.newswireIsEditor`, from Unit 4a). Design: `docs/superpowers/specs/2026-07-18-ios-surface-built-capabilities-design.md` §4 (esp. "4a implementation requirements" + the CORRECTED empty-roster semantic) + §8 Unit 4b + §9.

---

## HARD DEPENDENCY — Unit 4b lands AFTER Unit 4a's binding regen

**Unit 4b cannot go green until Unit 4a lands.** Unit 4a (`docs/superpowers/plans/2026-07-18-ios-surface-unit4a-newswire-is-editor.md`) adds the FFI method `MobileProfile.newswireIsEditor(descriptorEntryId:, subjectId:) -> Bool` via a **coordinated binding regen + native staticlib rebuild** (checksum-coupling discipline). Until that regen is committed:

- `try profile.newswireIsEditor(...)` does **not exist** in the generated `riot_ffi.swift` → 4b will not compile.
- Every 4b test that exercises the predicate against real FFI will fail to build, not merely fail red.

**Do NOT start 4b Task 1 Step 3 (the wrapper) until 4a's regenerated binding is on the branch and an iOS `RiotTests` smoke call `try profile.newswireIsEditor(...)` compiles + loads without a checksum abort** (that smoke is 4a Task 3 Step 2). Sequence: **4a merged (regen + staticlib) → 4b**. This dependency is called out again in the Self-Review.

**Shared-checkout:** claim `apps/ios/Riot/NewswireEditorial.swift`, `apps/ios/Riot/CommunityShell.swift`, `apps/ios/Riot/ConferenceShellView.swift`, `apps/ios/Riot/Core/ProfileRepository.swift`, `apps/ios/RiotTests/NewswireSurfaceTests.swift` in COLLABORATION.md before editing; pathspec commits only; absolute `git`/`grep`. 4b adds **no new Swift file** → **no pbxproj edit** (unlike Unit 1); it edits existing registered files only.

---

## Cross-unit coupling (gate r1) — Unit 4b vs Unit 7

**Unit 4b and Unit 7 both edit the same three surfaces** — `NewswireSurfaceModel.init` (`apps/ios/Riot/NewswireEditorial.swift`), the shell construction (`apps/ios/Riot/ConferenceShellView.swift:305-312`), and `apps/ios/RiotTests/NewswireSurfaceTests.swift`. Unit 7 (dead-end fixes) adds a `descriptorResolver:` init parameter (the `offlineStale` "re-derive the descriptor id" path, design §7). **Agreed reconciliation (Unit 7's writer is adding the mirror of this note):**

- **Land order: 4b BEFORE 7.**
- **Final `NewswireSurfaceModel.init` carries BOTH new params:** `authority:` (4b, replacing `roster:`) **and** `descriptorResolver:` (Unit 7). 4b drops `roster:` + adds `authority:`; Unit 7 then adds `descriptorResolver:` on top of 4b's signature.
- **Shell construction (`ConferenceShellView.swift:305-312`):** 4b passes `authority:` (predicate-driven, **no `roster:`**); Unit 7 later adds `descriptorResolver:`. The merged call carries both, never `roster:`.
- **Whoever lands updates ALL init call sites** to the then-current signature: 4b converts `liveModel` (`NewswireSurfaceTests.swift:83-98`) + the constructions at `:242` and `:252` (see Task 2 Step 0); Unit 7 re-updates every call site again when it adds `descriptorResolver:`.
- **Serialize on the files:** claim `NewswireEditorial.swift` / `ConferenceShellView.swift` / `NewswireSurfaceTests.swift` (and `CommunityShell.swift`) in COLLABORATION.md; **4b and 7 must NOT run concurrently** on these files.

---

## Ground truth (verified)

- **The session-only hack to REPLACE** (`apps/ios/Riot/NewswireEditorial.swift:198-213`):
  ```swift
  public enum EditorialAuthority {
      /// ... An UNKNOWN roster is never an editor here ... An EMPTY roster means
      /// core's default — the founder alone — so the founder is an editor. ...
      public static func isRecognizedEditor(myKeyHex: String, roster: [String]?) -> Bool {
          let me = myKeyHex.lowercased()
          if me.isEmpty { return false }
          guard let roster else { return false }
          if roster.isEmpty { return true }               // <-- founder-true; DIVERGES from core (design §4)
          return roster.contains { $0.lowercased() == me }
      }
  }
  ```
  Its comment claims "core's default — the founder alone — so the founder is an editor" for an empty roster. Per design §4 CORRECTED, the core admission gate has **no founder special-case for a literally-empty stored roster** (display == authority); this static's `if roster.isEmpty { return true }` is the exact divergence 4b removes. (Note: an empty *create input* is not necessarily an empty *stored* roster — Unit 4a owns that semantic; 4b just consumes the predicate, so it inherits whatever admission does, by construction.)

- **Where visibility is gated** (`NewswireEditorial.swift:519-522`):
  ```swift
  public var canOfferEditorialControls: Bool {
      !spaceDescriptorEntryID.isEmpty
          && EditorialAuthority.isRecognizedEditor(myKeyHex: myKeyHex, roster: roster)
  }
  ```
  Consumed once in the view, `NewswireEditorial.swift:741`:
  ```swift
  if model.canOfferEditorialControls {
      Button("Editorial action") { actionTarget = post } ...
  }
  ```

- **The model** (`NewswireEditorial.swift:479-514`) is `@MainActor public final class NewswireSurfaceModel: ObservableObject`, init takes `projector:`, `editor:`, `spaceDescriptorEntryID:`, `communityName:`, `myKeyHex:`, `roster: [String]?`, `initialDraftKind:`; `private let myKeyHex: String`, `private let roster: [String]?`. `load()` (`:526-544`) projects the wire; a missing descriptor id or a projection failure → `wire = .offlineStale` (no fabricated wire).

- **The signing seam to MIRROR** (`NewswireEditorial.swift:272-281`):
  ```swift
  public protocol NewswireEditorialActing {
      @discardableResult
      func createNewswireEditorialAction(spaceDescriptorEntryID: String, targetEntryID: String,
          kind: NewswireEditorialActionKind, reason: String?, correctionText: String?) throws -> NewswireSignedRecord
  }
  ```
  `RiotProfileRepository` conforms via an empty extension (`ProfileRepository.swift:1136`): `extension RiotProfileRepository: NewswireEditorialActing {}`.

- **Model construction** (`ConferenceShellView.swift:303-312`):
  ```swift
  let wireProjector: NewswireProjecting = model.profileRepository ?? UnavailableWireProjector()
  let editor: NewswireEditorialActing = model.profileRepository ?? UnavailableEditor()
  _newswire = StateObject(wrappedValue: NewswireSurfaceModel(
      projector: wireProjector, editor: editor,
      spaceDescriptorEntryID: community.newswireDescriptorEntryID ?? "",
      communityName: community.name,
      myKeyHex: me.id,                          // <-- already the REAL whoami hex
      roster: community.editorialRoster))       // <-- the dead session-only source
  ```
  `Unavailable*` stubs (`ConferenceShellView.swift:807-829`) throw `RepositoryError.profileClosed`; e.g. `UnavailableEditor`.

- **The dead session-only roster source — `CommunityContext.editorialRoster`** (`CommunityShell.swift:28-52`): `public let editorialRoster: [String]?` (comment: "known only for a community created on this device; a joined or loaded community carries `nil`"); init default `editorialRoster: [String]? = nil`. **Populated at create time** by `CommunityCreationCoordinator.create` (`CommunityShell.swift:395-414`):
  ```swift
  return CommunityContext(name: space.title, namespaceID: space.namespaceID,
      newswireDescriptorEntryID: record.entryId, isOrganizer: true,
      editorialRoster: request.editorialRoster)   // <-- CommunityShell.swift:412, the create-time population to DELETE
  ```
  **Consumed** only at `ConferenceShellView.swift:311` (`roster: community.editorialRoster`).

- **⚠️ Design line-number correction (verified — do NOT delete `ConferenceShellView.swift:98`):** design §4/§8 attributes the "`CommunityContext.editorialRoster` create-time population" to `ConferenceShellView.swift:98`. Line 98 is actually a **different** thing — the *founding roster fed into core*, which MUST be KEPT:
  ```swift
  // ConferenceShellView.swift:90-100
  Button("Create a community") {
      model.createCommunity(CommunityCreationRequest(
          name: trimmedCommunity,
          editorialRoster: model.me.map { [$0.id] } ?? []   // <-- :98 seeds CORE's roster; KEEP
      ))
  }
  ```
  This `CommunityCreationRequest.editorialRoster` flows to `createNewswireCommunity` → `createNewswireSpace(editorialRoster:)` (`AppModel.swift:772`, `ProfileRepository.swift:1119-1128`) and lands in the signed descriptor's stored roster — **this is exactly what makes the founder an editor in core and what the FFI predicate reads back.** Deleting it would make every created community effectively single-editor/unfounded. The dead *display* source to delete is `CommunityContext.editorialRoster` (`CommunityShell.swift:34/45/51/412`) + its consumption (`ConferenceShellView.swift:311`), **not** line 98.

- **The wrapper to MIRROR** (`ProfileRepository.swift:1093-1099`):
  ```swift
  func newswireShareReference(spaceDescriptorEntryID: String) throws -> NewswireShareReference {
      try profile.newswireShareReference(spaceDescriptorEntryId: spaceDescriptorEntryID)
  }
  ```
  `me()` / whoami hex (`ProfileRepository.swift:768-769`, `:824-829`): `func me() throws -> RiotPerson { RiotPerson(try profile.profile().whoami()) }`; `RiotPerson.id == RiotDirectoryRow.hex(who.id)` (lowercase hex of the 32-byte subspace id). `MobileProfile.whoami() -> WhoAmI` exists (`crates/riot-ffi/src/profile_ffi.rs:99`), `WhoAmI.id` is the raw 32 bytes; `RiotDirectoryRow.hex(_ bytes: Data) -> String` (`apps/ios/Riot/Directory/DirectoryModel.swift:68`) is the test-side hex.

- **Test harness** (`apps/ios/RiotTests/NewswireSurfaceTests.swift`): hostless XCTest, `@testable import RiotKit`, real FFI via `openLocalProfile()`. `spaceInput(_, roster:)` (`:55`), `postInput(_, _)` (`:66`), `liveModel(profile:spaceID:roster:myKeyHex:)` (`:83-98`, currently `myKeyHex: "aa".repeated(32)` — a FAKE key). `LiveNewswire` (`:38-52`) wraps a `MobileProfile` and conforms to the projector + editor seams. `openLocalProfile()` returns a `MobileProfile` on which `try profile.newswireIsEditor(...)` (Unit 4a) is callable directly.

- **The defense-in-depth analog to RETAIN + quote** (`NewswireSurfaceTests.swift:363-383`) — core rejects a non-editor's action *and the effect is absent*, proving the display gate is not the security boundary:
  ```swift
  func testANonEditorsActionIsIgnoredTheEffectIsAbsentNotJustTheControl() throws {
      let stranger = "11".repeated(32)
      let profile = try openLocalProfile()
      let space = try profile.createNewswireSpace(input: spaceInput("Delegated", roster: [stranger]))
      let post = try profile.createNewswirePost(input: postInput(space.entryId, "Standing report"))
      let model = liveModel(profile: profile, spaceID: space.entryId, roster: [stranger])
      model.draft = EditorialActionDraft(kind: .hide, reason: "I want this gone")
      let outcome = model.sign(targetEntryID: post.entryId)
      XCTAssertEqual(outcome, .rejected)                       // core refused to sign
      XCTAssertEqual(model.draft.reason, "I want this gone")   // draft preserved
      let projection = try profile.projectNewswireSpace(spaceDescriptorEntryId: space.entryId)
      let row = try XCTUnwrap(projection.openWire.first { $0.entryId == post.entryId })
      XCTAssertEqual(row.treatment, .ordinary, "a non-editor's hide must not hide the post")
      XCTAssertEqual(row.headline, "Standing report", "the payload must survive an unauthorized hide")
  }
  ```

---

## Task 1: `newswireIsEditor` wrapper + authority seam (ProfileRepository)

**Files:** Modify `apps/ios/Riot/Core/ProfileRepository.swift`, `apps/ios/Riot/NewswireEditorial.swift` (add the protocol); Test `apps/ios/RiotTests/NewswireSurfaceTests.swift` (a new repository-level case).

> **Precondition:** Unit 4a's regenerated binding is on the branch (see HARD DEPENDENCY). Confirm `try openLocalProfile().newswireIsEditor(descriptorEntryId:"", subjectId:"")` compiles before writing Step 3.

- [ ] **Step 1: Write the failing test.** Add to `NewswireSurfaceTests`:
```swift
func testRepositoryWrapperMatchesTheCoreAuthorityForMemberAndNonMember() throws {
    let profile = try openLocalProfile()
    let repo = RiotProfileRepository(profile)                       // mirror the repo's test ctor used elsewhere
    let mineHex = RiotDirectoryRow.hex(try profile.whoami().id)     // the founder's real subspace id
    // Founding roster = [me] (what ConferenceShellView:98 seeds): I am an editor of my own community.
    let space = try profile.createNewswireSpace(input: spaceInput("Wrapped", roster: [mineHex]))
    XCTAssertTrue(try repo.newswireIsEditor(spaceDescriptorEntryID: space.entryId, subjectID: mineHex))
    // A stranger key is NOT an editor.
    XCTAssertFalse(try repo.newswireIsEditor(spaceDescriptorEntryID: space.entryId,
                                             subjectID: "11".repeated(32)))
    // An unknown / not-yet-synced descriptor id → false, NOT a throw (drives the pending-sync note).
    XCTAssertFalse(try repo.newswireIsEditor(spaceDescriptorEntryID: "ab".repeated(32), subjectID: mineHex))
}
```
> If `RiotProfileRepository`'s live init differs, mirror whatever `NewswireShareTests`/`AppRepositoryTests` use to build a repository over a `MobileProfile`. The point is to exercise the wrapper against real core, not a stub.

- [ ] **Step 2: Run → FAIL** (`value of type 'RiotProfileRepository' has no member 'newswireIsEditor'`).
  `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -only-testing:RiotTests/NewswireSurfaceTests/testRepositoryWrapperMatchesTheCoreAuthorityForMemberAndNonMember`

- [ ] **Step 3: Implement the seam + wrapper.**
  Add to `apps/ios/Riot/NewswireEditorial.swift`, next to `NewswireEditorialActing` (`~:272`):
```swift
/// The one read the editorial surface makes to decide whether to OFFER a control:
/// core's descriptor-authenticated roster answer, identical to the authority core
/// enforces at admission (Unit 4a's shared `is_editorial_authority`). An unknown /
/// not-yet-synced descriptor answers `false` (never throws) so the surface can render
/// a "controls appear after first sync" note off a defined false. UI VISIBILITY only —
/// core still rejects a non-editor's action at signing regardless of what this returns.
public protocol NewswireEditorAuthorityChecking {
    func newswireIsEditor(spaceDescriptorEntryID: String, subjectID: String) throws -> Bool
}
```
  Add the wrapper to the `RiotProfileRepository` Newswire extension in `apps/ios/Riot/Core/ProfileRepository.swift`, mirroring `newswireShareReference` (`:1097`):
```swift
/// True iff `subjectID` may take editorial actions in the space identified by
/// `spaceDescriptorEntryID` — core's descriptor-authenticated roster answer (Unit 4a),
/// the SAME authority core enforces at admission. An unknown / not-yet-synced descriptor
/// returns `false`, never a throw.
func newswireIsEditor(spaceDescriptorEntryID: String, subjectID: String) throws -> Bool {
    try profile.newswireIsEditor(descriptorEntryId: spaceDescriptorEntryID, subjectId: subjectID)
}
```
  Conform the repository via an empty extension, alongside `extension RiotProfileRepository: NewswireEditorialActing {}` (`:1136`):
```swift
extension RiotProfileRepository: NewswireEditorAuthorityChecking {}
```
> **FFI argument labels:** the forwarded call uses Unit 4a's regenerated labels. The 4a plan names the Rust params `descriptor_entry_id` / `subject_id` (`newswire_is_editor`) → Swift `newswireIsEditor(descriptorEntryId:, subjectId:)`. If 4a's landed binding named them differently (e.g. `spaceDescriptorEntryId:`), match the regenerated symbol here — grep the generated `riot_ffi.swift` for `func newswireIsEditor` and copy its labels verbatim. The wrapper's OWN external labels stay `spaceDescriptorEntryID:` / `subjectID:` (app convention, matches the `newswireShareReference` neighbour).

- [ ] **Step 4: Run → PASS** (member true, stranger false, unknown false-not-throw).

- [ ] **Step 5: Commit.**
```bash
git add apps/ios/Riot/NewswireEditorial.swift apps/ios/Riot/Core/ProfileRepository.swift apps/ios/RiotTests/NewswireSurfaceTests.swift
git commit -m "feat(ios): newswireIsEditor repository wrapper + authority seam (Unit4b, consumes Unit4a)"
```

---

## Task 2: Predicate-driven gate + "appears after first sync" note (model + view)

**Files:** Modify `apps/ios/Riot/NewswireEditorial.swift` (`NewswireSurfaceModel` + `NewswireSurfaceView`); Test `apps/ios/RiotTests/NewswireSurfaceTests.swift`.

- [ ] **Step 0: Convert EVERY pre-existing `NewswireSurfaceModel(...)` construction to the new init (compile-break fix — do this in the SAME commit as Step 3, else RiotTests won't build).** Changing the init (drop `roster:`, add `authority:`) orphans three existing constructions in `apps/ios/RiotTests/NewswireSurfaceTests.swift`; convert them — do **not** leave the old `roster:` calls and do **not** add a parallel helper:

  **(a) Convert the `liveModel` helper itself** (`NewswireSurfaceTests.swift:83-98`) — this is the shared live-core helper that `testRecognizedEditorCanSignAllSixKindsAndEachTakesEffect` (`:308`), `testANonEditorsActionIsIgnoredTheEffectIsAbsentNotJustTheControl` (`:369`, retained by Task 4), `testHiddenControlAndRejectedActionAreIndependent` (`:394`), and `testAnInvalidDraftIsRejectedBeforeItEverReachesCore` (`:413`) all route through. Fix the FAKE key (`myKeyHex: "aa".repeated(32)`) at the same time by keying on the profile's REAL whoami id, and drop the now-unused `roster:` param (the roster lives in the descriptor created via `spaceInput(_, roster:)`, and the model now reads it through the predicate, not a passed array):
```swift
    // A model whose authority is the LIVE core, keyed on the profile's REAL whoami id
    // (the old `myKeyHex: "aa"*32` never mattered because the replaced static ignored the
    // key for an empty roster — the predicate does not, so the real id is load-bearing now).
    private func liveModel(profile: MobileProfile, spaceID: String) throws -> NewswireSurfaceModel {
        let live = LiveNewswire(profile)
        return NewswireSurfaceModel(
            projector: live, editor: live, authority: RiotProfileRepository(profile),
            spaceDescriptorEntryID: spaceID, communityName: "Riverside",
            myKeyHex: RiotDirectoryRow.hex(try profile.whoami().id))
    }
```
   Then update its four call sites to `try liveModel(profile: profile, spaceID: space.entryId)` (drop `roster:`/`myKeyHex:`; add `try`). **Where a test needs the founder to BE a recognized editor** (`testRecognizedEditorCanSignAllSixKinds...` `:307`, `testAnInvalidDraft...` `:411` — both currently create with an *empty* roster and assert the founder can sign), change the space creation to seed the founder explicitly, matching production (`ConferenceShellView.swift:98` seeds `editorialRoster: [me.id]`, never literally empty): `let mineHex = RiotDirectoryRow.hex(try profile.whoami().id); let space = try profile.createNewswireSpace(input: spaceInput("Six Kinds", roster: [mineHex]))`. This keeps those tests green under the live predicate **independent of how Unit 4a resolves the literally-empty-stored-roster edge** (4a's `FOUNDER_EMPTY_ROSTER_IS_EDITOR`) — 4b never relies on empty-roster-create implying founder-true. The non-editor tests (`:369`, `:394`) already create with `roster: [stranger]`, so their founder-is-not-an-editor assertions hold as-is once routed through the real key.

  **(b) Convert the two `ThrowingProjector`/`ThrowingEditor` constructions** at `NewswireSurfaceTests.swift:243-246` and `:252-255` to the new init — add `authority:` and drop `roster:`. Make the existing `ThrowingEditor` test double also satisfy the authority seam (throwing ⇒ `load()` maps to `isEditor == false`, which is all these `offlineStale` assertions need):
```swift
    // ThrowingEditor now also conforms to NewswireEditorAuthorityChecking:
    //   func newswireIsEditor(spaceDescriptorEntryID: String, subjectID: String) throws -> Bool {
    //       throw <the same error ThrowingEditor already throws> }
    let model = NewswireSurfaceModel(
        projector: ThrowingProjector(), editor: ThrowingEditor(), authority: ThrowingEditor(),
        spaceDescriptorEntryID: "", communityName: "Riverside",   // ":252" case: use "desc"
        myKeyHex: "aa".repeated(32))                               // key irrelevant here (offlineStale test)
```

- [ ] **Step 1: Write the failing tests** for the new predicate-driven behavior, using the converted `liveModel`. Add to `NewswireSurfaceTests`:
```swift
func testFounderInTheStoredRosterIsOfferedControlsViaTheCorePredicate() throws {
    let profile = try openLocalProfile()
    let mineHex = RiotDirectoryRow.hex(try profile.whoami().id)
    let space = try profile.createNewswireSpace(input: spaceInput("Mine", roster: [mineHex]))
    let model = try liveModel(profile: profile, spaceID: space.entryId)
    model.load()
    XCTAssertTrue(model.canOfferEditorialControls, "a roster member is offered controls")
    XCTAssertNil(model.editorialControlsPendingNote, "an editor sees no pending note")
}

func testNonMemberIsNotOfferedControlsAndSeesNoMisleadingPendingNoteWhenSynced() throws {
    let profile = try openLocalProfile()
    let space = try profile.createNewswireSpace(input: spaceInput("Others", roster: ["11".repeated(32)]))
    _ = try profile.createNewswirePost(input: postInput(space.entryId, "Report"))  // wire has content ⇒ synced
    let model = try liveModel(profile: profile, spaceID: space.entryId)             // my key ∉ roster
    model.load()
    XCTAssertFalse(model.canOfferEditorialControls, "a non-member is not offered controls")
    XCTAssertNil(model.editorialControlsPendingNote,
                 "a synced non-editor is a reader, not told controls 'appear after sync'")
}

func testUnknownDescriptorShowsThePendingSyncNoteNotABareEmptyView() throws {
    let profile = try openLocalProfile()
    let mineHex = RiotDirectoryRow.hex(try profile.whoami().id)
    // A descriptor id we hold no descriptor for (a joined community pre-first-sync).
    let model = NewswireSurfaceModel(projector: LiveNewswire(profile), editor: LiveNewswire(profile),
        authority: RiotProfileRepository(profile),
        spaceDescriptorEntryID: "ab".repeated(32), communityName: "Pending", myKeyHex: mineHex)
    model.load()  // projection fails ⇒ wire == .offlineStale; predicate ⇒ false
    XCTAssertFalse(model.canOfferEditorialControls)
    XCTAssertEqual(model.editorialControlsPendingNote,
                   "Editorial controls appear after this community's first sync.")
}

func testEmptyDescriptorIdIsNeverAnEditorAndShowsNoNote() throws {
    let profile = try openLocalProfile()
    let model = NewswireSurfaceModel(projector: LiveNewswire(profile), editor: LiveNewswire(profile),
        authority: RiotProfileRepository(profile),
        spaceDescriptorEntryID: "", communityName: "None", myKeyHex: RiotDirectoryRow.hex(try profile.whoami().id))
    model.load()
    XCTAssertFalse(model.canOfferEditorialControls)
    XCTAssertNil(model.editorialControlsPendingNote, "no descriptor id at all ⇒ no editorial affordance or note")
}
```

- [ ] **Step 2: Run → FAIL** (`extra argument 'authority'` / `no member 'editorialControlsPendingNote'`; and the pre-existing constructions fail to compile until Step 0's conversions land — Step 0 + Step 3 ship together).

- [ ] **Step 3: Implement.** In `NewswireSurfaceModel` (`NewswireEditorial.swift:479-522`):
  - Add the injected seam + a published editor flag; **drop `roster`**, keep `myKeyHex`:
```swift
    private let authority: NewswireEditorAuthorityChecking
    /// Whether core recognizes this profile as an editor of this descriptor — read from
    /// the FFI predicate in `load()`, never a locally-asserted roster. `false` until loaded
    /// and `false` for an unknown / not-yet-synced descriptor (no error), by construction.
    @Published public private(set) var isEditor: Bool = false
```
  - New init (replace `roster: [String]?` with `authority:`):
```swift
    public init(
        projector: NewswireProjecting,
        editor: NewswireEditorialActing,
        authority: NewswireEditorAuthorityChecking,
        spaceDescriptorEntryID: String,
        communityName: String,
        myKeyHex: String,
        initialDraftKind: EditorialActionKind = .feature
    ) {
        self.projector = projector
        self.editor = editor
        self.authority = authority
        self.spaceDescriptorEntryID = spaceDescriptorEntryID
        self.communityName = communityName
        self.myKeyHex = myKeyHex
        self.wire = .offlineStale
        self.history = []
        self.draft = EditorialActionDraft(kind: initialDraftKind)
    }
```
  - Gate reads the cached flag; the divergent local static is gone:
```swift
    public var canOfferEditorialControls: Bool {
        !spaceDescriptorEntryID.isEmpty && isEditor
    }

    /// The one honest line shown where a control would be, when this profile is not
    /// (yet) an editor AND the descriptor has not projected (a joined community before
    /// its first sync — the predicate can't tell "not synced" from "not a member", so
    /// the note is scoped to the offline/stale state to avoid telling a *synced* reader
    /// they will gain controls). `nil` for an editor, for a synced non-editor, and when
    /// there is no descriptor id at all — never a bare empty view for the pre-sync editor.
    public var editorialControlsPendingNote: String? {
        guard !spaceDescriptorEntryID.isEmpty, !isEditor, wire == .offlineStale else { return nil }
        return "Editorial controls appear after this community's first sync."
    }
```
  - Evaluate the predicate at the end of `load()` (both the success and `catch`/guard paths keep `isEditor` coherent). Add after the descriptor-id guard and after `wire`/`history` are set in each branch — simplest is to compute it once at the top of `load()` off the descriptor id, defaulting to `false` on any throw:
```swift
    public func load() {
        // Editor status is core's descriptor answer, resolved once per load. An unknown /
        // not-yet-synced descriptor (or a closed profile) answers false — never a throw here.
        isEditor = (try? authority.newswireIsEditor(
            spaceDescriptorEntryID: spaceDescriptorEntryID, subjectID: myKeyHex)) ?? false

        guard !spaceDescriptorEntryID.isEmpty else {
            wire = .offlineStale
            history = []
            return
        }
        do {
            let projection = try projector.projectNewswire(spaceDescriptorEntryID: spaceDescriptorEntryID)
            wire = .from(projection)
            history = projection.editorialHistory.map(EditorialHistoryRow.init)
        } catch {
            wire = .offlineStale
            history = []
        }
    }
```
  - In `NewswireSurfaceView` (`NewswireEditorial.swift:741`), render the note once at the wire header (NOT per-post) when it is non-nil — a real "waiting" affordance, not a blank:
```swift
    if let note = model.editorialControlsPendingNote {
        Text(note)
            .font(.riot(.body, size: 13, relativeTo: .caption))
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
            .accessibilityIdentifier("editorial-controls-pending-note")
    }
```
  Place it once above the wire list (e.g. next to the history/section header), not inside the per-post `if model.canOfferEditorialControls { ... }` block at `:741` (that block stays exactly as-is — it shows the per-post "Editorial action" button when `canOfferEditorialControls`).

- [ ] **Step 4: Update the model construction** at `ConferenceShellView.swift:303-312`: add the authority seam and drop the `roster:` argument:
```swift
    let editor: NewswireEditorialActing = model.profileRepository ?? UnavailableEditor()
    let authority: NewswireEditorAuthorityChecking = model.profileRepository ?? UnavailableEditor()
    _newswire = StateObject(wrappedValue: NewswireSurfaceModel(
        projector: wireProjector, editor: editor, authority: authority,
        spaceDescriptorEntryID: community.newswireDescriptorEntryID ?? "",
        communityName: community.name,
        myKeyHex: me.id))
```
  Make `UnavailableEditor` (`ConferenceShellView.swift:819`) also satisfy the authority seam (unavailable ⇒ not an editor; `load()`'s `try?` maps the throw to `false`, so an explicit `false` return or a throw are equivalent — a `throw` keeps it uniform with the other Unavailable stubs):
```swift
private struct UnavailableEditor: NewswireEditorialActing, NewswireEditorAuthorityChecking {
    func createNewswireEditorialAction(...) throws -> NewswireSignedRecord { throw RepositoryError.profileClosed }
    func newswireIsEditor(spaceDescriptorEntryID: String, subjectID: String) throws -> Bool {
        throw RepositoryError.profileClosed   // no live profile ⇒ never an editor (load() maps to false)
    }
}
```

- [ ] **Step 5: Run → PASS** (all four Task 2 tests green). **Step 6: Commit.**
```bash
git add apps/ios/Riot/NewswireEditorial.swift apps/ios/Riot/ConferenceShellView.swift apps/ios/RiotTests/NewswireSurfaceTests.swift
git commit -m "feat(ios): editor un-gate — canOfferEditorialControls reads newswireIsEditor + pending-sync note (Unit4b)"
```

---

## Task 3: Delete the dead session-only path (two roster sources → one)

**Files:** Modify `apps/ios/Riot/NewswireEditorial.swift`, `apps/ios/Riot/CommunityShell.swift`, `apps/ios/Riot/ConferenceShellView.swift`, `apps/ios/RiotTests/NewswireSurfaceTests.swift`.

- [ ] **Step 1: Delete `EditorialAuthority`** (`NewswireEditorial.swift:198-213`, the whole `public enum EditorialAuthority { ... isRecognizedEditor ... }`) — now that `canOfferEditorialControls` reads the predicate, nothing references it (Task 2 removed the only call site at `:521`). Delete its now-orphaned unit test `testEditorVisibilityIsAPureHintDecoupledFromAuthorization` (`NewswireSurfaceTests.swift:287-299`) — it asserted the *replaced, core-diverging* empty-roster-is-founder-true semantic (`XCTAssertTrue(...roster: [])` at `:291`), which the corrected predicate contradicts; keeping it would enshrine the divergence. The behavior it guarded (visibility ≠ authorization) is now covered end-to-end by Task 2 + the Task 4 defense-in-depth test.

- [ ] **Step 2: Delete `CommunityContext.editorialRoster`** (the dead *display* source; **not** `ConferenceShellView.swift:98`, which seeds core — see the Ground-truth correction):
  - `CommunityShell.swift:34` — remove `public let editorialRoster: [String]?` (and its doc-comment `:28-33`).
  - `CommunityShell.swift:45` — remove the `editorialRoster: [String]? = nil` init parameter; `:51` — remove `self.editorialRoster = editorialRoster`.
  - `CommunityShell.swift:412` — remove the `editorialRoster: request.editorialRoster` argument from the `CommunityContext(...)` constructed by `CommunityCreationCoordinator.create` (the create-time population).
  - `ConferenceShellView.swift:311` — already removed in Task 2 (the `roster:` argument to the model). Confirm no other reader of `community.editorialRoster` remains: `grep -rn "\.editorialRoster" apps/ios/Riot apps/macos` should show only `CommunityCreationRequest.editorialRoster` (the founding input into core, KEPT) and `NewswireSpaceInput.editorialRoster` / the repository create args — never `CommunityContext`.

- [ ] **Step 3: Fix any test constructing `CommunityContext(editorialRoster:)`.** `grep -rn "CommunityContext(" apps/ios/RiotTests apps/macos` and drop the now-removed `editorialRoster:` argument from each (e.g. `CommunityShellTests`, `ConferenceShell*Tests`). These are mechanical (the field carried no behavior post-Task-2).

- [ ] **Step 4: Run the full `NewswireSurfaceTests` + any `CommunityShell`/`ConferenceShell` tests → PASS** (no reference to `EditorialAuthority` or `CommunityContext.editorialRoster` compiles). **Step 5: Commit.**
```bash
git add apps/ios/Riot/NewswireEditorial.swift apps/ios/Riot/CommunityShell.swift apps/ios/Riot/ConferenceShellView.swift apps/ios/RiotTests/NewswireSurfaceTests.swift
git commit -m "refactor(ios): delete dead session-only editorial roster (EditorialAuthority + CommunityContext.editorialRoster) — one authority source (Unit4b)"
```

---

## Task 4: Defense-in-depth — core rejects a non-editor even with the UI forced

**Files:** Modify `apps/ios/RiotTests/NewswireSurfaceTests.swift`.

The point of the slice (design §4, §9): the predicate is a *display* gate; core is the authorization boundary. `testANonEditorsActionIsIgnoredTheEffectIsAbsentNotJustTheControl` (`:363-383`, quoted in Ground truth) already proves this end-to-end through the **live** model + real core; **retain its assertions** (the `.rejected` outcome, the preserved draft, the unchanged post) — its `sign()` path goes to core, which is unaffected by the display gate. Its ONE mechanical change is the `liveModel` call: it is **converted, not duplicated**, by Task 2 Step 0(a) — `liveModel(profile: profile, spaceID: space.entryId, roster: [stranger])` becomes `try liveModel(profile: profile, spaceID: space.entryId)` (the roster now lives in the descriptor created by `spaceInput("Delegated", roster: [stranger])`, and the model keys on the profile's real whoami id ∉ that roster ⇒ predicate false ⇒ core rejects). Then add ONE explicit "controls forced visible" assertion so the independence is stated against the new seam:

- [ ] **Step 1: Write the failing test.**
```swift
/// Even if a bug FORCED the editorial control visible (authority seam stubbed to true),
/// the core still refuses a non-roster author's action and the post is UNCHANGED — the
/// display predicate is never the security boundary (design §4 defense-in-depth).
func testForcingControlsVisibleDoesNotLetANonEditorChangeAnything() throws {
    struct AlwaysEditor: NewswireEditorAuthorityChecking {
        func newswireIsEditor(spaceDescriptorEntryID: String, subjectID: String) throws -> Bool { true }
    }
    let stranger = "33".repeated(32)
    let profile = try openLocalProfile()                       // my key ∉ roster
    let space = try profile.createNewswireSpace(input: spaceInput("Forced", roster: [stranger]))
    let post = try profile.createNewswirePost(input: postInput(space.entryId, "Untouched"))
    let live = LiveNewswire(profile)
    let model = NewswireSurfaceModel(projector: live, editor: live, authority: AlwaysEditor(),
        spaceDescriptorEntryID: space.entryId, communityName: "Forced",
        myKeyHex: RiotDirectoryRow.hex(try profile.whoami().id))
    model.load()
    XCTAssertTrue(model.canOfferEditorialControls, "seam forced true ⇒ control shown (the bug we defend against)")

    // …yet the action still fails at core and the post is unchanged.
    model.draft = EditorialActionDraft(kind: .hide, reason: "force it")
    XCTAssertEqual(model.sign(targetEntryID: post.entryId), .rejected)
    let projection = try profile.projectNewswireSpace(spaceDescriptorEntryId: space.entryId)
    let row = try XCTUnwrap(projection.openWire.first { $0.entryId == post.entryId })
    XCTAssertEqual(row.treatment, .ordinary, "core, not the UI, is the gate")
    XCTAssertEqual(row.headline, "Untouched")
}
```

- [ ] **Step 2: Run → FAIL** (`AlwaysEditor` unused before compile / assertion). **Step 3:** (no product code — the property already holds via core; this test only asserts it against the new seam). **Step 4: Run → PASS.** **Step 5: Commit.**
```bash
git add apps/ios/RiotTests/NewswireSurfaceTests.swift
git commit -m "test(ios): defense-in-depth — forced-visible controls still can't move a non-editor's action past core (Unit4b)"
```

---

## Task 5: Build both platforms + gates

- [ ] iOS: `xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -only-testing:RiotTests/NewswireSurfaceTests` — all new + retained cases green.
- [ ] iOS full `RiotKit` test run: only the known-red Bonjour two-peer sync fails (unrelated). No new red.
- [ ] iOS app + **macOS** app **BUILD SUCCEEDED** (`apps/macos/Riot.xcodeproj` references the same iOS sources via `path = ../ios/Riot/…`; 4b adds no iOS-only API — `NewswireEditorAuthorityChecking`, the FFI predicate, and the pending note are all platform-neutral, so no `#if os(iOS)` is needed).
- [ ] Coverage floor honored (`.coverage-thresholds.json`, CI-enforced). **Step: Commit** any fixups.

---

## Self-Review

- **Spec coverage (design §4 "4b" requirements → tasks):**
  - "show `EditorialActionSheet` controls iff `newswire_is_editor(activeDescriptorId, whoami.id)`" ✅ Task 2 (`canOfferEditorialControls == isEditor`, `isEditor` read from the predicate in `load()`, keyed on `myKeyHex == me.id == whoami hex`; descriptor id is `community.newswireDescriptorEntryID`).
  - "a note when the predicate is false / descriptor not yet synced" ✅ Task 2 (`editorialControlsPendingNote`, scoped to `offlineStale` so a *synced* reader is not misled — verified by `testNonMemberIsNotOfferedControls...`).
  - "DELETE the dead session-only path (`EditorialAuthority.isRecognizedEditor`, `CommunityContext.editorialRoster` create-time population) so two roster-authority sources don't coexist" ✅ Task 3.
  - "defense-in-depth (core still rejects a non-editor's action even if controls forced visible)" ✅ Task 4 (new `AlwaysEditor` stub test) + retained `testANonEditorsActionIsIgnored...`.
  - "CORRECTED empty-roster semantic (display == authority; founder + literally-empty stored roster ⇒ false)" ✅ inherited by construction — 4b consumes 4a's predicate, which IS admission authority; 4b removes the divergent `if roster.isEmpty { return true }` local static. 4b asserts nothing about the empty-roster truth itself (that is 4a's `FOUNDER_EMPTY_ROSTER_IS_EDITOR` decision) — 4b's tests seed `roster: [mineHex]` / `[stranger]`, never relying on the empty-roster edge, so they stay correct whatever 4a resolved.
- **Placeholder scan:** the one unresolved-by-me item is the FFI argument labels (`descriptorEntryId:` / `subjectId:`) forwarded in the Task 1 wrapper — flagged explicitly with a grep-the-binding instruction, because they are owned by 4a's regen and must match the landed symbol, not guessed. No other `/* ... */` or TODO in the plan; all Swift is real and copied from or mirrored on verified ground-truth lines.
- **Design line-number correction flagged:** `ConferenceShellView.swift:98` is the *founding roster into core* (KEEP), not the `CommunityContext.editorialRoster` display population (`CommunityShell.swift:412`, DELETE) — called out in Ground truth and Task 3 so the implementer does not delete the line that makes founders editors.
- **Type consistency:** ids are lowercase hex `String` throughout (`me.id`, `RiotDirectoryRow.hex(whoami().id)`, wrapper args); `NewswireEditorAuthorityChecking.newswireIsEditor(spaceDescriptorEntryID:subjectID:) throws -> Bool` used identically by the model seam, the live `RiotProfileRepository` wrapper, the `UnavailableEditor` stub, and the `AlwaysEditor` test stub.
- **Test seeding of a joined-vs-created roster (the flagged risk):** 4b's tests seed a **synthetic descriptor** via `createNewswireSpace(input: spaceInput(_, roster:))` on a single local profile — a *created* community with a chosen stored roster — **never live two-peer sync** (which is RED on main, see MEMORY `riot-two-peer-sync-red`). The "joined, pre-first-sync" case is modeled as an **unknown descriptor id** (`"ab".repeated(32)`) whose projection fails → `offlineStale` + predicate-false → the pending note (`testUnknownDescriptorShowsThePendingSyncNote...`). This exercises the exact 4b surface (predicate-false → note; predicate-true → controls) without depending on a real sync. A genuinely joined-then-synced roster read is a core/transport concern proven by Unit 4a's Rust/FFI tests, not re-litigated in Swift.
- **No orphaned call sites (gate r1 BLOCKER 1 resolved):** the init change (drop `roster:`, add `authority:`) converts EVERY pre-existing `NewswireSurfaceModel(...)` in `NewswireSurfaceTests.swift` in the same commit — Task 2 **Step 0** converts (a) the shared `liveModel` helper (`:83-98`, also fixing the FAKE `myKeyHex: "aa"*32` → real `RiotDirectoryRow.hex(try profile.whoami().id)`) and its four call sites (`:308/:369/:394/:413`), and (b) the two `ThrowingProjector`/`ThrowingEditor` constructions (`:243/:252`, with `ThrowingEditor` extended to the authority seam). `testANonEditorsActionIsIgnored...` (Task 4) is **converted through `liveModel`, not duplicated**. RiotTests compiles.
- **Fake-key fix is complete, not half-done:** `liveModel` itself now uses the real whoami hex, so the founder/non-member predicate reads are genuine; tests that need the founder to be an editor seed `spaceInput(_, roster: [mineHex])` (production reality, `ConferenceShellView.swift:98`), so they hold regardless of 4a's empty-stored-roster resolution.
- **Cross-unit coupling (gate r1 BLOCKER 2 resolved):** the "Cross-unit coupling" section near the top states the AGREED reconciliation with Unit 7 — land order **4b before 7**; the final `NewswireSurfaceModel.init` carries BOTH `authority:` (4b) and `descriptorResolver:` (Unit 7); the shell construction carries both, never `roster:`; whoever lands updates all init call sites; the shared files are claimed in COLLABORATION.md and 4b/7 do not run concurrently on them.
- **HARD DEPENDENCY restated:** 4b will not compile — let alone go green — until Unit 4a's regenerated binding (`newswireIsEditor` on `MobileProfile`) + rebuilt native staticlib are on the branch. Task 1 has an explicit precondition gate; do not begin the wrapper before the 4a smoke call compiles + loads without a checksum abort.
- **Dependency order (within 4b):** T1 (wrapper + seam) → T2 (Step 0 convert existing call sites → predicate gate + note, adds `authority`) → T3 (delete dead sources, now safe) → T4 (defense-in-depth) → T5 (build both). No new Swift file ⇒ no pbxproj edit.
