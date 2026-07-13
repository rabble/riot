# Demo Community Tools Implementation Plan

Plan review gate: **FAILED AFTER ITERATION 3 ON 2026-07-13 — DO NOT EXECUTE.**
Scope & Alignment passed. Feasibility and Completeness failed the frozen
`c8cbc0f1…` revision on five blockers: contradictory pending-package trust
projection; a nonexistent WebKit geolocation delegate; no executable acquisition
path behind **Add to this device**; no mapped recovery for nonce/rule-list startup
failure; and no direct Rust navigation-generation mismatch proof. A fresh review
cycle must correct and approve all five before implementation.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make both Apple products ship the complete eight-tool starter catalog and make a clean Riverside member safely open and use all eight built-ins plus Shift Signup directly, without Get, Review, an authority bypass, or a stale bridge that survives revocation.

**Architecture:** Rust's verified starter catalog produces a committed machine-readable catalog that Apple loads fail-closed; both Apple targets package every exact pair. Riverside gains a deterministic demo-only organizer whose signed trust markers are ordinary imported entries, and the repository deterministically admits each complete package with accepted organizer trust without letting one bad package block the rest. Every WebView receives an opaque Rust-owned execution session rather than a raw app ID; resource and bridge operations revalidate the exact namespace, package, and accepted organizer marker, while a CSP-independent WebKit rule list blocks network loads. Riverside is locally usable but excluded from Nearby, invitation, and app-sharing paths because its organizer seed is public fixture material. This is the complete safe tools vertical slice from the approved community-first design; readable Home/posting and the adaptive community shell remain separate product work, not prerequisites for using a tool safely.

**Tech Stack:** Rust 2021, Willow evidence store, UniFFI, Swift 6/SwiftUI, Xcode projects, XCTest/XCUITest, deterministic CBOR/JSON fixtures.

---

## Usable outcome and no-dead-end contract

This plan is successful only if a clean local member can load Riverside, see
all nine organizer-approved tools as **Open**, open any one in one action, finish
its primary job, close it, and still see the committed result after reopening or
relaunching. Browser tests alone do not satisfy that contract; the same matrix
runs through the native iPhone and macOS hosts.

Every package state has one honest result:

| Verified package and current organizer marker | Member result | Organizer result |
| --- | --- | --- |
| complete + Trust | automatic admission, **Open** | automatic admission, **Open** |
| valid manifest + Trust + package bytes absent | **Add to this device**; acquisition adds bytes but grants no authority | same action |
| valid manifest + no Trust/Revoke | **Not available in this community**; no acquisition action | **Review** with exact authority decision, then Add to device only if approved bytes are absent |
| invalid carried pair | quarantined with **Check again** and non-demo **Nearby**; other tools still admit | same quarantine; no raw CBOR/path error |
| Trust replaced by Revoke | no new launch; row becomes unavailable after refresh/relaunch | may review a future valid Trust decision |

The admission pass is per app: an invalid or incomplete Shift Signup cannot
prevent Checklist, Needs & Offers, Events, Decisions, Chat, Dispatches, Wiki,
or Photo Wall from opening. A member never sees an approval button and never
lands on a Review action they cannot complete.

A missing, extra, reordered, or damaged *named starter resource* is different:
it makes artifact tests and bootstrap fail as one fixed catalog error. The shell
stays on a persistent recovery screen with **Retry** and reinstall guidance; it
cannot be dismissed into an empty or half-open shell. That path is recovery for
an impossible-to-ship/corrupt artifact, while carried-package quarantine remains
per app so one peer package cannot disable healthy tools.

Every recovery surface has a **Technical details** disclosure containing only a
stable code: `RIOT-STARTER-001`, `RIOT-PACKAGE-ARRIVING`,
`RIOT-PACKAGE-INVALID`, `RIOT-PACKAGE-ADMISSION`, or
`RIOT-APP-INVALIDATED`. Raw CBOR, paths, signatures, stack traces, and localized
system errors are never shown.

An accepted Revoke, namespace change, repository replacement, navigation away,
or WebView destruction invalidates the opaque execution session before the next
resource, identity, read, watch delivery, or write result. The host closes the
tool with fixed copy and preserves no stale write. A CSP-stripped hostile page
also remains unable to perform external network activity.

This is deliberately not the whole community-first trial. It proves the
approved design's **Do useful work** goal on both Apple products. The old
Spaces/Board/Compose shell remains until the separately reviewed Home/posting
and single-community-shell plans land. The tool slice itself has no deferred
security or recovery dependency hidden behind the word “usable.”

## Scope and file map

### Blocking behavior-coverage prerequisite

- Modify `.coverage-thresholds.json` — exact integer-count Rust gate.
- Modify only the measured production rows and paired tests enumerated in Task
  0A across `riot-app-cli`, `xtask`, `riot-ffi`, and `riot-core`; no product
  behavior change is allowed in this prerequisite.

### Catalog generation and drift proof

- Modify `crates/riot-core/examples/pack_starter.rs` — emit canonical catalog JSON after all eight pairs verify.
- Modify `crates/riot-core/src/apps/starter.rs` — expose the exact ordered starter slugs beside the authoritative pairs.
- Modify `crates/riot-core/tests/apps_starter.rs` — assert exact order, visible names, IDs, and committed JSON drift.
- Create `fixtures/apps/starter-catalog.json` — generated metadata only; Rust CBOR pairs remain the authority.
- Modify `scripts/apps/repack-starter.sh` — preserve the single deterministic regeneration command.

### Apple packaging and fail-closed loading

- Modify `apps/ios/Riot/AppModel.swift` — replace the four-name `compactMap` loader with a testable catalog loader that rejects missing or mismatched resources.
- Modify `apps/ios/Riot/ConferenceShellView.swift` — offer Retry for the fixed starter-catalog bootstrap failure instead of an OK-only alert.
- Modify `apps/ios/Riot/Core/ProfileRepository.swift` — make any invalid named starter fail profile open instead of silently shrinking the catalog.
- Modify `crates/riot-ffi/src/apps_ffi.rs` — expose Rust pair verification for Apple catalog preflight.
- Modify `crates/riot-ffi/src/mobile_state.rs` — reuse the existing core verifier/error mapping behind that FFI.
- Modify `crates/riot-ffi/tests/apps_contract.rs` — prove verification returns the content-derived ID and rejects damage/mismatch.
- Modify `apps/ios/RiotTests/DemoModeTests.swift` — inspect all catalog resources through a supplied bundle and prove missing resources fail.
- Modify `apps/ios/RiotTests/ToolsSectionTests.swift` — expect all eight tools and bootstrap failure for an incomplete catalog.
- Modify `apps/ios/RiotTests/ShellNavigationTests.swift` — prove catalog bootstrap failure exposes Retry and does not start discovery.
- Modify `apps/ios/RiotTests/AppRepositoryTests.swift` — replace the old silent-skip starter assertion with fail-closed open behavior.
- Create `scripts/apps/audit-starter-artifacts.sh` — build both products and compare their exact starter resource names and bytes.
- Modify `apps/ios/Riot.xcodeproj/project.pbxproj` — add catalog JSON and all sixteen CBOR files to app and test resources.
- Modify `apps/macos/Riot.xcodeproj/project.pbxproj` — add the same resources to `Riot-macOS` and `RiotKitTests-macOS`.

### Riverside organizer authority and admission

- Modify `fixtures/demo/riverside/content.json` — replace the unrelated namespace seed with the deterministic demo organizer seed.
- Modify `crates/riot-core/src/demo_fixture.rs` — derive the communal namespace from that organizer and write nine ordinary trust markers.
- Modify `crates/riot-core/tests/demo_fixture_drift.rs` — prove recognized-organizer trust, exact approved IDs, and rejection of member authority.
- Modify `fixtures/demo/riverside/demo-space.riot-evidence` — deterministic regenerated output.
- Modify `crates/riot-core/src/apps/index.rs` — retain missing/invalid-bundle manifests as non-launchable pending records.
- Modify `crates/riot-core/tests/apps_index_io.rs` — prove pending, invalid, complete-wins, and deterministic dedup behavior.
- Modify `crates/riot-ffi/src/mobile_state.rs` — project pending records as bundle-absent directory rows without trust.
- Modify `crates/riot-ffi/tests/apps_contract.rs` — prove real FFI recovery rows instead of relying on a fake directory.
- Modify `apps/ios/Riot/Core/ProfileRepository.swift` — auto-admit complete organizer-trusted carried packages after import and persist that admission.
- Modify `apps/ios/Riot/Directory/DirectoryModel.swift` — make availability role-aware and add an explicit package-retry port/action.
- Modify `apps/ios/Riot/Directory/DirectoryView.swift` — render Open, organizer Review, member-unavailable, Check again, and Nearby without dead buttons.
- Modify `apps/ios/RiotTests/AppRepositoryTests.swift` — prove admission/relaunch and negative incomplete/untrusted cases.
- Modify `apps/ios/RiotTests/DirectoryStorefrontTests.swift` — prove the row action is Open rather than Get/Review.
- Modify `apps/ios/RiotTests/DirectoryRepositoryTests.swift` — prove role-aware Add to device, unavailable, retry, and failure isolation behavior.
- Modify `apps/ios/RiotUITests/ChecklistFlowUITests.swift` — replace leftover-state tolerance with an isolated clean Riverside member flow.
- Modify `apps/macos/Riot.xcodeproj/project.pbxproj` — add a macOS UI-test target that reuses the existing shared XCUITest source.
- Create `apps/macos/Riot.xcodeproj/xcshareddata/xcschemes/RiotUITests-macOS.xcscheme` — make the macOS UI proof explicit and CI-addressable.

### Demo-only lifecycle and revocable execution

