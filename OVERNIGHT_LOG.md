# Overnight Log — 2026-07-19

_(Summary goes at the TOP when done. Task entries append below in order.)_

## Bearings and scope
- Read all 98 repository Markdown files: 11 current/top-level/platform/product documents directly, plus 36 implementation plans, 30 design specs, and 21 research/decision/archive documents through independent metaswarm readers.
- Skills used: `metaswarm:start`, `superpowers:brainstorming`, `metaswarm:brainstorming-extension`, `metaswarm:design-review-gate`, `superpowers:using-git-worktrees`, and `superpowers:dispatching-parallel-agents`. Repo-local `skills/` inventory: none; installed project skills are the source of workflow conventions.
- Current authority: the four-tab community-first shell (Home, Tools, People, Nearby), local durable commit before exchange, preview/review before trust, core-owned authority decisions, per-community identity, full technical IDs behind disclosure, and honest bounded status language.
- Stale material: five-tab navigation, auto-trust/import, implicit factual meaning for “verified,” and old 100%-Rust coverage commands. `.coverage-thresholds.json` is authoritative.
- Assumption: “implement it” refers to the iOS/macOS SwiftUI UX audited on 2026-07-18. Android parity is not silently claimed; a separate Android UX audit is required because its current screen structure was not part of the evidence.
- Rejected alternative: broad protocol, Rust, gateway, or public-host refactoring. The audit found native interaction and information-hierarchy problems; changing core policy would be unrelated and risk trust semantics.
- Concurrency: another live agent owns the requested `.claude/worktrees/overnight` checkout and is appending to its log. Per `COLLABORATION.md`, this work is isolated on temporary branch `overnight/2026-07-19-ux`; reviewed commits and both append-only logs will be combined into `overnight/2026-07-19` when that checkout is released.

## Task: mandatory design review, revision 1
- Used `metaswarm:design-review-gate`. Independent Product, Architecture, UX,
  Security, and CTO/TDD reviewers all returned `NEEDS_REVISION`; no implementation
  began.
- Tightened the design with exact Home ordering, per-wire composer placement,
  report row/detail fields, trust language, responsive/focus behavior, draft
  persistence/reset semantics, setup gating, deterministic alert rules, notification
  injection, pure presentation seams, success checks, and a defect-to-test map.
- Important root-cause finding: the current Nearby space announce carries namespace
  and title but not the authenticated Newswire descriptor handle. First-run adoption
  therefore creates the documented “dead follow” state. I rejected a cosmetic sheet
  and an unreviewed wire/FFI expansion; the compact UI removes first-run Nearby and
  truthfully says it becomes available after joining. Existing-community Nearby,
  bilateral consent, preview, and namespace-bound admission stay intact.
- Existing policy gap logged: the normative Newswire design allows inspecting an
  ordinarily hidden original, but current core projection redacts hidden and
  tombstoned payloads alike. This UI slice corrects the false promise, preserves
  distinct treatments and signed history, and does not invent payload access.
- Residual privacy risk logged: draft words, AI choice, sources, and coarse location
  persist per community in plaintext `UserDefaults`/device backups; operational type
  and expiry do not survive relaunch. This slice makes the behavior explicit and
  resets every field on successful reuse; storage hardening remains separate.
- Baseline native-core package build passed for iOS device/simulator, macOS arm64,
  and Android arm64/x86_64. Baseline shared Swift tests were started before code.
