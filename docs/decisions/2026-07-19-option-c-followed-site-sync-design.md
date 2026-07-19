# Option C: Automatic Followed-Site Sync — Design + Implementation Plan

*Written 2026-07-19 (read-only design, no product code changed). Decision doc for rabble.*
*Companion to `docs/research/2026-07-19-owned-namespace-propagation-design.md` (the scoping) and
Option B (`import_followed_site_bundle`, the manual-import stopgap).*

File:line citations reference the current composite-site code state (the `overnight/2026-07-19`
worktree, which is where `site_ffi.rs` carries the moderation read+write paths — 1551 lines). Paths
are logical (`crates/...`); they resolve on whichever branch carries the moderation work.

---

## 1. Problem recap + what Option B already gives us

Composite-site **owned-namespace** content — `O:/mod/` moderation records and `O:/articles/`
editorial — never reaches followers automatically. The gap is on **both** sides:

- **Owner has no offer.** The per-community sync inventory is pruned to the *active-community*
  namespace and enforces exact equality against it (`mobile_state.rs:1740` `install_sync_inventory`,
  the guard at `:1757`). An owned site is never an active community, so `O:/mod/` entries are never in
  any inventory the owner offers. `import_owned_mod` commits the owner's `/mod/` record locally but
  **explicitly does not add it to `sync_inventory`** (`site_ffi.rs:790-794`), returning the signed
  bytes for out-of-band propagation instead.
- **Follower has no automatic importer.** The sync session is single-namespace and hardwires the
  admission root to the *active community* (`prepare_sync_import`, `mobile_state.rs:1305`
  `followed_root = active namespace`). The code itself flags that owner `/mod/` + `/manifest` can
  only enter via the crate-internal `inspect_core_with_root` path — "there is no public single-shot
  site import, only the full sync session" (`site_ffi.rs:836-842`).

**What Option B already ships** (the stopgap): an explicit, out-of-band `import_followed_site_bundle`
FFI. The owner shares signed bytes (already returned by `create_site_moderation_action` →
`SiteModerationOutcome { action, epoch }`, `site_ffi.rs:638-641`); the follower imports them via a new
FFI that (a) requires an existing `Relationship::Following` record for the root, (b) calls
`inspect_core_with_root(&store, bytes, route, Some(root))`, (c) family-gates every eligible entry to
owned `/mod`,`/articles`,`/manifest`, (d) commits **without touching `sync_inventory`**. Option B
proves the admission core end-to-end. It is manual (someone must carry the bytes).

**Option C is scoped to exactly one thing Option B does not do: AUTOMATIC delivery.** The owner serves
`O` over a live transport; the follower pulls it on its own, no manual byte-passing. Everything about
*admission* (the `inspect_core_with_root(Some(root))` + Following-record + family-gate discipline) is
reused verbatim from B. C changes only how the bytes *travel*: a dedicated followed-site sync session.

---

## 2. The two options

Both options share one primitive: **a per-namespace inventory built from that namespace's live ids.**
Today only one exists (`active_namespace_live_ids`, `mobile_state.rs:1712`, which prefix-queries
`entries_with_prefix_in_namespace` at `:1733`). Both C-variants generalize it to any namespace `ns`.
They differ only in **where the followed-site offer set lives**.

### Option C1 — Separate per-followed-site inventory (a distinct offer set per site)

The community inventory (`profile.sync_inventory`, `mobile_state.rs:77`) is left **byte-for-byte
untouched**, including its guard. A followed-site sync opens a **second** `ByteSyncSession` keyed on the
site namespace `O`, whose offer set is built from `O`'s live ids — and, critically, **is never stored in
`profile.sync_inventory`**. Two sub-variants of *where* the O offer lives:

- **C1a (recommended): build the offer just-in-time.** On `open_followed_site_sync_session(root)`,
  compute `namespace_live_ids(profile, root)` fresh, encode, hand to `ByteSyncSession::new(root, …)`.
  **No persistent second unstamped set exists at all.** The community-inventory field is the only
  stored offer set, and it still equals exactly the active community's live ids.