- Modify `apps/ios/Riot/Core/ProfileRepository.swift` — refuse demo export/app sharing and open opaque execution sessions.
- Modify `apps/ios/Riot/Transport/SpacePairing.swift` — announce only a repository's explicitly shareable non-demo community.
- Modify `apps/ios/Riot/ConferenceShellView.swift` and `apps/ios/Riot/Peers/PeerProfileView.swift` — omit Nearby/invite/share actions while Riverside demo mode is loaded.
- Modify `apps/ios/RiotTests/{DemoModeTests,SpaceAdoptionTests,DirectoryRepositoryTests,AppRepositoryTests}.swift` — prove demo content remains locally usable while every presentation/export path is denied.
- Modify `crates/riot-ffi/src/{apps_ffi,mobile_state}.rs` — add opaque `AppExecutionSession` open/close and session-bound resource, identity, profile, data, and watch operations.
- Modify `crates/riot-core/src/apps/trust.rs` and `crates/riot-core/tests/apps_trust.rs` — preserve the accepted marker entry ID used as approval generation.
- Modify `crates/riot-ffi/tests/apps_contract.rs` — prove every session operation refuses revoke, namespace replacement, explicit close, package mismatch, and stale marker generation before returning or committing.
- Modify `apps/ios/Riot/Apps/{AppRuntimeView,AppBridgeController,AppSchemeHandler}.swift` — consume only the opaque session and close the host on invalidation.
- Create `apps/ios/Riot/Apps/AppNetworkBackstop.swift` — install the CSP-independent `WKContentRuleList` before any page load.
- Modify `apps/ios/Riot/Apps/RiotJS.swift` — give watch registrations explicit IDs and cancel them on teardown.
- Modify `apps/ios/RiotTests/{AppRuntimeHostTests,AppSyncReplicationTests}.swift` and create `apps/ios/RiotTests/NetworkBackstopTests.swift` — prove UI close/watch cancellation and hostile-page containment.
- Modify `scripts/apps/miniapp-browser.spec.mjs` — cover the browser-side hostile API matrix without treating it as proof of the native network wall.
- Modify both `apps/ios/Riot.xcodeproj/project.pbxproj` and `apps/macos/Riot.xcodeproj/project.pbxproj` — add both new Swift files to the app and matching test targets.

`AppNetworkBackstop.swift` and `NetworkBackstopTests.swift` are new Swift files.
The implementation must add the production file to both Apple app targets and
the test file to both Apple test targets in the same commit. If execution needs
any other new Swift file, stop, amend this plan, and add it to both projects
before implementation.

## Compatibility and rollback contract

This plan adds one backward-compatible persisted Swift field,
`appAuthorityBundles: [Data]`, decoded as empty for older snapshots. It stores
only Rust-produced signed Trust/Revoke bundle receipts. `trustedAppIDs` remains
a legacy projection cache but is never replayed as authority; on open it is
recomputed from verified marker receipts and an unsupported local true bit is
cleared. `AppExecutionSession`, watch IDs, approval generation, and namespace
generation remain process-local. App-data writes keep the existing signed
receipt format in `PersistedProfile.appDataBundles`, and admitted carried apps
keep the existing `PersistedAppPack` representation. Riverside's namespace
remains the exact pre-plan value
`66dbc1bd4b9484434559f09b376a6e8743559c46cae246ef680c1a44b540fe08`.

Extend `AppRepositoryTests.testOpensSnapshotWrittenBeforeTrustedAppIDsField`
with an immutable base64 snapshot fixture embedded in that test file and
captured before this plan. It contains an ordinary community, one
trusted starter, one carried app, and app-data receipts. After each repository
task, open it and prove the same community/app data are visible. After Task 6,
open that snapshot through the opaque session and write/reopen one value. Also
record the fixture's SHA-256 in the assertion so accidental regeneration is
visible. A rollback of Task 6 can still replay the same
receipts through the legacy host; a rollback of catalog/UI code ignores the
extra packaged resources; an older binary simply ignores the additive JSON key.
No task deletes a carried pack, rewrites existing signed evidence, or changes
the demo namespace.

Each product task is one dependency-ordered commit; Task 0A lands one
behavior-preserving commit per crate-sized coverage group. If a task fails its focused or full
gate, revert only that task/group commit before proceeding; never leave later commits
on top of a failed prerequisite. Data created by a successful tool action is
ordinary signed app data and is intentionally not erased by code rollback.

## Task 0: Hot-checkout and coverage preflight

**Files:**
- Modify only if unclaimed and authorized: `.coverage-thresholds.json`
- Modify coordination state: `COLLABORATION.md`

- [ ] **Step 1: Synchronize and prove every implementation path is free**

Run:

```bash
git pull --rebase --autostash
git status --short
tail -n 120 COLLABORATION.md
```

Expected: no active claim owns any file listed above. The currently dirty iOS/macOS project and transport files must have an explicit owner release before this plan touches either project; do not race or overwrite them.

- [ ] **Step 2: Claim the exact Task 0A–6 paths**

Use `apply_patch` to append one active claim naming every path in this plan.
`SpacePairing.swift` is the only transport file in scope and may be claimed only
after its current owner releases it; do not touch the other dirty transport
files. Re-read the appended section immediately.

- [ ] **Step 3: Install one self-contained exact-count coverage gate**

If `.coverage-thresholds.json` is still untracked, claim it and encode the
design's exact four 100% thresholds with commands supported by the installed
`cargo-llvm-cov 0.8.7`. Do not compare rounded percentages; require covered and
count integers to match:

```json
{
  "thresholds": {"lines": 100, "branches": 100, "functions": 100, "statements": 100},
  "enforcement": {
    "command": "set -eu; cargo tarpaulin --workspace --all-features --fail-under 100; cargo llvm-cov clean --workspace; cargo llvm-cov --workspace --all-features --branch --json --summary-only --output-path target/llvm-cov-summary.json; jq -e '.data | length == 1 and (.data[0].totals | (.lines.covered == .lines.count) and (.functions.covered == .functions.count) and (.regions.covered == .regions.count) and (.branches.covered == .branches.count))' target/llvm-cov-summary.json >/dev/null",
    "blockPRCreation": true,
    "blockTaskCompletion": true
  }
}
```

Run the source-of-truth command exactly as later gates will run it:

```bash
jq -r '.enforcement.command' .coverage-thresholds.json | sh
```

Record the exact baseline. Expected today: the gate remains
RED near the retained 83.37% Tarpaulin line baseline and the 88.921/86.484/
87.234/74.645 LLVM line/function/region/branch baseline. Continue only to Task
0A. Task 1 cannot begin until this exact command is green. A threshold change is
legal only as an explicit separately committed Rabble decision; no executor may
lower or exclude code to unblock this plan.

## Task 0A: Close the measured repository coverage debt first

**Files:**
- Modify only the production rows and paired tests in the table below.
- Modify: `.coverage-thresholds.json`

This is a behavior-preserving prerequisite, not permission to refactor protocol
or product behavior. Generate fresh missing-line output before touching a row:

```bash
cargo llvm-cov clean --workspace
cargo llvm-cov --workspace --all-features --branch --json \
  --output-path target/llvm-cov-before-tools.json
cargo llvm-cov report --branch --show-missing-lines \
  > target/llvm-cov-before-tools.txt
```

The measured finite worklist and required test homes are:

| Production source | Required behavior tests |
| --- | --- |
| `crates/riot-app-cli/src/{lib,main}.rs` | existing internal + `tests/{cli_pack,cli_commands}.rs`: every error/source arm, manifest/path/file/link/size boundary, atomic publish failure, command/arity/clock/output path |
| `crates/xtask/src/main.rs` | internal tests: missing/unknown/all commands, root failure, command spawn/failure, absent or mismatched outputs/manifests/locks/schemas/fixtures |
| `crates/riot-ffi/src/{mobile_state,mobile_api}.rs` | adjacent tests + focused contracts: every `MobileError`, poison/closed/stale/consumed/limit path, organizer/member, bundle/listing empty/nonempty/error projection |
| `crates/riot-core/src/demo_fixture.rs` | `tests/demo_fixture_drift.rs`: every JSON type/missing/range/hex/time/signer shape and single/multiple deterministic bundle paths |
| `crates/riot-core/src/session.rs` | existing import lifecycle/concurrency tests: every consumed/stale/limit/closed transition, empty/selective commit, rollback, route accounting, poison |
| `crates/riot-core/src/apps/{index,endorse,trust,bridge,bundle,mod,manifest,entry,starter,directory}.rs` | matching `tests/apps_*.rs`: malformed path/key/value/pair, duplicates/revokes/ties, resolver/store failures, all limits and Display/From arms |
| `crates/riot-core/src/import/{bundle,join}.rs` | existing bundle/join tests: zero/exact/over limits, malformed/trailing/duplicate/wrong namespace, live/expired/consumed/stale selection |
| `crates/riot-core/src/sync/{state,wire,reconcile,ffi_bridge}.rs` | matching sync tests: every state/frame conversion, order/duplicate/terminal/error and empty/local/remote/overlap set branch |
| `crates/riot-core/src/model/mod.rs` | `tests/public_alert.rs`: every error Display arm and text/source/freshness boundary |
| `crates/riot-core/src/profile/{mod,resolver,card,path}.rs` | resolver tests: Display/From arms, local/foreign/unknown cards, signature/capability/recency ties and name/path bounds |
| `crates/riot-core/src/willow/{entry,identity,clock,mod,owned}.rs` | public Willow + adjacent tests: deterministic success, invalid object/revision, entropy/clock failure, identity relationships and every error arm |

- [ ] **Step 1: Close one row group with RED→GREEN behavior tests**

For hard-wired OS failures only, add the smallest crate-private injectable port:
`PlatformFs` for app-cli filesystem checkpoints; `run(args, now, out, err)` for
the CLI; `run(root, args, command_runner, out, err)` for xtask; existing
`EntropySource`/`ClockSource` `_with` helpers for core. These ports remain absent
from public/FFI APIs. No coverage-only conditional, dead-code allowance, fake
execution, wildcard directory exclusion, or generated-source counting is legal.

After each row group, run its focused tests, the workspace tests, and regenerate
the LLVM report. Newly introduced lines/branches must be fully covered in the
same commit. Commit per crate so a failed group can be reverted without mixing
product work.

- [ ] **Step 2: Require the exact configured gate before Task 1**

Run:

```bash
jq -r '.enforcement.command' .coverage-thresholds.json | sh
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
```

Expected: Tarpaulin passes 100%; LLVM covered/count equality passes for lines,
functions, regions/statements, and branches. Re-read every changed source/test,
stage only the row-group paths, inspect the cached diff, and land all coverage
commits before claiming Task 1. If any count is below 100%, remain in Task 0A.

## Task 1: Generate one canonical starter catalog

**Files:**
- Modify: `crates/riot-core/examples/pack_starter.rs`
- Modify: `crates/riot-core/src/apps/starter.rs`
- Modify: `crates/riot-core/tests/apps_starter.rs`
- Create: `fixtures/apps/starter-catalog.json`
- Modify: `scripts/apps/repack-starter.sh`

- [ ] **Step 1: Write the failing Rust catalog drift test**

Add `STARTER_SLUGS` beside `STARTER_CATALOG` and a test that verifies equal
length/order, then compares a parsed committed JSON file to these exact records:

