# Composite Site — Unit 2: Site Manifest Record — Implementation Plan

**Date:** 2026-07-16
**Design:** `docs/superpowers/specs/2026-07-15-composite-site-namespace-manifest-design.md` — §2.1 (manifest record), §2.2 (invariants), §5.2 (anti-rollback), §8 Unit 2 + §8.1.
**Depends on:** Unit 0 (`OwnedMasthead` owned zero-delegation cap) **and** Unit 1 (owned-namespace admission — the manifest is an owned record admitted through Unit 1's gate). **Do not start until Units 0 and 1 are on `main`.**

---

## 1. Scope

Unit 2 defines the **owner-signed site manifest record** at the reserved, non-delegatable path `O:/manifest` — the single binding that turns three typed namespaces into one composite site. It covers: the record schema + canonical CBOR codec, signing with an owned zero-delegation cap, **validation independent of admission**, the three load-bearing invariants, per-member rule/key-structure classification (producing the `member-unverified` datum), and the **durable monotonic version floor** (anti-rollback + equivocation alarm). It surfaces the validated manifest through FFI as a resolved record.

**Out of scope:** the composite render pipeline + resolved view-model overlay (Unit 4 — Unit 2 produces the validated manifest + per-member verified/unverified *classification*, not the rendered site view); moderation records (Unit 3); the ticket/transport-floor parsing that supplies the followed root (Unit 5 — Unit 2 consumes the followed root established by Unit 1's admission context); native UI (Unit 6).

## 2. The manifest record (§2.1)

`site-manifest v1` lives at `O:/manifest`, signed by an **owned, zero-delegation** cap whose `receiver() == O owner key`:

```
site-manifest v1 {
  root:             <O owner pubkey>        # self-attesting: == O namespace owner key
  members: [ { ns, role, rule, display }, … ]   # role/rule/display = OPEN enums
  moderation_path:  /mod/
  transport_policy: { allow: [iroh, arti], require: none | arti }
  version:          <monotonic u64>
  layout:           <CLOSED enum>           # core resolves to a section order; apps render verbatim
}
```

- `role`/`rule`/`display` are **open enums** (future `community`/`page` describable by the same schema; v1 emits only the site shape).
- `layout` is a **closed enum** — core resolves it to a section order the shells render verbatim. **No owner-authored render blob is ever parsed in the apps** (shared-core rule; a free-form layout string would be an injection surface).
- Canonical CBOR codec in the newswire/model.rs style (deterministic encode, byte-identical across platforms — the golden-vector discipline).

## 3. Load-bearing invariants (§2.2)

Invariants 1 and 2 are Unit 2 RED cases (below). **Invariant 3 is admission-enforced (Unit 1) and CONSUMED here** — Unit 2 reads the followed root established by Unit 1's admission context; it does not re-verify it (that would duplicate Unit 1 and tension with §4's "independent of admission"). No Unit-2 RED case for inv 3; its enforcement is Unit 1's `wrong-root` RED.

1. **Rule is intrinsic to key structure; the manifest only references.** For each member, the client derives its rule *class* from the namespace **key structure** (`NamespaceId::is_owned()`/`is_communal()` — the marker bit) and the manifest's declared `rule` **must agree**, else the member is **dropped** to a `member-unverified` classification (never silently disappeared). A manifest can never relabel a communal namespace as gated. This binds the two rule **classes**, not roles — role-confusion within a class is display-only (design §9 residual, out of scope to fix).
2. **Root self-attests.** `manifest.root` must equal the owner key of the hosting namespace O, and the manifest must be carried by an **owned, zero-delegation** cap. A manifest whose `root != hosting-namespace owner`, OR carried by a delegated cap, is rejected.
3. **Site identity binds the root.** O's namespace owner key must equal the **followed root** (from Unit 1's admission context). A different owned namespace is a different site.

## 4. Validation is INDEPENDENT of admission (the load-bearing subtlety)

Admission (Unit 1) proves an entry was authored under *some* valid cap chain rooted at the followed site. That is **not sufficient** for the manifest: a broad `Area::full` owned cap, or a *delegated* cap under O, would pass Unit 1's admission yet must **not** be accepted as the manifest signer. Manifest validation independently requires:

- the signing cap `is_owned()` **and** has **zero delegations** (`delegations().is_empty()`), **and**
- `capability.receiver() == manifest.root == O owner key`.

Do NOT assume area-scoping implies this — assert it directly. (Design §8 Unit 2: "require an owned zero-delegation cap whose receiver == root … NOT assumed by area-scoping.")

## 5. Durable version floor (§5.2) — anti-rollback + equivocation

- Persist, in the durable profile, the **highest manifest `version` seen per site root**.
- **Refuse any lower version** (Willow LWW only protects same-coordinate writes — this floor is Riot-side).
- **Two conflicting owner signatures at the SAME version → equivocation alarm** surfaced to the user (a compromise signal), never a silent pick.
- `transport_policy.require` may never be lowered below the durably-seen floor OR the ticket floor, whichever is stricter (the `require`-monotonicity half; the ticket floor itself is Unit 5).

## 6. Tasks (TDD — RED first)

