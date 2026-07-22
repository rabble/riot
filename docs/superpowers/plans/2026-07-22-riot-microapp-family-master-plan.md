# Riot Microapp Family — Master Implementation Plan

> **For agentic workers:** This is the master decomposition. Each work unit (WU) below gets its own detailed TDD plan file `docs/superpowers/plans/2026-07-22-riot-microapp-family-wuNNN-<slug>.md`, written just-in-time before that WU executes, then run through the plan-review-gate and metaswarm orchestrated-execution (4-phase loop).

**Source spec:** `docs/superpowers/specs/2026-07-22-riot-microapp-family-design.md` (five-role design review approved).

**Goal:** Redesign Riot's eight built-in community microapps into one offline visual family with a shared bottom toolbar, six personal themes, a host-font pack, and a crash-safe content-addressed v1/v2 coexistence + trust model — without changing sandbox, signing, or Willow semantics.

**Architecture:** Shared Rust core owns catalog split, admission/quota preflight, and the locked prepare/persist/finalize transactions; `riot-ffi` exposes them; native shells (iOS/macOS/Android) own host chrome + theme/font injection + preference storage; web (`fixtures/apps/`) owns the eight v2 tool sources + `_shared` token/toolbar/helper system; CI gains a blocking `miniapps` job.

**Tech Stack:** Rust 2021 (riot-core, riot-ffi/UniFFI), Swift 6/SwiftUI, Kotlin 2.2/Compose, vanilla HTML/CSS/JS microapps, Node + Playwright contract/browser tests.

---

## Ground truth (verified 2026-07-22, origin/main `49dbe38`)

- `fixtures/apps/` has 8 app source dirs + committed `*.bundle.cbor`/`*.manifest.cbor`; `_shared/` holds only `tokens.css`. No `v2/` or `legacy/` dir yet.
- `crates/riot-core/src/apps/starter.rs`: `STARTER_CATALOG: &[(&[u8], &[u8])]` of 8 pairs + `verify_starter_catalog()`. No generation marker.
- 16-app cap lives in `crates/riot-ffi/src/mobile_state.rs` (memory). No aggregate byte quota today.
- Existing tests: `apps_starter.rs`, `apps_trust.rs`, `apps_bridge.rs`, `apps_index_scan.rs`, `riot-ffi/tests/apps_contract.rs`.
- `scripts/apps/`: `repack-starter.sh`, `miniapp-contracts.mjs`, `miniapp-browser.spec.mjs`, `miniapp-preview-host.mjs`, `playwright.config.mjs` already exist.
- **Slice 0 breadcrumb prereq is NOT on main.** Design commit `f9d3d58` (docs only) lives in the `overnight-2026-07-22` worktree; host implementation must be reconciled/landed before Events host integration (WU-007).

## Work-unit arc

Each WU produces working, tested software on its own. `→` = hard dependency.