```rust
#[test]
fn committed_starter_catalog_matches_verified_rust_catalog() {
    let expected = [
        ("checklist", "Checklist", "3fe5f89af18d9244756c8925750280f0c51479030cf3cd7b4d26940b51eaa4b7"),
        ("supply-board", "Needs & Offers", "05200e07ca8c11da106366dbe2f7386dc11826aa723479352a916158ac649ac8"),
        ("roll-call", "Events", "266b7978d2bcd143d7b93b6246884c85343ca4b6e4bb4aa406dbf8d87e39d382"),
        ("quick-poll", "Decisions", "36a4c50030b5dbac3e84d40c503b6413e2b39b276f6010215e87c29c96453d1a"),
        ("chat", "Chat", "6a5cadd381460f15b871cf898b59a4db97d5ddb80130cef335136c619bacdfac"),
        ("dispatches", "Dispatches", "848a8e1551f34a1443eb1c1dc6601b730db413eee500a49695c8956cac5f2459"),
        ("wiki", "Wiki", "c2a54df288701afe8ed95e91af8fafec34a56d9132cde914b9ec76ce826ac714"),
        ("photo-wall", "Photo Wall", "ae1ac55cfe563dab67c4139ff2fc84fa59647e75848ffaa0132ef1110ff8066b"),
    ];
    assert_catalog_json(&expected);
}

fn assert_catalog_json(expected: &[(&str, &str, &str)]) {
    let path = format!("{}/../../fixtures/apps/starter-catalog.json", env!("CARGO_MANIFEST_DIR"));
    let raw = std::fs::read_to_string(path).expect("committed starter-catalog.json");
    let value = serde_json::from_str::<serde_json::Value>(&raw).expect("catalog JSON");
    let rows = value
        .as_array()
        .expect("catalog array")
        .iter()
        .map(|row| {
            (
                row["slug"].as_str().expect("slug"),
                row["name"].as_str().expect("name"),
                row["app_id"].as_str().expect("app_id"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.as_slice(), expected);
}
```

Run:

```bash
cargo test -p riot-core --test apps_starter committed_starter_catalog_matches_verified_rust_catalog -- --exact
```

Expected: FAIL because `fixtures/apps/starter-catalog.json` does not exist.

- [ ] **Step 2: Emit deterministic JSON from the verified packed records**

Extend `PackedApp` with `name`, serialize only after every pair passes `verify_starter_catalog`, and write a trailing-newline JSON array:

```json
[
  {"slug":"checklist","name":"Checklist","app_id":"3fe5f89af18d9244756c8925750280f0c51479030cf3cd7b4d26940b51eaa4b7","manifest":"checklist.manifest.cbor","bundle":"checklist.bundle.cbor"}
]
```

The real file contains all eight rows in `STARTER_CATALOG` order. Do not read IDs from handwritten JSON; compute them from freshly encoded manifest and bundle bytes. Make `scripts/apps/repack-starter.sh` report the catalog path after the existing pack command. Remove the packer's local `STARTERS` array and iterate core `STARTER_SLUGS`, so generation cannot reorder its authority.

- [ ] **Step 3: Regenerate and prove catalog drift is green**

Run:

```bash
sh scripts/apps/repack-starter.sh
cargo test -p riot-core --test apps_starter
git diff --check -- crates/riot-core/examples/pack_starter.rs crates/riot-core/tests/apps_starter.rs fixtures/apps/starter-catalog.json scripts/apps/repack-starter.sh
```

Expected: all `apps_starter` tests PASS; frozen Checklist CBOR remains byte-identical; JSON contains exactly eight unique rows.

- [ ] **Step 4: Re-read and commit only Task 1**

Run the mandatory pull/rebase, reread every Task 1 file, stage the five exact paths, inspect `git diff --cached`, run `sh scripts/green.sh`, then commit:

```bash
git commit -m "build(apps): generate canonical starter catalog"
```

## Task 2: Bundle all eight pairs and fail closed on Apple

**Files:**
- Modify: `apps/ios/Riot/AppModel.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`
- Modify: `apps/ios/Riot/Core/ProfileRepository.swift`
- Modify: `crates/riot-ffi/src/apps_ffi.rs`
- Modify: `crates/riot-ffi/src/mobile_state.rs`
- Modify: `crates/riot-ffi/tests/apps_contract.rs`
- Modify: `apps/ios/RiotTests/DemoModeTests.swift`
- Modify: `apps/ios/RiotTests/ToolsSectionTests.swift`
- Modify: `apps/ios/RiotTests/ShellNavigationTests.swift`
- Modify: `apps/ios/RiotTests/AppRepositoryTests.swift`
- Create: `scripts/apps/audit-starter-artifacts.sh`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/macos/Riot.xcodeproj/project.pbxproj`

- [ ] **Step 1: Write failing bundle and loader tests**

In `DemoModeTests`, decode `starter-catalog.json` from `testBundle` and require each named manifest/bundle. In `ToolsSectionTests`, pass all eight fixture pairs and assert names in canonical order. Add an incomplete-catalog bootstrap assertion:

```swift
func testStarterCatalogIsCompleteInBuiltResources() throws {
    let packs = try StarterPackLoader.load(bundle: testBundle)
    XCTAssertEqual(packs.map(\.name), [
        "Checklist", "Needs & Offers", "Events", "Decisions",
        "Chat", "Dispatches", "Wiki", "Photo Wall",
    ])
}

func testMissingNamedStarterFailsBootstrap() {
    let model = RiotAppModel()
    model.bootstrap(starterBundle: Bundle(for: XCTestCase.self))
    XCTAssertFalse(model.isProfileOpen)
    XCTAssertEqual(model.errorMessage, "Starter tools are missing or damaged.")
    XCTAssertTrue(model.canRetryBootstrap)
    XCTAssertNil(model.nearbySpaceHost)
}

func testCatalogFailureCannotBeDismissedIntoTheShell() {
    let model = RiotAppModel()
    model.bootstrap(starterBundle: incompleteStarterBundle)
    let state = ShellProjection(model: model)
    XCTAssertEqual(state.route, .starterRecovery)
    XCTAssertFalse(state.showsTabBar)
    XCTAssertTrue(state.showsRetry)
}

func testRetryReusesTheSameVerifiedCatalogBoundary() {
    let model = RiotAppModel()
    model.bootstrap(starterBundle: repairedInPlaceBundle)
    XCTAssertTrue(model.canRetryBootstrap)
    repairResources()
    model.retryBootstrap()
    XCTAssertTrue(model.isProfileOpen)
    XCTAssertFalse(model.canRetryBootstrap)
}
```

Add Rust RED tests for a public `verify_starter_catalog(slugs,
manifest_bytes, bundle_bytes)` UniFFI function. The exact eight ordered records
return content-derived IDs and manifest names. A flipped byte, entry-point
mismatch, reordered pair, changed/unique slug, extra/missing row, or name/ID
mismatch returns `MobileError::AppRejected`.

Keep the loader internal in `AppModel.swift` so `@testable import RiotKit` can call it; do not add a Swift file.

Run the focused `RiotKit` test scheme. Expected: FAIL because the loader/type does not exist and seven resource pairs are absent.

- [ ] **Step 2: Implement a fail-closed catalog loader**

Replace `starterAppNames` and `compactMap` with:

```swift
struct StarterCatalogEntry: Decodable, Equatable {
    let slug: String
    let name: String
    let appID: String
    let manifest: String
    let bundle: String

    enum CodingKeys: String, CodingKey {
        case slug, name, manifest, bundle
        case appID = "app_id"
    }
}

enum StarterPackLoader {
    enum Failure: Error { case unavailable }
    struct Pack { let name: String; let appID: Data; let manifest: Data; let bundle: Data }

    static func load(bundle resources: Bundle) throws -> [Pack] {
        do {
            guard let catalogURL = resources.url(forResource: "starter-catalog", withExtension: "json") else {
                throw Failure.unavailable
            }
            let entries = try JSONDecoder().decode([StarterCatalogEntry].self, from: Data(contentsOf: catalogURL))
            guard entries.count == 8, Set(entries.map(\.slug)).count == 8 else {
                throw Failure.unavailable
            }
            let packs = try entries.map { entry in
                guard let manifestURL = resources.url(forResource: entry.manifest.replacingOccurrences(of: ".cbor", with: ""), withExtension: "cbor"),
                      let bundleURL = resources.url(forResource: entry.bundle.replacingOccurrences(of: ".cbor", with: ""), withExtension: "cbor") else {
                    throw Failure.unavailable
                }
                let manifest = try Data(contentsOf: manifestURL)
                let bundle = try Data(contentsOf: bundleURL)
                guard entry.manifest == "\(entry.slug).manifest.cbor",
                      entry.bundle == "\(entry.slug).bundle.cbor" else {
                    throw Failure.unavailable
                }
                return Pack(name: entry.name, appID: Data(), manifest: manifest, bundle: bundle)
            }
            let verified = try verifyStarterCatalog(
                slugs: entries.map(\.slug),
                manifestBytes: packs.map(\.manifest),
                bundleBytes: packs.map(\.bundle)
            )
            guard verified.count == packs.count else { throw Failure.unavailable }
            return try zip(entries, zip(packs, verified)).map { entry, pair in
                let (pack, record) = pair
                guard entry.name == record.name,
                      entry.appID.lowercased() == RiotDirectoryRow.hex(record.appId) else {
                    throw Failure.unavailable
                }
                return Pack(name: record.name, appID: record.appId, manifest: pack.manifest, bundle: pack.bundle)
            }
        } catch {
            throw Failure.unavailable
        }
    }
}
```

Add `starterBundle: Bundle = .main` to `bootstrap`, wire production bootstrap to
the following conversion, and map `Failure.unavailable` to the fixed user-safe
error:

```swift
let packs = try starterPacks ?? StarterPackLoader.load(bundle: starterBundle)
    .map { (manifest: $0.manifest, bundle: $0.bundle) }
