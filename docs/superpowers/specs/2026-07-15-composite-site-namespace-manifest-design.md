# Composite Site / Namespace-Manifest Design

**Date:** 2026-07-15
**Status:** Design — pending design-review gate
**Scope:** v1 "indymedia site" as a composite of typed Willow namespaces bound by a signed manifest, with delegated editorial write, dual moderation, and policy-driven transport (iroh built, arti parked).

---

## 1. Problem & Motivation

Today in Riot, joining a space is fully **open**: `join_public_space(namespace_ref)` starts reconciling someone's public namespace and mints a per-community author — no gate, no approval, no request. willow25 ships the capability machinery (owned namespaces, meadowcap delegation, read/write caps, `PrivateInterest`) but Riot uses **only** self-minted communal write caps and explicitly *rejects* owned caps and delegations at admission (`import/bundle.rs:496`). Remote reach doesn't exist — discovery is nearby (BLE/LAN) or an out-of-band share link.

We want the **indymedia model**: public "sites" you find and follow by syncing, with a real editorial gate (who may publish articles), open participation (comments, open-publishing wire), moderation (ban a person, hide content), remote reach over the internet, and honest privacy properties for an activist user base.

This spec designs that as a **composite site**: one logical entity presented as a single surface, composed of several typed namespaces each with its own rule, bound by an owner-signed manifest.

### Scope decisions (locked in brainstorming)

- **Narrow now, universal-ready.** v1 designs *only* the indymedia site type. The manifest schema is generic enough to later describe `community`/`page`, but v1 **migrates nothing**.
- **Write gate = owned namespace + meadowcap delegation.** Multi-editor from v1 (owner delegates section-scoped write caps).
- **Transport = policy-driven, iroh built / arti parked / fail-closed.** The manifest declares reachability policy; the client obeys.
- **Moderation = full: revoke (person) + tombstone (content), dual-tier (admission best-effort + render guarantee).**
- **Binding model = Approach A:** manifest is an owner-signed record inside the editorial namespace E. Single root of trust = E's owner key.

### Out of scope (parked, dependent slices)

- Gated-**read** spaces / private groups (`PrivateInterest` / MLS). v1 sites are read-open.
- Gossip site-directory discovery. v1 discovery = handed ticket/QR/share-ref.
- The **arti/Tor** transport channel (embedding + onion-service hosting). Policy ships; channel is a follow-on.
- **nym** mixnet transport — research only.
- Redefining `community`/`page` as composites (the universal model). Schema is ready; migration is a later decision.

---

## 2. Architecture

**One composite site = 4 typed namespaces + a manifest record.**

```
Site  (identity = E owner key)
├─ E  editorial   OWNED     only owner + delegates write   → articles, masthead, MANIFEST
├─ C  comments    COMMUNAL  open, own-subspace, unlinkable  → threads
├─ W  open-wire   COMMUNAL  open publishing                 → tips / wire column
└─ M  moderation  OWNED (E key)  owner (+ deleg mods) write → tombstones, revocations
```

- **E / M are owned** namespaces (rooted in a namespace keypair). Only the owner key — and keys it delegates to — may write.
- **C / W are communal** — today's open publishing, unchanged: any key writes its own subspace, authors unlinkable by design.
- Read is **open** on all four (public site). Follow = sync.

### 2.1 Manifest record (Approach A)

Lives in **E**, signed by E's owner key:

```
site-manifest v1 {
  root:            <E owner pubkey>          # self-attesting: == E namespace owner
  members: [
    { ns: E, role: editorial,  rule: owned-write,   display: front/articles },
    { ns: C, role: comments,   rule: communal-open, display: under-articles },
    { ns: W, role: open-wire,  rule: communal-open, display: wire-column },
    { ns: M, role: moderation, rule: owned-write,   display: none/overlay },
  ]
  moderation_ns:   M
  transport_policy: { allow: [iroh, arti], require: none | arti }
  version:         <monotonic>               # latest owner-signed wins
  layout:          { ... }                   # render hints
}
```