- **C1b: store a second field** `followed_site_inventory: Map<[u8;32], Vec<SignedWillowEntry>>`. Adds a
  second unstamped stored set — the thing the isolation guard exists to protect against. Strictly worse
  than C1a for no benefit (the offer is cheap to rebuild; sync is not hot-loop). **Reject C1b.**

### Option C2 — Generalized multi-namespace inventory (replace the single field)

Replace `sync_inventory: Vec<SignedWillowEntry>` with `inventory: Map<[u8;32], Vec<SignedWillowEntry>>`
keyed by namespace, and rework `install_sync_inventory` / `ensure_complete_sync_inventory` /
`active_namespace_live_ids` / `track_committed_entry` / `prospective_sync_inventory` to operate
per-key. `open_sync_session` selects the active-community key; a followed-site session selects the `O`
key. Sessions stay single-namespace (each picks one key).

### Analysis against the four criteria

| Criterion | C1a (JIT per-site) | C2 (multi-ns map) |
|---|---|---|
| **Isolation invariant** (the exact-equality unstamped guard, `mobile_state.rs:1757`) | **Best.** Community field + guard are *unchanged*. No second stored set exists, so nothing can be offered to the wrong peer even under a switch. The O offer is derived by a namespace-prefix query (`:1733`) → cannot physically contain another ns's ids; `ByteSyncSession::new` re-validates every entry's ns == root (`sync/state.rs:57`) as a second gate. | **Worst.** The single guarded field becomes a map; the exact-equality check must be re-proven per-key, and the "unstamped set, equality is the sole guard" reasoning (`:1752-1756`) now spans N keys. Every touch of the map is a new place the guard can be weakened to a subset check. |
| **Complexity + blast radius** | **Small.** One extracted helper (`namespace_live_ids(ns)`), one new session-open path, one new session slot. `install_sync_inventory` and the community path are not touched. | **Large.** Rewrites 5+ inventory functions and the `sync_inventory` field's type; every caller of the field changes; `track_committed_entry` must route by the committed entry's namespace. High regression surface on the most safety-critical code in the FFI. |
| **Single-namespace `ByteSyncSession`** (`sync/state.rs:57,87` reject ns ≠ session ns) | **Natural fit.** A followed-site sync *is* a second single-namespace session keyed on `root == namespace` of the site. No change to the session type. | Also single-namespace per session (the map just picks a key), but the map tempts a future multi-ns session that the wire format forbids. |
| **Owner side (serve O to followers)** | **Natural fit.** "Owner follows own site." Serving O = the owner opens the same followed-site session as the *server*, offering `namespace_live_ids(O)` — the identical JIT primitive. Owner already commits `/mod/` locally (`import_owned_mod`). | Owner puts O in its map and serves that key — works, but carries the whole map-rewrite cost for a case C1a handles with the same one helper. |

**Recommendation: C1a — a separate, just-in-time followed-site inventory + a second single-namespace
sync session, one per followed site.**

Reasoning: it makes the followed-site offer a *derived* value scoped by construction to `O`, so the
load-bearing community-isolation guard survives **completely untouched** — the single most important
constraint in the task. It matches the `ByteSyncSession` single-namespace shape exactly (a followed-site
sync literally is a second single-namespace session). It has the smallest blast radius: the dangerous
inventory code is not modified, only *read from* through one new pure helper. And the owner-serve case
falls out of the same primitive ("owner follows own site"). C2's only advantage — a uniform structure —
buys nothing the wire format can use (sessions are single-namespace regardless) at the cost of rewriting
the exact code we most want to leave alone.

---

## 3. End-to-end flow (recommended C1a), traced to exists-vs-build

Legend: **[EXISTS]** already in tree · **[BUILD]** this design · **[EXISTS/other-branch]** on the
`riot-follow` / `riot-transport` follow branch, needs wiring.