```

On that one catalog failure, set `canRetryBootstrap = true`, keep
`isProfileOpen = false`, and do not expose a repository/nearby host. Add
`retryBootstrap()` that retries the same retained `starterBundle` through the
same Rust verifier; production retains `Bundle.main`, while tests retain their
injected bundle. It clears the fixed error and retry flag only after a successful
open.

In `ConferenceShellView`, route `canRetryBootstrap` to a persistent full-window
`StarterRecoveryView` before rendering any tabs. It explains “Starter tools are
missing or damaged. Reinstall Riot if retry keeps failing.” and offers one
primary **Retry** action with identifier `starter-retry`. It has no close,
Not now, tab bar, Nearby, or keyboard escape path into the shell. Other operation
errors retain the existing dismissible OK alert. A secondary **Technical
details** disclosure reveals only `RIOT-STARTER-001`. `ShellNavigationTests` proves
the recovery route cannot be dismissed, retry uses the same catalog boundary,
disclosure contains no raw underlying error, and discovery remains closed while
the catalog is invalid.

Test-injected `starterPacks` remains supported for isolated repository tests and
takes precedence over `starterBundle`.

Implement `verify_starter_catalog` in `apps_ffi.rs` as an exported wrapper over a
new `pub(crate)` `mobile_state::verify_starter_catalog`. It requires the exact
`STARTER_SLUGS`, verifies every supplied pair, compares every derived ID/name to
the corresponding authoritative `STARTER_CATALOG` record, and uses the existing
private app-error mapping.
The FFI record is a concrete `VerifiedStarterRecord { app_id: Vec<u8>, name:
String }`; no JSON metadata is returned as authority.
Run `cargo run --locked --package xtask -- generate-bindings`, then verify the
generated Swift/Kotlin contract is clean. In `RiotProfileRepository.open`, make
starter installation fail the open when any supplied starter pair fails instead
of using `try?`; retained user-carried packs continue to be individually
quarantined for backward compatibility. Update
`testCorruptedStarterPairIsSilentlySkipped` in `AppRepositoryTests` to assert
that a corrupt named starter throws and leaves no open profile.

- [ ] **Step 3: Add every exact resource to both projects**

After the current project-file owners release both files, add `starter-catalog.json` plus these pairs to the iOS app/test resource phases and macOS app/`RiotKitTests-macOS` phases:

```text
checklist, supply-board, roll-call, quick-poll,
chat, dispatches, wiki, photo-wall
```

Each slug has `.manifest.cbor` and `.bundle.cbor`. Use unique PBX IDs, add one file reference and one build file per target membership, and do not touch source build phases. Re-read both complete project files and run `plutil -lint` only if Xcode accepts the pbxproj as a plist; the authoritative check is `xcodebuild -list` plus both builds.

- [ ] **Step 4: Prove both built products contain the same exact set**

Create `scripts/apps/audit-starter-artifacts.sh` with the exact deterministic
build and byte audit:

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

rm -rf build/catalog-ios build/catalog-macos build/catalog-audit
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64'
xcodebuild build -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -derivedDataPath build/catalog-ios
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS'
xcodebuild build -project apps/macos/Riot.xcodeproj -scheme Riot-macOS \
  -destination 'platform=macOS' -derivedDataPath build/catalog-macos
mkdir -p build/catalog-audit
jq -r '"starter-catalog.json", (.[] | .manifest, .bundle)' \
  fixtures/apps/starter-catalog.json | sort > build/catalog-audit/expected.txt
find build/catalog-ios/Build/Products/Debug-iphonesimulator/Riot.app -type f \
  -exec basename {} \; | rg '^(starter-catalog\.json|.+\.(manifest|bundle)\.cbor)$' \
  | sort > build/catalog-audit/ios.txt
find build/catalog-macos/Build/Products/Debug/Riot.app -type f \
  -exec basename {} \; | rg '^(starter-catalog\.json|.+\.(manifest|bundle)\.cbor)$' \
  | sort > build/catalog-audit/macos.txt
diff -u build/catalog-audit/expected.txt build/catalog-audit/ios.txt
diff -u build/catalog-audit/expected.txt build/catalog-audit/macos.txt
while IFS= read -r name; do
  cmp "fixtures/apps/$name" "build/catalog-ios/Build/Products/Debug-iphonesimulator/Riot.app/$name"
  cmp "fixtures/apps/$name" "build/catalog-macos/Build/Products/Debug/Riot.app/Contents/Resources/$name"
done < build/catalog-audit/expected.txt
```

Run:

```bash
bash scripts/apps/audit-starter-artifacts.sh
```

Expected: both diffs are empty, each list has exactly 17 names, and the loader
tests return eight records. Every `cmp` is silent and exits zero, proving
`starter-catalog.json` and all sixteen CBOR files match, not merely share names.

- [ ] **Step 5: Re-read and commit only Task 2**

Pull/rebase, stage the thirteen exact Task 2 paths, inspect the cached diff, run `sh scripts/green.sh`, and commit:

```bash
git commit -m "fix(apple): ship every starter community tool"
```

## Task 3: Give Riverside real organizer-signed tool trust

**Files:**
- Modify: `fixtures/demo/riverside/content.json`
- Modify: `crates/riot-core/src/demo_fixture.rs`
- Modify: `crates/riot-core/tests/demo_fixture_drift.rs`
- Modify: `fixtures/demo/riverside/demo-space.riot-evidence`

- [ ] **Step 1: Write failing organizer/trust drift assertions**

After ordinary import, compute the recognized organizer as the namespace bytes. Require exactly one accepted Trust marker from that coordinate for each of the eight starter IDs and Shift Signup's content-derived ID. Also construct a marker value from one seeded member and prove `is_trusted` ignores it. Import `is_trusted`, `trust_markers_for`, `TrustMarker`, and `TrustMarkerKind` from `riot_core::apps::trust`.

```rust
let namespace_id = subspace_from_hex(
    "66dbc1bd4b9484434559f09b376a6e8743559c46cae246ef680c1a44b540fe08",
);
let recognized_organizers = vec![namespace_id];
let shift_id = shift_signup.app_id;
let mut approved_ids = verify_starter_catalog(STARTER_CATALOG)
    .into_iter()
    .map(|app| app.app_id)
    .collect::<Vec<_>>();
approved_ids.push(shift_id);
approved_ids.sort_unstable();
approved_ids.dedup();
assert_eq!(approved_ids.len(), 9);

assert_eq!(recognized_organizers, vec![namespace_id]);
for app_id in approved_ids {
    let markers = trust_markers_for(&store, &namespace_id, &app_id).expect("trust markers");
    assert_eq!(markers.len(), 1, "one live organizer coordinate per app");
    assert!(is_trusted(&app_id, &markers, &recognized_organizers));
}
let member_marker = TrustMarker {
    app_id: shift_id,
    author_subspace_id: by_name["Ana"],
    kind: TrustMarkerKind::Trust,
    timestamp_micros: u64::MAX,
};
assert!(!is_trusted(&shift_id, &[member_marker], &recognized_organizers));
```

Run:

```bash
cargo test -p riot-core --features conformance --test demo_fixture_drift -- --nocapture
```

Expected: FAIL because Riverside contains no recognized organizer marker.

- [ ] **Step 2: Derive the namespace from a deterministic organizer author**

Rename the fixture field to `organizer_subspace_secret_seed`. In the generator, derive the organizer signing key first and use its public subspace bytes as the communal namespace. Reject any generator state where organizer subspace and namespace differ.

Keep the existing seed value
`75e730b2fa9866ee898783be8abf68ba56c6c66f6f9fcaba8858a89327159efa`;
as a subspace seed it deterministically derives the even/communal public ID
`66dbc1bd4b9484434559f09b376a6e8743559c46cae246ef680c1a44b540fe08`,
which is Riverside's current namespace, so this authority correction does not
silently move the demo into a different community.

```rust
let organizer = organizer_from_seed(text(space, "organizer_subspace_secret_seed")?)?;
let namespace_id = *organizer.namespace_id().as_bytes();
if organizer.subspace_id().as_bytes() != &namespace_id {
    return Err("demo organizer must occupy the recognized namespace coordinate".into());
}
```

Define the helper in the same file; it is the deterministic conformance twin of
`generate_space_organizer_author` and rejects a seed whose public key is not
communal-shaped:

```rust
fn organizer_from_seed(seed_hex: &str) -> Result<EvidenceAuthor, String> {
    let seed = hex32(seed_hex)?;
    let subspace_secret = SubspaceSecret::from_bytes(&seed);
    let namespace_id =
        NamespaceId::from_bytes(subspace_secret.corresponding_subspace_id().as_bytes());
    if !namespace_id.is_communal() {
        return Err(format!(
            "organizer_subspace_secret_seed {seed_hex} derives a non-communal namespace id"
        ));
    }
    Ok(EvidenceAuthor::from_parts_for_tests(namespace_id, &seed))
}
```

Keep the seed confined to conformance generation. The production bundle carries
only signed public entries and continues to enter through `loadDemoSpace`; this
plan adds no general import/export path for the public demo seed.

- [ ] **Step 3: Write nine ordinary Trust markers**

After Shift Signup's app ID is derived and the eight starters verify, sort the
nine IDs. The fixture generator has an entry vector, not an `EvidenceStore`, so
encode and sign each ordinary marker directly using the same codec/path/signing
functions as production:

```rust
const TRUST_MARKER_BASE_MICROS: u64 = 1_782_940_000_000_000;

for (offset, app_id) in approved_ids.iter().enumerate() {
    let offset = u64::try_from(offset).map_err(|_| "too many demo trust markers")?;
    let marker = TrustMarker {
        app_id: *app_id,
        author_subspace_id: *organizer.subspace_id().as_bytes(),
        kind: TrustMarkerKind::Trust,
        timestamp_micros: 0,
    };
    let payload = encode_trust_marker(&marker)
        .map_err(|error| format!("encode trust marker: {error:?}"))?;
    let path = app_index_trust_path(app_id, organizer.subspace_id().as_bytes())
        .map_err(|error| format!("trust marker path: {error:?}"))?;
    entries.push(sign_at(
        &organizer,
        &path,
        &payload,
        TRUST_MARKER_BASE_MICROS + offset,
    )?);
}
```

Do not add a local `trustedAppIDs` bit and do not sign with Ana, Priya, or the importing profile.

Also add this exact top-level object to `content.json`:

```json
"shift_signup": {
  "written_at_unix": 1782997200,
  "rows": [
    {"key":"shifts/court-support","label":"Court support, Thursday 9am","starts_at":1783069200000,"taken_by_id":""}
  ]
}
```

After Shift Signup's `app_id` is derived, sign that row at its ordinary app-data
path with the organizer (the imported entry remains public evidence, not a
special runtime seed):

```rust
let shift_signup = field(content, "shift_signup")?;
for row in list(shift_signup, "rows")? {
    let key = text(row, "key")?;
    let value = serde_json::json!({
        "label": text(row, "label")?,
        "starts_at": number(row, "starts_at")?,
        "taken_by_id": text(row, "taken_by_id")?,
    });
    let payload = serde_json::to_vec(&value)
        .map_err(|error| format!("encode shift-signup row: {error}"))?;
    let path = app_data_path(&app_id, key)
        .map_err(|error| format!("shift-signup path: {error:?}"))?;
    entries.push(sign_at(
        &organizer,
        &path,
        &payload,
        willow_micros(number(shift_signup, "written_at_unix")?)?,
    )?);
}
```

