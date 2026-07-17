# Composite Site — Unit 4: Composite Resolver + Resolved View-Model Contract — Implementation Plan

**Date:** 2026-07-18
**Design:** `docs/superpowers/specs/2026-07-15-composite-site-namespace-manifest-design.md` §6 (composite render + resolved view-model), §2.2 (member invariants), §4.3 (moderation freshness), §5.2 (manifest floor).
**Depends on:** Unit 2 (manifest + `site/` module — landed #27) and **Unit 3 (moderation data + freshness verdict — NOT yet built; see its plan).** **Do not start until Unit 3 is on `main`.**
**Grounded against HEAD** (2026-07-18 recon).

---

## 1. Scope

Unit 4 is the **read-side composition**: it turns the three typed namespaces + manifest + moderation overlay into ONE **resolved view model** in Rust core, exposed via FFI, that the native shells render with **no business logic** (shared-core rule). It is where trust-tier tags, moderation treatment, and honest degradation states are *resolved by core* so iOS and Android cannot diverge.

**In scope (§6 contract — all core-resolved):**
- **Per-item trust-tier tag** — `editorial | open-wire | comment`, resolved by core. Core owns "W never masquerades as editorial." **New — no first-class trust flag exists today** (only manifest-level `SiteRole` at `manifest.rs:60`).
- **Moderation treatment per item** — reuse core `PostTreatment` (`newswire/projection.rs:76`: `Ordinary | Hidden{actions} | Tombstoned{actions}`); moderated rows stay as accountable placeholders, never dropped.
- **Composite degradation enum** (named, plain-language, honest — matching the `ShellRecoveryState`/`pendingFirstSync` convention): `moderation-loading`, `editorial-only`, `transport-blocked`, `manifest-invalid`, `manifest-rollback-alarm`, `member-unverified`. **New.**
- **Transport-status field** carrying the fail-closed reason (from Unit 5's ticket floor).
- The **render pipeline** (§6 steps 1–7): manifest → validate members → sync C/W → build view w/ trust tags → apply moderation overlay once moderation-current → resolve soft links (tolerate dangling).
- **FFI resolved view model** mirroring the `NewswireProjectionView` template (`newswire_ffi.rs:205`).

**Out of scope:** moderation *record semantics* + freshness verdict (Unit 3 produces them; Unit 4 *applies* them); the manifest record itself (Unit 2); transport dialing (Unit 5); native rendering (Unit 6 — Unit 4 produces the view model, Unit 6 draws it); **writer-side expired-cap warning is produced here as view-model state but rendered in Unit 6.**

## 2. Load-bearing invariants (§2.2, §6)

1. **Trust tier is resolved by core, styled by shells.** The shells must never infer "is this editorial?" — they render exactly the tag core produced. `O:/articles` → editorial (cap-chain verified at admission); `C` → comment; `W` → open-wire. A W item tagged editorial is a security defect (impersonation).
2. **Moderated rows are accountable placeholders, never vanished** (Riot's accountable-degradation convention). Content nulled; identity + ordering + freshness preserved.
3. **`member-unverified` is shown, never silent** — a member dropped for rule/key-structure mismatch (§2.2 inv 1, `ClassifiedMember::Unverified` at `validate.rs:102`) renders "this section couldn't be verified," not a disappearance.
4. **Moderation overlay applies ONLY when moderation-current** (Unit 3's freshness verdict). If `moderation-loading`, hold the open namespaces with "posts appear once moderation syncs" — never render un-moderated content as if clean, never blank.
5. **Manifest version ≥ durable floor** (`version_floor.rs` `admit_manifest_version`); a rollback/downgrade surfaces as `manifest-rollback-alarm`, an equivocation as the equivocation alarm. `ManifestValidationStatus` (`site_ffi.rs:114`) already carries these variants — Unit 4 attaches them to the resolved model.

## 3. Surface (verified against HEAD 2026-07-18)

| Area | Location (CURRENT) | Unit 4 action |
|---|---|---|
| View-model template | `crates/riot-ffi/src/newswire_ffi.rs:205` `NewswireProjectionView`; core `newswire/projection.rs:120` `NewswireProjection` | Mirror: NEW `crates/riot-core/src/site/resolve.rs` (core) + FFI `ResolvedCompositeSite` view model |
| Treatment enum | core `PostTreatment` `newswire/projection.rs:76`; FFI `NewswirePostTreatment` `newswire_ffi.rs:123` | REUSE core `PostTreatment` for moderation treatment (design says reuse — do not fork a parallel enum) |
| Member classification | `site/validate.rs:102` `ClassifiedMember` (Verified/Unverified) | Feed `Unverified` → `member-unverified` degradation |
| Manifest status | `crates/riot-ffi/src/site_ffi.rs:114` `ManifestValidationStatus` (Valid/MemberUnverified/ManifestInvalid/ManifestRollbackAlarm/EquivocationAlarm) | Map into the composite degradation enum |
| Version floor | `site/version_floor.rs` `admit_manifest_version` (STATEFUL — `resolve_site_manifest` in FFI is currently STATELESS) | Unit 4 integrates the stateful floor into resolution (recon flagged this gap) |
| Moderation overlay | Unit 3's revoked-keys / tombstoned-ids / freshness verdict | Consume; apply per §6 step 6 (root exempt, owner-precedence, allow-list) |
| Trust tier | NONE | NEW core-resolved per-item tag |
| Degradation model precedent | `crates/riot-ffi/src/mobile_api.rs:32` `CommunityRow` (available/quarantined/archived) | Follow the named-honest-state convention; NEW composite enum |

**New `uniffi::Record`/`Enum` is certain** (resolved view model, trust-tier tag, degradation enum). **UniFFI gate:** binding regen + native staticlib rebuild in the SAME commit (`site_ffi.rs:99` note; checksum-abort trap). FFI smoke test on iOS + Android.

## 4. Tasks (TDD — RED first)

- **Task 1 — core trust-tier resolution.** NEW `site/resolve.rs`: given manifest members + entries, tag each item `editorial | open-wire | comment` by its source namespace/role. RED (security): a W (open-wire) entry is NEVER tagged editorial; an `O:/articles` entry IS editorial; a C entry is comment. This is the impersonation guard.
- **Task 2 — member validation → `member-unverified`.** Wire `ClassifiedMember::Unverified` (`validate.rs:102`) into a degradation datum. RED: a member whose key-structure mismatches its declared rule (§2.2 inv 1) yields `member-unverified`, not a dropped section.
- **Task 3 — composite degradation enum + manifest floor integration.** Define the enum; integrate the STATEFUL `admit_manifest_version` (recon: FFI resolve is currently stateless). RED: manifest below the durable floor → `manifest-rollback-alarm`; equivocation → alarm; invalid → `manifest-invalid`; O-only synced → `editorial-only`.
- **Task 4 — moderation overlay application (consumes Unit 3).** Apply revoked-keys + tombstoned-ids as `PostTreatment::Hidden/Tombstoned` ONLY when Unit 3's verdict is moderation-current; else `moderation-loading` holding open namespaces. RED: revoked cap-holder entry → placeholder; tombstoned id → placeholder; **loading verdict → held, not applied, not blank**; root exempt; owner-precedence; re-endorse allow-list honored.
- **Task 5 — soft-link resolution + dangling collapse.** Resolve comment→article parent at render; dangling ref collapses gracefully (C is open — no admission-time referential integrity). RED: a dangling soft link collapses, does not crash or 500 the view.
- **Task 6 — writer-side state (produced here, rendered in Unit 6).** An editor whose time-boxed cap expired → view-model state "editorial access expired on <date>"; a local-but-peer-rejected entry → `failed/pending`, never "published." RED: expired cap surfaces the warning datum; a rejected write is `failed`, not silently "published."
- **Task 7 — FFI resolved view model.** Mirror `NewswireProjectionView`: `ResolvedCompositeSite` FFI record carrying tagged items + treatment + degradation + transport-status. Binding regen + native rebuild same commit; FFI smoke test both platforms.

## 5. Adversarial / correctness RED cases (§8.1 Unit 4)

1. **Partial-sync degradation states** — O-only ⇒ `editorial-only`; `/mod/` not current ⇒ `moderation-loading`; each holds content (not blank) with honest copy.
2. **Dangling soft-link collapse** — no crash, graceful.
3. **Trust-tier separation** — W never tagged editorial (the impersonation guard; mutation-prove it).
4. **Moderation-loading-timeout fallback** — a slow provider must not leave a permanent spinner; there is a documented fallback (no infinite `moderation-loading`).
5. **Manifest-rollback-alarm surfaces** — downgraded manifest is not silently accepted.
6. **member-unverified is visible** — never a silent section disappearance.

## 6. File scope (claim before editing)

`crates/riot-core/src/site/resolve.rs` (NEW), `crates/riot-core/src/site/mod.rs` (exports), possibly `crates/riot-core/src/site/validate.rs` (stateful floor integration — coordinate; Unit 2 owns it), `crates/riot-ffi/src/site_ffi.rs` (resolved view model + degradation enum), NEW tests under `crates/riot-core/tests/`. **New FFI records → UniFFI regen + native staticlib rebuild in the SAME commit** (coordinate the native rebuild centrally per the staleness trap). `site_ffi.rs` and the `site/` module are Unit 2/3 territory too — `gh pr list` before + during, pathspec commits, worktree.

## 7. Verification gates

- `cargo test --workspace --all-features`; `cargo clippy --workspace --all-features --all-targets -- -D warnings`; `cargo fmt --all -- --check`; `cargo run -p xtask -- validate-contracts`.
- Coverage at `.coverage-thresholds.json` floor.
- Trust-tier separation (case 3) mutation-proven (remove the tag guard ⇒ a W-as-editorial test fails).
- FFI smoke test loads the new records on iOS + Android (checksum-abort guard).

## 8. Sequencing & hazards

1. **Blocked on Unit 3** — needs the revoked/tombstoned sets + freshness verdict. Do not start until Unit 3 is on main; re-verify Unit 3's produced types at HEAD (they will be new).
2. **Unblocks Unit 6** — the native UI renders this view model.
3. **Stateless→stateful shift** — recon: `resolve_site_manifest` (`site_ffi.rs:256`) is stateless today; Unit 4 introduces the stateful version-floor read into resolution. This is the main new integration risk; de-risk it first.
4. **No business logic in shells** — every tier/treatment/degradation decision resolves in core. If a decision is tempting to make in Swift/Kotlin, it belongs here.
5. **UniFFI trap** — new records/enums; regen + native rebuild same commit; smoke test.
6. **Shared-checkout** — `gh pr list --search "composite resolver OR unit 4"` before + during (the Unit-1 duplication lesson).
