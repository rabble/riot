# Followed-site manifest delivery

**Date:** 2026-07-19
**Status:** Proposed (design for review; drives the PR3.5 build)
**Author:** design scoping (agent), for rabble review
**Scope:** How a follower who HTTP-pulls a composite site's bundle obtains the site
MANIFEST bytes needed to RENDER the moderated view (`resolve_composite_site`), and
how a full-site export (manifest + `/mod` + `/articles`) imports cleanly.

All file:line references are against `origin/main` at `1f6ecb2`.

---

## 1. The problem, precisely

A follower (Option C, HTTP-pull, all-merged) can import an owner's `/mod` + `/articles`
records via `import_followed_site_bundle` (`crates/riot-ffi/src/site_ffi.rs:986`). Those
land in the shared evidence store. But to RENDER the moderated site the follower must call
`resolve_composite_site` (`site_ffi.rs:549`), and that method takes the **manifest as four
owner-signed wire arguments** (`entry_bytes`, `capability_bytes`, `signature`,
`payload_bytes`) — not from the store. A followed-site follower has **no way to obtain or
hold** those bytes today. Result: the follower is stuck at `SiteDegradation::ManifestInvalid`
with zero items, unable to render.

Two intertwined facts had to be confirmed. Both are confirmed below, and the first is
**stronger than assumed**.

---

## 2. Fact 1 — the manifest is ARG-VALIDATED, never STORE-ADMITTED (CONFIRMED, and stronger)

### 2.1 Admission refuses `/manifest`

`import/bundle.rs` admits an owned composite-site namespace's entries only for `/articles/`
or `/mod/` (`crates/riot-core/src/import/bundle.rs:605-620`):

```rust
let schema_ok = if entry.namespace_id().is_owned() {
    // ... The reserved `/manifest` carries no schema here and is refused (Unit 2
    // validates it on an independent path). ...
    crate::willow::site_paths::is_under_articles(entry.path())
        || crate::willow::site_paths::is_under_mod(entry.path())
}
```

An entry at `O:/manifest` fails `schema_ok`, so `decode_bundle_with_root` classifies it
`ItemStatus::Invalid` (`bundle.rs:376-377`). **The store cannot hold a `/manifest` entry.**

### 2.2 Validation is a standalone function over passed-in bytes

`validate_site_manifest(signed: &SignedWillowEntry, followed_site_root: &[u8;32])`
(`crates/riot-core/src/site/validate.rs:146`) does the entire security check on an
argument — decode entry/cap, `/manifest` path shape, `is_owned()`, **zero delegations**
(`SignerDelegated` at `validate.rs:184`), `granted_namespace == entry namespace`,
`manifest.root == entry namespace` (invariant 2, `RootMismatch`), `manifest.root ==
followed_site_root` (invariant 3, `SiteIdentityMismatch`), and finally `verify_entry`
(the cryptographic chain). It never touches a store.

### 2.3 Resolve consumes the manifest as an argument, reads only `/mod` + `/articles` + members from the store

`resolve_composite_site_from_store` (`site_ffi.rs:585-593`) validates the passed-in `signed`
manifest first (`let Ok(validated) = validate_site_manifest(signed, &root) else { return
Ok(manifest_invalid_view(root)) }`), then loads `O:/mod/` records, the protected set, and
each member namespace's content **from the store by namespace**. The manifest itself is
never loaded from the store.

### 2.4 STRONGER FINDING: the manifest is stored NOWHERE, and no owner-side publish path exists

There is **no** FFI or core path that writes a manifest into any store. `create_owned_site`
(`site_ffi.rs:77`) only generates an `OwnedMasthead` and returns the sealed root — no
manifest. A repo-wide search finds `MANIFEST_COMPONENT` used only in `validate_site_manifest`,
`site_paths` path helpers, `masthead.rs` tests, and `site_ffi.rs` **test helpers**
(`manifest_wire()` at `site_ffi.rs:1148`, which synthesizes an owner-signed manifest and hands
it straight to resolve). In production the manifest exists only as an artifact the caller
constructs and passes in.

**Consequence for Fact 2:** `build_followed_site_offer` (§3) reads the owner store's live
entries. Since the owner store holds no manifest, today it offers **only** `/mod` + `/articles`.
The "manifest poisons the export" bug is therefore **latent-conditional** — it fires the moment
an owner-side manifest exists to export, not today. The `/mod`-only round-trip test passes
because a `/mod`-only bundle is the **only kind the system can currently produce**.

### 2.5 Verdict on the first-instinct fix

