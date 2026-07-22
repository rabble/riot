# Willow-Gap Master Plan (MLS last)

> **For agentic workers:** This is a phased master sequencing plan, not a per-task TDD plan. Each phase below is executed via its own detailed implementation plan (listed per phase; some exist, some are to be written at phase start using superpowers:writing-plans, then gated through metaswarm:plan-review-gate). Do not implement directly from this document.

**Goal:** Close the four Willow-capability gaps (scoped read/write authority, confidential sync, destructive editing, E2E-encrypted groups) in an order that front-loads everything Riot can build alone and pushes MLS-dependent work to the very end.

**Architecture:** Meadowcap becomes the real authority layer via the approved 8-slice design (`docs/superpowers/specs/2026-07-11-full-meadowcap-management-design.md`); destructive-editing schemas ride Willow prefix pruning; protected sync implements a transport-independent PIO contract over the existing nearby transports; the MLS-based Groups module is deferred to the final phase by owner decision (2026-07-22).

**Tech Stack:** Rust workspace (`riot-core`, `riot-ffi`, `riot-transport`), pinned `willow25 =0.6.0-alpha.3` (load-bearing alpha pin, enforced by `cargo xtask validate-contracts`), UniFFI shells on iOS/Android, nearby-only mobile transport (BLE + local-network IP).

---

## Owner decision driving this plan (2026-07-22)

MLS goes last. Rationale, as stated by the owner:

1. MLS is very hard — mobile viability, concurrent commits, long-offline recovery are unsolved for Riot's offline-first posture (dual-mode design open questions).
2. The ecosystem signal is negative: p2panda is abandoning MLS. The pool of proven Rust MLS-over-p2p prior art is shrinking, which raises Riot's integration risk.
3. Groups + property-preserving encryption depend on upstream Willow-team work maturing; everything else in this plan Riot can build alone against the pinned crate.

Consequence: the roadmap doc's earlier suggestion to draft the Phase 0B groups evidence contract "in parallel" (`docs/decisions/2026-07-22-willow-gap-roadmap.md` §3.2) is overridden — no Groups work, including design work, before Phase 7. Revisit the MLS-vs-alternatives question (e.g. openmls maturity, simpler sender-key schemes) when Phase 7 starts, not before.

---

## Phase map

| Phase | What | Detailed plan | Status | Depends on |
|---|---|---|---|---|
| 1 | Meadowcap slice 1 — capability core (codec, creation, delegation, verification, fingerprints, conformance fixtures) | `docs/superpowers/plans/2026-07-22-meadowcap-slice1-capability-core.md` | Plan in draft today | Nothing |
| 2 | Meadowcap slice 2 — governance ledger, deterministic evaluator, transitive revocation | To write at phase start | Not started | Phase 1 on main |
| 3 | Meadowcap slice 3 — shared contextual admission (local writes, imports, synced entries, legacy compat) | To write at phase start | Not started | Phases 1-2 on main |
| 4 (parallel track) | Gap 4 — destructive editing: tombstone + mutable-pointer schemas, migration story, honest "retraction ≠ erasure" UI language | Short design pass, then plan | Not started | Phase 1 only (path-profile stability); independent of 2-3 |
| 5 | Owned-namespace propagation to followers, then meadowcap slice 4 — protected-sync read gate + PIO state machine (transport-independent, nearby first, no WTP/Confidential Sync interop claims) | Propagation: extend `docs/research/2026-07-19-owned-namespace-propagation-design.md`; slice 4 plan at phase start | Not started | Phases 1-3 on main |
| 6 | Meadowcap slices 5-8 — managed Spaces (roles, invites, vault, recovery, migration), Manifest V2 + app permission broker, native management UX, cross-device conformance + field exercise + security review | Per-slice plans at phase start | Not started | Phase 5 (slice 5 needs protected sync for its release journey) |
| 7 (LAST) | Groups module — Phase 0B evidence contract (MLS library eval, threat model, invite state machine), then MLS control plane + opaque encrypted padded group-drops | Phase 0B contract first; no code before it passes review | Deferred by owner decision | Phases 1-6; upstream/library landscape re-evaluated at start |

---

## Phase 1 — Meadowcap slice 1: capability core

**Scope:** Canonical Meadowcap codec, capability creation, delegation, inspection, verification, fingerprints, and golden conformance fixtures, wrapping the pinned willow25 APIs. Includes the mandatory willow25 API inventory and upstream-gap list (design requirement, lines 1346-1348). No sync, no governance, no admission rewrite, no UI.

