# Composite Site / Namespace-Manifest Design

**Date:** 2026-07-15
**Status:** Design — **gate PASSED (accepted 2026-07-15)**. PM/Designer/Architect/CTO approved (code-verified); Security's blockers resolved across 3 rounds, the final ticket-freshness fix applied per its pre-committed approval. User accepted at the 3-round boundary rather than run a 4th Security pass. Next: writing-plans → plan-review gate.
**Scope:** v1 "indymedia site" as a composite of typed Willow namespaces bound by an owner-signed manifest, with delegated editorial write, dual moderation, and policy-driven transport (iroh built, arti parked).

**Revision note (round 3 gate):** the signed ticket now binds **freshness** — a monotonic `epoch` + `exp` inside the signature (§5.1), so a genuinely-root-signed *stale* lower-floor ticket can't be replayed against a returning follower (refused below the durable epoch floor) and is TTL-bounded for a first-time follower; unknown `require` tokens **fail closed**; the `mod_epoch` heartbeat now carries a **`mod_set_digest`** committing to the revoke/tombstone set, so *tail*-suppression by a withholding provider is detectable (not just middle gaps). Documented that escalating a site's `require` cannot revoke already-distributed tickets without key rotation (ties to the parked successor-key slice).

**Revision note (round 2 gate):** the ticket transport floor is now **root-signed and verified pre-dial** (§5.1) — a bare digest was only checkable post-connection, so an unsigned floor could be stripped to re-open the bootstrap IP-leak (Security blocker); added a signed `mod_epoch` **moderation heartbeat** (§4) that makes the render guarantee detectable against a withholding provider and gives the freshness window a concrete non-gameable definition; elevated the single-root seizure limitation to a **mandatory user-facing disclosure** (§9.3, Unit 6); restored the consolidated per-unit test checklist + coverage ratchet-floor + UniFFI-rebuild gate (§8.1); manifest validation now independently requires an owned zero-delegation cap (§8 Unit 2); root secret is keystore-backed (§8 Unit 0).

**Revision note (round 1 gate):** collapsed editorial + moderation into a single owned "masthead" namespace (was two, which made the moderation trust-root incoherent); moved the transport floor into the signed ticket (bootstrap leak defeated fail-closed); added a durable monotonic manifest-version floor (rollback downgrade); made `capability.is_owned()` a load-bearing admission **invariant** rather than a test (a communal cap can name an owned namespace); enumerated the full admission-gate surface (a 4th gate exists); added Unit 0 for owner-side cap minting/signing/delegation-issuance (was unbuilt and unassigned); added a resolved view-model contract with honest degradation states; scoped person-ban honestly (inert in communal namespaces).

---

## 1. Problem & Motivation

Today in Riot, joining a space is fully **open**: `join_public_space(namespace_ref)` starts reconciling someone's public namespace and mints a per-community author — no gate, no approval, no request. willow25 ships the capability machinery (owned namespaces, meadowcap delegation, read/write caps, `PrivateInterest`) but Riot uses **only** self-minted communal write caps and explicitly *rejects* owned caps and delegations at admission. Remote reach doesn't exist — discovery is nearby (BLE/LAN) or an out-of-band share link.

We want the **indymedia model**: public "sites" you find and follow by syncing, with a real editorial gate (who may publish articles), open participation (comments, open-publishing wire), moderation (ban a person, hide content), remote reach over the internet, and honest privacy properties for an activist user base.

This spec designs that as a **composite site**: one logical entity presented as a single surface, composed of typed namespaces each with its own rule, bound by an owner-signed manifest.

### Scope decisions (locked in brainstorming + round-1 gate)

- **Narrow now, universal-ready.** v1 designs *only* the indymedia site type. The manifest schema is generic enough to later describe `community`/`page`, but v1 **migrates nothing** and serializes only the site shape.
- **Write gate = owned namespace + meadowcap delegation.** Multi-editor from v1 (owner delegates section-scoped write caps).
- **Transport = policy-driven, iroh built / arti parked / fail-closed**, with the transport floor carried in the **signed ticket** (not only inside the namespace).
- **Moderation = revoke (person, cap-holders only) + tombstone (content), dual-tier (admission best-effort + render guarantee).**
- **Binding model = Approach A:** manifest is an owner-signed record at a reserved, non-delegatable path inside the owned masthead namespace. Single root of trust = the masthead owner key.
- **Key recovery:** v1 has a **single root key, no rotation/recovery** — documented limitation (§9). Successor-key/rotation is a named future slice.
- **`require:arti` sites:** creatable in v1 but **unfollowable** until the arti channel lands; clients fail closed.
- **QR:** generation + camera scanning built on iOS + Android in v1 (Unit 6).

### Out of scope (parked, dependent slices)