Re-admitting `/manifest` to the store (Option C, §4) is **wrong**. It fights Unit 2's
independent-path model on both ends: (a) `resolve_composite_site` is explicitly built to take
the manifest as an argument and would have to be re-plumbed to read it from the store; (b)
`validate_site_manifest`'s whole contract is "validate this artifact, independent of admission."
Storing the manifest also means the manifest — an owner record — would sit in the same namespace
the resolve step treats as "protected against moderator tombstones," conflating the trust anchor
with the content it anchors. Do not do this.

---

## 3. Fact 2 — the latent export/import rejection bug (CONFIRMED)

`build_followed_site_offer(profile, namespace_id)` (`crates/riot-ffi/src/mobile_state.rs:2091`)
returns **all live signed entries** in namespace `O`, gated only by the live-id equality guard —
no family filter. If a manifest ever becomes a live entry in the owner store, it is included.

`import_followed_site_bundle` (`site_ffi.rs:986`) routes through the single canonical gate
`admit_followed_site_frame` (`crates/riot-core/src/site/follow.rs:50`). That gate decodes the
whole bundle under `root` and requires **every** item to be `ItemStatus::Valid` AND to pass the
family gate `is_followed_site_family` = `/mod` + `/articles` ONLY (`follow.rs:35-38`). It is
strictly all-or-nothing (`follow.rs:65-83`):

```rust
let ItemStatus::Valid(_) = &item.status else {
    return Err(FollowedSiteAdmitError::Rejected);   // one bad item ⇒ whole bundle rejected
};
...
if !is_followed_site_family(&entry) { return Err(FollowedSiteAdmitError::Rejected); }
```

A manifest entry is `ItemStatus::Invalid` (§2.1), so a genuine full-site export (manifest +
`/mod` + `/articles`) is **rejected wholesale**. Confirmed. The same gate backs the WU2 sync
session and the WU3 transport follower (`crates/riot-transport/src/iroh.rs:312`), so the trap is
uniform across all three delivery paths.

---

## 4. Options

Every option must answer the same real requirement, which the "return the manifest to the caller"
framing under-weights: **`resolve_composite_site` needs the manifest on EVERY render** (app
launch, tab re-open, moderation refresh), not once at import. So whatever delivers the manifest,
the follower must **durably HOLD** it keyed by root. The `CommunityRecord` (the Following record)
is the natural home — it already carries `fetch_url: Option<String>` and
`last_sync_unix_seconds` with an optional-field codec (`crates/riot-ffi/src/community_registry.rs:113,232-285`).

| Option | How follower gets the manifest | Fixes export/import bug? | Security | Unit 2 consistency | Complexity |
|---|---|---|---|---|---|
| **A — import extracts + validates + returns + persists** | Manifest rides IN the bundle; import pulls it out, validates via `validate_site_manifest`, persists on the Following record, returns bytes. Resolve loads from the record. | **Yes** — manifest is HANDLED, not rejected | Strong — `validate_site_manifest` is the full check; never store-admitted | **Consistent** — validated as an artifact, kept out of store | Medium: core partition helper + FFI persist + `resolve_*_by_root` + registry field |
| **B — separate manifest delivery** | Owner publishes `manifest.bin` to the mirror separately; follower fetches it separately, persists on the record (or passes straight to resolve). Bundle stays `/mod`+`/articles`. | Only if paired with D (exclude `/manifest` from offer) so a future manifest-in-store can't poison the bundle | Same check (`validate_site_manifest`) | Consistent | Medium, but **two artifacts, two fetches** — worse for the single-mirror HTTP-pull model |
| **C — re-admit `/manifest` to the store** | Store holds it; resolve reads it from the store. | Yes | Weaker — conflates trust anchor with protected content; must re-plumb resolve | **Fights Unit 2** (§2.5) | High + wrong |
| **D — offer EXCLUDES `/manifest`** | It doesn't — follower gets no manifest. | Fixes rejection only | n/a | Consistent | Low, but **dead-end alone** (no manifest ⇒ can't resolve) |

Note: A and B **converge** on "persist the manifest on the Following record." Their only real
difference is packaging — one artifact (A) vs two (B). D is not a solution on its own but its
filter is a useful defense (see §5.4). C is rejected.

---

## 5. Recommendation — Option A (bundle-carried, extract + validate + return + persist)