**Exit criteria:**
- Capability round-trip (create → delegate → encode → decode → verify) conformance fixtures pass, stamped in TAI/J2000 microseconds (`tai_j2000_micros_from_unix_seconds` — never raw Unix seconds in a TimeRange).
- Existing owned-cap sites (`crates/riot-core/src/willow/masthead.rs` defines `owner_write_capability`/`delegate_section`; callers in `crates/riot-core/src/willow/owned.rs`, `crates/riot-core/src/site/validate.rs`, `crates/riot-ffi/src/site_ffi.rs`, and the admission chokepoint `crates/riot-core/src/import/bundle.rs`) either call the new core or are explicitly listed for Phase 3 unification — no duplicated subset validators (recurring defect class: `riot-reuse-canonical-gate`).
- Full workspace gates green: `cargo build --workspace`, `cargo test --workspace --all-features`, fmt, clippy `-D warnings`, `cargo xtask validate-contracts` (update the Cargo.lock sha256 in `fixtures/manifest.json` if any dependency is added).

## Phase 2 — Meadowcap slice 2: governance

**Scope:** Versioned governance schemas, actor/device/action chains, durable authority repository, deterministic policy evaluator, transitive revocation (design lines 1314-1315, §Governance ledger, §Leases and revocation). Governance stays separate from protocol validity — Meadowcap answers "may this receiver read/write this area"; governance answers "which role should this person have."

**Exit criteria:** Deterministic evaluator produces identical decisions from identical ledgers on all platforms (property test); revoking a mid-chain delegation transitively invalidates descendants; durable repository survives restart via the SQLite store; if governance records are stored as new Willow record families, they are registered at every FFI classification site with a riot-ffi classification test (see cross-cutting rules).

## Phase 3 — Meadowcap slice 3: shared admission

**Scope:** One shared contextual admission engine for local writes, file imports, and synced entries, replacing today's per-entry-point validators without weakening schema checks (design line 1337). The owned-site chokepoint (`bundle.rs verify_frame` via `decode_bundle_with_root`) becomes a caller of the shared gate.

**Exit criteria:** Every forgery class rejected identically at every gate (shared test matrix across the three entry points); legacy public-community compatibility proven by replaying existing fixtures; no entry point retains a hand-rolled subset validator.

## Phase 4 (parallel track) — destructive editing schemas

**Scope:** Intentional tombstones and mutable pointers using Willow prefix pruning, per the audit's "later schemas" (`docs/research/2026-07-10-willow-implementation-audit.md` finding 9). Short design pass first (join-semantics invariants, migration story for existing 4-component paths — careless prefix layout could retroactively prune durable data), then a small implementation plan. UI/marketing language must say retraction, not deletion: copies already received cannot be erased, and the website contract gate (`scripts/marketing/protocol-page-contracts.mjs`) must be updated atomically with any user-facing claim.

**Why now (not held):** Owner direction 2026-07-22 — "the rest we can do." No upstream blocker; small; independent of Phases 2-3, so it can run as a parallel track (separate session/worktree) once Phase 1 stabilizes the path profile.

**Exit criteria:** Reviewed schema doc; tombstone + mutable-pointer records admit, project, and prune correctly in workspace tests; migration test proving no existing fixture entry becomes prunable; the new record families registered at every FFI classification site (see cross-cutting rules — a riot-ffi test must import a bundle containing the new family and assert it is classified, not rejected); guide/marketing copy updated with the retraction-not-erasure boundary in the same change as any UI.

## Phase 5 — propagation + protected sync (slice 4)

**Scope, part A — owned-namespace propagation:** `install_sync_inventory` (`crates/riot-ffi/src/mobile_state.rs`) currently prunes sync to active community namespaces, so owned-site content never auto-reaches followers (memory `riot-composite-owned-ns-no-autosync`; design sketch `docs/research/2026-07-19-owned-namespace-propagation-design.md`). Build propagation first — the read gate has little traffic to protect until followers actually receive owned-namespace entries.

**Scope, part B — protected sync:** The slice-4 state machine (`fresh → handshake → authenticated → private-overlap → capability-bound → reconciling → closed`): authenticated DH handshake, session-salted private-interest hashes, receiver-bound read capabilities, PIO L0/L1/L2 leakage levels pinned by tests, traffic-shape padding. Implemented transport-independently over the existing nearby transports. The legacy `/1` Hello/Summary codec remains public-only. **No WTP or Confidential Sync interoperability claims** — upstream is a sketch/proposal; Riot pins its own contract (design lines 18-21). This keeps Phase 5 free of the Willow-team dependency: we implement our own conformance-gated profile and adopt upstream later if/when it finalizes.