`role` / `rule` / `display` are **open enums** so a future `community`/`page` is describable by the same schema. v1 emits only the site shape.

### 2.2 Two load-bearing invariants

1. **Rule is intrinsic; the manifest only references.** The client derives each member's rule from its namespace **key structure** (`is_owned()` / `is_communal()`) and the manifest's declared `rule` **must agree** — mismatch ⇒ member dropped. A manifest can never relabel a gated namespace as open (or vice versa).
2. **Root self-attests.** `manifest.root` must equal the owner key of the namespace hosting it (E) and must sign the manifest. A manifest whose root ≠ its hosting-namespace owner ⇒ rejected.

---

## 3. Editorial write & delegation

**E is owned. Two write paths:**

- **Owner** — `WriteCapability::new_owned(ns_keypair, owner_subspace)`, grants `Area::full()`. Writes masthead, manifest, any article.
- **Editors** — owner runs `WriteCapability::try_delegate(owner_kp, area, editor_key)` where
  `area = { subspace: editor's, path_prefix: /section/*, time_range: [now, expiry] }`.
  Section-scoped (path prefix), per-editor (subspace), **time-boxed** (expiry = soft-revoke lever). Columnist = narrower path; guest = short expiry.

**The delegated cap IS the editorial invite.** The delegation artifact travels via iroh / QR / share-ref. Holding it = you can write E; peers accept your entries because the chain verifies to E's owner. No pending-request server — the capability is the grant.

### 3.1 Admission change (the load-bearing edit)

Today `import/bundle.rs:496` rejects `is_owned() || !delegations().is_empty() || !namespace.is_communal()`. New logic for **owned** namespaces:

```
if namespace.is_owned():
    verify cap chain roots at namespace owner key       # genesis == manifest.root
    verify each delegation link (area containment + correct signer)  # try_delegate rules
    verify entry ∈ final granted area                   # includes(entry)
    verify entry timestamp ∈ cap time_range
    then existing checks: WILLIAM3 digest + subspace sig  # does_authorise
```

Applied at all three admission mirrors: `import/bundle.rs:496`, `session.rs:658`, `sync/state.rs:277`. **Communal namespaces (C, W) keep today's rule unchanged.**

**Retire the string-roster.** The app-level `editorial_roster` (`newswire_ffi.rs:43`) is replaced/backed by the cap check — cryptographic, not a name-compare.

---

## 4. Moderation (dual mechanism)

M is owned (E key); only owner (+ optionally delegated moderators, same delegation machinery) writes it. Two owner-signed record types, **two ban targets**:

- **Ban a PERSON** — `revoke { author_key, effective_ts }`
- **Ban CONTENT** — `tombstone { target_ns, target_entry }`

### 4.1 Two-tier enforcement (belt + suspenders)

**Tier 1 — admission, best-effort (shrinks the leak):**
- Cap **expiry** (`time_range`) — self-enforcing, airtight for routine offboarding.
- **Timestamp-monotonic** reject (entry ts < max-seen-for-author) — stops *lazy* backdating. **Not airtight:** an attacker controlling delivery order can seed a backdated entry to a fresh peer before the higher-timestamp entries arrive. Accepted limitation.
- Deny-list check against M's revocations where M is synced.

**Tier 2 — render, the guarantee (closes the leak):**
- **Hide by author-key:** any entry whose cap-receiver ∈ revoked keys ⇒ not rendered. Identity is signed and unforgeable — holds regardless of timestamp lies or delivery order.
- **Hide by entry-id:** tombstoned entries ⇒ not rendered.
- Applied as a filter over the whole composite (E/C/W) every render.

### 4.2 The guarantee, stated honestly

A banned person or post may still exist in *some peer's store* — you cannot delete bytes from others' disks, nor force global sync order. But **on any client rendering the site with M synced, it is invisible.** Publication = what is shown. Admission shrinks the window; render closes it.