The fixture drift test requires exactly this open row, so Take this shift cannot
pass against an empty fake page.

- [ ] **Step 4: Regenerate and prove deterministic fixture authority**

Run:

```bash
cargo run -p riot-core --features conformance --example pack_demo_space
cargo test -p riot-core --features conformance --test demo_fixture_drift
cargo test -p riot-ffi --test apps_contract
```

Expected: committed bytes equal a second rebuild; nine exact organizer markers survive ordinary import; member marker remains ignored.

- [ ] **Step 5: Re-read and commit only Task 3**

Pull/rebase, stage the four exact Task 3 paths, inspect the cached diff, run `sh scripts/green.sh`, and commit:

```bash
git commit -m "fix(demo): seed organizer-approved Riverside tools"
```

## Task 4: Auto-admit complete trusted packages and prove clean member use

**Files:**
- Modify: `crates/riot-core/src/apps/index.rs`
- Modify: `crates/riot-core/tests/apps_index_io.rs`
- Modify: `crates/riot-ffi/src/apps_ffi.rs`
- Modify: `crates/riot-ffi/src/mobile_state.rs`
- Modify: `crates/riot-ffi/tests/apps_contract.rs`
- Modify: `apps/ios/Riot/Core/ProfileRepository.swift`
- Modify: `apps/ios/Riot/AppModel.swift`
- Modify: `apps/ios/Riot/Directory/DirectoryModel.swift`
- Modify: `apps/ios/Riot/Directory/DirectoryView.swift`
- Modify: `apps/ios/RiotTests/AppRepositoryTests.swift`
- Modify: `apps/ios/RiotTests/DirectoryStorefrontTests.swift`
- Modify: `apps/ios/RiotTests/DirectoryRepositoryTests.swift`
- Modify: `apps/ios/RiotTests/DemoModeTests.swift`
- Modify: `apps/ios/RiotUITests/ChecklistFlowUITests.swift`
- Modify: `apps/macos/Riot.xcodeproj/project.pbxproj`
- Create: `apps/macos/Riot.xcodeproj/xcshareddata/xcschemes/RiotUITests-macOS.xcscheme`

- [ ] **Step 1: Make the real directory expose recoverable packages**

Add RED Rust tests with these exact contracts:

```rust
fn manifest_without_bundle_is_a_pending_record()
fn invalid_bundle_is_a_pending_record_not_an_indexed_app()
fn ffi_complete_listing_suppresses_pending_duplicate_for_same_app_id()
fn ffi_missing_bundle_preserves_valid_organizer_trust_without_becoming_launchable()
fn ffi_invalid_bundle_has_fixed_invalid_package_state()
```

Today only the missing-bundle case reaches core `pending_manifests`; invalid
pairs vanish, and `mobile_state::directory_listings` ignores every pending
record, so the invalid and FFI tests must fail before implementation. In
`scan_app_index`, send both a missing bundle and a present-but-invalid bundle to
`pending_manifests`; neither enters `apps`, trust, or supersession.

Update `PendingManifest`'s documentation to say “no matching verified bundle”
so the type contract covers both states without claiming invalid bytes are still
in flight, and add `PendingPackageState::{MissingBundle, InvalidBundle}`. Update the existing invalid-bundle assertions in
`apps_index_io.rs` from `pending_manifests.is_empty()` to the exact quarantined
record; they must continue asserting `apps.is_empty()`. Implement the match as:

```rust
match bundles.get(&(app_id, namespace_id, subspace_id)) {
    Some(bundle_bytes)
        if verify_app_pair(&candidate.bytes, bundle_bytes).ok() == Some(app_id) =>
    {
        candidates.push((
            namespace_id,
            subspace_id,
            IndexedApp {
                app_id,
                manifest: candidate.manifest,
                bundle_present: true,
                provenance: AppProvenance::Carried { carrier_subspace_id: subspace_id },
                manifest_timestamp_micros: candidate.timestamp_micros,
            },
        ));
    }
    None | Some(_) => pending_manifests.push(PendingManifest {
        claimed_app_id: app_id,
        manifest: candidate.manifest,
        carrier_namespace_id: namespace_id,
        carrier_subspace_id: subspace_id,
        manifest_timestamp_micros: candidate.timestamp_micros,
    }),
}
```

Keep the existing deterministic sort. In FFI, first assemble complete listings,
collect their app IDs, then deterministically keep at most one pending carrier
per remaining claimed ID. Append a `DirectoryListing` with the decoded manifest
copy, `bundle_present: false`, `installed: false`, `built_in: false`, the pending
carrier, empty `trusted_in_spaces`/endorsements, and no supersession. Never run
`is_trusted` for this row: without a verified bundle, its claimed ID is display
and recovery context only, not execution authority. Run:

```rust
let complete_ids = listings
    .iter()
    .map(|listing| listing.app_id)
    .collect::<std::collections::BTreeSet<_>>();
let mut pending_by_id = std::collections::BTreeMap::new();
for pending in pending_manifests {
    if !complete_ids.contains(&pending.claimed_app_id) {
        pending_by_id.entry(pending.claimed_app_id).or_insert(pending);
    }
}
for pending in pending_by_id.into_values() {
    output.push(crate::apps_ffi::DirectoryListing {
        app_id: pending.claimed_app_id.to_vec(),
        name: pending.manifest.name,
        description: pending.manifest.description,
        version: pending.manifest.version,
        author_signing_key_id: pending.manifest.author.signing_key_id.to_vec(),
        permissions: pending.manifest.permissions,
        bundle_present: false,
        built_in: false,
        installed: false,
        carrier_subspace_id: Some(pending.carrier_subspace_id.to_vec()),
        trusted_in_spaces: trusted_spaces_for_claimed_id,
        endorsing_met_subspaces: Vec::new(),
        endorsing_unmet_count: 0,
        superseded_by: None,
        package_state: match pending.state {
            PendingPackageState::MissingBundle => DirectoryPackageState::MissingBundle,
            PendingPackageState::InvalidBundle => DirectoryPackageState::InvalidBundle,
        },
    });
}
```

Add closed UniFFI enum `DirectoryPackageState::{Complete, MissingBundle,
InvalidBundle}` to every listing. Evaluating a signed organizer marker for a
valid manifest's claimed app ID is safe even before bytes arrive: it records
authority but does not make the package launchable. Therefore missing-bundle
rows preserve accepted organizer Trust in `trusted_in_spaces`; invalid-bundle
rows preserve the marker for explanation but remain `.InvalidBundle` and can
never open/admit. `bundle_present` remains false for both. Authority and package
availability are independent facts.

Name the converted complete-listing vector `output`; destructure `ScannedIndex`
up front so `pending_manifests`, `apps`, `endorsements`, and `spaces` can each be
moved once without cloning.

Run:

```bash
cargo test -p riot-core --test apps_index_io
cargo test -p riot-ffi --test apps_contract ffi_missing_bundle_preserves_valid_organizer_trust_without_becoming_launchable -- --exact
```

Expected: complete apps appear once; missing/invalid pairs appear once as
non-launchable recovery rows; a complete carrier suppresses every pending
duplicate for that ID.

- [ ] **Step 2: Write executable repository admission failures first**

Do not ask a Riverside member repository to manufacture organizer mutations:
after import, Swift intentionally has only the fresh member secret. Put marker
authority/order tests in Rust, where signed organizer/member entries are
constructible, and put repository persistence/projection tests in Swift.

First close the legacy-authority migration hole with RED tests:

```swift
func testLegacyTrustedAppIDWithoutSignedMarkerIsClearedNotReissued() throws
func testOrganizerTrustAndRevokeReceiptsSurviveRelaunch() throws
func testMemberCannotTurnLegacyCacheBitIntoAuthority() throws
func testSnapshotWithoutAppAuthorityBundlesStillOpens() throws
```

Add `AppRuntimeSession.trust_app_with_receipt` and
`untrust_app_with_receipt`; both return the exact signed bundle produced by
`commit_local_app_entries`. Add `replay_app_authority_bundle`, which accepts
only verified `app-index/<id>/trust/<organizer>` entries and rejects app-data,
manifests, bundles, alerts, and profile cards. `PersistedProfile` gains
`appAuthorityBundles: [Data]` with `decodeIfPresent ?? []`.

On `RiotProfileRepository.open`, replay those signed authority receipts before
computing listings, then replace `persisted.trustedAppIDs` with the exact IDs
whose current accepted recognized-organizer marker is Trust. Delete the current
loop that calls `trustApp` for each legacy cached ID. `trustApp`/`untrustApp`
must call the receipt-returning FFI methods, persist the receipt first, then
refresh the projection cache. A legacy true bit with no signed receipt becomes
false and never causes a Willow write. This is a fail-closed migration, not a
grandfather path.

Add these real-boundary tests:

```swift
func testCompleteOrganizerTrustedCarriedAppIsAdmittedAcrossRelaunch() throws
func testAdmissionProjectionLeavesCompleteUntrustedCarriedAppUnavailable() throws
func testAdmissionProjectionLeavesIncompleteTrustedCarriedAppRecoverable() throws
func testAdmissionProjectionIgnoresMemberSignedTrust() throws
func testAdmissionProjectionTreatsLatestOrganizerRevokeAsUnavailable() throws
func testOneRejectedTrustedPackageDoesNotBlockLaterTrustedPackages() throws
func testPackageRecoveryTechnicalDetailsExposeOnlyStableCodes() throws
```

The first test imports the committed Riverside fixture and asserts
`installedApps()` includes Shift Signup immediately, persists its exact verified
manifest/bundle, reopens the repository, and can open an execution session. It
does not mutate authority; at this task boundary that is the existing
`AppRuntimeLaunch`, which Task 6 replaces with the opaque Rust session. The
other projection tests call the internal
admission helper with deterministic `DirectoryListing` values and injected
retain closures, then assert no persisted carried pack and no session for
untrusted, incomplete, member-only, or latest-Revoke states.

In `crates/riot-ffi/tests/apps_contract.rs`, add organizer/member profiles using
the existing public create/join/sync APIs and prove the real accepted-marker
projection for organizer Trust, member Trust, organizer Revoke, and later
organizer Trust. Rust holds the organizer profile that can legally write both
transitions; no test-only secret injection or privileged Swift API is added.