- **Task 1 — schema + canonical CBOR codec.** `SiteManifestV1` type + encode/decode in `riot-core` (newswire/model.rs codec style). RED: round-trip byte-identity; unknown-field / arity forgery rejected; open-enum forward-compat — unknown **role, rule, AND display** values decode to an `unknown` variant (not a hard error) — vs closed `layout` enum (unknown value → reject).
- **Task 2 — sign + validate (independent of admission).** Sign with an owned zero-delegation cap; validate per §4. RED cases: **root != owner → reject**; **delegated (non-zero-delegation) cap → reject** (even though it passes Unit 1 admission — prove both: admission accepts the entry, manifest-validation rejects it); **unsigned / bad signature → reject**; `receiver() != root → reject`.
- **Task 3 — member rule/key-structure classification.** For each member, compare declared `rule` class against the namespace marker bit. RED: a manifest declaring a communal ns as `owned-write` (or vice versa) → that member classified `member-unverified`, the rest of the site still resolves (no whole-manifest failure for one bad member).
- **Task 4 — durable version floor + require-monotonicity + equivocation (persisted, survives restart).** Persist, in the durable profile (`local_state` KV, keyed per site root), the highest-seen manifest `version` AND the strictest-seen `require` level. RED cases:
  - **version-rollback → reject** (a manifest at version N after N+1 was seen);
  - **require-downgrade → reject even at a HIGHER version** (a manifest at version N+1 that lowers `require: arti → none` below the durably-seen floor — passes the version check, must still be refused; §5.2 require-monotonicity, distinct from version-rollback);
  - **same-version two-signature → equivocation alarm** state (not a silent pick);
  - a higher version with equal-or-stricter `require` updates the floor;
  - **restart durability** — after the floor is persisted and the profile is reloaded from disk (simulated relaunch), a rollback/downgrade is STILL refused. A memory-only floor re-opens rollback on relaunch; the test must prove the disk round-trip.
- **Task 5 — FFI resolved record.** New `uniffi::Record` for the validated manifest (members + classifications + version + transport_policy + a **manifest validation status** enum incl. `manifest-invalid` / `manifest-rollback-alarm` / `member-unverified`). *(Name it "manifest validation status," NOT the composite "degradation enum" — Unit 4 owns the composite degradation enum and folds this in; Unit 2 reports only its own manifest-produced states.)* **UniFFI gate: the binding regen AND native staticlib rebuild land in the SAME commit** — a new `uniffi::Record` without the rebuild is a runtime checksum abort in the apps, not a compile error (documented recurring defect; see the UniFFI-record-change note). FFI smoke-loads on iOS + Android.

## 7. Adversarial RED cases (§8.1 Unit 2 — consolidated)

Each RED-then-green, forging raw records/caps as a hostile peer (not via the friendly signing API):
1. **`root != owner`** → reject.
2. **Delegated (non-zero-delegation) cap on `/manifest`** → reject, **independent of admission** (assert admission accepts, manifest-validation rejects).
3. **Rule/key-structure mismatch** → that member `member-unverified` (site still resolves).
4. **Version-rollback** (lower than durable floor) → reject.
5. **Require-downgrade at a higher version** (version N+1 lowers `require` below the durable floor) → reject (§5.2 require-monotonicity; distinct from #4).
6. **Same-version equivocation** (two valid owner sigs, same version) → alarm, not silent pick.
7. **Unsigned / bad-sig** → reject.
8. **Restart durability** — floor persisted, profile reloaded from disk, rollback/downgrade still refused.

## 8. File scope (claim in COLLABORATION.md; pathspec commits; worktree)

`crates/riot-core/src/newswire/` or a new `crates/riot-core/src/site/` module (manifest codec + validation + version-floor logic), `crates/riot-core/src/store/` (durable highest-version-per-root persistence — reuse the existing `local_state` KV pattern, no schema churn if avoidable), `crates/riot-ffi/src/` (new resolved manifest record + free fns), new tests under `crates/riot-core/tests/` + `crates/riot-ffi/tests/`. **New `uniffi::Record` → coordinator centralizes the native staticlib rebuild in the same commit.** `mobile_state.rs` classification may need the `/manifest` path family (coordinate with Unit 1's FFI classification work).

## 9. Verification gates

- `cargo test --workspace --all-features` green; clippy `-D warnings`; fmt; `validate-contracts`.
- Coverage at the `.coverage-thresholds.json` floor.
- Every §7 RED case demonstrably RED before, green after.
- **The independent-of-admission proof (RED 2) is the keystone** — a delegated/broad cap that passes admission but is refused as manifest signer. Without it, "validation independent of admission" is unproven.
- FFI smoke test loads the new record on both platforms (checksum-abort guard).

## 10. Sequencing & hazards

1. **Depends on Units 0 + 1 on `main`** (owned zero-deleg cap + admission gate). Unit 2 is the prerequisite for Unit 4 (render composes the validated manifest) and interacts with Unit 3 (moderation records live at `/mod/`, referenced by `moderation_path`).
2. **Independent-of-admission is the trap** — the natural implementation reuses the admission verdict; that is a security hole (broad/delegated cap). Validate the signer cap independently.
3. **Durable version floor is per-root and must survive restart** — a floor kept only in memory re-opens rollback on relaunch. Persist it.
4. **UniFFI checksum-abort trap** — new record + staticlib rebuild in one commit.
5. **Shared-checkout** — coordinate `mobile_state.rs` / newswire module edits with in-flight sessions; rebase on `main`, pathspec commits, STOP on foreign edits.