1. **Owner authors + commits O content.** `create_site_moderation_action` → `import_owned_mod`
   commits `O:/mod/` + a mod-epoch heartbeat into the owner's store via
   `inspect_core_with_root(Some(root))` (`site_ffi.rs:650, 795-819`). **[EXISTS]**
2. **Owner serves O.** Owner opens a followed-site sync session as *server*, offering
   `namespace_live_ids(profile, O)` (the JIT primitive). Transport routes an inbound follower
   connection carrying `Ticket { root: O, namespace: O, … }` to this session. Session-open + the
   primitive: **[BUILD]**. Ticket parse/verify: **[EXISTS]** (`riot-transport/src/ticket.rs:78-89,
   :114 verify`). Transport routing by root: **[EXISTS/other-branch]** (`riot-follow` syncs one
   namespace given a ticket).
3. **Follower follows.** `follow_site(ticket)` verifies the ticket, persists a `Relationship::Following`
   registry record for `root` (today only the `#[cfg(test)]` seam `follow_site_for_test`,
   `mobile_state.rs:2374`), and records the transport node hint. Production entry point + verify wiring:
   **[BUILD]**. Following data model + `following` accessor filtered out of `list_communities`
   (`mobile_state.rs:2314,2360`): **[EXISTS]**.
4. **Follower opens a followed-site sync session** keyed on `root`, in a **new session slot distinct
   from the community `sync_session`** (`mobile_state.rs:77`). Guard: refuse unless a `Following` record
   for `root` exists (mirrors Option B). Build the *empty-or-current* offer from
   `namespace_live_ids(profile, root)` (the follower may already hold some O entries). **[BUILD]**.
5. **Sync exchanges frames.** `ByteSyncSession`/`ReconcileSession` drive the summary/request/entries
   protocol, single-namespace, rejecting any frame whose ns ≠ `root` (`sync/state.rs:57,87`).
   **[EXISTS]**.
6. **Owned admission on import.** When the session yields `ImportBundle`, the followed-site import path
   calls `inspect_core_with_root(&store, bytes, "site-follow-sync", Some(root))` — `root` here is the
   **site**, not the active community — and **family-gates** eligible entries to owned
   `/mod`,`/articles`,`/manifest`, rejecting anything else (a communal/alert entry, or an entry in a
   third namespace). This is the same admission Option B proved; only the *root wiring differs from the
   community path's `:1305`*. **[BUILD]** (new import path) reusing **[EXISTS]** admission core
   (`session.rs:171,186` `ImportContext::with_followed_root`; `import/bundle.rs:289,484,510` cap-rooted
   admission; `site/validate.rs:148,200` manifest `root == followed_site_root`).
7. **Resolve.** With `O:/mod/` + epoch now in the follower's store, `resolve_composite_site`
   (`site_ffi.rs:436`) recomputes freshness and advances off `SiteDegradation::ModerationLoading`
   (`:342, :1150`). **[EXISTS]**.

**Everything net-new is in steps 2, 4, 6 (and the `follow_site` production entry in 3).** The admission
brain, the sync protocol, the resolver, and the Following data model already exist.

---

## 4. Security analysis

### 4.1 How the followed-site offer stays isolated from the community inventory (invariant survives)