- Gated-**read** spaces / private groups (`PrivateInterest` / MLS). v1 sites are read-open.
- Gossip site-directory discovery. v1 discovery = handed ticket/QR/share-ref.
- The **arti/Tor** transport channel (embedding + onion-service hosting). Policy + fail-closed ship; channel is a follow-on.
- **nym** mixnet transport — research only.
- **Owner-key rotation / successor keys / threshold multisig** — future slice.
- **Automated anti-flood** (rate-limit / proof-of-work / per-author accumulation) for the open communal namespaces — future slice.
- Redefining `community`/`page` as composites (the universal model). Schema is ready; migration is a later decision.

---

## 2. Architecture

**One composite site = 3 typed namespaces + a manifest record.**

```
Site  (identity = O owner key)
├─ O  masthead   OWNED    single owner keypair = root of trust
│    ├─ /manifest       reserved, NON-DELEGATABLE — owner (owned, zero-deleg cap) only
│    ├─ /articles/<section>/…   delegatable to editors (section-scoped write caps)
│    └─ /mod/           reserved, NON-DELEGATABLE by editors — owner (+ deleg moderators)
├─ C  comments   COMMUNAL open, own-subspace, unlinkable  → threads
└─ W  open-wire  COMMUNAL open publishing                 → tips / wire column
```

- **O is one owned namespace** rooted in a single namespace keypair. Editorial articles, the manifest, and moderation records all live in O, separated by **path region**. This makes "root of trust = O owner key" literally true for all three, and lets moderation sync *with* editorial (no cross-namespace "did M arrive?" gap on the owned side).
- The **manifest** and **moderation** paths are **reserved and never delegated** — the owner only ever mints editor caps whose `path_prefix` is under `/articles/…`, so a delegated editor can never write the manifest or a revoke/tombstone.
- **C / W are separate communal** namespaces — today's open publishing, unchanged: any key writes its own subspace, authors unlinkable by design. (They must be separate namespaces because the *rule class* differs from O — see invariant 1.)
- Read is **open** on all three (public site). Follow = sync.

### 2.1 Manifest record (Approach A)

Lives at `O:/manifest`, signed by O's owner (an **owned, zero-delegation** cap whose receiver == O owner key):

```
site-manifest v1 {
  root:            <O owner pubkey>          # self-attesting: == O namespace owner key
  members: [
    { ns: O, role: masthead,  rule: owned-write,   display: front/articles },
    { ns: C, role: comments,  rule: communal-open, display: under-articles },
    { ns: W, role: open-wire, rule: communal-open, display: wire-column },
  ]
  moderation_path: /mod/                     # where clients read revoke/tombstone (in O)
  transport_policy: { allow: [iroh, arti], require: none | arti }
  version:         <monotonic u64>           # durable highest-seen floor; see §5.2
  layout:          <closed enum>             # resolved section order; NOT free-form hints
}
```

`role` / `rule` / `display` are **open enums** so a future `community`/`page` is describable by the same schema; v1 emits only the site shape. `layout` is a **closed enum** resolved by core into a section order the shells render verbatim (no owner-authored render blob parsed in the apps).

### 2.2 Load-bearing invariants

1. **Rule is intrinsic to key structure; the manifest only references.** The client derives each member's rule class from its namespace **key structure** — `NamespaceId::is_owned()` / `is_communal()` (the marker bit) — and the manifest's declared `rule` **must agree** or the member is **dropped** (with a resolved "unverified member" state, §6). A manifest can never relabel a communal namespace as gated. *Note:* this binds the two **rule classes**, not roles; role-confusion within a class (e.g. swapping which communal ns is "comments" vs "wire") is only producible by the legitimate signer and is display-only — see §9 residual risk.
2. **Root self-attests.** `manifest.root` must equal the owner key of the namespace hosting it (O) and must sign the manifest with an **owned, zero-delegation** cap. A manifest whose root ≠ its hosting-namespace owner, or carried by a delegated cap, is rejected.
3. **Site identity binds the root.** A follower follows a specific site root (from the ticket, §5). On admission, O's namespace owner key must equal the followed root — a different owned namespace is a different site, never silently accepted as this one.

---

## 3. Editorial write & delegation

**O is owned. Two write paths:**

- **Owner** — `WriteCapability::new_owned(O_keypair, owner_subspace)`, grants `Area::full()`. Writes manifest (`/manifest`), moderation (`/mod/`), any article.
- **Editors** — owner runs `WriteCapability::try_delegate(O_owner_kp, area, editor_key)` where
  `area = { subspace: editor's, path_prefix: /articles/<section>/*, time_range: [now, expiry] }`.
  Section-scoped (path prefix **under `/articles/`only** — never `/manifest` or `/mod/`), per-editor (subspace), **time-boxed** (expiry = soft-revoke lever). Columnist = narrower path; guest = short expiry.