**Exit criteria:** Part A has its own gate before part B starts: a follower device demonstrably receives owned-namespace entries (`/articles`, `/mod/`) via normal nearby sync without hand-carried bundles, proven by an end-to-end two-device test. Then for part B: receiver-authenticated read gating demonstrably precedes any protected-sync availability claim (design line 1343); a read capability never functions as a bearer token (design failure condition, line 1304); leakage-level and traffic-shape tests pin the documented disclosures; two-peer nearby sync test matrix green on iOS (re-run the known-flaky two-peer suite before claiming success — memory `riot-two-peer-sync-red`).

## Phase 6 — managed Spaces, apps, UX, conformance (slices 5-8)

**Scope:** Open/Managed Space creation, invitations, roles, app-independent membership, secure-vault adapters, recovery, migration (slice 5); Manifest V2, permission algebra, app approvals, directory role authority, opaque app execution sessions (slice 6); iOS + Android management/consent/recovery/audit UX (slice 7); cross-device conformance, partition, migration, field-exercise, performance, and independent security review (slice 8).

**Release rule (verbatim constraint):** No partial slice is marketed as full managed-Space security. The minimum releasable journey requires all eight slices: create a managed Space, complete recovery protection, invite a second device/person, grant a restricted role, perform protected sync, approve an app subset, revoke the role offline, and reconcile to the same policy without a server (design lines 1329-1333). The website contract gate must not gain any "managed-Space security" string before this journey works end-to-end.

## Phase 7 (LAST) — Groups / MLS

**Deferred by owner decision.** When Phases 1-6 are done:

1. Re-evaluate the MLS landscape first: openmls maturity on mobile, what p2panda's abandonment implies, whether a simpler scheme (per-group sender keys + epoch rotation without full TreeKEM) meets the dual-mode design's stated leakage boundary (encrypted padded drops; existence/timing/size visible).
2. Write and review the Phase 0B evidence contract (threat model, MLS/library viability, invite state machine, agent-hour budget) — dual-mode design line 143. No Groups code before it passes.
3. Then Track B: encrypted store, QR + invite joins, group sync via opaque drops over nearby transports.

**Honesty gate:** no confidentiality or encryption claim anywhere (app, guide, website) until this phase ships — the newswire's "signed plaintext" framing is permanent and the contract gate pins it.

---

## Cross-cutting rules for every phase

- Each phase starts by writing its detailed plan (superpowers:writing-plans) and passing metaswarm:plan-review-gate before implementation.
- Shared checkout discipline: pathspec commits only, claim rows, no `git add -A`, no stash (memory `git-stash-shared-checkout-danger`, `shared-checkout-multi-agent`).
- Full-workspace verification every unit (`cargo build --workspace && cargo test --workspace --all-features`) — scoped `-p` tests have hidden cross-crate breaks before (memory `riot-scoped-test-hides-cross-crate-break`).
- Coverage floors per `.coverage-thresholds.json` are blocking; raise, never silently lower.
- Any timestamp/TimeRange in code, tests, or fixtures uses TAI/J2000 microseconds via the helper — never Unix seconds.
- Any dependency change updates the Cargo.lock sha256 pinned in `fixtures/manifest.json` or `validate-contracts` fails CI.
- Any user-visible capability claim updates `scripts/marketing/protocol-page-contracts.mjs` in the same change.
- **Any new Willow record family (Phase 4 tombstones/mutable pointers, Phase 5-6 governance/manifest records) must be hand-registered at every non-compiler-forced classification site in the same work unit**: `crates/riot-ffi/src/mobile_state.rs` `inspectable_entries` (unmatched non-alert families cause the whole import bundle to be REJECTED at the FFI boundary) and its sibling `list_current_entries` (miss it and the board fails to project), plus the `store.rs` prefix scan (miss it and records admit but never project, silently). Exit evidence per phase: a riot-ffi test importing a bundle with the new family and asserting classification. Workspace tests alone cannot catch this.
- Any record-family or FFI-signature change rebuilds UniFFI bindings and the native staticlib together — skew is a runtime checksum abort on device, invisible to `cargo test`. Native app test suites (xcodebuild / gradlew per CLAUDE.md) run before any phase that touched FFI is declared done.
- `.beads/plans/active-plan.md` gets refreshed at each phase boundary (it is currently stale; see roadmap doc §5).