The invariant to preserve: the community offer set `profile.sync_inventory` is **unstamped** and
one store holds every held community's entries; the exact equality `inventory_ids ==
active_namespace_live_ids` (`mobile_state.rs:1757`, also enforced at session-open in
`ensure_complete_sync_inventory:1801`) is the **sole** guard between community A's entries and community
B's peers after a switch (`:1752-1756`).

C1a preserves it by **construction, not by adding a parallel guard**:

1. **The community field and its guard are never modified.** No work unit edits
   `install_sync_inventory`, `ensure_complete_sync_inventory`, `active_namespace_live_ids`, or the
   `sync_inventory` field. Its behavior is unchanged, so its proof is unchanged.
2. **The followed-site offer is derived, never stored.** It is `namespace_live_ids(profile, O)` computed
   at session-open and dropped when the session closes. There is **no second persistent unstamped set**
   that a later community switch could accidentally offer to a community peer. (This is precisely why
   C1a beats C1b/C2.)
3. **The offer is namespace-scoped by the query itself.** `namespace_live_ids` prefix-queries
   `entries_with_prefix_in_namespace(&O, all_prefix)` (`:1733`) → it *cannot* return a community entry.
4. **`ByteSyncSession::new(O, offer)` re-validates.** Every entry is decoded and rejected if
   `namespace_id != O` (`sync/state.rs:57`) — a second, independent gate. So even a bug in (3) is caught.
5. **The two sessions never share the offer.** The community session is built from
   `profile.sync_inventory` for the active namespace (`open_sync_session:1210`); the followed-site
   session is built from the JIT O offer. Distinct `ByteSyncSession` instances, distinct namespaces.

### 4.2 New cross-contamination risks C introduces, and how each is closed

- **R1 — Import under the wrong root.** If the followed-site import reused the community path's
  `followed_root = active namespace` (`:1305`), an owner-session peer's `O:/mod/` would fail closed
  (good) OR a community entry could be admitted under O (bad). *Closed:* the followed-site import path
  sets `followed_root = root` (the site) explicitly and is a **separate function** from
  `prepare_sync_import` — no shared mutable root.
- **R2 — Injecting arbitrary entries via the followed-site session.** A hostile server could offer
  entries beyond owned editorial (an alert record, a profile card, a third namespace's entry). *Closed:*
  (a) `inspect_core_with_root(Some(root))` only admits entries authored under a cap rooted at `root`
  (`import/bundle.rs:565` `admissible_capability`); (b) an explicit **family gate** rejects any eligible
  entry outside owned `/mod`,`/articles`,`/manifest` (mirrors Option B, and mirrors the eligible-count
  equality check `prepare_sync_import` uses at `:1312`); (c) `sync/state.rs:57` rejects any entry ns ≠
  root.
- **R3 — Syncing an unfollowed namespace.** An attacker convinces the app to open a followed-site
  session for an arbitrary `O'` and admit its owned content. *Closed:* `open_followed_site_sync_session`
  **requires an existing `Relationship::Following` record** for `root` (mirrors Option B's central
  guard) and `follow_site` only creates that record after **ticket verify** (`ticket.rs:114`,
  root-signature + expiry + rollback floor).
- **R4 — Followed-site import polluting the community offer.** If the followed-site import triggered
  `track_committed_entry` → `install_sync_inventory` with O entries, pruning would drop them (a no-op)
  BUT the code path must not *assert* against them. *Closed:* the followed-site import path does **not**
  call the community inventory install — same discipline `import_owned_mod` already follows
  (`site_ffi.rs:790-794`, "NOT added to the per-community sync inventory").
- **R5 — Concurrency between the two sessions.** Today there is exactly one `sync_session` slot and
  ~10 `sync_session_is_active(profile)` guards gate other operations (`mobile_state.rs:1442`, and the
  scattered `if sync_session_is_active` checks around `:467,562,670,776,867`). A second session type must
  not confuse those guards or let a community switch silently offer the wrong set. *Closed by design:*
  the followed-site session lives in a **separate slot with its own generation/follow guard**; the
  community `sync_session_is_active` predicate is unchanged; a community switch invalidates the community
  session as today and, per WU5, either invalidates or leaves independent the followed-site session
  (open question Q2). Because the O offer is JIT and namespace-scoped, a switch **cannot** cause a
  community peer to receive O entries regardless of session lifetime.
- **R6 — Freshness / silent staleness (not a leak, a liveness risk).** Moderation freshness degrades if
  the owner's heartbeat window lapses ("a forgotten heartbeat leaves followers at ModerationLoading
  forever", `site_ffi.rs:597,636`). C must define a re-sync trigger (Q4) so followers don't sit at
  `ModerationLoading` because no one initiated a pull.