**The delegated cap IS the editorial invite** — but it is a **two-way handshake**, not a one-way handoff: `try_delegate` binds the invitee's subspace, so the owner needs the **invitee's author key first**. Flow: invitee presents key (QR/paste) → owner mints section-scoped cap → returns cap (QR/paste). Peers then accept the editor's entries because the chain verifies to O's owner. No pending-request server.

### 3.1 Admission change (the load-bearing edit)

**Do not hand-roll chain verification.** willow25's validated capability decode (`is_valid()` — checks the owned genesis root signature, every delegation link's signature + strict area nesting) plus `does_authorise` (checks `includes(entry)` over namespace + subspace + path + **time_range**, and the receiver signature) already perform the full cryptographic chain check correctly. The Riot-side edit is **minimal**:

Today the admission gate rejects `capability.is_owned() || !capability.delegations().is_empty() || !namespace.is_communal()`. Replace with:

```
if namespace_id.is_owned():
    REQUIRE capability.is_owned()                 # INVARIANT — a communal genesis is
                                                  # unconditionally is_valid() and can name
                                                  # an owned namespace_id; without this,
                                                  # anyone forges masthead writes
    REQUIRE genesis.namespace_key == namespace_id == followed_site_root   # invariant 3
    then let verify_entry / does_authorise run    # willow25 validates the rest
else:  # communal
    existing communal rule, unchanged             # C, W
```

