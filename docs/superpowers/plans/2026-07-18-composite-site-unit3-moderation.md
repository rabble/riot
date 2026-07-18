# Composite Site — Unit 3: Moderation (`/mod/` revoke + tombstone + heartbeat) — Implementation Plan

**Date:** 2026-07-18
**Design:** `docs/superpowers/specs/2026-07-15-composite-site-namespace-manifest-design.md` §4 (dual-mechanism moderation), §4.1–4.3, §8 Unit 3, §8.1 Unit 3 RED cases.
**Depends on:** Unit 0 (`OwnedMasthead` cap minting/signing), Unit 1 (owned-namespace admission — landed #14), Unit 2 (site manifest + `site/` module — landed #27). **All three are on `main`.**
**Plan-review gate:** PASSED 3/3 (Feasibility + Completeness + Scope&Alignment), 2026-07-18 — reviewers verified every cited symbol against HEAD.
**Grounded against HEAD** (2026-07-18 recon), not the spec's assumptions — the spec's line numbers and some claims drifted before (gate-4, roster); every symbol below was verified in code.

---

## 1. Scope

Unit 3 adds the **owner-signed moderation records** at `O:/mod/` and the **render-guarantee data** the resolver (Unit 4) overlays. It is a security unit: the guarantee is "on any honest client whose `/mod/` is current, a banned person/post is invisible" (§4.3). Getting freshness wrong (a false "current") silently breaks that guarantee.

**In scope:**
- Three owner-signed record types at `O:/mod/`: `revoke{author_key, effective_ts}`, `tombstone{target_ns, target_entry}`, `mod_epoch{seq, ts, mod_set_digest}` (the freshness heartbeat).
- **`/mod/`-scoped moderator delegation** — `OwnedMasthead::delegate_section` today is **articles-only** (belt = `is_under_articles`); moderator caps need a `/mod/`-scoped mint that **cannot reach `/manifest` or the root**.
- **Admission (Tier 1, best-effort):** extend the owned schema gate (`bundle.rs:606`, currently refuses `/mod/`) to ADMIT `/mod/` records under the owner cap or a `/mod/`-scoped moderator cap; keep `/manifest` refused-by-delegated. Timestamp-monotonic + deny-list checks are best-effort per §4.2 (not airtight — documented).
- **`mod_epoch` freshness evaluation** — the `moderation-current` predicate: heartbeat `ts` in window AND no `seq` gap AND client holds every record named by `mod_set_digest`. This is the anti-tail-suppression core (§4.3).
- **FFI classification** — register the `/mod/` family in BOTH `mobile_state.rs` sites (`:1566` inspectable, `:951` list_current) or bundles reject / board bricks (§3.1, newswire-0B prior art).
- The **overlay data** Unit 4 consumes: the set of revoked author-keys, tombstoned entry-ids, and the resolved freshness state — but NOT the render itself (that is Unit 4).

**Out of scope:** the composite render / view-model overlay application (Unit 4 — Unit 3 produces the moderation *data + freshness verdict*, Unit 4 *applies* it); transport (Unit 5, landed); native UI (Unit 6); automated anti-flood (parked, §9 Risk 2).

## 2. Load-bearing invariants (do not weaken)

1. **Root is exempt from revocation AND from tombstone.** Two distinct protections (design §4.1: a rogue moderator "cannot brick the site by revoking the owner, **and cannot tombstone `/manifest`**"): (a) any `revoke{author_key == manifest.root}` is hard-ignored; (b) any `tombstone{target = /manifest}` or a tombstone of a root/owner-authored entry is hard-ignored. A `/mod/`-scoped moderator cap legitimately *writes* a `tombstone` record at `O:/mod/` whose `target` field merely *names* `/manifest` — admission admits the record, so the exemption MUST be enforced in the overlay/freshness output (the tombstone analogue of the root-exempt revoke), not at admission. Owner records take precedence over moderator records on conflict.
2. **Moderator caps are `/mod/`-scoped and cannot target `/manifest` or the root.** Enforced cryptographically by the delegation area (like the articles belt), verified at admission by willow25 `does_authorise`, NOT by a Riot-side path string compare alone. Belt-and-suspenders: mint-side refuses a non-`/mod/` area, admission relies on the signed area.
3. **`moderation-current` is a POSITIVE signed signal, never an absence.** Willow reconcile gives no completeness guarantee, so "haven't seen a revoke" ≠ current. Current iff: a heartbeat whose `ts` is within the freshness window, no visible `seq` gap, AND every record named by `mod_set_digest` is held. Otherwise ⇒ `moderation-loading` (open namespaces held, never a false "current"). This is what defeats tail-suppression.
4. **`mod_set_digest` commits to the revoke+tombstone record ids ≤ seq** (rolling hash / small Merkle root). Detection is **recompute-over-held-and-compare**, NOT name-enumeration: the client recomputes the digest over its own held `≤ seq` record set and compares to the heartbeat's `mod_set_digest`; a mismatch ⇒ it is missing (or has extra) records ⇒ `moderation-loading`. (A one-way hash cannot enumerate the names of records the client lacks — only reveal that the held set differs. Phrase every task/test as recompute-and-compare.) A provider serving a fresh contiguous prefix but withholding the *latest* revoke shows no `seq` gap, yet its held-set digest differs from the heartbeat ⇒ detected.

## 3. Surface (verified against HEAD 2026-07-18)

| Area | Location (CURRENT) | Unit 3 action |
|---|---|---|
| Moderation records | NONE — no `moderation.rs`/revoke/tombstone/`mod_epoch` types exist | NEW module `crates/riot-core/src/site/moderation.rs` (mirror `site/manifest.rs` codec shape) |
| `/mod/` delegation | `willow/masthead.rs:70` `delegate_section` — belt = `is_under_articles`, articles-only | Add `delegate_moderation` (or generalize) with a `/mod/` belt; **must not admit `/manifest`/root** |
| Path helper | `willow/site_paths.rs` — has `is_under_articles`, `is_owned_editorial_entry`; NO `is_under_mod` | Add `is_under_mod` + `is_owned_moderation_entry` |
| Admission schema gate | `import/bundle.rs:606` — owned schema check; `/mod/` currently REFUSED | Extend to admit `/mod/` owned records (owner or `/mod/`-mod cap); keep `/manifest`-by-delegate refused |
| `admissible_capability` | `import/bundle.rs:481` | A moderator cap is owned + `/mod/`-scoped, non-zero-delegation allowed for owner→moderator; verify area via willow25 |
| Sync | `sync/state.rs:272` `verify_received_bundle` (`decode_bundle_with_root(bytes, Some(namespace_id))`) | `/mod/` rides the SAME owned admission path; verify it threads (Unit 1's F1 lesson: prove threading, don't assume) |
| FFI classification | **THREE** sites call `is_owned_editorial_entry`: `mobile_state.rs:951` (list_current), `:1566` (inspectable), **`:2217` (`reproject_active`, on open/switch)** | Add `/mod/` family to ALL THREE via one shared `is_owned_moderation_entry` predicate. `:1566` REJECTS the whole import on a miss (board bricks); `:2217` is tolerant (`continue`s on decode_alert failure — silently skips, no brick) but must still exclude mod records or reproject needlessly attempts alert-decode. Contract test **greps every `is_owned_editorial_entry`/`is_owned_moderation_entry` call site** rather than hardcoding a count — the site count grew 2→3 since the memory note |
| Freshness eval | NONE | NEW `moderation.rs`: `evaluate_freshness(held_records, now) -> ModerationState` (current / loading + reason) |
| Version-floor pattern | `site/version_floor.rs` `VersionFloorStore` trait (durable KV) | Reuse the durable-store pattern if `seq` needs a durable floor (anti-rollback of the heartbeat) |

**No new `uniffi::Record` is free:** the overlay data Unit 3 hands to FFI (revoked keys, tombstoned ids, freshness state) is new FFI surface → **binding regen + native staticlib rebuild in the SAME commit** (checksum-abort trap; see `site_ffi.rs:99` UniFFI-gate note). Decide in Task 6 whether Unit 3 exposes FFI now or hands Rust structs to Unit 4 which owns the FFI view-model.

## 4. Tasks (TDD — RED first, per CLAUDE.md)

- **Task 1 — moderation record types + canonical codec.** `moderation.rs`: `Revoke`, `Tombstone`, `ModEpoch` structs + a deterministic CBOR encoder/decoder mirroring `site/manifest.rs:257/290`, with strict bounds (`MAX_*_BYTES`). Declare the **re-endorse allow-list representation explicitly** (a record type vs a field on the manifest/mod set — pick one and state it; case 8 depends on it). RED: round-trip + reject malformed/oversized/non-canonical. No signing yet.
- **Task 2 — owner signing of mod records.** Sign a `revoke`/`tombstone`/`mod_epoch` with the owner cap (`OwnedMasthead::authorise_owner_entry`, `masthead.rs:158`) at the `O:/mod/…` path. RED: sign→verify round-trip; a wrong-path (`/articles/`, `/manifest`) entry under a mod payload is rejected.
- **Task 3 — `/mod/`-scoped moderator delegation.** Add `is_under_mod`/`is_owned_moderation_entry` (`site_paths.rs`) + a `/mod/` delegation mint. RED (security): a `/mod/`-scoped moderator cap CANNOT author `/manifest` or the root (area does not `includes` them → willow25 `does_authorise` false); a moderator cap CAN author `/mod/revoke-…`. Mirror the articles-belt test in `masthead.rs`.
- **Task 4 — admission of `/mod/` records + Tier-1 best-effort (§4.2).** (a) Extend the `bundle.rs:614` owned `schema_ok` (`is_under_articles(p) || is_under_mod(p)`, inside the `is_owned()` branch — no ripple to communal/articles) to admit `/mod/` under owner or `/mod/`-mod caps; `/manifest` stays refused **for all callers at this gate** (Unit 2 validates the manifest on an independent path). RED: owner-signed revoke ADMITTED; `/mod/` entry under an *articles* editor cap REJECTED (wrong scope); a `/manifest` entry under a moderator cap REJECTED. Prove the sync path threads (Unit 1 F1 lesson — hardcode-None mutation must fail a test).
  - (b) **Tier-1 leak-shrinking checks (§4.2), explicitly in scope:** the **deny-list-at-admission** check (reject an owned editorial entry whose cap-receiver ∈ the synced revoked set — it consumes exactly the revoked set Task 5 produces) and the **timestamp-monotonic reject** (entry ts < max-seen-for-author). RED for each. Both are **best-effort, not airtight** (an attacker controlling delivery order can seed a backdated entry to a fresh peer first — §4.2); the render guarantee (Unit 4 Tier 2) is the real backstop. Cap **expiry** is inherent (willow25 `time_range`), no new code. Do NOT silently drop these — implement with honest "not airtight" comments, or if deferred, defer explicitly with justification (they are not the guarantee, but they are named Unit-3 behavior).
- **Task 5 — freshness evaluation + exemption filtering (`moderation-current`, the security core).** `evaluate_freshness(records, manifest_root, now) -> ModerationState { Current{revoked, tombstoned} | Loading{reason} }`. The emitted `revoked`/`tombstoned` sets MUST already have the exemptions applied (case 5 + 5b): drop `revoke{root}` and drop `tombstone{target ∈ /manifest, root/owner entries}` before they reach the sets, so Unit 4 overlays a pre-filtered set. `mod_set_digest` is **recompute-over-held-and-compare** (invariant 4), real not stubbed. RED cases §5. This is the anti-suppression + anti-brick keystone.
- **Task 6 — FFI classification (ALL THREE sites) + overlay data hand-off.** Register `/mod/` via one shared `is_owned_moderation_entry` predicate at ALL THREE `mobile_state.rs` classifiers (`:951` list_current, `:1566` inspectable, `:2217` reproject_active). Contract test **greps every classifier call site** and asserts a committed `/mod/` record is non-alert at each — omission from `:1566` bricks the import, from `:2217` silently mis-processes. Decide FFI exposure boundary with Unit 4 (see §3 note). If a new `uniffi::Record`/`Enum` lands, regen binding + rebuild native staticlib in the same commit; add an FFI smoke test.

## 5. Adversarial RED cases (§4 + §8.1 — the security core)

Each RED-then-green, driving real willow25 (forge raw records/caps as a hostile peer, not only the friendly minting API):

1. **Revoke hides even with backdated timestamp** — a revoked editor backdates a new in-window article; the render-identity data still lists the author-key as revoked (guarantee rests on identity at render, not the clock). (§4.3 timestamp reality.)
2. **Tombstone hides the entry** — a tombstoned entry-id is in the tombstone set.
3. **`seq` gap ⇒ moderation-loading** — heartbeats {1,2,4} held (3 missing) ⇒ `Loading`, never `Current`.
4. **Tail-suppression ⇒ `mod_set_digest` mismatch ⇒ moderation-loading** — a fresh contiguous prefix + latest-heartbeat held, but the record the digest names is withheld ⇒ `Loading`. (The keystone; the whole point of the digest.)
5. **Root exempt (revoke)** — `revoke{author_key == manifest.root}` is ignored; the owner's entries still render.
5b. **Root/manifest exempt (tombstone) — BLOCKING, was missing (§4.1).** A `/mod/`-scoped moderator cap writes a `tombstone{target = /manifest}` (admitted — the record sits legitimately at `O:/mod/`, its `target` field merely names `/manifest`); the overlay/freshness output MUST hard-ignore it, so manifest resolution is unaffected. Symmetrically, a moderator tombstone of a root/owner-authored entry is subordinate to owner precedence. Without this, a rogue/seized moderator cap bricks the site — exactly the attack §4.1 forbids. This filters manifest/root-targeting tombstones out of the `tombstoned` set the verdict emits.
6. **Moderator cap cannot WRITE `/manifest`** — proven at admission (Task 3/4), asserted with its own RED. (Distinct from 5b: this is write-scope at admission; 5b is a legitimately-written tombstone whose *target* names `/manifest`.)
7. **Owner precedence on conflict** — an owner record and a moderator record disagree; the owner wins.
8. **Re-endorse allow-list survives** — an allow-listed pre-ban entry stays rendered after a revoke of its author (selective un-hide). (§4.3 keep-pre-ban-good-work.)
9. **Stale heartbeat ⇒ loading** — a heartbeat whose `ts` is outside the freshness window ⇒ `Loading`, even with no `seq` gap.
10. **Person-ban inert in C/W (honest-scope)** — a `revoke{author_key}` against a communal subspace has no cap-holder to bite; the record is accepted but the guarantee note says content-tombstone is the C/W lever (§4.1). Assert the design's honest scope, do not overclaim.

## 6. File scope (claim in COLLABORATION.md before editing)

`crates/riot-core/src/site/moderation.rs` (NEW), `crates/riot-core/src/site/mod.rs` (exports), `crates/riot-core/src/willow/site_paths.rs` (`is_under_mod`), `crates/riot-core/src/willow/masthead.rs` (`/mod/` delegation), `crates/riot-core/src/import/bundle.rs` (schema gate — HIGH-TRAFFIC, coordinate), `crates/riot-ffi/src/mobile_state.rs` (×2 classifiers — HIGH-TRAFFIC), possibly `crates/riot-ffi/src/site_ffi.rs` (overlay FFI, if not deferred to Unit 4), NEW tests under `crates/riot-core/tests/`. **`bundle.rs` + `mobile_state.rs` are touched by many sessions — pathspec commits, work in a worktree, `gh pr list` before AND during (the Unit-1 duplication lesson).** If FFI surface changes: UniFFI regen + native staticlib rebuild in the SAME commit.

## 7. Verification gates

- `cargo test --workspace --all-features`; `cargo clippy --workspace --all-features --all-targets -- -D warnings` (**`--all-targets`** — a WU-1 test file was already red without it); `cargo fmt --all -- --check`; `cargo run -p xtask -- validate-contracts`.
- Coverage at the `.coverage-thresholds.json` floor (CI: `cargo llvm-cov --fail-under-lines`).
- Every §5 case demonstrably RED-then-green; freshness guards mutation-proven (delete the digest-membership check ⇒ tail-suppression test fails).
- The freshness keystone (case 4) is the merge gate: without it "moderation-current" is unproven.

## 8. Sequencing & hazards

1. **Hard dependency satisfied** — Units 0/1/2 on main. Verify `OwnedMasthead::authorise_owner_entry` + `admissible_capability` shapes at HEAD before Task 2 (they drift).
2. **Unblocks Unit 4** — the resolver overlays Unit 3's revoked/tombstoned sets + freshness verdict.
3. **Atomicity** — admission (Task 4) + BOTH FFI classifiers (Task 6) must land together; a `/mod/` family admitted but classified in only one FFI site bricks the board (newswire-0B).
4. **Do not hand-roll crypto** — willow25 `is_valid()`/`does_authorise` verify the moderator cap chain + `/mod/` area nesting. Riot supplies the payload schema + freshness logic only.
5. **Freshness is the subtle part** — `mod_set_digest` must commit to a canonical record-id set; an ordering/canonicalization bug makes the digest non-reproducible and every honest client falsely `Loading`. Nail the canonical form with a golden vector.
6. **Shared-checkout** — see §6. Re-check `gh pr list --search moderation` before starting; another session could pick up Unit 3 (the Unit-1 lesson).