| WU | Slice | Title | Primary surface | Depends on |
| --- | --- | --- | --- | --- |
| WU-000 | 0 | Breadcrumb baseline reconcile | macOS/iOS/Android host + tests | overnight-2026-07-22 |
| WU-001 | 1 | Catalog split + legacy resolver + capacity preflight (Rust core+FFI) | riot-core, riot-ffi | — |
| WU-001N | 1 | Persist generation marker + Android 4 MiB codec-ceiling preflight | Android `PersistedProfile.kt`, iOS/macOS `ProfileRepository.swift`, FFI restore sig | WU-001 |
| WU-002 | 1 | Locked prepare/persist/finalize: trust grant/revoke + app-data | riot-core, riot-ffi, native shells | WU-001N |
| WU-002P | 1 | Existing-user presentation: `Legacy 1`/`Redesigned · Version 2` cards, install-warning copy, distinct count-full vs storage-full copy (spec §"Existing-user presentation") | iOS/macOS/Android Tools UI | WU-001N, WU-002 |
| WU-003 | 2 | Semantic tokens + 6 theme presets + Night Garden fallback + drift contract | `fixtures/apps/_shared`, contract test | — |
| WU-004 | 2 | `appearanceProfileID` lifecycle + theme picker + native preference store | riot-core/ffi, iOS/macOS/Android | WU-003 |
| WU-005 | 2 | `RiotToolFonts.v1` pack + reserved-path resolver + per-ID CSP + nosniff + normalization vectors | riot-ffi, native, preview | WU-003 |
| WU-006 | 2 | Host→WebView theme contract (document-start injection, 8-ID allowlist, fail-closed) | native, preview | WU-004, WU-005 |
| WU-006L | 3 | Relocate the 7 non-Checklist v1 pairs byte-for-byte into `fixtures/apps/legacy/` as sole editable authority; wire `LEGACY_BUILTIN_CATALOG` + packer to it; audit any root mirror is byte-identical (spec L674-680) | `fixtures/apps/legacy/`, `pack_starter.rs`, `starter.rs` | WU-001 |
| WU-007 | 3 | **Events vertical pilot** (hard continuation gate) | `fixtures/apps/v2/roll-call`, all hosts, `_shared` toolbar/helper | WU-000..006, WU-006L |
| WU-008 | 4 | Supply Board v2 | `fixtures/apps/v2/supply-board` + hosts | WU-007 |
| WU-009 | 4 | Quick Poll (Decisions) v2 | `fixtures/apps/v2/quick-poll` | WU-007 |
| WU-010 | 4 | Chat v2 | `fixtures/apps/v2/chat` | WU-007 |
| WU-011 | 4 | Dispatches v2 | `fixtures/apps/v2/dispatches` | WU-007 |
| WU-012 | 4 | Wiki v2 | `fixtures/apps/v2/wiki` | WU-007 |
| WU-013 | 4 | Photo Wall v2 | `fixtures/apps/v2/photo-wall` | WU-007 |
| WU-014 | 4 | Checklist v2 (v1 frozen; new slug) | `fixtures/apps/v2/checklist` | WU-007 |
| WU-015 | 5 | Deterministic preview fixtures + enum validation | `scripts/apps/miniapp-preview-host.mjs`, fixtures | WU-003 |
| WU-016 | 6 | CI `miniapps` job + `repack-starter.sh --check` + Playwright visual/assistive captures | `.github/workflows/ci.yml`, `scripts/apps` | WU-003, WU-015 |
| WU-017 | — | Size/perf budgets + moderated offline usability gate (release gates; largely manual, recorded) | release harness/docs | WU-007..016 |

## Ordering rules

1. **WU-001 → WU-002** first: the crash-safe core is the foundation; no visual/native-UI dependency, fully TDD-testable.
2. **WU-003** (tokens) can run in parallel with WU-001/002 (disjoint file scope: web vs Rust).
3. **WU-004/005/006** need WU-003. **WU-000** (breadcrumb) runs any time before WU-007.
4. **WU-007 (Events) is a hard gate.** Any failed Events navigation/keyboard/perf/coexistence criterion revises the shared system (WU-003..006, `_shared`) before any WU-008+ starts. One app per reviewed WU after.
5. **WU-014 (Checklist v2)** is last in Slice 4; Checklist **v1** source/manifest/bundle/app-ID stay byte-frozen — never an edit target.
6. **WU-016** CI wiring should land incrementally as fixtures appear, but the blocking job is gated on WU-015.

## Per-WU deliverables (uniform Definition of Done)

Every WU: RED tests first (watch fail) → GREEN minimal impl → REFACTOR; **coverage gate = `cargo llvm-cov --workspace --all-features --fail-under-lines <thresholds.llvm.lines=95>`** (the CI-enforced gate; do NOT gate on `cargo tarpaulin --fail-under 97` — tarpaulin's 97 floor is fiction, measures ~94.5%, and its ptrace engine hangs on this workspace); adversarial review; commit with pathspec (shared checkout — never `git add -A`). WU-007+ also: deterministic repack + generated inventory/allowlist audit + `repack-starter.sh --check` green + first-useful-content within 96 CSS px at 390×844.

## Cross-cutting invariants (apply to all WUs)

- Runtime selection/authorization always by full app ID — never name or semver.
- Absent `starterCatalogGeneration` == generation 1 (durable, zero-byte). Fresh profiles atomically record generation 2 in the first save.
- Never `git add -A`; commit exact pathspecs. Scope against `origin/main`, not local HEAD (shared checkout, many concurrent sessions).
- No `--no-verify`, no `--force` without explicit approval. Build `--workspace` when touching a matched-on enum.
- Theme key/appearanceProfileID/titles/drafts excluded from logs, crash annotations, screenshots, test artifact names.

## Persistence for recovery

Approved plan → `.beads/plans/active-plan.md`; project context → `.beads/context/project-context.md`; execution state → `.beads/context/execution-state.md` (updated per phase). `bd prime --work-type recovery` reloads after compaction.
