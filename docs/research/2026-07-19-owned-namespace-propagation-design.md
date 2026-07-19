# Design Scoping: Delivering Owned-Namespace Content to Followers

*Produced overnight 2026-07-19 (read-only scoping). Against `origin/main`. For rabble's morning review.*
*Companion to the merged moderation read (#54) + write (#58) paths, which are wired end-to-end EXCEPT delivery.*

## TL;DR

Composite-site owned-namespace content (`O:/mod/` moderation, `O:/articles/` editorial) does not
reach followers automatically. The gap is real on **both** sides:
- **Owner has no offer:** the per-community sync inventory is pruned to the *active-community*
  namespace, and an owned site is never an active community — so `O:/mod/` entries are never in any
  inventory to offer.
- **Follower has no importer:** the code itself flags "there is no public single-shot site import,
  only the full sync session" (`site_ffi.rs:836-843`), and the sync session hardwires the admission
  root to the *active community* namespace, not the followed site.

**Recommendation: Option B (explicit import-bundle FFI)** — smallest safe step, additive, reuses the
already-proven `inspect_core_with_root(Some(root))` admission, touches no inventory invariant.
**Option C (dedicated followed-site sync session)** is the correct architectural end state but is a
LARGE change needing a human design decision — NOT built.

## 1. Sync-inventory mechanics

`LocalProfile.sync_inventory: Vec<SignedWillowEntry>` = the entries a profile OFFERS to a peer.
- `install_sync_inventory` (`mobile_state.rs:1742`) retains only ids ∈ `active_namespace_live_ids`,
  then enforces **exact equality** `inventory_ids == live_ids` else `MobileError::Internal`.
- `active_namespace_live_ids` = all live entries in the **active community namespace only**.
- `track_committed_entry` reinstalls after adding a local entry — but pruning makes it a **no-op for
  owned-namespace entries** (documented at `site_ffi.rs:797-802`).
- **Isolation property (load-bearing, `mobile_state.rs:1755-1761`):** the inventory is UNSTAMPED;
  one store holds every held community's entries; the exact `inventory == active_namespace_live_ids`
  equality is the sole guard between community A's entries and community B's peers after a switch.
  The comment explicitly says "do not relax it to a subset/superset check."

## 2. How a follower "syncs a site" today

- Following exists only as a data model (Rung 1 / #59) and does NOT drive sync. A followed site is a
  `CommunityRecord { namespace_id = O, relationship = Following }`, filtered out of `list_communities`.
- `follow_site(ticket)` production entry point **does not exist yet** (only a `#[cfg(test)]` seam).
- The sync session is single-namespace, bound to the ACTIVE community: `open_sync_session`
  (`mobile_state.rs:1190`) offers `profile.sync_inventory` for `profile.space.namespace_id`;
  `ByteSyncSession` rejects any entry whose namespace ≠ the session namespace (`sync/state.rs:57`);
  import sets `followed_root = active namespace` (`mobile_state.rs:1305`).
- **So owned namespace O propagates not at all** — never active, never offered, never imported.

## 3. Existing plumbing for owned admission (already built)

- `ImportContext.followed_site_root` is "the ONLY carrier for owned-namespace admission"; `None` fails
  closed; set via `with_followed_root` (`session.rs:160-189`). The import loop admits
  `is_owned_moderation_entry`/`is_owned_editorial_entry` only under a cap rooted at that key.
- Manifest invariant 3: `validate_site_manifest(signed, followed_site_root)` requires
  `manifest.root == followed_site_root` (`validate.rs:146-199`).
- Transport `Ticket { root, namespace, … }` already separates root from namespace
  (`riot-transport/src/ticket.rs:78-89`); `riot-follow` already syncs one namespace given a ticket.
- **Concrete follower-side gap** (`site_ffi.rs:836-843`): owned `/mod` + `/manifest` can only enter via
  the crate-internal `inspect_core_with_root` path — no public single-shot import; the sync session
  hardwires `followed_root = active community`, so a communal-active follower has no reachable way to
  import a shared `O` bundle with `followed_root = O`.

## 4. Options

| Option | Complexity | Risk | Additive vs architectural |
|---|---|---|---|
| **A. Widen the sync inventory** to carry followed owned namespaces | High | **High — unsafe.** Breaks the load-bearing `inventory == active_namespace_live_ids` equality (the sole cross-community leak guard on an unstamped set). `ByteSyncSession` is single-namespace anyway. | Architectural. **Reject.** |
| **B. Explicit import-bundle FFI** — owner shares signed bytes, follower imports via new FFI calling existing `inspect_core_with_root(Some(root))`; inventory untouched; out-of-band carrier | Low–Med | **Low.** Reuses proven admission; owned entries stay out of the community inventory (correct). Must gate to owned `/mod`,`/articles`,`/manifest` AND require an existing `Following` record for the root. | **Additive.** |
| **C. Dedicated followed-site sync session** keyed on `followed_site_root` (separate per-site inventory, owner-side seed, Rung 5 `follow_site(ticket)`) | High | Medium | **Architectural — needs human decision.** |
| **D. Make O a switchable "community"** | Med | Med — against the data model (Following is author-less, filtered out) | Architectural + against-grain. Not recommended. |

## 5. Recommendation + first-PR sketch (Option B)

Smallest safe step that makes moderation actually reach a follower. Closes the exact hole the code
flags; touches no inventory invariant; reuses proven admission; works over any out-of-band carrier
(already the stated model). The owner already receives the bytes (`create_site_moderation_action`
returns `SiteModerationOutcome { action, epoch }` with `signed_bytes`).

**First PR:**
1. `#[uniffi::export] fn import_followed_site_bundle(&self, bytes, followed_site_root) -> Result<ImportSummary, MobileError>` in `site_ffi.rs`.
2. Body: parse 32-byte root; **require an existing `Relationship::Following` record** for it (reject
   otherwise — stops smuggling an unfollowed owned ns); clear preview/plan; call
   `inspect_core_with_root(&store, &bytes, "site-follow-import", Some(root))`; **family-gate** every
   eligible entry to owned `/mod`,`/articles`,`/manifest` (reject anything else); `plan_all()` →
   `commit()`; **do not touch `sync_inventory`**; optionally stamp `last_sync_unix_seconds` on the
   Following record.
3. Tests (in-crate): follower with a Following record imports an owner mod bundle → `resolve_composite_site`
   advances off `ModerationLoading`; a bundle for an *unfollowed* root is rejected; a bundle carrying a
   communal/alert entry is rejected.
4. UniFFI regen required (new export).

**For the human:** Option C is the eventual architecture (automatic delivery over Rung 5's
`follow_site(ticket)`, a per-followed-site inventory, an owner-side seed serving O). It's LARGE and
hinges on a decision the code hasn't made — separate per-site inventory vs generalized multi-namespace
inventory. **That decision needs your sign-off.** Option B ships independently and does not foreclose C.