**Net:** the isolation invariant is preserved by *not touching* the guarded code and by deriving the
followed-site offer as a namespace-scoped, unstored, double-validated value. Every new risk reduces to
"reuse Option B's already-proven admission gates on the sync-delivered bytes."

---

## 5. Implementation plan — ordered work units

Dependencies first, riskiest unknowns early. Each unit: scope · files · tests · adversarial cases.
"needs owner-side/transport" flags units touching `riot-transport`/`riot-follow` (the follow branch).

### WU1 — Per-namespace inventory primitive (core/ffi) · *riskiest invariant work, do first*
- **Scope.** Extract `namespace_live_ids(profile, ns: [u8;32])` from `active_namespace_live_ids`
  (`mobile_state.rs:1712`); add `build_followed_site_offer(profile, ns) -> Vec<SignedWillowEntry>` that
  applies the SAME size/encoding checks (`MAX_SYNC_IDS`, `MAX_SYNC_INVENTORY_BYTES`) and a
  **per-namespace** exact-equality check, but **returns** the offer (does NOT store it in
  `profile.sync_inventory`). Leave `active_namespace_live_ids` / `install_sync_inventory` behavior
  identical (may delegate to the new helper for the active ns).
- **Files.** `crates/riot-ffi/src/mobile_state.rs`.
- **Tests.** Offer for `ns=O` contains exactly O's live ids; community path (`ns = active`) unchanged
  (regression); size-limit paths return `SessionLimit`.
- **Adversarial.** Store holding community + owned entries: assert O offer excludes every community id
  and the community inventory excludes every O id; empty-O offer is valid (follower with no O yet).

### WU2 — Followed-site sync session + owned-admission import (core/ffi) · *depends WU1*
- **Scope.** New session slot `followed_site_session: Option<StoredFollowedSiteSession>` (root +
  its own generation/follow guard), distinct from `sync_session` (`mobile_state.rs:77`). Add
  `open_followed_site_sync_session(root)`: require a `Following` record for `root`
  (`following` accessor, `:2360`); build offer via WU1; `ByteSyncSession::new(root, offer)`. Add the
  import path `prepare_followed_site_import` (parallel to `prepare_sync_import:1283`) that sets
  `followed_root = root` and **family-gates** eligible entries to owned `/mod`,`/articles`,`/manifest`,
  rejecting all else; commit via `plan_all` → `commit` WITHOUT touching `sync_inventory`.
- **Files.** `crates/riot-ffi/src/mobile_state.rs`, `crates/riot-ffi/src/site_ffi.rs` (reuse
  `inspect_core_with_root`, `import_owned_mod` patterns).
- **Tests.** Follower with a `Following` record for `O` syncs an owner mod bundle →
  `resolve_composite_site` advances off `ModerationLoading` (`:1150`); `/articles/` editorial admitted
  the same way; community `sync_session` unaffected while the followed-site session is open.
- **Adversarial (R1–R4).** Open for an unfollowed root → reject (R3); followed-site frame carrying a
  communal/alert entry → reject, whole bundle rejected (R2); frame carrying a third-namespace entry →
  `NamespaceMismatch` (R2); assert the community offer is byte-identical before/after a followed-site
  import (R4).

### WU3 — Owner-side serve + follower connect (transport) · *depends WU1,WU2 · needs owner-side/transport*
- **Scope.** Owner: open a followed-site session as *server* for `O`, offering `build_followed_site_offer
  (profile, O)`; route an inbound `Ticket { root: O }` connection to it (transport). Follower: connect
  over the ticket's transport (node hint, floor) and drive the session to completion.
- **Files.** `crates/riot-transport/*`, `riot-follow` (follow branch), `crates/riot-ffi` glue.
- **Tests.** Two-profile end-to-end: owner `create_site_moderation_action` → follower connects + syncs →
  follower store has owner `/mod/` + epoch → `resolve` reaches `Current`.
- **Adversarial.** Ticket `root != served namespace` → reject; expired ticket / rollback epoch →
  `TransportBlocked` (`ticket.rs:63-64`); server offering a non-O entry → client rejects (R2).

