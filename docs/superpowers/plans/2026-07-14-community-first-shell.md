# Community-First Shell — Implementation Plan

## Execution progress (2026-07-15)

| Unit | State | Commit | Evidence |
|---|---|---|---|
| P1 native rebuild | ✅ done | — | 5 staticlibs, `nm` confirms symbols |
| P2 in-flight app work | ✅ done | `cb90b14` | iOS 198 / Android 110 |
| P3 coverage baseline | ✅ done | `2040ccc`, `e43286c` | 94.60%→96.47%; 121 lines unreachable; **100% gate is unreachable — owner decision open** |
| dead-code cleanup | ✅ done | `0e5c34c` | phantom `closed` guard + orphaned oracle helpers deleted |
| **1A newswire projection** | ✅ done | `d150e79` | 746 Rust tests (isolated); bindings+staticlibs rebuilt; macOS RiotKit compiles. **This is the CurrentEntryV2 deviation — owner ratification open** |
| **0A canonical catalog** | ✅ done | `09413e6` | Rust 8/8; iOS StarterResourceTests 7/7; iOS app builds with all 8 pairs |
| **0C runtime containment (SECURITY)** | ✅ done | `d9699e8` | Rust 75 suites (apps_contract 44/44); iOS 209 (2 known-red); Android 113/0; JS egress 5/5+2/2. Gate reaches the page (end-to-end revocation test through the REAL bridge); §4.7 revoked→Return-to-Tools via is_valid(); CSP-stripped egress backstop, fail-closed. **⚠️ KNOWN RESIDUAL — see Risk 9.** |
| **0B Riverside authority** | ✅ done | `1276024` | 75 Rust suites; iOS RiversideMemberToolUITests SUCCEEDED; full iOS 205 (only 2 known-red). **Also fixed a real 1A bug:** the FFI list/inspect path never handled newswire entries (`is_newswire_prefix` missing from `inspectable_entries` + `list_current_entries`) — closed with a demo-independent regression test |
| **1B post an update** | ✅ done | `7758beb` | PostUpdateTests 12/12 (iOS + macOS); iOS app builds; no FFI change |
| **1C known contributors** | ✅ done | `774f629` | PeopleSurfaceTests 10/10 (incl. adversarial organizer-impostor); Rust contributors 6/6 + FFI 2/2. Clippy regression later cleared in `0ab1ccd` |
| **1E newswire merge & share** | ✅ done | `eb5392f` (Rust/FFI) + `9b9c90b` (native) | Rust import 13/13 + FFI share 4/4; iOS NewswireShareTests 4/4 (golden-digest byte-identity); Android 3/3; macOS builds. Cross-platform golden harness built. **⚠️ Android digest identity assumed — Risk 10.** |
| **2A adaptive shell (THE PAYOFF)** | ✅ done | `0a07d59` | iOS 254 tests / 252 pass (only 2 known-red); ShellNavigationTests 28/28; iOS Riot.app + macOS Riot-macOS both BUILD SUCCEEDED; no FFI change. Four routes (Home/Tools/People/Nearby); create-community signs `SpaceDescriptorV1` with founding roster. **⚠️ joined/loaded-community Home has no content yet — Risk 11 (Unit 3 closes).** |
| **2B nearby ownership & recovery (SECURITY)** | ✅ done + ratified | `e9a1b77` + `4cbebee` | Independent security-auditor verdict **HOLDS** — no pre-confirm community-metadata disclosure on either iOS transport or Android; **found + fixed a REAL leak** (auto-connect/accept + immediate `SpaceAnnounce`). Bilateral confirmation gate; import/switch race fails closed; coordinator survives routing. iOS 266/264 (2 known-red); TransportContractTests 43/43; both apps build. Hardened 2 phantom/untested gate gaps (raw-byte namespace opacity + anti-fingerprint call-site). No FFI. Known property: fixed BLE service UUID = passive app-presence beacon (not community metadata). |
| **2C editorial actions, front page & open wire** | ✅ done + ratified | `7ea4ad1` | iOS 285/283 (2 known-red); NewswireSurfaceTests 19/19; Android RiotControllerNewswireTest 13/13; both apps build. Non-editor action **IGNORED — effect absent** (proven through REAL core: roster-excludes-founder → hide rejected AND post treatment stays `.ordinary` on re-projection, not just a hidden control); closed field table all 6 kinds; 3 distinct empty states; cross-platform identity by reading core's `frontPage`/`openWire`/`editorialHistory` verbatim. `AlertDetailView` removed (Home-only). Android native-signing rejection assumed-not-proven (host-JVM, Risk 10 pattern). No FFI. |

Combined tree after all: workspace Rust green, clippy 0, fmt clean; full iOS 254 tests / 252 pass (2 known-red Bonjour only); iOS + macOS apps build.

**Next up:** 2B (nearby ownership & recovery — coder + security review on the both-devices-confirm gate), then 2C (editorial actions / front page / open wire), then 3 (multiple communities). 2B and 2C are **serial** — both add Swift test files → both need the pbxproj, which §4/Rule-5 serializes.

**Recurring finding across 1A/0A/0B:** four times now a subsystem looked finished from one entry point but had an unhandled path from another (newswire write-vs-read, catalog Rust-vs-Apple, session guard that never fired, newswire create-vs-list). Verify both directions.

**Two owner decisions still open** (see the P3 escalation block below): (1) the coverage threshold — the 100% Tarpaulin gate cannot be met; (2) ratify 1A's projection-completion in place of the gate-approved `CurrentEntryV2`.

**Two flagged code questions** (non-blocking, in the P3 block): `MAX_SYNC_INVENTORY_BYTES` is a no-op bound; the `store/backup.rs` double-checks.

---

**Status:** IN EXECUTION — plan review gate ran its full 3 iterations (9 independent adversarial reviewers, all fresh instances).

| Iteration | Feasibility | Completeness | Scope & Alignment |
|---|---|---|---|
| 1 | FAIL | FAIL | FAIL |
| 2 | **PASS** | FAIL | **PASS** |
| 3 | **PASS** | FAIL | **PASS** |