**`capability.is_owned()` is a stated invariant, not merely a test.** (Verified: `NamespaceId::is_owned()` is only the LSB marker bit and is not bound to the cap's genesis variant.)

**Enumerate the ENTIRE admission/inspection surface — this is one atomic change.** Grep for every gate that branches on `is_owned()` / `is_communal()`. Known gates:
- `crates/riot-core/src/import/bundle.rs:496` — `verify_frame` (the single policy chokepoint; edit the policy **here only**).
- `crates/riot-core/src/session.rs:658` — `into_authorised_entry` (routes through `decode_bundle`→`verify_frame`; add a test seam, do **not** duplicate the policy).
- `crates/riot-core/src/sync/state.rs:277` — namespace-id equality (routes through the same; test seam).
- `crates/riot-core/src/newswire/entry.rs:326` — **4th gate**, newswire inspection/projection path, rejects owned caps. **CORRECTION (as implemented in Unit 1 / PR #14): editorial articles do NOT hit this gate.** It is behind `is_newswire_prefix` (path `["newswire","v1",…]`); editorial articles are `["articles",<section>,…]` — disjoint. It was **left refusing owned caps** (relaxing it would admit owned *newswire* records, which §2 forbids); #14 pins that gate 1 and gate 4 agree in refusing an owned newswire record with a test, rather than unifying them.
- FFI **alert/non-alert classification** splits records in **two** places (`mobile_state.rs`: `inspectable_entries` + `list_current_entries`); new owned record families (article, manifest, revoke, tombstone) must be added to **both** or bundles reject / the board bricks (prior art: newswire 0B).

A **cross-gate consistency test** asserts an owned-namespace editorial entry admitted at `verify_frame` is also accepted/classified at every other gate.

~~**Retire the string-roster.**~~ **CORRECTION (Unit 1 / #14 did NOT do this — category error):** `editorial_roster` (`newswire_ffi.rs:43`) is the *communal newswire* founding roster — a different namespace and write model than owned-namespace editorial delegation, with which it coexists (§2). The owned cryptographic cap check governs owned articles, not communal newswire membership; it does not replace this field (also a `uniffi::Record` field → native-rebuild trap). Left unchanged.

---

## 4. Moderation (dual mechanism)

Moderation records live at `O:/mod/` (owned; only owner + explicitly delegated moderators write them — moderator caps are `/mod/`-scoped and **cannot target `/manifest` or the root**). Owner-signed record types, **two ban targets** plus a **freshness heartbeat**:

- **Ban a PERSON** — `revoke { author_key, effective_ts }`
- **Ban CONTENT** — `tombstone { target_ns, target_entry }`
- **Freshness heartbeat** — `mod_epoch { seq: monotonic u64, ts, mod_set_digest }` (owner-signed). The owner advances `seq` on a schedule and on every revoke/tombstone; `mod_set_digest` **commits to the set of revoke+tombstone record ids ≤ seq** (a rolling hash / small Merkle root). A client's `/mod/` is **current** iff it holds a heartbeat whose `ts` is within the freshness window, no `seq` gap is visible, AND it holds every record named by `mod_set_digest`; otherwise ⇒ `moderation-loading` (open namespaces held), never falsely "current." The digest is what defeats **tail-suppression**: a lone provider that serves a contiguous fresh prefix but withholds the *latest* revoke presents no `seq` gap, yet the heartbeat's `mod_set_digest` names a record the client lacks → detected. This gives "moderation-current" a **concrete, testable, non-gameable definition** (resolves §10 Q3). New owned record family ⇒ register in both `mobile_state.rs` classification sites (§3.1).

### 4.1 Honest scope (round-1 gate correction)

- **Person-ban only bites capability-holders** (editors in O). In **C / W**, authors are deliberately unlinkable, disposable self-minted subspaces — a flooder mints a fresh key per post, so `revoke{author_key}` is **inert** there. The real lever for C/W is **content-tombstone** (per-entry, whack-a-mole). **Automated anti-flood is parked** (§9 risk). The design does not claim to solve anonymous flooding in v1.
- **Root is exempt from revocation.** Render hard-ignores any `revoke{author_key == manifest.root}`; a rogue delegated moderator cannot brick the site by revoking the owner, and cannot tombstone `/manifest`. Owner records take precedence over moderator records on conflict.

### 4.2 Two-tier enforcement (belt + suspenders)

**Tier 1 — admission, best-effort (shrinks the leak):**
- Cap **expiry** (`time_range`) — self-enforcing, airtight for routine offboarding.
- **Timestamp-monotonic** reject (entry ts < max-seen-for-author). Stops *lazy* backdating; **not airtight** — an attacker controlling delivery order can seed a backdated entry to a fresh peer first. Accepted limitation.
- Deny-list check against `/mod/` revocations where synced.

**Tier 2 — render, the guarantee (closes the leak):**
- **Hide by author-key** (cap-holders): any O entry whose cap-receiver ∈ revoked keys → rendered as a **`Tombstoned`/`Hidden` placeholder row** (content nulled, identity + ordering + freshness preserved — **not vanished**, per Riot's accountable-degradation convention).
- **Hide by entry-id:** tombstoned entries → same placeholder treatment.
- Applied as a filter over the whole composite every render.

### 4.3 The guarantee, stated honestly

A banned person or post may still exist in *some peer's store* — you cannot delete bytes from others' disks, nor force global sync order. But **on any honest client whose `/mod/` is current (heartbeat within window, no `seq` gap), it is invisible.** The guarantee is scoped to honest clients with current moderation; a forked build that strips the filter is outside the trust model.

**"Moderation current" is a positive, signed freshness signal, not an absence** (the `mod_epoch` heartbeat, §4). Willow reconcile gives no completeness guarantee, so the fail-safe cannot rest on "haven't seen a revoke." The heartbeat makes staleness *detectable*: the resolved view model distinguishes *moderation-loading* (no current heartbeat / `seq` gap) from *moderation-current-and-empty* (current heartbeat, no bans).

**Withholding-provider bound (round-2/3 security).** A dishonest/coerced provider (the §9.4 gateway-seed focal point) cannot *forge* bans (that needs caps) — it can only try to *suppress* the latest revoke/tombstone to un-hide content. Suppression is **detectable**: a middle gap opens a `seq` gap, and *tail*-suppression (withholding the latest record while serving a fresh prefix) mismatches the heartbeat's `mod_set_digest` (which names records the client then lacks) ⇒ `moderation-loading`, never a false "current." The reseed mesh routes around a single withholder; **full suppression requires an eclipse** of every provider. Honest scope of the guarantee: *completeness ultimately rests on reconcile reaching at least one uncensored provider (Willow set-reconcile); the heartbeat makes a withholding provider detectable rather than silent; a total eclipse can still suppress moderation and is a documented residual (§9.5).*

**Timestamp reality:** Willow enforces last-writer-wins only at the same coordinate; there is **no global per-author monotonic clock**, so a determined revoked editor can backdate a new article within its cap window. This is why the *guarantee* rests on **identity at render**, not the clock at admission.

**Keep pre-ban good work:** owner may maintain an explicit **re-endorse allow-list**. Expiry vs revoke: a lapsed cap does **not** retroactively hide already-accepted in-window articles (they stay rendered); revoke + allow-list is the lever for hiding past work selectively.

---

## 5. Transport (policy-driven)

Transport is a `FrameChannel` family; every channel carries the same `SyncFrame` bytes through the existing `ByteSyncSession` bridge; the transport-agnostic reconcile FSM (`sync/state.rs`, explicitly "owns no transport") never learns which channel it's on.

```
FrameChannel family:
  loopback · BLE · LAN     existing — offline / in-person
  iroh                     v1 — internet, fast, NO metadata privacy
  arti / Tor               parked (policy-ready) — onion, IP-hiding
  nym                      research — not v1/v2
```

**Placement:** the concrete iroh transport + its tokio runtime live in **`riot-ffi` (or a dedicated transport crate), NOT `riot-core`** — the reconcile core stays transport-agnostic and headless-testable. Only a thin adapter feeds `SyncFrame` bytes: `iroh Connection ⇄ ByteSyncSession ⇄ ReconcileSession`.

### 5.1 Ticket carries a ROOT-SIGNED transport floor (bootstrap fix)

The `require` floor is carried in the ticket **and signed by the site root key**, so the client can authenticate the floor **before opening any connection**. A bare digest is insufficient — the existing `NewswireShareReference.content_digest` (`newswire/share.rs`) is an *unsigned* digest checkable only *after* fetching, i.e. post-connection; relying on it would let an attacker strip `require:arti`→`none` (keeping root+digest) and leak a first-time follower's IP over iroh before the real floor is ever read (round-2 security blocker). The ticket therefore carries an explicit signature:

```
riot://site/v1/<O-namespace>?root=<owner-key>&require=<none|arti>&epoch=<u64>&exp=<ts>&digest=<content_digest>&node=<NodeAddr>&sig=<root-sig>
sig = root-key signature over canonical(root, O-namespace, require, epoch, exp, digest)
```
- **The client MUST verify `sig` against `root` BEFORE opening any `FrameChannel`.** Unverifiable signature ⇒ `transport-blocked`, no dial. This makes `require` the authoritative pre-connection floor — a `require:arti` site is never dialed over iroh even for the first manifest fetch.
- **Freshness (defeats stale-signed-ticket replay).** `epoch` is monotonic per site; the owner bumps it whenever it tightens `require`. The client **refuses any ticket whose `epoch` < its durable per-site floor** (§5.2) — protecting returning followers absolutely — and **rejects an expired ticket** (`exp` past), bounding a first-time follower's replay exposure to the TTL window. A short TTL is recommended for `require:arti` sites, longer for public, surfaced at creation.
- **Unknown / future `require` token ⇒ fail closed** (`transport-blocked`), never parsed as `none`.
- `digest` additionally binds the manifest for post-connection substitution detection (now *inside* the signed payload).
- `node` is an **untrusted seeding hint**, never site identity; fail-closed is decided *before* `node` is used, so an attacker-chosen `node` on a `require:arti` site cannot induce a dial.
- **Residuals (documented, §9.5):** (a) the signature defeats *downgrade-in-place* (real root kept, floor flipped); (b) the epoch/`exp` defeats *stale-replay* against returning followers absolutely and bounds first-time followers to the TTL; (c) it cannot defeat *whole-ticket substitution* (attacker mints an entirely different `root`+`require`+`sig` → victim follows a *different* site) — inherent to out-of-band distribution (TOFU), narrowed by `digest` + out-of-band root confirmation. **Escalating a site's `require` cannot retroactively revoke already-distributed lower-floor tickets without key rotation** — this ties to the parked successor-key slice (§9.3); until then, old tickets remain a bearer-downgrade vector within their TTL.

### 5.2 Anti-rollback (manifest downgrade fix)

The client persists, in the durable profile, the **highest manifest `version` seen per site**, and **refuses any lower version** (Willow LWW only protects same-coordinate writes, so this floor is Riot-side). Two conflicting owner signatures at the same version = **equivocation alarm surfaced to the user** (itself a compromise signal), never a silent pick. `require` may never be lowered below the durably-seen floor **or** the ticket floor, whichever is stricter.

### 5.3 FAIL CLOSED (critical security property)

If a site's floor names a transport the client cannot provide, the client **refuses to connect and shows a resolved warning state** (§6) — it MUST NEVER silently fall back to iroh and leak the follower's IP. **v1 consequence (chosen):** `require:arti` sites are **creatable but unfollowable** until the arti channel lands; every follower correctly fails closed with a clear "this site requires Tor, unavailable in this version" state. No false privacy.

### 5.4 iroh channel (v1)

- Node keypair stored in the durable profile; **transport identity ≠ content identity** (NodeId never reveals authorship). **Follower NodeId is ephemeral** (rotates; only seeds need stable identity) to reduce cross-session linkability.
- NodeId→address via iroh discovery (pkarr/DNS/DHT).
- **Seeding:** the gateway (`apps/gateway/`) runs an always-on iroh node seeding the site's namespaces; any follower who holds data reseeds others (Willow set-reconcile). Origin offline ≠ site dead while ≥1 provider is reachable. (Centralization/liability tradeoff — §9.)
- **panic=unwind interaction:** detached async iroh tasks are **not** on the FFI call stack; the adapter must catch task panics and route them to session quarantine, and must **never hold the arbiter mutex across an `await`** (would deadlock/serialize the FSM). New FFI handles follow the handle+arbiter pattern (ID + `Arc<Mutex<SessionState>>`, re-acquire per method).

### 5.5 iroh privacy properties (documented, not solved)

iroh gives **confidentiality (encrypted content), not anonymity.** Relays + the always-on gateway seed learn follower IP ↔ NodeId + timing; ephemeral NodeId does **not** hide IP from the seed. For public sites the residual harm is the **follow-graph** (following == exposing your IP to a seed). Honestly stated; metadata privacy is the parked **arti** channel's job.

---

## 6. Composite render + resolved view-model contract

Composition/overlay logic lives in **Rust core** (`riot-core`, headless-testable), exposed via FFI as a **resolved view model** (like `NewswireProjectionView`); native apps render it with **no business logic** (shared-core rule). The contract MUST enumerate (so iOS + Android don't diverge):

- **Per-item trust-tier tag, resolved by core** — `editorial | open-wire | comment`. Core owns "W never masquerades as editorial"; the shells only style what core tagged. (Today there is no first-class trust flag — this is new.)
- **Moderation treatment per item** — reuse `NewswirePostTreatment` (`Visible | Hidden | Tombstoned`); moderated rows stay as accountable placeholders, never dropped.
- **Composite degradation enum** (named, plain-language, honest — matching `CommunityUnavailable` / `pendingFirstSync` convention). At minimum:
  - `moderation-loading` — `/mod/` not yet current; open namespaces held (not blank) with "posts appear once moderation syncs."
  - `editorial-only` — O synced, C/W pending ("comments loading").
  - `transport-blocked` — required transport unavailable ("this site requires Tor, unavailable in this version").
  - `manifest-invalid` / `manifest-rollback-alarm` — bad or downgraded manifest.
  - `member-unverified` — a member dropped for rule/key-structure mismatch (§2.2 inv 1); shown as "this section couldn't be verified," never a silent disappearance.
- **Transport status field** carrying the fail-closed reason.
- **Writer-side state (critical):** an editor whose time-boxed cap has **expired** must be warned **at compose** ("your editorial access expired on <date>"), and a local-but-peer-rejected entry must surface as **failed/pending**, never as "published." Silent write-rejection is the worst publishing UX.

**Render pipeline:**
```
1. From ticket: know site root + transport floor. Fail closed if floor unmet (§5.3).
2. Sync O → read /manifest (owner-signed, owned-zero-deleg; version ≥ durable floor, §5.2).
3. Validate members: key-structure must match declared rule, else member-unverified.
4. Sync C, W per transport_policy.  (O carries /mod/, so moderation arrives with editorial.)
5. Build view, core-resolving trust-tier tags:
     O:/articles → editorial (cap-chain verified at admission)
     C           → comment (open; author = subspace, unlinkable)
     W           → open-wire (open publishing)
6. Apply moderation overlay from O:/mod/ once moderation-current (freshness, §4.3):
     revoked cap-holder entries → Tombstoned/Hidden placeholder
     tombstoned ids            → Tombstoned/Hidden placeholder
     root exempt; owner-precedence; re-endorse allow-list honored
7. Resolve soft links (comment → article parent) at render; tolerate dangling (collapse).
```
- **Soft links** across namespaces resolved at render only (C is open — no admission-time referential integrity); dangling ref → graceful collapse.

---

## 7. Owner-side capability plumbing (was unbuilt — round-1 gate)

`crates/riot-core/src/willow/owned.rs` is currently **generation-only** (mints the namespace; its comments defer the owned write-capability, owned author, and sealed owned-root envelope to "later tasks"). Nothing yet **mints** an owned write cap, **issues** section-scoped delegations, or **signs** records with the owned-root secret. Units 2/3/6 cannot reach green without this, so it is an explicit unit (Unit 0), ordered first:

- Mint the owner's owned `WriteCapability` (`new_owned`) + persist the O keypair in the durable profile.
- Issue section-scoped, time-boxed delegations (`try_delegate`) — the editorial-invite artifact.
- Sign manifest / revoke / tombstone / article records with the owned root or a delegated cap.
- FFI surface for all of the above (handle+arbiter pattern).

---

## 8. Work units (decomposition)

| # | Unit | Touches | Depends |
|---|---|---|---|
| **0** | Owner-side owned-cap minting + delegation issuance (hard-refuse any cap whose Area escapes `/articles/`) + owned-root signing + FFI; **root secret at-rest protection (keychain/keystore-backed, never plaintext SQLite)** | `willow/owned.rs`, `riot-core` new, `riot-ffi` | — |
| **1** | Owned-namespace admission: `is_owned()` invariant + root binding; relax only `is_owned`/`delegations` in the compound gates (preserve namespace/receiver/includes); confirm `receiver()` = final delegatee; enumerate & unify ALL gates + FFI classification; cross-gate consistency test (both accept AND reject directions) | `import/bundle.rs:496` (policy), `session.rs:658`, `sync/state.rs:277`, `newswire/entry.rs:326`, `mobile_state.rs` (×2) | 0 |
| **2** | Site manifest record (reserved `/manifest`): schema, sign, **validate independently of admission — require an owned zero-delegation cap whose receiver == root (NOT assumed by area-scoping; a broad `Area::full` cap would pass admission)**, invariants 1–3, durable version floor | `riot-core` new module, FFI, durable profile | 0,1 |
| **3** | Moderation: `/mod/` revoke + tombstone + **`mod_epoch` heartbeat** records + admission best-effort + moderator-cap containment + signed moderation-current freshness signal | `riot-core` moderation module, `sync/state.rs` | 0,1,2 |
| **4** | Composite resolver + resolved view-model contract (§6): trust-tier tags, treatment, degradation enum, transport-status; consumes Unit 3's overlay data | `riot-core` render module, FFI view model | 2,3 |
| **5** | iroh `FrameChannel` adapter (in riot-ffi/transport crate) + **root-signed ticket floor (sig verified pre-dial)** + fail-closed + ephemeral NodeId + gateway seed | `riot-ffi`/transport, share-ref, `apps/gateway/` | 2 |
| **6** | Native UI: follow-site (**incl. iOS follow/join view — close the Android-only share-join asymmetry**), composite surface, trust-tier styling, degradation/transport states (**designed copy + next-step for alarm/rollback states**), editor-invite handshake, **QR gen + camera scan (iOS + Android)**, writer expired-cap warning, **mandatory seizure disclosure + compose-time "unfollowable (require:arti)" notice** | iOS + Android | 4,5 |

Unit 1 is the critical-path security unlock; Unit 0 unblocks everything.

### 8.1 Testing (TDD — per-unit RED cases)

TDD is mandatory (CLAUDE.md). Coverage honors the **`.coverage-thresholds.json` ratchet floor** (not 100% — that was fiction; the Rust line floor is CI-enforced via `tarpaulin --fail-under`). Each unit enters TDD with its RED cases enumerated:

- **Unit 0:** mint owned cap; issue `/articles/`-scoped delegation; **issuance refuses an over-broad Area (escapes `/articles/`)**; sign/verify round-trip; root secret is keystore-backed, not plaintext.
- **Unit 1 (adversarial, security core):** forged delegation chain; over-broad area; expired cap; wrong root; **communal-cap-naming-an-owned-namespace** (the marker-bit forgery); cross-namespace cap reuse; delegation loops; **cross-gate consistency in BOTH directions** — a valid owned editorial entry is accepted/classified identically at every gate, AND a forgery is rejected identically at every gate (no gate stricter than another).
- **Unit 2:** manifest `root != owner` reject; **delegated (non-zero-delegation) cap on `/manifest` reject** (independent of admission); rule/key-structure mismatch → member-unverified; version-rollback reject (durable floor); same-version equivocation → alarm; unsigned reject.
- **Unit 3:** revoke hides cap-holder **even with backdated timestamp** (identity-guarantee); tombstone hides entry; **heartbeat `seq` gap ⇒ moderation-loading**; **tail-suppression (fresh prefix, withheld latest revoke) ⇒ `mod_set_digest` mismatch ⇒ moderation-loading**; root exempt from revoke; moderator cap cannot write `/manifest`; re-endorse allow-list survives.
- **Unit 4:** partial-sync degradation states; dangling soft-link collapse; trust-tier separation (W never tagged editorial); moderation-loading-timeout fallback (no permanent spinner on a slow provider).
- **Unit 5:** **ticket-downgrade test** — `require` stripped/flipped but root intact ⇒ signature verification fails ⇒ fail closed, **no iroh dial**; **stale-replay (returning follower)** — old `epoch` < durable floor ⇒ refuse, no dial; **expired ticket** (`exp` past) ⇒ refuse; **unknown `require` token ⇒ fail closed** (never `none`); attacker-chosen `node` on `require:arti` ⇒ no dial; ephemeral NodeId rotation; loopback + iroh carry identical frames.
- **Unit 6:** editor-invite two-way handshake; QR round-trip both platforms; writer expired-cap warning at compose; mandatory seizure disclosure present at creation.

**Per-unit UniFFI gate (Units 2/3/4/6):** every new `uniffi::Record`/`Enum` requires regenerating the binding AND rebuilding the native staticlib **in the same commit** — the failure mode is a runtime checksum abort in the apps, not a compile error (documented recurring defect). A smoke test loads the FFI on iOS + Android.

---

## 9. Risks

1. **Admission-core edit (Unit 1).** The copy-on-write preview boundary is load-bearing; a bug admits forgeries or corrupts state. *Verified:* admission verification runs on the verify side, outside state mutation (`session.rs:632`), so the CoW boundary is unchanged — but the `is_owned()` invariant and the multi-gate enumeration are the failure modes. Mitigation: adversarial tests first (forged chain, over-broad area, expired cap, wrong root, **communal-cap-in-owned-namespace**, cross-namespace cap reuse, delegation loops), isolated unit, cross-gate consistency test.
2. **Anonymous flooding of C / W.** The historical indymedia killer. v1 has **no automated anti-flood** — person-ban is inert against rotating communal keys; only per-entry tombstone. Explicitly parked; owner-tombstone is the v1 lever. Automated anti-flood (rate-limit / PoW / accumulation) is a future slice.
3. **Owner-key loss / compromise / seizure (single root).** v1 = single root key, **no rotation or recovery**: key loss → site unpublishable; device seizure → attacker can publish as the site, revoke real editors, tombstone real reporting. Accepted v1 limitation; successor-keys / rotation / threshold-multisig is a named future slice. **MANDATORY user-facing disclosure (Unit 6 acceptance item):** at site creation, a required in-app string must spell out that device seizure = full site takeover — the captor can *impersonate the site and revoke the real editors* — not merely "key loss." An activist must be able to make an informed threat decision before minting a masthead on a phone.
4. **Gateway-seed centralization/liability.** An always-on seed is a subpoena/takedown/metadata focal point. Deliberate tradeoff: follower-reseed means origin-offline ≠ site-dead, but the seed observes follow-graph metadata. State plainly.
5. **Ticket / manifest downgrade + residuals.** *Downgrade-in-place* (strip `require:arti`→`none`, keep root) is **closed** by the root-signed ticket (§5.1). *Stale-signed-ticket replay* (a genuinely root-signed OLD lower-floor ticket) is **closed for returning followers** by the durable epoch floor and **bounded to the TTL** for first-time followers by signed `epoch`+`exp` (§5.1). *Manifest rollback / equivocation* is mitigated by the durable version floor + equivocation alarm (§5.2). Documented residuals: (a) **first-time-follower replay within TTL** — until a site's `require` escalation propagates and old tickets expire, a first-time follower can be handed a still-valid old lower-floor ticket; retroactive revocation of old tickets needs **key rotation** (parked, §9.3). (b) **whole-ticket substitution / TOFU** — attacker mints an entirely different `root`+`require`+`sig`, so a first-time follower follows a *different* site; inherent to out-of-band distribution, narrowed by `digest` + out-of-band root confirmation. (c) **moderation eclipse** — a total eclipse of every provider can suppress the latest bans; a single withholder (middle or tail) is detected via `seq` gap / `mod_set_digest` mismatch and routed around by the reseed mesh (§4.3).
6. **arti maturity.** Parked; onion-service hosting may be immature on mobile. De-risked by fail-closed (no false privacy meanwhile) + declarable-but-unfollowable sites.
7. **Timestamp backdating.** Unsolved at admission by design; render-identity guarantee is the backstop. Documented.
8. **Member-namespace reframing (residual).** A manifest can reference a victim's communal namespace as its "comments," reframing it under this masthead (display-only; owner-signed). v1 accepts this as display-only; member opt-in cross-attestation is a future option.
9. **Namespace sprawl / sync cost.** 3 namespaces × N sites. Lifecycle: unfollow drops members; monitor sync-state growth.
10. **Universal-model scope-creep.** Schema is generic but v1 migrates nothing and serializes only the site shape; resist redefining `community` now.

---

## 10. Open questions (post round-2)

- **RESOLVED (round 1):** single-owned-masthead collapse — Architect + Security verified path-prefix containment enforces it; adopted.
- **RESOLVED (round 2):** moderation-current freshness — the signed `mod_epoch` heartbeat (§4) gives a concrete, testable, non-gameable definition (last heartbeat within window + no `seq` gap) and detects withholding. Remaining residual is total eclipse (§9.5).
- Is `is_owned()` (marker bit) + `capability.is_owned()` + root-binding a sufficient basis for the rule-intrinsic invariant, or is a canonical namespace-type tag still wanted for defense-in-depth? (Design: sufficient; tag deferred.)
- Editor-invite two-way handshake assumes the invitee can round-trip a key (co-present QR/paste). Is an **async / remote pseudonymous editor invite** an in-scope use case, or is co-presence an accepted v1 constraint? (Design: co-presence assumed for v1; async invite is a follow-on.)
- Is "narrow now / universal-ready / migrate nothing" holding, given the manifest's open `role`/`rule` enums — v1 only *serializes* the site shape even though the enum space is open.