**Timestamp reality:** Willow enforces last-writer-wins only *at the same coordinate*; there is **no global per-author monotonic clock**. A new article at a new path may carry any timestamp within the cap's `time_range`, so a determined revoked editor can backdate. This is why the *guarantee* rests on **identity at render**, not on the clock at admission.

**Keep pre-ban good work:** owner may maintain an explicit **re-endorse allow-list** so a fired editor's earlier articles are not auto-hidden unless intended.

---

## 5. Transport (policy-driven)

Transport is a `FrameChannel` family; every channel carries the same `SyncFrame` bytes through the existing `ByteSyncSession` bridge; the transport-agnostic reconcile FSM (`sync/state.rs`) never learns which channel it's on.

```
FrameChannel family:
  loopback · BLE · LAN     existing — offline / in-person
  iroh                     v1 — internet, fast, NO metadata privacy
  arti / Tor               parked (policy-ready) — onion, IP-hiding
  nym                      research — not v1/v2
```

### 5.1 Manifest-declared policy → client obeys

```
transport_policy: { allow: [iroh, arti], require: none | arti }
```
- Public newswire → `require:none` → client uses fast iroh.
- Dissident site → `require: arti` → onion only.

### 5.2 FAIL CLOSED (critical security property)

If a site's `require` names a transport the client cannot provide, the client **refuses to connect and warns** — it MUST NEVER silently fall back to iroh and leak the follower's IP. Downgrade-to-leak is the bug that gets an activist hurt. The policy floor is a hard gate, not a preference. **Consequence in v1:** a `require: arti` site cannot be served until the arti channel lands — clients correctly fail closed rather than offer false privacy.

### 5.3 iroh channel (v1)

- iroh is a Rust crate ⇒ integrate in `riot-core`/`riot-ffi`, not per-app. An iroh `Connection` stream ⇄ `ByteSyncSession` ⇄ `ReconcileSession`.
- **Node keypair** stored in the durable profile; **transport identity ≠ content identity** (NodeId never reveals authorship).
- **Ticket / share-ref:** `riot://site/v1/<E-namespace>/<manifest-hint>?node=<NodeAddr>`. NodeId→address via iroh discovery (pkarr/DNS/DHT).
- **Seeding:** the gateway (`apps/gateway/`) runs an always-on iroh node seeding the site's namespaces; any follower who holds data reseeds others (Willow set-reconcile). Origin offline ≠ site dead while ≥1 provider is reachable.

### 5.4 iroh privacy properties (documented, not solved)

iroh gives **confidentiality (encrypted content), not anonymity.** Relays see NodeId ↔ NodeId, IP, timing, volume; peers learn each other's IP; the follow-graph is inferable. Fine for public sites (content isn't secret; metadata is the residual risk). Metadata privacy is the **arti** channel's job (parked). v1 cheap mitigations: **ephemeral follower NodeId** (clients rotate; only seeds need stable identity), transport-identity ≠ content-identity, self-hosted-relay option.

---

## 6. Composite render

Composition/overlay logic lives in **Rust core** (`riot-core`, headless-testable), exposed via FFI as a resolved **view model**; native apps render the view model with no business logic (per CLAUDE.md shared-core rule).

**Pipeline:**
```
1. Follow site → sync E → read latest site-manifest (owner-signed, highest version wins)
2. Validate members: key-structure must match declared rule, else DROP member
3. Sync M FIRST (moderation must be present before showing anything)
4. Sync E, C, W per transport_policy
5. Build view:
     E → articles / masthead   (trusted: cap-chain verified at admission)
     C → threads under articles (open; author = subspace, unlinkable)
     W → wire column           (open publishing)
6. Apply moderation overlay (M):
     drop entries whose author_key ∈ revoked
     drop entries whose id ∈ tombstoned
     keep owner re-endorsed allow-list
7. Resolve soft links (comment → article parent); tolerate dangling
```