Iteration-3 Completeness raised 8 findings — **all 8 are incorporated into this revision** (`ai_assisted` + `operational_profile` + founding `editorial_roster` folded into 1A's single FFI change; the full recovery-state contract as §4.7; "Pending exchange" status; the complete 6-task/4-fail-condition trial gate; focused proofs for 1B/1C/1E/3; 2C's closed editorial field table; 2A's founding-roster deviation disclosed). They were not rebutted — they were fixed. Per the gate's own rule, 3 iterations are exhausted, so this goes to the user rather than a 4th round.

**Pattern worth noting:** Feasibility and Scope each passed twice consecutively. Completeness failed all three times, but on *progressively finer-grained* design fidelity each round (round 1: whole missing subsystems → round 3: individual empty-state copy contracts). That is a ratchet converging, not a plan that is fundamentally broken.
**Date:** 2026-07-14
**Designs (gate-approved):**
- `docs/superpowers/specs/2026-07-13-community-first-navigation-design.md` (5/5) — normative for navigation
- `docs/superpowers/specs/2026-07-13-multi-community-open-newswire-mvp-design.md` — normative for the newswire product

**Objective:** Riot stops being a five-tab debug shell and becomes a community-first product: find or create a community, see *what's happening* (Home), *do useful work* (Tools).

---

## 1. Verified starting state (measured 2026-07-14)

| Signal | Result |
|---|---|
| `cargo test --workspace --all-features` | **PASS** — ~290 tests, 0 failures |
| `cargo clippy --workspace --all-features` | **PASS** — 0 warnings |
| `cargo fmt --all -- --check` | **PASS** |
| Gateway `python3 -m unittest` | **PASS** — 18 tests |
| `npm run test:web:unit` | **PASS** — 29 tests |
| `cargo tarpaulin --workspace --all-features --fail-under 100` | **FAIL — 94.60%** (8214/8683; **469 uncovered lines**) |
| `scripts/web/coverage.sh` toolchain | **RESOLVES** — all pins present, incl. all five native rustup targets |

Essentially zero `todo!()` in 46k lines of Rust. **Not built:** private groups / MLS (CI-forbidden, §8.4), read capabilities of any kind, owned-namespace capability minting, any web client.

---

## 2. Correction: the newswire is a THIN SLICE

Revision 1 of this plan claimed `NewswireProjectedPost` already carried headline/expiry/author/history. **That was false.** Verified:

```rust
// crates/riot-ffi/src/newswire_ffi.rs:81-94
pub struct NewswireProjectedPost {
    pub entry_id: String, pub author_id: String,   // raw hex, not a rendered name
    pub body: Option<String>, pub source_claims: Vec<String>,
    pub treatment: NewswirePostTreatment,          // no headline, no expiry, no ordering key
}
pub struct NewswireProjectionView {
    pub open_wire: Vec<NewswireProjectedPost>,
    pub front_page: Vec<NewswireProjectedPost>,    // no `earlier`, no `editorial_history`
}
```
- Core `ProjectedPost` (`newswire/projection.rs:82-91`) also has **no headline**.
- `create_newswire_post` **hardcodes** `expires_at_unix_seconds: None`, `event_time_unix_seconds: None` (`newswire_ffi.rs:142-143`); `NewswirePostInput` has no expiry field.
- Today's `CurrentEntry` (`mobile_api.rs:71-78`) *does* carry headline + `AlertFreshness{created_at, valid_from, expires_at}`. **Swapping Home onto the newswire as-is would be a downgrade.**

**But the data is already signed and stored.** `NewsPostV1` (`newswire/model.rs:73-84`) holds `headline`, `event_time_unix_seconds`, `expires_at_unix_seconds`, `source_claims`, `ai_assisted` — the projection simply drops them. And the author-rendering path exists: `profile/resolver.rs` exports `resolve_display_names`, `render_display_name`, `key_tag` (`profile/mod.rs:9` calls `render_display_name` "the only sanctioned way to display one").

**So Unit 1A = carry existing signed fields through the projection + reuse the existing name resolver.** Real work, but plumbing — not a new content model. It is also mandatory for newswire DoD #6, so building `CurrentEntryV2` on the alert model would mean building these same fields twice.

**Decision: complete the newswire projection surface (Unit 1A) instead of adding `CurrentEntryV2`.** This deviates from the nav design's gate-approved Unit 1A text — it is *more* work than that text, and it is a prerequisite of the other approved design.

> **⚠️ CONFIRM #2:** This is the plan's **only** deviation from a gate-approved normative design. Approve it, or Unit 1A reverts to the approved `CurrentEntryV2` text and Unit 3 inherits the newswire migration. **No agent may start 1A without this sign-off.**

---

## 3. Scope boundary — what this plan does and does not deliver

**This plan delivers the navigation design's four destinations, its interaction/accessibility contracts, and the in-app half of the newswire MVP.** It does **not** deliver the public-web half.

| Newswire DoD | Delivered? | Where |
|---|---|---|
| 1. Create/follow multiple public spaces | Yes | 2A (create/join, signs `SpaceDescriptorV1`) + 3 (multi) |
| 2. Reopen last available; switch in one action | Yes | 3 |
| 3. Publish the same signed record from Riot **or a web browser** | **NO — deferred** | — |
| 4. Readable locally; merges from nearby, file, **or gateway** | **Partial** — nearby + file (Unit 1E); **gateway deferred** | 1E, 2B |
| 5. Editor signs feature/verify/correct/hide/tombstone/retract | Yes | 2C |
| 6. Every client derives same front page, open wire, editorial history | Yes (for the clients that exist — iOS/macOS/Android) | 1A + 2C |
| 7. Space exposes approved apps alongside Newswire | Yes | 0A/0B/2A |
| 8. Gateway rebuildable from signed public data | **NO — deferred** | — |

**Deferred to a follow-on plan: the public web surface (DoD #3, #8, gateway half of #4).** There is no web client in `apps/` at all, and `apps/gateway/riot_gateway.py` contains **zero** newswire code (it renders static alert exports). This is a distinct product surface and roughly doubles the plan.

**Also deferred (newswire design, explicitly out of scope here):** quorum/threshold signatures, editorial-roster rotation UI, post editing/deletion, ranking, private/connections-only spaces, canonical global directory.

> **⚠️ CONFIRM #1:** you chose "community-first product shell," described as navigation + multi-community newswire MVP. This plan builds the **app**. If you want the **web/gateway** half in scope, say so and I add Units 4A–4C — expect roughly double the work.

---

## 4. Global rules every unit obeys

These are not per-unit reminders; they are invariants. Violating one is a failed unit.

0. **Paths in this plan are shorthand.** `xtask/…`, `riot-core/…`, `riot-ffi/…` all live under `crates/`. Resolve `xtask/src/main.rs` → `crates/xtask/src/main.rs`, etc. Line numbers are exact against the real files.
1. **Xcode project membership.** Both `apps/ios/Riot.xcodeproj` and `apps/macos/Riot.xcodeproj` use explicit `PBXFileReference` entries (verified: zero `PBXFileSystemSynchronizedRootGroup` in either). **Every new Swift file must be hand-registered in BOTH project files.** `scripts/green.sh`'s own header names "a Swift file committed but never added to an Xcode target" as a failure mode that "cost us hours." This applies to **P2, 0A, 1B, 1C, 1E, 2A, 2B, 2C** — every unit that adds a Swift file, not just 2A.
2. **The macOS test target is a SUBSET of iOS**, not a mirror (verified: `ShellNavigationTests` appears in the iOS pbxproj, **zero** times in the macOS one). Never claim "iOS + macOS tests pass" as if symmetric; state which suites ran where.
3. **No unit begins while either Apple project file is owned or dirty in another session** (nav design's hard gate). Claim files first (§7.1).
4. **FFI record changes are compile-breaking across platforms.** Kotlin constructs UniFFI records **positionally** — e.g. `RiotController.kt:135` builds `NewswirePostInput(spaceDescriptorEntryId, headline, body, …)`. Adding a field breaks Android compilation. Any unit touching an FFI record must regenerate bindings, rebuild the native core, and fix **all** call sites on both platforms in the same unit.
5. **Coverage is four-dimensional plus Swift.** `.coverage-thresholds.json` → `scripts/web/coverage.sh` requires 100% Tarpaulin lines **and** LLVM lines/functions/regions/branches **and** authored-JS coverage. Nav Prerequisite C additionally requires `xccov` reports on **both** the iOS and macOS schemes. Tarpaulin lines alone are not the gate.
6. **Accessibility is a contract, not a polish pass** (nav design 377–393): VoiceOver identifiers, 44×44 targets, largest Dynamic Type without clipping, selection never by color alone, announcements that don't steal focus, macOS focus restoration, full IDs only behind **Technical details**. Every UI unit asserts these.
7. **The recovery-state contract is also binding** (nav design **362–375** — the whole table, not just the accessibility half). Every UI unit must implement its applicable rows, and **never expose a raw internal error**:
   - *Profile/store loading* → accessible progress, bounded wait, then Retry. **(2A)**
   - *No updates yet* → "Post the first update" / "Find nearby". **(2A)**
   - *No tools* → organizer sees "Add a tool"; a member sees "Find nearby" — **explain the role, never render a dead button**. **(2A)**
   - *Catalog/package failed* → Retry package + **Technical details with a fixed error code**. **(0A)**
   - *Sync interrupted* → Retry, **keep already-accepted content**. **(2B)**
   - *Unauthorized / revoked / stale session* → close to a **named destination ("Return to Tools")** with fixed copy. **(0C)**
   - *Bluetooth / local-network denied* → offer Settings. **(2B)**
   - *No community* / *community unavailable* / *post write-or-sign failure* → **(2A, 2A, 1B)**
   Newswire's three distinct states are **2C's**: *empty wire* ("no reports have arrived"); *posts but no feature* ("the collective has not selected a feature" + link to Open wire); *offline/stale projection* — all three are different, and none may be collapsed into a generic empty view.

---

## 5. Prerequisites

### P1 — Rebuild the native core (blocking, first)
`build/native/` artifacts are **2026-07-13 12:28**, predating newswire FFI commit `df15ac5` (2026-07-14 16:22). Verified: `nm -gU build/native/macos/libriot_ffi.a | grep -ci newswire` → **0**. Bindings declare symbols the libraries lack; **no native target links today**.

**Work:** `sh scripts/conference/build-native-core.sh`. **Exit:** all five artifacts rebuilt; `nm` shows newswire symbols in each. All five rustup targets are already installed, so the most likely failure mode is retired. **If it still fails: report; do not work around it.** Nothing native proceeds without this.

### P2 — Land the in-flight working tree, with real tests
11 modified files (6 iOS, 5 Android), zero tests. **Verified accurate:** `RiotDestination` still has exactly its five original cases and **no Settings or Newswire tab was ever written** — revision 1's "drop workstreams D/E" was a phantom. **Nothing to drop.**

Existing iOS suites are `AppRepositoryTests`, `DirectoryRepositoryTests`, `DirectoryStorefrontTests`, `ShellNavigationTests`, `SpaceAdoptionTests` (there is **no** `ProfileRepositoryTests`/`DirectoryModelTests`/`PeerProfileTests` — revision 2 invented those names). Extend existing suites where they fit; any genuinely new file gets registered in **both** pbxproj (§4.1).

| Behavior | Files | RED test |
|---|---|---|
| App untrust ("Turn off") — iOS | `Core/ProfileRepository.swift`, `AppModel.swift`, `ConferenceShellView.swift` | **`AppRepositoryTests`**: `untrustApp` drops the ID from `persisted.trustedAppIDs` **and survives a simulated relaunch**; `canApproveApps == false` → no control rendered |
| App untrust — Android | `RiotAppsController.kt`, `AppPersistence.kt`, `RiotController.kt`, `MainActivity.kt` | **`RiotAppsControllerTest` — NEW file** (does not exist today; Gradle auto-discovers it. Existing Android suites are `AppPersistenceTest`, `ConferenceSurfaceTest`, `apps/{AppBundleCodecTest, AppResourceResolverTest, DirectoryControllerTest, InstalledAppsStoreTest, RiotJsBridgeTest}`): `untrust()` clears the persisted flag; `isOrganizer()==false` → no control |
| Endorsement retraction — iOS | `Directory/DirectoryModel.swift`, `DirectoryView.swift`, `Core/ProfileRepository.swift` | **`DirectoryRepositoryTests`**: row shows "Take back recommendation" iff `endorsedByMe`; `retract()` clears it |
| Endorsement retraction — Android **(UI missing)** | `apps/DirectoryController.kt`, `MainActivity.kt` *(new UI)* | **`DirectoryControllerTest`**: `retractRecommendation()` → `endorse(appId, "", retract=true)`. **Then add the Android UI affordance** — the controller exists but no user can reach it. |
| `AlertDetailView` tappable rows — iOS | `ConferenceShellView.swift:437` | **`ShellNavigationTests`**: tapping a board row presents detail with entry/signer/validity; **Technical details disclosure** hides full IDs by default (§4.6) |
| `PeerProfileView` copy fix — iOS | `Peers/PeerProfileView.swift` | **`SpaceAdoptionTests`**: a synced out-of-range identity shows "Already in your network", not "No space to invite them to" |

**Keep the inert newswire wrappers** in `ProfileRepository.swift` / `RiotController.kt` — 1A extends them. **⚠️ Note for 1A:** `RiotController.kt:135` constructs `NewswirePostInput` **positionally**; 1A's added fields will break it (§4.4).

**Exit:** both platforms compile against the rebuilt core; new tests pass; `sh scripts/green.sh` green; commit with explicit pathspecs.

### P3 — Coverage baseline (only code no unit rewrites)

**Sequencing, stated honestly:** the enforcement command is a **workspace-wide** `cargo tarpaulin --fail-under 100`. It does not care which file a unit touched. So although nav **Prerequisite C**'s rule is "before any product slice is declared **complete**" (not "before work begins"), the practical consequence is that **these 283 lines must be green before *any* unit can COMMIT.** P3 is therefore a real blocker on the first commit, not on the first keystroke — work on 0A may begin in parallel, but it cannot land until P3 does. Revision 1 overstated this as "nothing begins"; revision 2 understated it as purely a per-unit exit criterion. This is the accurate reading.

Of the **469** uncovered Tarpaulin lines, **186 sit in files that 0B and 1A will rewrite.** Covering those first, then deleting the tests, is pure waste.

| Lines | File | Owner |
|---|---|---|
| 83 | `riot-ffi/src/mobile_state.rs` | **P3** |
| 46 | `riot-core/src/store/database.rs` | **P3** |
| 33 | `riot-core/src/store/evidence.rs` | **P3** |
| 21 | `riot-core/src/store/backup.rs` | **P3** |
| 20 | `riot-ffi/src/mobile_api.rs` | **P3** |
| 16 | `riot-core/src/apps/index.rs` | **P3** |
| 15 | `riot-core/src/session.rs` | **P3** |
| 13 | `riot-core/src/store/schema.rs` | **P3** |
| 8 | `xtask/src/main.rs` | **P3** |
| ~28 | remainder | **P3** |
| 62 | `xtask/src/sign_conference_fixture.rs` | **0B** |
| 48 | `xtask/src/verify_conference_export.rs` | **0B** |
| 17 | `riot-core/src/demo_fixture.rs` | **0B** |
| 27 | `riot-core/src/newswire/store.rs` | **1A** |
| 19 | `riot-core/src/newswire/model.rs` | **1A** |
| 13 | `riot-ffi/src/newswire_ffi.rs` | **1A** |

**P3 scope = 283 lines** in never-rewritten files. Deferred files are covered by the unit that rewrites them, as that unit's own exit criterion. **Each unit exits at 100% (all four LLVM dimensions + Tarpaulin + Swift xccov) or does not commit.** Branch/region/function debt is currently unmeasured — **P3's first task is to measure it** (`cargo llvm-cov --branch`) and add it to this table.

If some line is genuinely unreachable: change `.coverage-thresholds.json` explicitly, with justification, reviewed. Never silently.

---

### ⚠️ P3 RESULT (2026-07-14): the 100% gate appears unreachable. **Owner decision required — I did not change the threshold.**

Coverage moved **94.60% → 96.25%** (8214 → 8357 / 8683) via 52 real error-path tests (commit `2040ccc`). **141 lines remain, and roughly 120 of them are unreachable by construction.** Two real code defects were found in the process and fixed (commit `0e5c34c`).

**The hard blocker — 6 lines no test can ever reach.** `riot-app-cli/src/lib.rs:819,830,841,852,866,952` are **executed but reported zero-hit**. Proof: `src/tests/unit.rs:476` asserts `parse_manifest_input(valid_manifest).is_ok()`, and `:866` (`Ok(ManifestInput {`) is the *only* `Ok`-return in that function. The test passes, so the line runs, and Tarpaulin still reports 0. They are trailing-arg lines of multi-line calls plus a multi-line struct literal — **Tarpaulin region mis-attribution**. Same class: `willow/identity.rs:189`. *Independently verified.* **No amount of testing fixes this. 100% Tarpaulin lines is not achievable on this codebase as configured.**

**The rest, by category:**
- **8** `apps/index.rs` — the import admission gate refuses *every* malformed app-index record, so the store can never hold one and the scanner's per-record error arms cannot fire. Proven by a new test that forges 9 records with raw CBOR (what a hostile peer actually sends) and shows admission rejects all of them. **Keep the guards — they are defence in depth — but they are not coverable.**
- **33** `store/evidence.rs` — `CorruptDatabase` assertions that fire only if SQLite returns a row inconsistent with what this code itself wrote. Schema CHECK/FK constraints make most physically impossible.
- **12** `store/database.rs`, **10** `session.rs`, **6** `store/backup.rs`, **4** `store/schema.rs`, **7** `import/join.rs`, **4** identity/owned (two are retry-exhaustion arms at ~2⁻¹²⁸ probability), **49** `riot-ffi/mobile_state.rs` (panic-catch arms, internal invariants, 256-marker caps needing 257 real commits).
- **2** `xtask/main.rs` — the success arms of 0B's subcommands. **Should land with 0B.**
- **9** `mobile_state.rs:921-935` — **the one genuinely reachable gap.** Sync import-rejection path; reassigned.

**Your options (pick one — do not let an agent decide this):**
1. **Lower the threshold** to a justified figure (e.g. 96–97% lines) with this inventory committed as the rationale. Honest, and unblocks every unit's COMMIT phase.
2. **Keep 100% and switch the metric** — the gate also runs `cargo llvm-cov` (regions/branches/functions), which does not suffer Tarpaulin's line mis-attribution. Measure that first; it may already be attainable.
3. **Keep 100% Tarpaulin lines** — then no unit can ever commit, because 6 lines are impossible. Not viable as written.

**Nothing was silently excluded.** No `#[cfg(not(tarpaulin_include))]` was added, and `.coverage-thresholds.json` is untouched.

**Update (`e43286c`):** the one reachable gap is now closed — the sync import-rejection path is covered by driving the real wire protocol as a hostile peer. **96.25% → 96.47%.** Remaining in P3 scope: **121**, all unreachable for the reasons above.

### ⚠️ SECOND OWNER DECISION: `MAX_SYNC_INVENTORY_BYTES` bounds nothing

`crates/riot-ffi/src/mobile_state.rs:48` declares `const MAX_SYNC_INVENTORY_BYTES: usize = MAX_BUNDLE_BYTES;` — a straight alias. `encode_bundle` already fails at exactly that threshold, so **both** guards behind it (`prospective_sync_inventory` and the inventory revalidation) are unreachable. A named constant that reads like a sync-specific ceiling enforces nothing.

This governs **how much a peer can make us buffer during reconciliation**, so it is security-relevant. Two readings, and I deliberately did **not** guess:
1. A tighter sync-specific bound *was* intended and got lazily aliased → give it a real value below `MAX_BUNDLE_BYTES`. This is a **protocol change**: the two guards become live and need tests.
2. The alias is deliberate and the guards are belt-and-braces → delete both guards and the constant, and rely on `encode_bundle` alone.

Behaviour is unchanged; the constant is now documented in place so nobody reads it as a live bound. **Pick one.**

*(Related, same class — a second check standing behind an equally-strict first one: `store/backup.rs:102,132` and `store/backup.rs:350,354`. Worth a sweep for this pattern.)*

**One more thing worth knowing about sync:** the **count** ceiling (`MAX_SYNC_IDS`) is enforced at the *Summary* step by `ReconcileSession::checked_missing` (`sync/state.rs:179`), two frames before admission — so it can never reach the admission check. The **byte** ceiling is enforced twice. Count once, bytes twice.

---

## 6. Work units

Each runs the metaswarm 4-phase loop (IMPLEMENT → VALIDATE → ADVERSARIAL REVIEW → COMMIT), TDD, RED first, and obeys §4.

**Sequencing note:** content **views** are built and tested in isolation (1B/1C), then **2A arranges them into the new shell**. Editorial surfaces (2C) come *after* 2A, because Home does not exist until 2A — building them into the doomed five-tab shell would be the exact throwaway pattern §2 rejects.

**Sequencing correction (2026-07-14, during execution): 1A now runs BEFORE 0B.**
0B regenerates the Riverside fixture as newswire records; 1A changes the shape of a newswire record (adds headline/expiry/event_time/`ai_assisted` to the projection, `operational_profile` to the input, `editorial_roster` to space creation). Running 0B first would build the fixture against a record shape that 1A immediately invalidates — forcing a second fixture rebuild. Same throwaway logic the plan applies to `CurrentEntryV2` and to P3's coverage deferrals. **Order: P1 → P2/P3 → 1A → 0A → 0B → 0C → 1B/1C/1E → 2A → 2B → 2C → 3.** 1A is pure Rust/FFI + call-site fixes, so it is disjoint from 0A's catalog work.

| Unit | Title | Depends | RED contract → GREEN |
|---|---|---|---|
| **0A** | Canonical catalog & Apple artifacts | P1, P2 | `STARTER_CATALOG` (`apps/starter.rs:81`) has **8** pairs; `AppModel.swift:628` names **4** behind a `.compactMap` that silently drops the rest, and both Apple products bundle only Checklist → **the Tools surface is nearly empty.** **RED:** `apps_starter.rs` asserts the exact ordered 8-pair catalog; new `StarterResourceTests` (register in **both** pbxproj) inspects both built `.app` resource dirs and fails today. **GREEN:** derive Apple resources from Rust's catalog; missing/extra/invalid pairs become **fatal**; delete the `compactMap` tolerance path. |
| **0B** | Deterministic Riverside authority | 0A | The demo profile is a *member* — it cannot approve or use Checklist. The demo cannot demo. **RED:** `demo_fixture_drift` expects the recognized-organizer coordinate + nine Trust markers; `apps_contract` proves member-signed trust is ignored; `RiversideMemberToolUITests` fails on Get/Review. **GREEN:** organizer-shaped namespace + markers; deterministic admission of organizer-trusted packages, no authority bypass. **Also:** regenerate the fixture to emit newswire records (`SpaceDescriptorV1` + `NewsPostV1` + editorial roster) and **restate the drift-snapshot contract** to pin the descriptor path/namespace. **⚠️ Hazard:** the `create_signed_*_with_clock` builders are `#[cfg(feature = "conformance")]` (`newswire/entry.rs:264-295`) — reachable from `riot-core`'s `demo_fixture.rs`/`examples/`, **not** from `xtask`. Regenerate **from inside riot-core**; adding `conformance` to xtask risks tripping the release-closure guard (`xtask/src/main.rs:419`). **Owns coverage:** `demo_fixture.rs`, `xtask/sign_conference_fixture.rs`, `xtask/verify_conference_export.rs`. |
| **0C** | Runtime containment & invalidation | 0B | **Security-critical.** **RED:** `apps_contract` proves revoke / namespace replacement / explicit destruction / stale approval-generation all fail *before* read or commit; Swift `AppRuntimeHostTests` proves the bridge cancels watches and closes UI; hostile-page tests defeat every exfiltration vector **with CSP stripped**. **GREEN:** Rust-owned `AppExecutionSession`, generation revalidation, independent iOS network backstop. |
| **1A** | Complete the newswire projection surface | **0B** *(not 0C — see note)* | *(Replaces `CurrentEntryV2`; §2. **Requires CONFIRM #2.**)* **RED:** `newswire_contract.rs` asserts headline, body, rendered author + key tag, source claims (signed 1–16 order), expiry, event time, ordering key, treatment, `earlier`, `editorial_history` — all failing today; malformed-payload mapping; closed-enum rejection. **GREEN — do the FFI record change ONCE, carrying every dropped field** (a second pass would mean a second Android positional break): add `headline`, `expires_at`, `event_time`, **and `ai_assisted`** to core `ProjectedPost`; extend `NewswireProjectedPost` + `NewswireProjectionView` (`earlier`, `editorial_history`); add expiry, event-time, **and `operational_profile`** to `NewswirePostInput` and **stop hardcoding `None`** for all three (`newswire_ffi.rs:142-143` + `operational_profile: None`); **add the founding `editorial_roster` parameter to `create_newswire_space`** (it currently hardcodes `vec![signer_id]`, `newswire_ffi.rs:111`); reuse `render_display_name`/`key_tag` for the author; regenerate bindings; **rebuild native core; fix the positional Kotlin call site at `RiotController.kt:135` and every other call site on both platforms** (§4.4).
**Why these four extras ride along:** `ai_assisted` is a provenance flag both designs require to survive publication and display; `operational_profile` is what 1B's stricter-fields rule needs; the roster parameter is what 2A's create-community needs. All three are the *same* record change. Doing them in one unit costs one binding regen and one Android fix instead of three. **Non-regression (real suites):** `crates/riot-core/tests/newswire_{codec,entry,import,projection}.rs`, `crates/riot-ffi/tests/{newswire_contract,mobile_contract,persistence_contract}.rs`, Swift `BindingSemanticsTests`, `AppSyncReplicationTests`. `CurrentEntry` is **not** deleted. **Owns coverage:** `newswire/{store,model,projection,entry,path}.rs`, `newswire_ffi.rs`, plus any `session.rs`/`import/bundle.rs` lines it touches. |
| **1B** | Post an update | 1A | **Composer contract (conflict resolved):** the nav design requires source claims + expiry; the newswire design's freeform `NewsPostV1` requires **headline + body only**, sources/expiry optional — and it **explicitly supersedes** the nav design's field requirements for the newswire route. **Follow the newswire design.** Stricter fields apply only when an operational-alert/request profile is selected (**`operational_profile`, added by 1A**). **RED:** outcome-language labels ("Post an update", never "Compose & sign"); model assistance **off** by default; exact review of identity + community before one signed write; **draft survives backgrounding**; fixed failure states (write/sign failure preserves the draft); **ephemeral one-off publishing identity clearly labeled**; **after commit, Home shows the update with a "Pending nearby exchange" status** (nav Posting step 5 / newswire Publishing step 6). **Focused proof:** Rust signing contract (`crates/riot-ffi/tests/newswire_contract.rs`) + new Swift `PostUpdateTests` (register in **both** pbxproj) — one deterministic happy path and one write-failure path. Built as a **view**, tested in isolation; 2A hosts it as a primary Home action. |
| **1C** | Known contributors | 1A | **RED:** DTO/view tests reject membership/presence labels; resolve rendered names **with key tags**; derive organizer **only** from the recognized coordinate; actionable empty state. **GREEN:** `ContributorRowV1` projection + People view. **Focused proof:** Rust projection tests + new Swift `PeopleSurfaceTests` (register in **both** pbxproj). |
| **1E** | Newswire merge & share | 1A | *(newswire DoD #4, non-gateway half — previously unmapped.)* **RED:** the same signed newswire record merges **idempotently** from nearby **and from a file import**, is readable locally before any gateway sees it, and produces byte-identical projections; **shared golden vectors** prove Rust, iOS, and Android encode identical records. **⚠️ Scope warning:** the cross-platform golden-vector harness **does not exist**. The only vector fixture (`fixtures/willow/william3-vectors.json`) is read solely by `crates/riot-conformance/tests/william3_vectors.rs`; **zero** Swift or Kotlin tests consume any shared vector. 1E must first build that harness — bundle the fixture into both Apple test targets and Android test resources — before it can assert byte-identical encoding. Budget for it. Also **RED:** the descriptor's canonical digest is **bound into the join/share reference**, so a relay or gateway cannot silently substitute a different community name or editorial roster (newswire design). **GREEN:** newswire file import + share (link/QR per newswire Data Flows step 5). **Focused proof:** `crates/riot-core/tests/newswire_import.rs` (idempotent merge) + new Swift `NewswireShareTests` and Android `NewswireImportTest` (register Swift in **both** pbxproj). |
| **2A** | Adaptive single-community shell | 1A–1C, 1E | **The payoff.** **RED — `ShellNavigationTests` (rewritten) prove:** the four routes (Home/Tools/People/Nearby); exact iPhone (bottom bar) vs macOS (`NavigationSplitView`, tool in the **detail pane, never a modal**) presentation; **deterministic Home shortcuts** (first four approved tools in canonical catalog order, continuing past unapproved ones — never a hole); **profile/settings relocation** — the avatar opens **Your profile**, a separate gear opens **Community settings** (two distinct labeled paths, distinct macOS sidebar-footer actions); **`Command-1…4`** select destinations, **Escape** returns from a tool when safe; **focus restoration** to the invoking tool card; **dirty tool/post draft requires Stay-or-Discard confirmation before a community change**; launch states (no retained community → *Create a community* / *Find one nearby*, display name inline and skippable; one retained → its Home; unavailable → in-place recovery Retry / Find nearby / Remove-after-confirm, never blank); **the existing tab-lifecycle performance contract still holds.** **Create a community signs a `SpaceDescriptorV1`** via `createNewswireSpace` — and the founding collective **chooses its initial editorial public keys and approved starter apps** (newswire Data Flows step 2). This needs the `editorial_roster` parameter **1A adds**; today the FFI hardcodes `vec![signer_id]` (`newswire_ffi.rs:111`), which would make every user-created community permanently single-editor. *(Roster **rotation** stays deferred per the design; initial **selection** does not.)* **Recovery states (§4.7):** profile/store loading, no-updates, no-tools (role explained, never a dead button), no-community, community-unavailable. **Accessibility (§4.6) asserted.** **Performance proof:** starter tool opens **< 500 ms**, measured with XCTest `measure(metrics:)` on the **agreed demo device — name it before the unit starts** (§8.3). **GREEN:** typed `CommunityContext`/`CommunityRoute` over a selection protocol shaped for future `RiotDatabase`; do not bind views to a singleton space. **Cross-file: §7.** |
| **2B** | Nearby ownership & recovery | 2A | **RED:** routing does not deallocate the coordinator; discovery cannot auto-connect or auto-accept; **both devices confirm before any public metadata disclosure**; switching cancels old callbacks; pre-confirmation metadata is opaque; denied permissions offer Settings; a switch/write/import race **fails closed**. **Focused proof:** `SpaceAdoptionTests`, `LocalNetworkNearbyTests`, `AppSyncReplicationTests`, `TransportContractTests`. **GREEN:** coordinator ownership moves to the selected community; enforce the public-communal visibility gate. |
| **2C** | Editorial actions, front page & open wire | 2A | *(newswire DoD #5/#6. After 2A, because Home must exist.)* The Rust half is **already green** — `create_newswire_editorial_action` exists (`newswire_ffi.rs:158`, six kinds at `:169-174`) and `newswire_contract.rs` already has `editorial_action_hides_a_post()` / `editorial_action_from_non_editor_fails()`. **The failing half is the apps: zero consumers of `frontPage`/`openWire`/`editorialAction` anywhere in `apps/`.** **RED (app-side):** new `NewswireSurfaceTests` (iOS, registered in **both** pbxproj) + `RiotControllerNewswireTest` (Android) prove a recognized editor can sign each of the six kinds; a non-editor's action is ignored; iOS/macOS/Android all derive the **identical** front page, open wire, and editorial history from the same records; **an ordinary hide shows a warning interstitial**; retraction/tombstone treatment renders correctly. **Enforce the closed field table** (newswire design): *feature*/*verify* → reason **forbidden**; *correct* → reason **and** replacement text **both required**, and it renders with the mandatory **"Editorial correction"** label; *hide*/*tombstone*/*retract* → reason **required**, text **forbidden**. **Immutable pre-signing review** shows the complete target entry ID, community, acting editor key, action, reason, and replacement text; a failed sign **preserves the draft**. **"UI visibility is never an authorization check"** — a hidden control and a rejected action are independently tested. **Empty/failure states (§4.7):** empty wire, posts-but-no-feature, offline/stale projection are three *distinct* views. **GREEN:** add the `createNewswireEditorialAction` wrapper (iOS + Android — it does not exist yet) and the Home front-page / open-wire / editorial-history surfaces. |
| **3** | Multiple communities | 2A, 2B, 2C | **RED (before implementation):** the chooser lists name, relationship (organizer/member/public-reader), recent activity, sync freshness in plain language; **returning opens the last available community directly**; if unavailable, the chooser opens and **preserves the record with recovery actions**; switching **cancels in-flight work** and a switch/write race fails closed; communities are **isolated** (no cross-community leakage of entries, app approvals, or coordinator state); archive and restore round-trip; a bad migration is **quarantined**, not silently dropped; **`Command-K`** focuses community selection. **Performance proof:** cached community switch < 300 ms. **GREEN:** resume the reviewed SQLite registry/session work **only after** rewriting its approval projection to this design's organizer-marker rule (the nav design supersedes the old per-device approval rule). **Focused proof:** `crates/riot-ffi/tests/persistence_contract.rs` + the full shell/runtime/sync isolation suites (`ShellNavigationTests`, `AppRuntimeHostTests`, `AppSyncReplicationTests`, `SpaceAdoptionTests`) — all must stay green across a community switch. |

**First product trial = Units 0A–2C together.** Not 0A alone.

---

## 7. Cross-file integration inventory

**`RiotDestination` (6 files):** `apps/ios/Riot/{AppModel,ConferenceShellView}.swift`, `apps/ios/Riot/Design/RiotTabBar.swift`, `apps/ios/RiotTests/{ShellNavigationTests,RiotTabBarTests}.swift`, `apps/ios/RiotUITests/RiotTabNavigationUITests.swift`.
→ All three test files are **rewritten** against the four new routes, not deleted — the new shell still needs coverage.

**`CurrentEntry` (9 tracked source files):** `crates/riot-ffi/src/{mobile_api,mobile_state}.rs`, `crates/riot-ffi/tests/persistence_contract.rs`, `apps/ios/Riot/Core/ProfileRepository.swift`, `apps/android/.../{BindingModels,RiotController,MainActivity}.kt`, `apps/android/.../transport/GeneratedMobileSyncBridge.kt`, `apps/android/app/src/test/.../PersistedSyncImportTest.kt`.
→ **`CurrentEntry` is NOT deleted.** 1A adds the newswire surface alongside it; the alert model still backs `IncidentBoardView` and sync. Retiring it is a later decision.

**Newswire records (1A's blast radius — claim ALL of these in the ledger):** `crates/riot-ffi/src/newswire_ffi.rs` (the FFI records themselves — `:40`, `:81`, `:91`, `:130`, `:191-245`), `crates/riot-core/src/newswire/{model,projection,store,entry,path,mod}.rs`, `crates/riot-core/src/session.rs`, `crates/riot-core/src/import/bundle.rs`; tests `crates/riot-core/tests/newswire_{codec,entry,import,projection}.rs` + `crates/riot-ffi/tests/newswire_contract.rs`; **apps** `apps/ios/Riot/Core/ProfileRepository.swift` (`:962`, `:976-977`), `apps/android/.../RiotController.kt` (`:13-14`, **positional constructor at `:135` — will break**, `:146-147`).

*(Good news, verified: core `NewswireProjection` **already computes** `earlier`, `editorial_history`, and `future_quarantine` — 1A adds three fields to `ProjectedPost` and then surfaces already-computed structures through FFI. Narrower than it looks. UniFFI bindings are generated at build time and not committed, so there is no committed-binding blast radius.)*

**macOS shares the iOS sources** (`apps/macos/Riot/` holds only `RiotMacApp.swift` + plist/entitlements). Every 2A shell change lands in macOS too, and **both** pbxproj files need editing (§4.1). The macOS test target is a **subset** (§4.2).

---

## 8. Execution, dispatch, coordination, risks

### 8.1 Shared-checkout claim protocol — must be re-established
The nav design mandates claiming exact files in `COLLABORATION.md` before every unit. **That file no longer exists** — commit `cff0844` archived it to `docs/archive/COLLABORATION-2026-07.md`. Multiple agent sessions share this checkout, so this control is load-bearing.

**Before Unit 0A:** create a fresh root `COLLABORATION.md` claim ledger (agent, unit, exact paths, status, timestamp). Claim before editing; release on commit. **Explicit pathspecs only — never `git add -A`.** An agent finding foreign edits in its declared scope **stops and reports** rather than merging blind. **No unit starts while either pbxproj is dirty or owned.**

### 8.2 Dispatch
`.metaswarm/external-tools.yaml` **does not exist**, so Codex/z.ai delegation is not wired today. Either (a) **Claude-only** metaswarm orchestration (works now), or (b) create the config for genuine **cross-model** adversarial review (one setup step).

**Units are SERIAL by default.** Revision 2 proposed running 1B/1C/1E in parallel — that is unsafe: all three edit `apps/ios/Riot/Core/ProfileRepository.swift` **and** both pbxproj files, which §8.1 turns into a guaranteed claim deadlock. **Run them serially, or give one agent sole ownership of `ProfileRepository.swift` + the project files across all three.**

| Unit | Agent |
|---|---|
| P1, P2 | coder + platform verification |
| P3 | test-automator |
| 0A, 0B | coder |
| **0C** | **security-auditor** — hostile-page containment with CSP stripped is a security boundary |
| **1A** | coder (Rust/FFI) — highest-risk unit; cross-model adversarial review recommended |
| 1B, 1C, 1E | coder, **serially** (shared file ownership) |
| 2A | coder + designer review; accessibility + visual review mandatory |
| 2B | coder + **security review** (the both-devices-confirm gate) |
| 2C, 3 | coder + designer review |

Every unit gets a **fresh** adversarial reviewer that did not write the code.

### 8.3 Verification
Per unit: focused RED/GREEN → `cargo fmt` + strict Clippy → `cargo test --workspace --all-features` → binding regen + dirty check **and native core rebuild** when FFI changed → JS miniapp tests when the runtime changed → iOS tests/build, **macOS tests/build (subset — state which suites ran)**, Android via `green.sh` → `sh scripts/green.sh` → the four-dimensional coverage gate + `xccov` for touched Swift (§4.5).

**Physical-radio honesty:** loopback and Bonjour on one Mac **do not** prove BLE between two iPhones. Every sync report states proven vs assumed paths separately. This repo already recorded once that "our headline two-peer test never ran." Do not repeat that.

**Whole-product acceptance (the nav design's own gate, in full):** after 2C, run the **measurable trial** — 5 first-time evaluators **per platform**, each starting from the same clean retained-Riverside fixture; timer starts at task-card handover and stops on the observable outcome. **≥4 per platform must complete ALL SIX tasks uncoached:**
1. state the selected community and explain one current update — **≤20 s**;
2. open a named tool from Home and change shared state — **≤30 s**;
3. post an update — **≤60 s** — *and describe what they did as **posting**, not as "signing"*;
4. find **Your profile** and **Community settings**, mistaking neither for Home;
5. identify the community and say whether a nearby peer means **Join / Get changes / Already current / Different community**;
6. on macOS, open a tool via sidebar **and** keyboard, and return with focus intact.

**The trial FAILS outright if any of these is true**, regardless of times: any Riverside tool still says *Review*; technical IDs dominate any primary surface; the Mac shows the phone's bottom bar or opens a tool as a modal sheet; or the report implies untested BLE works.

**Name the demo device before 2A starts** — the <500 ms / <300 ms gates are meaningless without a named device and harness.

Report honestly if it fails. Do not quietly restate the bar.

### 8.4 Knowledge capture
`.beads/knowledge/*.jsonl` are seven untouched templates despite 516 commits — `/self-reflect` has never captured anything. Run it for real before the PR; commit the results.

### 8.5 Risks
1. **P1 is a hard single point of failure.** Nothing native proceeds without it. No workaround authorized.
2. **1A is the riskiest unit:** it changes core projection types, FFI record shapes, and generated bindings, and **breaks Android's positional constructor by construction**. It deviates from gate-approved text (§2) — **needs your sign-off (CONFIRM #2)**.
3. **0C is a security boundary** — security-auditor, not a general coder.
4. **Private groups stay out of scope and CI-forbidden.** `xtask/src/main.rs:414-417` fails the build if `openmls` or `willow25`'s `drop_format` enter the dependency graph. That guard stays until a deliberate, threat-modeled Phase 0B. **After this plan Riot is still the newswire half of the dual-mode vision** — say so honestly in any external communication.
5. **No read capabilities exist anywhere.** `ReconcileSession::select` (`sync/state.rs:236-246`) serves any held entry to any peer requesting the ID — no capability check. Units 0A–3 are **public-communal only**, so this is survivable. **Personal, connections-only, managed, and private communities MUST remain unselectable** until receiver-authenticated, capability-bound sync exists. Never ship a "connections-only" affordance on a wire with no read gate.
6. **Coverage may be unreachable on some lines** → explicit, justified, committed threshold change. Never silent.
7. **Transport logic is duplicated** (~90KB Swift / ~50KB Kotlin) outside the shared core. 2B touches it; do not deepen the duplication.
8. **The web/gateway half is deferred (§3)** — CONFIRM #1.
9. **⚠️ SECURITY RESIDUAL (0C, `d9699e8`) — WebRTC egress is not hard-blocked on iOS.** The hosted-app egress backstop (`WKContentRuleList`, fail-closed) blocks every URL-loader vector, proven with CSP stripped: fetch/XHR/WebSocket/EventSource/beacon/img/script/link/iframe/form/css-url/dns-prefetch/preconnect/favicon → zero connections. But **WebRTC (`RTCPeerConnection` → STUN/TURN) does not flow through the URL loader**, so a content rule list cannot block it. It is disabled *best-effort* via the only lever WKWebView exposes — a **private** `peerConnectionEnabled` preference — which may be an App Store review risk and can silently break on an OS update. So a hostile hosted app could in principle still exfiltrate via WebRTC/STUN. Strictly better than the prior state (no backstop at all), every other vector genuinely closed — but not complete. **Owner threat-model decision:** accept + document as a known limitation, or invest in a stronger block (WKWebView content-world/process network policy, or refuse to host bundles that reference WebRTC APIs — detectable at bundle scan). Do not report app-runtime egress as fully contained until resolved.
10. **CROSS-PLATFORM PROOF RESIDUAL (1E, `eb5392f`+) — Android byte-identity is format-level, not native-encoder.** The golden-vector harness proves **Rust↔iOS** byte-identity by running the *real native encoder* on both platforms against the committed fixture (`fixtures/newswire/newswire-golden-1.json`). **Android proves only string + generated-record-shape parity**: `testDebugUnitTest` runs on a host JVM that never loads `libriot_ffi` (device-ABI `.so` only on jniLibs), so the Android leg recomputes the deterministic share-reference *string* in pure Kotlin and constructs the generated `NewswireShareReference` record (proving the record shape crossed the binding) — it does **not** run the native CBOR/WILLIAM3 encoder. So **descriptor digest byte-identity on Android is assumed, not proven.** Low risk (same Rust staticlib ships to every platform; only UniFFI marshalling could diverge, and record construction exercises that shape). **Way to close:** an Android *instrumentation* test on an emulator/device that loads `libriot_ffi` and recomputes the digest. Deferred — the repo's unit harness is host-JVM by design. Never report "Android byte-identical"; report "Android format-level parity, digest assumed."
11. **PRODUCT RESIDUAL (2A, `0a07d59`) — a joined/loaded community's Home shows no newswire content yet.** Home is fully wired for a community **created in the same session**, but the MVP FFI has **no descriptor-discovery accessor** to re-hydrate the newswire projection for a community that was loaded from storage or joined from a nearby peer, so those communities' Home correctly falls back to the **no-updates recovery state** rather than inventing content. 2A chose this over inventing new FFI (per its dispatch: escalate FFI, don't add it silently). **Way to close: Unit 3's SQLite registry** — its per-community re-hydration is exactly the missing accessor; 3 must surface the persisted descriptor so Home reprojects on load/join. Until then, the shell demo is honest only for freshly-created communities. Track so 2B (nearby join) and 3 don't assume joined-Home content exists.
12. **⚠️ FLAGGED DESIGN DECISION (Unit 3) — per-community sealed identity. OWNER RATIFICATION, like CONFIRM #2.** Faithful multi-community requires holding *organizer-of-A* and *member-of-B* **simultaneously with identity continuity**, but today `LocalProfile` holds ONE `author` and `join_public_space` **destructively regenerates** it (both key-gen paths mint fresh random keypairs, non-re-derivable). The approved specs only gesture at this ("the real immutable SpaceSession"). **Decision made in the owner's absence (2026-07-15), two parts:**
- **(a) Per-community DISTINCT authors — FORCED, not a choice.** Reusing one author subspace key across communities would make the same pseudonym **linkable across every community a person joins** — a privacy regression for an activist tool (same class as 2B's anti-fingerprint guard, and almost certainly why communal authors are per-namespace random today). So each community gets its own random, non-re-derivable author, which therefore must be **persisted**.
- **(b) At-rest protection = S2 (sealed), not S1 (raw).** Each community author is **sealed via the EXISTING `seal_identity` wrapping-key path** (no new crypto), the wrapping key held in-session and zeroized on drop. Rejected S1 (raw `subspace_secret` BLOB in SQLite relying on OS at-rest protection) because the threat model includes **device seizure** — raw secrets at rest would compromise every community identity at once. Switch is the only path that unseals a community's author and re-seals the outgoing one; a corrupt sealed row **quarantines** (reuses `authority_quarantined`), never drops or leaks a partial key. **Load-bearing caveat under verification:** the real shipping first-run path must carry a wrapping key (minted on first run, held in iOS Keychain / Android Keystore) so *real* users get durable sealed multi-community; the keyless in-memory fallback is only for ephemeral `open_local_profile()` test/demo profiles. If real first-run is keyless today, Unit 3 wires the secure-store-backed key rather than falling back to raw/keyless-durable.

**Why proceed rather than wait:** reuses a reviewed mechanism, is the only faithful + unlinkable path to multi-community, and the coordinator runs an **independent adversarial isolation review** targeting the at-rest sealing (a sealed B author must be genuinely un-loadable while A is active) plus device-seizure posture. **Owner to ratify** the per-community-distinct-sealed-identity model. Do not describe multi-community identity as final until ratified.