The manifest travels inside the same bundle the follower already pulls, so the follower gets the
whole site in **one HTTP pull** (rabble's single-mirror model). Import partitions the bundle:
the manifest is validated and **held app-side / persisted on the Following record** (never
store-admitted, per Unit 2); the `/mod` + `/articles` records go through the existing canonical
gate unchanged. Resolve gains a by-root form that loads the held manifest, so the app never
re-supplies it.

This keeps `validate_site_manifest` as the sole security check, keeps the manifest out of every
store, and fixes the all-or-nothing rejection by **handling** the manifest rather than rejecting
it.

### 5.1 Core changes (`crates/riot-core`)

Add a partitioning admit helper beside the existing gate so all three delivery paths (manual,
WU2 sync, WU3 transport) share one manifest-aware boundary — the same reason `admit_followed_site_frame`
is canonical:

```rust
// crates/riot-core/src/site/follow.rs
pub struct AdmittedFullSite {
    pub committed: u32,
    /// The validated owner-signed manifest bytes, if the bundle carried one.
    /// NOT stored — returned for the caller to hold. None ⇒ manifest-less bundle.
    pub manifest: Option<SignedWillowEntry>,
}

pub fn admit_full_site_bundle(
    store: &EvidenceStore,
    root: [u8; 32],
    bytes: &[u8],
    route: &str,
) -> Result<AdmittedFullSite, FollowedSiteAdmitError>;
```

Behaviour (fail-closed, all-or-nothing over the `/mod`+`/articles` portion):
1. `decode_bundle_with_root(bytes, Some(root))`. **Do not** require every item `Valid` up front
   — the manifest item is expected to be `Invalid` (UnsupportedSchema) under the store schema.
2. Partition decoded items by path: the single `O:/manifest` entry (exact one-component
   `/manifest`, owned namespace) vs the rest.
3. If a manifest entry is present, reconstruct its `SignedWillowEntry` and run
   `validate_site_manifest(&signed, &root)`. **Any** failure (forged signature, communal,
   delegated signer, wrong root) rejects the WHOLE import (`FollowedSiteAdmitError::Rejected`).
   More than one manifest entry, or a `/manifest/...` sub-path, is also a rejection.
4. Admit the remaining items exactly as `admit_followed_site_frame` does today (family-gate to
   `/mod`+`/articles`, `Valid`-only, eligible-count equality, plan+commit). A manifest-less
   bundle keeps today's behaviour precisely.
5. Return `{ committed, manifest: Some(signed) | None }`.

`admit_followed_site_frame` stays as the `/mod`+`/articles`-only primitive; `admit_full_site_bundle`
delegates to it for the non-manifest remainder (re-encode the remainder via `encode_bundle`, or
share an internal items-list admit). The WU3 transport follower (`iroh.rs:312`) and WU2 sync
session should migrate to `admit_full_site_bundle` so manifests delivered over sync/transport are
handled identically — but that migration can be a follow-up; the manual path is PR3.5's target.

### 5.2 FFI changes (`crates/riot-ffi`)

- **Registry:** add `manifest_bytes: Option<Vec<u8>>` (the four wire fields serialized as one
  `SignedWillowEntry` blob) to `CommunityRecord` (`community_registry.rs`), with the matching
  optional-field encode/decode (mirror `fetch_url`/`last_sync_unix_seconds` at lines 232-285).
  Capped by `MAX_SITE_MANIFEST_BYTES` so the persisted registry blob can't balloon.

- **Import returns + persists the manifest:** `import_followed_site_bundle` calls
  `admit_full_site_bundle`; on a returned manifest it (a) persists it on the Following record and
  `persist_registry`, and (b) surfaces it in `ImportSummary`:

  ```rust
  pub struct ImportSummary {
      pub imported: u32,
      pub manifest_present: bool,   // or Option<Vec<u8>> if the app wants it inline
  }
  ```

- **Resolve by root:** add a by-root form that loads the held manifest so the app never re-passes
  it:

  ```rust
  pub fn resolve_composite_site_by_root(
      &self, root: Vec<u8>, now_unix_seconds: u64,
  ) -> Result<ResolvedCompositeSite, MobileError>;
  // loads CommunityRecord.manifest_bytes for `root`; None ⇒ manifest_invalid_view(root)
  ```

  Keep the existing arg-taking `resolve_composite_site` (tests and any owner-side direct render
  still use it).

- **Owner-side export must attach the manifest** (see §7 open question): `build_followed_site_offer`
  reads the store and the store has no manifest, so a full-site export needs the owner's app-held
  manifest bytes attached. Recommended: an owner-facing wrapper
  `export_full_site_offer(root, manifest_signed) -> bytes` that prepends the manifest
  `SignedWillowEntry` to `build_followed_site_offer(root)`'s output, rather than widening the
  existing signature. This is only needed once an owner-mint exists.

- **UniFFI coupling:** `ImportSummary`, `CommunityRecord`, and the new `resolve_*_by_root` change
  the generated surface. Bindings + staticlib MUST regenerate together
  (`scripts/conference/build-native-core.sh`) or the apps runtime-checksum-abort. (See memory
  `riot-uniffi-record-change-coupling`.)

### 5.3 Adversarial tests (all core-level unless noted)

1. **Forged/communal manifest in bundle** → `validate_site_manifest` fails (`SignatureInvalid` /
   `NamespaceNotOwned`) → whole import rejected; store unchanged.
2. **Manifest for a DIFFERENT root** in the bundle → `SiteIdentityMismatch` / `RootMismatch` →
   whole import rejected.
3. **Delegated-signer manifest** → `SignerDelegated` → rejected (guards the zero-delegation keystone).
4. **Full-site export (manifest + `/mod` + `/articles`) round-trips**: export → import commits the
   `/mod`+`/articles`, returns the manifest; previously (pre-fix) this was rejected wholesale.
5. **Resolve advances off degradation**: after importing a manifest + `/mod` heartbeat,
   `resolve_composite_site_by_root` returns a non-`ManifestInvalid` view (e.g. `ModerationLoading`
   → then `Current`), proving the persisted manifest is loaded.
6. **Manifest-less bundle still admits**: a `/mod`-only bundle commits (unchanged), and
   `resolve_*_by_root` stays `ManifestInvalid` (honest degraded — no manifest yet).
7. **Two manifest entries / `/manifest/x` sub-path** → rejected.
8. **Persistence round-trip** (FFI): import a manifest, drop + restore the profile, resolve still
   renders (manifest survived on the registry record).
9. **Unfollowed root** (existing invariant preserved): a full-site bundle for an unfollowed root is
   refused before admission (`site_ffi.rs:999-1004`).

### 5.4 Defense note on Option D

Because Option A deliberately carries the manifest in the bundle, `build_followed_site_offer` must
**not** silently drop it — but it also must not blindly ship whatever is in the owner store. Since
the manifest is never store-admitted (§2.4), the offer builder's live-set will never contain one;
the manifest is attached explicitly by `export_full_site_offer`. That is the correct posture: the
manifest is added deliberately, not leaked incidentally. No change to the live-id equality guard.

### 5.5 Rollout — two PRs

- **PR3.5a (Rust core + FFI):** `admit_full_site_bundle`, `CommunityRecord.manifest_bytes` +
  codec, `import_followed_site_bundle` persist+return, `resolve_composite_site_by_root`,
  regenerate bindings, all §5.3 tests. Self-contained and independently verifiable.
- **PR3.5b (iOS + Android shells):** call `import_followed_site_bundle` then
  `resolve_composite_site_by_root(root, now)`; drop any manual manifest-passing. Native protocol
  vectors / anchor tests updated if the record shape is pinned.

Rust-then-shell (not one PR) because the UniFFI regeneration is the seam and the Rust side is
testable on its own.

---

## 6. Why not the alternatives (summary)

- **B (separate fetch):** same security, but two artifacts and two mirror round-trips for what is
  logically one site pull. If a future design needs the manifest to refresh independently of
  content (owner re-signs members without republishing articles), B's decoupling becomes
  attractive — worth noting, not worth adopting now.
- **C (store-admit):** fights Unit 2, re-plumbs resolve, conflates trust anchor with protected
  content. Rejected.
- **D (exclude only):** leaves the follower with no manifest. Not a solution; folded into A as a
  posture note (§5.4).

---

## 7. Open questions for rabble

1. **Owner-side manifest mint/hold.** There is no owner FFI that produces or holds a signed
   manifest today (§2.4) — it exists only in test helpers. Option A's export needs the owner's
   app-held manifest bytes. Is a `mint_site_manifest` / owner-hold path already planned, and is it
   in PR3.5's scope or a predecessor? (Follower render is testable with a test-minted manifest, so
   PR3.5a need not block on this.)
2. **Persist on the record vs re-fetch each session.** Recommendation is to persist
   `manifest_bytes` on the `CommunityRecord` so render survives an offline app restart. Acceptable
   to grow the persisted registry blob by up to `MAX_SITE_MANIFEST_BYTES` per followed site?
3. **Manifest supersession.** When the owner republishes a v2 manifest (new members, rotated
   transport policy), how does the follower replace the stored one — last-write-by-`version`, a
   monotonic floor, or newest-valid-wins on import? Needs a rule so a stale mirror can't pin an old
   member set.
4. **First-import policy.** Should following a site REQUIRE a manifest in the first bundle, or is a
   manifest-less-then-later-manifest sequence allowed (follower sits at `ManifestInvalid` until one
   arrives)? Recommendation: allow the sequence (honest degraded state), matching the existing
   fail-closed resolve.
5. **Sync/transport parity.** Migrate the WU2 sync session and WU3 transport follower
   (`iroh.rs:312`) to `admit_full_site_bundle` in the same PR, or as a fast follow? (They currently
   can't carry a manifest either — same latent trap.)