- **Progressive degradation:** have E not C ⇒ articles render, comments "loading." **M-not-synced ⇒ fail-safe: do not render open namespaces** (blank beats un-moderated). Owner-authored E always safe (root never revoked/tombstoned).
- **Trust tiers legible in UI:** E editorial = full trust, front page; W open-wire = visibly "unverified/open submission," never masquerading as editorial; C comments = open, unlinkable.
- **Soft links** across namespaces resolved at render only (C is open — no referential-integrity constraint at admission); dangling ref ⇒ collapse gracefully.

---

## 7. Work units (decomposition)

| # | Unit | Touches | Depends |
|---|---|---|---|
| **1** | Owned-namespace admission + delegation-chain verify | `import/bundle.rs:496`, `session.rs:658`, `sync/state.rs:277`, `willow/identity.rs` | — |
| **2** | Site manifest record: schema, sign, validate (root self-attest + rule-intrinsic invariant) | `riot-core` new module, FFI | 1 |
| **3** | Moderation: revoke + tombstone records; admission best-effort + render filter | `riot-core` moderation module, `sync/state.rs` | 1, 2 |
| **4** | Composite resolver + view model (per-ns rules, overlay, progressive, soft links) | `riot-core` render module, FFI view model | 2, 3 |
| **5** | iroh `FrameChannel` + transport-policy + fail-closed gate + ephemeral NodeId | `riot-core`/`riot-ffi` transport, share-ref, gateway seed | 2 |
| **6** | Native UI: follow-site, composite surface, trust-tier styling, editor-invite (cap handoff) | iOS + Android | 4, 5 |

Each unit ships independently behind the shared core. Unit 1 is the critical-path unlock (admission core).

---

## 8. Testing (TDD)

- **Unit 1 = adversarial-heavy (security core, most test weight):** forged delegation chain, over-broad area, expired cap, wrong root, cross-namespace cap reuse, communal cap used in an owned namespace.
- **Manifest:** root≠owner reject; rule/key-structure mismatch reject; version rollback reject; unsigned reject.
- **Moderation:** render hides revoked-author **even with a backdated timestamp** (the identity-guarantee test); tombstone hides entry; M-not-synced fails safe; re-endorse allow-list survives.
- **Transport:** **fail-closed test** — `require:arti`, arti absent ⇒ refuse, no iroh fallback (the activist-safety test); ephemeral NodeId rotation; loopback + iroh carry identical frames.
- **Render:** partial-sync degradation; dangling soft-link collapse; trust-tier separation.
- **Coverage:** honor `.coverage-thresholds.json` ratchet floor (per CLAUDE.md — not 100%, but must not regress; Rust line floor is CI-enforced).

---

## 9. Risks

1. **Admission-core edit (Unit 1).** The copy-on-write preview boundary is load-bearing; a bug admits forgeries or corrupts state. Mitigation: adversarial tests first, isolated unit, no other changes bundled.
2. **arti maturity.** Parked; onion-service *hosting* may be immature on mobile. De-risked by parking + fail-closed (no false privacy meanwhile).
3. **Timestamp backdating.** Unsolved at admission by design; render-identity guarantee is the backstop. Documented, not hidden.
4. **Namespace sprawl / sync cost.** 4 namespaces × N sites. Lifecycle: unfollow drops members; monitor sync state growth.
5. **Universal-model scope-creep.** Schema is generic but v1 migrates nothing; resist redefining `community` now.

---

## 10. Open questions for the design-review gate

- Is the **manifest a first-class Willow record** or a distinguished path within E? (Assumed: distinguished owner-signed record at a reserved path in E.)
- Should **delegated moderators** exist in v1, or owner-only moderation to start? (Assumed: delegation machinery reused, owner-only is the common case.)
- Does the **rule-intrinsic invariant** need a canonical namespace-type tag, or is `is_owned()`/`is_communal()` sufficient to bind role↔rule?
- Is **"narrow now, universal-ready"** actually buildable without accidentally committing to the universal model in the manifest schema?