### WU4 — `follow_site(ticket)` production entry + native surface (ffi/native) · *depends WU3*
- **Scope.** Replace the `#[cfg(test)]` `follow_site_for_test` seam (`mobile_state.rs:2374`) with a
  `#[uniffi::export] follow_site(ticket)` that verifies the ticket, persists the `Following` record +
  node hint, and (optionally) kicks an initial sync. Surface follow / list-following / unfollow +
  a "sync now" trigger to iOS/Android. UniFFI regen (new exports).
- **Files.** `crates/riot-ffi/src/mobile_state.rs`, `apps/ios/*`, `apps/android/*`, bindings.
- **Tests.** follow → `following` lists it → sync → unfollow drops the record AND forbids further
  followed-site sessions for that root (R3 stays closed after unfollow).
- **Adversarial.** follow with a bad-signature ticket → reject, no record written; unfollow mid-session.

### WU5 — Concurrency + generation hardening (ffi) · *depends WU2 (WU4 for native paths)*
- **Scope.** Define the interaction of the community `sync_session` and the followed-site session under a
  community switch, the ~10 `sync_session_is_active` guards (`:1442`), and store-full/session-limit. Add
  a followed-site generation/follow guard analogous to `community_generation` (`:1442-1454`). Decide
  Q2 (invalidate-on-switch vs independent lifetime).
- **Files.** `crates/riot-ffi/src/mobile_state.rs`.
- **Tests.** Community session + followed-site session open simultaneously; switch community; assert (a)
  no O entry ever offered to the community peer, (b) no community entry ever offered to the O peer, (c)
  the correct `sync_session_is_active` semantics for each operation guard.
- **Adversarial (R5).** Interleave frames from both sessions; switch mid-followed-site-sync; confirm the
  unstamped-offer leak is impossible because the O offer is JIT and namespace-scoped.

**Build-vs-reuse summary:** WU1–WU2, WU5 are pure core/ffi (no transport). WU3 needs the owner-side seed
and `riot-transport`/`riot-follow` wiring. WU4 needs FFI + native. The admission brain
(`inspect_core_with_root`, `ImportContext::with_followed_root`, manifest/cap validation), the sync
protocol (`ReconcileSession`/`ByteSyncSession`), the resolver (`resolve_composite_site`), the ticket
codec, and the Following data model are all **reused, not built**.

---

## 6. Open questions for rabble

1. **JIT vs stored offer (Q1).** Recommendation is C1a — build the O offer just-in-time, never store a
   second unstamped set. Confirm you accept the (small) recompute cost at each followed-site session open
   in exchange for the strongest isolation story. (If followed-site sync ever becomes hot, revisit — but
   sync is not a hot loop.)
2. **Concurrency model (Q2).** How many followed-site sessions may be open at once — one-at-a-time, or a
   bounded set (one per followed site)? And does a community switch invalidate open followed-site
   sessions, or do they run independently? Affects the slot design (single `Option` vs a small map) and
   native UX.
3. **Owner serving model (Q3).** Does an owner passively serve `O` to any follower whenever the app is
   online (a background responder), or only while actively "publishing"? The freshness-heartbeat window
   (`site_ffi.rs:597,636`) means followers must reach the owner periodically; who initiates and how often?
4. **Re-sync cadence / trigger (Q4).** On app open? On follow? On a push/transport hint? A stale mod-epoch
   strands followers at `ModerationLoading`; C needs an explicit re-sync policy, not just a first-sync.
5. **Transport reuse (Q5).** Does `follow_site(ticket)` reuse the community iroh transport path, or a
   dedicated followed-site channel? The ticket already carries an untrusted node hint (`ticket.rs:85-87`).
6. **`/articles/` scope (Q6).** Editorial `O:/articles/` rides the SAME followed-site session/namespace as
   `/mod/` (recommended — same namespace `O`, family gate admits both). Confirm editorial is in-scope for C
   now, or defer to a follow-up while C ships moderation delivery first.