The isolation test calls an internal admission helper with two complete trusted
listings and an injected retain closure that throws `MobileError.AppRejected`
for the first and succeeds for the second; it requires the later app ID in the
report. The separate real-Riverside happy test proves actual persistence/open.
The Rust transition test keeps verified carried bytes across Revoke (bytes are
not authority), and requires the directory row plus `is_app_trusted` to become
untrusted. Task 6 adds and tests `open_app_execution` refusal and proves an
already-mounted WebView closes;
Task 4 makes no standalone safe-runtime completion claim.

Run focused `AppRepositoryTests`. Expected: the happy test FAILS because the current directory requires explicit Get.

- [ ] **Step 3: Implement deterministic trusted-package admission**

Add one internal repository method (also required by `DirectoryPorting`) and call
it after successful demo import, after accepted sync refresh, and when a person
taps **Check again**:

```swift
public struct TrustedAdmissionReport: Equatable {
    let admittedAppIDs: [String]
    let unavailableAppIDs: [String]
}

func admitCompleteTrustedCarriedApps() throws -> TrustedAdmissionReport {
    try admitCompleteTrustedCarriedApps(
        listings: appRuntime.directoryListings(),
        retain: self.retainDirectoryApp(appID:)
    )
}

func admitCompleteTrustedCarriedApps(
    listings: [DirectoryListing],
    retain: (Data) throws -> RiotSpaceApp
) throws -> TrustedAdmissionReport {
    guard let space = persisted.space else { throw RepositoryError.noCurrentSpace }
    let currentNamespace = space.namespaceID.lowercased()
    var admitted: [String] = []
    var unavailable: [String] = []
    for listing in listings {
        guard listing.bundlePresent,
              listing.trustedInSpaces.contains(where: { RiotDirectoryRow.hex($0) == currentNamespace }),
              installedApp(appID: RiotDirectoryRow.hex(listing.appId)) == nil
        else { continue }
        do {
            _ = try retain(listing.appId)
            admitted.append(RiotDirectoryRow.hex(listing.appId))
        } catch let error as MobileError where error == .AppRejected {
            unavailable.append(RiotDirectoryRow.hex(listing.appId))
            continue
        }
    }
    return TrustedAdmissionReport(admittedAppIDs: admitted, unavailableAppIDs: unavailable)
}
```

Factor the complete body of `getCarriedApp`—`installFromDirectory`,
`appPairBytes`, `retain`, installed-array deduplication, `PersistedAppPack`
deduplication, save, and notification—into private
`retainDirectoryApp(appID:)`. `getCarriedApp` returns that helper's result, and
organizer-trusted admission calls the same helper. Call admission after a
successful `loadDemoSpace` and from `RiotAppModel.refreshFromStore()` before
`refreshApps()`, so an accepted sync is reprojected through the same rule. Only
`MobileError.AppRejected` is isolated per package; storage/save/profile errors
still abort the pass because continuing could lie about persistence. A rejected
package leaves no partial `installed` or `PersistedAppPack` record. Ordinary UI
gets the fixed unavailable state, never raw Rust/CBOR/path text.

`refreshFromStore()` catches an admission error only at the model boundary,
keeps already-installed tools visible, and sets exactly “Riot couldn’t refresh
community tools. Try again.” It must not overwrite successfully refreshed board,
identity, or organizer state and must not feed `String(describing:)` to UI.

- [ ] **Step 4: Make storefront state truthful and role-aware**

Extend `DirectoryPorting` with `isOrganizer()` and
`admitCompleteTrustedCarriedApps()`. Keep row construction pure by passing the
role explicitly:

```swift
static func make(
    listing: DirectoryListing,
    installed: RiotSpaceApp?,
    space: RiotSpace?,
    canApprove: Bool
) -> RiotDirectoryRow
```

Replace `.get`/`.arriving` with explicit `.addToDevice`,
`.recoverPackage(code:)`, `.reviewListing`, and `.unavailable` cases. The matrix
is exhaustive:

```swift
switch (installed, trusted, listing.bundlePresent, canApprove) {
case let (.some(app), true, _, _): availability = .open(app)
case let (.some(app), false, _, true): availability = .review(app)
case (.some(_), false, _, false): availability = .unavailable
case (.none, _, _, _) where listing.packageState == .invalidBundle:
    availability = .recoverPackage(code: "RIOT-PACKAGE-INVALID")
case (.none, true, false, _): availability = .addToDevice
case (.none, true, true, _):
    availability = .recoverPackage(code: "RIOT-PACKAGE-ADMISSION")
case (.none, false, _, true): availability = .reviewListing
case (.none, false, _, false): availability = .unavailable
}
```

`RiotDirectoryModel.retryPackage` runs the admission pass and refreshes. If the
refreshed row remains `.recoverPackage` (including a skipped pending record), it
shows “This tool isn’t ready on this device yet.”;
if the pass throws, it shows “Riot couldn’t check this tool. Try again.” Both
messages preserve the row and never include raw error text. The view renders
**Check again** for `.recoverPackage` and a **Nearby** button that
calls `model.select(.connection)` only when a non-demo shareable community
exists. In Riverside it instead says “Reload the demo to restore its tools” and
keeps **Check again**; it never routes the public-seed demo into Nearby.
`.unavailable`
renders “This tool isn’t available in this community. Ask an organizer to turn
it on.” with no Review/approval or acquisition button. `.addToDevice` appears
only when the current organizer marker is Trust and `packageState ==
.missingBundle`; it uses that exact label and confirms that acquiring bytes does
not grant authority. An unapproved missing package is member-unavailable or
organizer Review, never Add to this device. Each recovery row has **Technical
details** revealing only its fixed package code. Use stable identifiers
`directory-add-<name>`, `directory-retry-<name>`, and
`directory-nearby-<name>` for those actions. Add assertions that
Riverside Shift Signup is `.open`, incomplete trusted is `.recoverPackage`, a
member-held untrusted app is `.unavailable`, and only an organizer sees
`.review`.

- [ ] **Step 5: Replace leftover-state UI tolerance with clean Riverside tests**

Add `RiotUITests-macOS` to the macOS project, hosted by `Riot-macOS`, and compile
the existing `apps/ios/RiotUITests/ChecklistFlowUITests.swift` into both UI-test
targets. Replace the one leftover-tolerant test with nine named test methods and
a `launchCleanRiverside()` helper. The helper uses the existing
`RIOT_PROFILE_ID` seam with a new UUID for every test, launches, taps the
existing `demo-load` control, and waits for admission. Tests never share a
profile; Checklist alone relaunches within its own test using the same ID to
prove persistence.

```swift
let app = XCUIApplication()
app.launchEnvironment["RIOT_PROFILE_ID"] = "riverside-ui-\(UUID().uuidString)"
app.launch()
app.buttons["demo-load"].tap()
XCTAssertTrue(app.buttons["open-Checklist"].waitForExistence(timeout: 10))
XCTAssertFalse(app.buttons["review-Checklist"].exists)
XCTAssertFalse(app.buttons["review-Shift Signup"].exists)
XCTAssertFalse(app.buttons["directory-review-Shift Signup"].exists)
XCTAssertFalse(app.buttons["directory-add-Shift Signup"].exists)
app.buttons["open-Checklist"].tap()
// Add "Bring water", toggle it, relaunch, and prove it persists.
```

Run this exact interaction matrix on iPhone and macOS, returning with
`app-close` between tools:

| Tool | Required committed UI action |
| --- | --- |
| Checklist | add `Bring water`, toggle it, relaunch, observe it checked |
| Needs & Offers | enter `Two folding tables`, Post item, Mark resolved, reopen and observe resolved |
| Events | Create event, fill title/date/place, Save event, RSVP, reopen and observe RSVP |
| Decisions | Ask a new question, fill two choices, Post question, cast vote, reopen and observe vote |
| Chat | enter `Court at nine`, Send, reopen and observe message |
| Dispatches | Write a dispatch, fill title/body, Publish, reopen it after closing the tool |
| Wiki | open Meeting guide, Edit page, append text, Save page, reopen and observe edit |
| Photo Wall | choose UI-test resource `courtyard.svg`, enter caption, Share photo, reopen and observe caption |
| Shift Signup | Take this shift, close/reopen while still taken and observe local identity + **Give it back**; give it back, close/reopen again, and observe **Take this shift** |

For every row first wait for `open-<visible name>`, assert the matching Spaces
and Directory Add/Review controls are absent, and assert the resulting text/state
before closing. Assert the UI identifies the local profile as member and never
renders `approve-app`. Do not accept Review-or-Open and do not reuse simulator
state. The focused `DirectoryRepositoryTests` prove the incomplete-package
recovery state and button-driving model actions without inventing a production
launch seam. Playwright remains exhaustive browser-contract evidence, but does
not replace either native UI run.

For iOS, convert the checked-in SVG to PNG, boot the named simulator, and import
it into that simulator's photo library before launching XCUITest:

```bash
mkdir -p target/ui-fixtures
sips -s format png fixtures/apps/photo-wall/courtyard.svg \
  --out target/ui-fixtures/courtyard.png
xcrun simctl boot 'iPhone 17 Pro' 2>/dev/null || true
xcrun simctl bootstatus 'iPhone 17 Pro' -b
xcrun simctl addmedia 'iPhone 17 Pro' target/ui-fixtures/courtyard.png
```

The iPhone test taps Choose a photo, selects the first Recents item, and waits
for Ready to share. On macOS the test enters the absolute checked-in SVG path in
the standard file panel. Both assert the visible caption after Share, so opening
the picker without a committed bridge write cannot pass.

The Shift Signup test deliberately observes the intermediate taken state before
undoing it. Seeing only the initial open state after Take→Give back would be
indistinguishable from a runtime that persisted neither write and is not an
acceptable persistence proof.

- [ ] **Step 6: Run cross-layer proof**

Run:

```bash
cargo test -p riot-core --features conformance --test demo_fixture_drift
cargo test -p riot-ffi --test apps_contract
npx --yes playwright@1.61.1 test --config scripts/apps/playwright.config.mjs
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64'
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' -only-testing:RiotUITests/ChecklistFlowUITests
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS -destination 'platform=macOS'
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotUITests-macOS -destination 'platform=macOS'
sh scripts/green.sh
```

Expected: all commands GREEN; clean Riverside shows Open and a committed primary
action for all nine tools on both Apple platforms. This proves fixture/import/
admission/host behavior on the tested local devices, not BLE.

- [ ] **Step 7: Re-read and commit only Task 4**

Pull/rebase, reread all sixteen Task 4 paths, stage only those paths, inspect `git diff --cached`, rerun the focused tests and `scripts/green.sh`, then commit:

```bash
git commit -m "fix(tools): open trusted Riverside apps directly"
```

## Task 5: Keep the public-seed demo local-only

**Files:**
- Modify: `apps/ios/Riot/Core/ProfileRepository.swift`
- Modify: `apps/ios/Riot/Transport/SpacePairing.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`
- Modify: `apps/ios/Riot/Peers/PeerProfileView.swift`
- Modify: `apps/ios/Riot/Directory/DirectoryModel.swift`
- Modify: `apps/ios/Riot/Directory/DirectoryView.swift`
- Modify: `apps/ios/RiotTests/DemoModeTests.swift`
- Modify: `apps/ios/RiotTests/SpaceAdoptionTests.swift`
- Modify: `apps/ios/RiotTests/DirectoryRepositoryTests.swift`
- Modify: `apps/ios/RiotTests/ShellNavigationTests.swift`
- Modify: `apps/ios/RiotUITests/ChecklistFlowUITests.swift`

- [ ] **Step 1: Write failing local-only lifecycle tests**

Add tests for all exits by which Riverside could be presented as a real
community:

```swift
func testDemoSpaceIsUsableLocallyButHasNoNearbyAnnouncement() throws
func testPairingEncodesNilInsteadOfDemoNamespace() throws
func testDemoCannotOpenSyncBoundary() throws
func testDemoCannotShareAnAppIntoItsNamespace() throws
func testDemoOmitsInviteShareAndNearbyActions() throws
func testDemoContainmentSurvivesRepositoryAndAppRelaunch() throws
func testOrdinaryPublicCommunityStillAnnouncesInvitesAndShares() throws
```

The first test imports the real committed fixture, observes nine locally open
tools, and then asserts `shareableNearbySpace == nil`. The pairing test uses the
existing recording `NearbyConnection`, decodes the first `SpaceAnnounceCodec`
frame, and requires `nil`; it never merely inspects a Boolean. The repository
tests require fixed `RepositoryError.demoOnly` from both `openSyncBoundary()`
and `shareApp(appID:)` before either reaches FFI. Shell/directory/peer projections
must omit Connect, Nearby, “Share to this space,” and “Invite to space” actions
while the demo is active. A normal organizer-created public community exercises
the opposite assertions to prevent a blanket feature disable.

The relaunch test reopens the same `ProtectedProfileStorage`, proves
`isDemoSpaceLoaded == true`, `shareableNearbySpace == nil`, and the same fixed
`.demoOnly` errors, then launches the app twice with one `RIOT_PROFILE_ID` and
asserts Connect, invite, Directory share, and package Nearby controls remain
absent after the second process launch while Checklist still opens with its
persisted item. Immediate-after-import assertions do not satisfy this test.

Run the four focused XCTest classes. Expected: FAIL because
`NearbySpaceHost.currentSpace` currently returns Riverside and `SpacePairing`
encodes it directly.

- [ ] **Step 2: Make shareability an explicit repository capability**

Replace the transport protocol's ambiguous `currentSpace` read with:

```swift
public protocol NearbySpaceHost: AnyObject {
    var shareableNearbySpace: RiotSpace? { get }
    func joinSpace(_ space: RiotSpace) throws
    func openSyncBoundary() throws -> MobileSyncSessionBoundary
}
```

`RiotProfileRepository.shareableNearbySpace` returns `nil` when
`isDemoSpaceLoaded`, otherwise `currentSpace`. `SpacePairing.begin` encodes only
`host.shareableNearbySpace`, and its decision compares against the same value.
`RiotProfileRepository.openSyncBoundary` and `shareApp` independently throw
`.demoOnly` when the demo flag is set, so a stale UI or direct caller cannot
bypass presentation policy.

Expose read-only `canPresentCurrentCommunity` through the shell/directory ports.
It is true only for a non-demo public community. Gate the Connect destination,
Peer invite, Directory share, and recovery Nearby buttons on that capability;
for Riverside show the local-only sentence and the already-defined reload/
Check-again recovery instead. Do not delete, rewrite, or special-case imported
entries: local Tools and app data remain ordinary signed evidence.

- [ ] **Step 3: Prove local use and every presentation denial together**

Run:

```bash
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/DemoModeTests \
  -only-testing:RiotTests/SpaceAdoptionTests \
  -only-testing:RiotTests/DirectoryRepositoryTests \
  -only-testing:RiotTests/ShellNavigationTests
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS'
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotUITests/ChecklistFlowUITests/testDemoContainmentSurvivesRelaunch
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotUITests-macOS \
  -destination 'platform=macOS' \
  -only-testing:RiotUITests/ChecklistFlowUITests/testDemoContainmentSurvivesRelaunch
sh scripts/green.sh
```

Expected: all tests GREEN; Riverside's nine local tools remain open, the exact
demo namespace never appears in a pairing announcement, sync/app-share entry
points refuse it, and ordinary public communities retain their actions. This is
a code-enforced lifecycle rule, not reliance on presenter discipline.

- [ ] **Step 4: Re-read and commit only Task 5**

Pull/rebase, reread all eleven Task 5 paths, stage only those paths, inspect the
cached diff, rerun the focused suites and `scripts/green.sh`, then commit:

```bash
git commit -m "fix(demo): keep Riverside authority local-only"
```

## Task 6: Replace raw app-ID authority with revocable execution sessions

**Files:**
- Modify: `crates/riot-core/src/apps/trust.rs`
- Modify: `crates/riot-core/tests/apps_trust.rs`
- Modify: `crates/riot-ffi/src/apps_ffi.rs`
- Modify: `crates/riot-ffi/src/mobile_state.rs`
- Modify: `crates/riot-ffi/src/mobile_api.rs`
- Modify: `crates/riot-ffi/tests/apps_contract.rs`
- Regenerate (ignored build output): `build/generated/riot-ffi/**`
- Modify: `apps/ios/Riot/Core/ProfileRepository.swift`
- Modify: `apps/ios/Riot/Apps/AppRuntimeView.swift`
- Modify: `apps/ios/Riot/Apps/AppBridgeController.swift`
- Modify: `apps/ios/Riot/Apps/AppSchemeHandler.swift`
- Modify: `apps/ios/Riot/Apps/RiotJS.swift`
- Create: `apps/ios/Riot/Apps/AppNetworkBackstop.swift`
- Modify: `apps/ios/RiotTests/AppRuntimeHostTests.swift`
- Modify: `apps/ios/RiotTests/AppSyncReplicationTests.swift`
- Modify: `apps/ios/RiotTests/AppRepositoryTests.swift`
- Create: `apps/ios/RiotTests/NetworkBackstopTests.swift`
- Modify: `scripts/apps/miniapp-browser.spec.mjs`
- Modify: `apps/ios/Riot.xcodeproj/project.pbxproj`
- Modify: `apps/macos/Riot.xcodeproj/project.pbxproj`

- [ ] **Step 1: Write the Rust session contract RED**

In `apps_contract.rs`, open a trusted installed app and exercise every operation
through one opaque `AppExecutionSession`, never through the legacy raw app-ID
methods:

```rust
execution_session_serves_resource_identity_profile_and_data()
execution_session_put_returns_a_persistable_receipt()
execution_session_watch_delivery_revalidates_and_cancel_is_idempotent()
execution_session_revoke_fails_every_operation_before_result_or_commit()
execution_session_namespace_replacement_fails_every_operation()
execution_session_marker_replacement_changes_approval_generation()
execution_session_rejects_wrong_webview_nonce_or_origin()
execution_session_explicit_close_and_double_close_are_safe()
execution_session_wrong_package_and_untrusted_open_are_rejected()
```

For each invalidation case, table-drive `resource`, `whoami`, `profile`, `get`,
`list`, `put_with_receipt`, `begin_watch`, and `deliver_watch`. Snapshot the
target app-data key before the stale `put_with_receipt` call and prove it is
unchanged afterwards. The organizer and member are separate real profiles using
the existing public sync harness; organizer Revoke/Trust entries are therefore
legal and no test-only authority hook is introduced.

The nonce/origin test opens with a 32-byte random test nonce and exact
`riot-app://<app-id>` origin, then table-drives every operation with one changed
nonce byte, wrong scheme, wrong host, and a subframe flag. All must invalidate
before response or write.

Run:

```bash
cargo test -p riot-ffi --test apps_contract execution_session -- --nocapture
```

Expected: FAIL because the opaque object and open method do not exist.

- [ ] **Step 2: Implement one Rust-owned authorization object**

Add UniFFI records `AppExecutionDescriptor` and `AppExecutionResource`, plus an
opaque `AppExecutionSession`.
`AppRuntimeSession.open_app_execution(app_id, webview_nonce, expected_origin,
navigation_generation)` is the only constructor. At open, under the profile
mutex, it:

1. requires the exact verified installed manifest/bundle pair;
2. captures the current communal namespace and profile/subspace identity;
3. captures the manifest digest and content-derived app ID;
4. resolves the latest accepted marker at the recognized-organizer coordinate,
   requires `Trust`, and captures an approval generation derived from that
   marker's signed entry ID plus Trust/Revoke value; and
5. binds a 32-byte WebView nonce, exact `riot-app://<app-id>` origin, main-frame
   requirement, and initial navigation generation; and
6. captures a monotonic `namespace_generation` stored on `LocalProfile` and
   incremented by create/join/demo-load/demo-hide namespace replacement.

The object owns its closed flag and bounded watch-ID set. It exposes only
relative resource/data/profile/watch inputs; no method accepts an app ID or
namespace. Before every response or mutation, `with_valid_execution` acquires
the same profile mutex, verifies closed state, namespace generation and bytes,
installed pair/manifest digest, and current accepted approval generation/kind.
It also requires the bound nonce, origin, main-frame bit, and navigation
generation on every resource/bridge/watch call. A wrong origin/nonce or any
unexpected committed navigation increments the generation and permanently
invalidates the object.
The stale check and data write occur under that one mutex, so Revoke cannot land
between authorization and commit. Invalidated calls return the new closed
`MobileError::AppExecutionInvalidated`; explicit/double close is idempotent and
cancels all watch IDs.

Core's current `trust_markers_for` projection discards the winning Willow entry
ID. Add a parallel `resolved_trust_markers_for`/`ResolvedTrustMarker` projection
that preserves `{ marker, entry_id }` from the same live-entry scan, and make the
old marker-only function map over it. `apps_trust.rs` tests prove equal timestamp
payload-digest winners produce different entry IDs/generations and that
Trust→Revoke→Trust changes the generation each time. FFI must consume this
resolved projection; it may not synthesize approval generation from a local
Boolean or timestamp alone.

Keep legacy raw app-data functions for non-WebView compatibility tests only,
mark them as not valid host authority, and add a source contract assertion that
the Apple runtime files contain no calls to them. Regenerate bindings with:

```bash
cargo run --locked --package xtask -- generate-bindings
sh scripts/conference/test-native-core-package.sh
```

The generator intentionally writes ignored `build/generated/riot-ffi/**`, which
both Xcode projects and Android compile directly. Do not stage or hand-edit that
output. `apps_ffi.rs` and `mobile_state.rs` are the committed symbol definitions;
the package test proves fresh Swift and Kotlin bindings contain the new object
before either Apple build compiles against it.

- [ ] **Step 3: Write Swift invalidation and teardown tests RED**

Add focused tests that mount the real native host and prove:

- accepted sync carrying organizer Revoke closes an already-open tool with
  “This tool is no longer available here” plus a Technical details disclosure
  containing only `RIOT-APP-INVALIDATED`, before another watch callback;
- a post-Revoke JavaScript put rejects and the key remains unchanged;
- changing/hiding the current community closes the tool;
- top-level navigation away, Web content process termination, representable
  dismantle, and coordinator deinit call `close()` and cancel watches once;
- resource requests after invalidation receive a fixed unavailable response,
  never cached bytes; and
- a message from a subframe, wrong `WKSecurityOrigin`, wrong host, or wrong
  nonce is rejected before dispatch and closes the session; and
- repository replacement destroys the old coordinator/session before the new
  repository can mount a tool.

Use an injected `ExecutionSessionPort` spy only for lifecycle-count assertions;
the revoke/write tests use the real Rust session and committed marker sync. Run
`AppRuntimeHostTests` and the focused `AppSyncReplicationTests`; expect RED.

- [ ] **Step 4: Route every Apple host operation through the opaque session**

`RiotProfileRepository.openAppExecution(appID:webViewNonce:origin:)` converts
the selected ID once and returns the Rust session plus its descriptor. The
`AppRuntimeLaunch` initializer creates the 32-byte nonce with
`SecRandomCopyBytes`, derives the exact origin from the verified app ID, and
passes both into open before constructing the coordinator; randomness
failure aborts launch. It exposes a
`persistExecutionReceipt(_:)` callback that saves only the signed receipt
returned by session `put_with_receipt`; the WebView bridge never receives a raw
app ID. Replace `AppRuntimeDataBridge` with an adapter over the execution object.
Make `whoami`/`profile` throwable so invalidation cannot be converted to fallback
identity. Make `AppSchemeHandler` resolve resources through the same session.

Inject the nonce into `RiotJS` as a closure-private constant and include it in
every bridge/watch envelope. `AppBridgeController` rejects unless the envelope
nonce matches, `message.frameInfo.isMainFrame` is true, and
`frameInfo.securityOrigin` has protocol `riot-app` plus the descriptor's exact
app-ID host. It forwards that validated nonce/origin/navigation generation to
Rust on every call. The controller never dispatches a body based only on its
shape. `AppSchemeHandler` likewise supplies the bound nonce/origin to resource
calls. The nonce is cleared from Swift ownership and invalidated in Rust on
close; it is never persisted or exposed as a `window` property.

`AppRuntimeCoordinator` owns the session and one `onInvalidated` closure. It
validates before data-change/watch delivery and foreground callbacks, closes on
navigation outside `riot-app://`, Web content termination, dismantle, or any
`AppExecutionInvalidated` error, removes the script handler/observer, cancels
watches, calls Rust `close()`, then dismisses once. `RiotJS.watch` registers a
host watch ID and returns an unsubscribe closure; `pagehide` cancels remaining
watches. No stale callback may evaluate JavaScript after close.

The containing shell retains the fixed invalidation notice after dismissal with
primary **Return to Tools** and secondary **Technical details**; the disclosure
shows only `RIOT-APP-INVALIDATED`. It never flashes a blank host or raw FFI error.

- [ ] **Step 5: Add the CSP-independent WebKit network wall RED→GREEN**

In `NetworkBackstopTests`, serve a deliberately CSP-stripped hostile page from
the `riot-app` scheme. Start loopback TCP and UDP canaries and attempt external
`fetch`, XHR, WebSocket, image/script/link subresources, form submit,
`window.open`, download, custom scheme navigation, DNS prefetch, WebRTC/STUN,
beacon, and media/geolocation/powerful APIs. Assert no canary connection or
datagram, no secondary window/download, fixed promise refusal, and no bridge on
the foreign navigation. Also assert a normal `riot-app` script/style/image
resource still loads.

Implement `AppNetworkBackstop.prepare(configuration:) async throws` with a
compiled `WKContentRuleList` that blocks every URL then ignores the block only
for `^riot-app://`. Compilation must finish and the list must be installed
before `WKWebView.load`; compilation failure closes the tool without loading a
page. Keep the navigation/UI/download delegates as independent top-level,
window, media-permission, and download denials.

`WKContentRuleList` does **not** govern WebRTC/ICE/STUN. In the same preparation
step, install an `atDocumentStart`, `forMainFrameOnly: false` lockdown script in
the page content world before any app script. It defines the network-capable
constructors/functions `RTCPeerConnection`, `webkitRTCPeerConnection`,
`RTCDataChannel`, `WebSocket`, `EventSource`, `Worker`, `SharedWorker`,
`navigator.sendBeacon`, and service-worker registration as non-configurable,
non-writable refusing values. It also freezes the relevant navigator objects.
The navigation delegate rejects `about:blank`, `srcdoc`, and non-`riot-app`
subframes, so an app cannot mint an uninstrumented realm. Media capture and
geolocation delegates always deny. Tests first prove the lockdown descriptors
cannot be reassigned/deleted, then attempt RTC offer/ICE gathering and require
the UDP STUN canary to remain silent. A control `WKWebView` without the lockdown
must reach that canary, proving the test can detect the path rather than passing
because WebRTC is unavailable on the runner.

The rule list—not CSP or the navigation delegate—is the network-process HTTP/
WebSocket/subresource wall; the frozen all-frame API lockdown is the separate
WebRTC/STUN/worker wall. Neither is described as covering the other's mechanism.
Browser-side
`miniapp-browser.spec.mjs` covers the same API matrix, but only the native
loopback-canary test counts as Apple backstop proof.

Add `AppNetworkBackstop.swift` to both app targets and
`NetworkBackstopTests.swift` to both test targets in the same project-file
commit. Re-read both pbxproj files and verify no dangling build-file reference.

- [ ] **Step 6: Prove the complete safe native path**

Run:

```bash
cargo test -p riot-ffi --test apps_contract execution_session -- --nocapture
cargo test -p riot-ffi --test apps_contract
npx --yes playwright@1.61.1 test --config scripts/apps/playwright.config.mjs
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotTests/AppRuntimeHostTests \
  -only-testing:RiotTests/AppSyncReplicationTests \
  -only-testing:RiotTests/NetworkBackstopTests
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS'
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -only-testing:RiotUITests/ChecklistFlowUITests
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotUITests-macOS \
  -destination 'platform=macOS'
sh scripts/green.sh
```

Expected: all GREEN. Rerun the nine-tool matrix after the host migration so the
functional proof is against the opaque session, not the superseded raw-ID
bridge. This proves local simulator/macOS runtime containment and writes; it
does not prove physical BLE.

- [ ] **Step 7: Re-read and commit only Task 6**

Pull/rebase, reread every Task 6 path, stage only those exact files, verify the
cached diff contains definitions for every new symbol and both Xcode target
memberships, rerun Step 6 plus coverage, then commit:

```bash
git commit -m "feat(apps): revoke opaque execution sessions safely"
```

## Task 7: Final audit and honest handoff

**Files:**
- Modify coordination record only: `COLLABORATION.md`

- [ ] **Step 1: Prove artifact parity from built products**

Run:

```bash
bash scripts/apps/audit-starter-artifacts.sh
```

Expected: its two name-set diffs and all 34 byte comparisons exit zero. This is
fresh final-audit evidence from both built `.app` products, not reuse of Task 2
output or an inspection of the source tree.

- [ ] **Step 2: Run all quality gates**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace --all-features
sh scripts/green.sh
jq -r '.enforcement.command' .coverage-thresholds.json | sh
rm -rf build/ios-coverage build/macos-coverage
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -derivedDataPath build/ios-coverage -enableCodeCoverage YES
xcrun xccov view --report build/ios-coverage/Logs/Test/*.xcresult \
  > target/xccov-ios.txt
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS' -derivedDataPath build/macos-coverage \
  -enableCodeCoverage YES
xcrun xccov view --report build/macos-coverage/Logs/Test/*.xcresult \
  > target/xccov-macos.txt
```

Expected: every command is GREEN and all four coverage counts are exactly 100%.
If coverage is red, Task 0A regressed or the new code introduced
an uncovered branch; return to the owning task and add behavior tests. Do not
mark this plan complete or request review with a red count. Attach the separate
iOS and macOS `xccov` reports to the handoff and call out every changed Swift
file's measured line coverage; neither report is mislabeled as Rust branch/
function/statement coverage or replaced by `green.sh`.

- [ ] **Step 3: Record proof versus assumptions**

Mark the claim done with commit hashes and exact commands. State separately:

- proved: eight pairs in each tested Apple artifact; exact organizer markers; clean member direct-open; local primary writes and relaunch;
- proved: incomplete/invalid carried-package isolation, member-vs-organizer action states, persistent bootstrap/package recovery, and demo-only presentation denial;
- proved: opaque execution-session revalidation and close on Revoke/namespace/teardown, no stale write in the tested races, and the CSP-independent Apple network wall against the listed local canaries;
- not proved: two-physical-iPhone BLE, protected/private sync, the community navigation redesign, OS/network vectors outside the enumerated containment suite, or behavior on hardware not named in the commands.

Name the continuation in order so this slice does not become a cul-de-sac:
readable Home/posting plus the adaptive single-community shell before the first
community-first product trial; Nearby ownership/recovery next; durable
multi-community selection after those foundations. Runtime containment is done
by this plan and must not be listed as a future assumption. None of the remaining
items is relabeled as done by this tools milestone.

- [ ] **Step 4: Leave the checkout reviewable**

Run `git status --short` and `git diff --cached`. Expected: no staged files and no uncommitted changes from this plan; unrelated shared-session changes remain named, untouched, and unclaimed by this worker.
