# Area-Scoped Windowed Sync

Date: 2026-08-03
Status: Draft — for design review gate.

## Purpose

Riot cannot hold a community larger than 64 records. Not 64 posts — 64 records of every kind, counting the descriptor, every post, every comment, every reaction and every profile card. Past that, the app stops accepting writes entirely, offline included.

This design replaces namespace-scoped whole-inventory sync with **area-scoped windowed sync**: a device declares which slice of a community it is responsible for, and syncs that. The community itself stays one namespace forever.

The immediate motivation is a production outage. The deeper motivation is that Riot is built for long-running communities — a tenants union, a mutual aid network, a newswire that runs for years — and the current design cannot express one.

## Background: what broke, and what it revealed

On 2026-08-03 every write in the macOS app failed with "Reactions aren't available for this post." Three distinct root causes produced that one string, because `ReactionFailure` collapses `Internal`, `InvalidInput`, `StalePreview` and `AppRejected` into a single bucket.

Two were fixed (`bad1b4ea`, `44fc0379`): a relaunch left the profile's author bound to the wrong namespace, and every commit rewrote the entire live set so the WAL estimate grew with store size until all writes were refused at ~44 entries.

The third is not a bug. It is the design:

```rust
// crates/riot-core/src/sync/wire.rs:7
pub const MAX_SYNC_IDS: usize = 64;
```

Enforced in eight places, including `ensure_complete_sync_inventory`, which runs on **every newswire write**. The 65th record makes the app read-only with no network involved.

### The concept underneath

`LocalProfile.sync_inventory: Vec<SignedWillowEntry>` is a second, in-RAM copy of the entire community, which must byte-match the store's live set exactly, re-derived and re-encoded on every write. `org.riot.conference-sync/1` matches it: peer A sends a `Summary` frame containing its complete entry-id list, B replies with a `Request`, A sends one bundle. No paging, no rounds.

A community is therefore an **atomic blob** — fully re-derived, re-encoded and re-sent on every interaction. Every operation is O(community).

Measured on a live profile: entries average **312 bytes** signed (47 entries, 14,649 bytes). So the whole-community re-encode on each write costs ~20 KB at the current cap. Raising the cap to 4096 would make it ~1.3 MB **per reaction**, and the 8 MB bundle ceiling caps the design at ~27,000 records regardless.

Raising `MAX_SYNC_IDS` moves the wall without changing the shape. This design changes the shape.

## How Willow is designed to work

The vendored `willow25-0.6.0-alpha.3` provides the data model, and it is built around exactly this problem.

**You do not sync a namespace. You sync an `AreaOfInterest`.**

```rust
AreaOfInterest::new(
    Area::new(subspace, path_prefix, TimeRange),  // WHICH entries
    max_count,                                    // at most N (spec: the newest)
    max_size,                                     // at most M payload bytes
)
```

An `Area` is a 3D selection — subspace × path prefix × time range. Storage keys are ordered `encode_spt_key(subspace, path, timestamp)`, so an area is a native ordered range scan, not a filter.

A phone syncing a five-year-old community asks for a window. The history is not deleted, not in another namespace, and remains fetchable on demand.

**What willow25 does not provide:** reconciliation. The string `fingerprint` appears zero times in the crate. There is no WGPS. Riot's hand-rolled single-shot protocol exists because nothing else was available.

## Riot's paths are already area-shaped

```
newswire/v1/<descriptor>/posts/<8-byte timestamp>/<hash>
newswire/v1/<descriptor>/comments/<8-byte timestamp>/<hash>
newswire/v1/<descriptor>/reactions/<8-byte timestamp>/<hash>
newswire/v1/descriptors/<8-byte timestamp>/<hash>
```

Posts, comments and reactions are separate subtrees, each time-ordered by a **timestamp path component**. "Posts from the last month" is one contiguous range scan today. This design does not require a path migration.

⚠️ Timestamps are TAI/J2000 **microseconds**. A `TimeRange` built from Unix seconds compiles and authorizes zero entries.

## Decisions

1. **A community remains one namespace, permanently.** Segmentation happens in what devices ask for, never by splitting a community.
2. **The sync inventory concept is deleted.** Offers are derived from the store, scoped to an area, at sync time. Nothing is held in RAM, nothing is compared, nothing is re-encoded on write.
3. **The offer-derivation API is area-shaped from its first commit**, even while the only area in use is "everything." A namespace-shaped API would be immediate rework.
4. **Windowing and fingerprint reconciliation are separate projects, in that order.** Windowing decides what a device is responsible for and changes what the app is. Fingerprints make syncing it cheap. Windowing alone still sends the full id list *for the window* — a smaller O(n), still O(n) — and that is acceptable as an intermediate state.
5. **Records whose absence is interpreted as permission are exempt from windowing.** See below. This is a safety property, not an optimization.
6. **Different subtrees get different retention policies.** Reactions are high-churn and low-value to backfill; posts are the durable record.

## The safety constraint

Riot derives moderation state from the records a device holds. `crates/riot-core/src/newswire/projection.rs:536`:

```rust
let treatment = if !tombstone_ids.is_empty() {
    PostTreatment::Tombstoned { .. }
} else if !hide_ids.is_empty() {
    PostTreatment::Hidden { .. }
} else {
    PostTreatment::Ordinary      // body renders
};
```

No tombstone record means `Ordinary` means **the redacted body renders**.

If a window drops an old tombstone while the device still holds — or later re-fetches — the post it targets, moderated content silently reappears. In a tool used during protests, that can mean a name someone asked to have taken down comes back because of a window boundary.

**Therefore: editorial actions, tombstones, redactions and prefix-pruning entries sync in full regardless of age.** They are small and rare, so the exemption is cheap. Prefix-pruning entries are included because an entry that supersedes a subtree cannot be aged out without resurrecting everything it pruned.

Generalized, and worth applying beyond this design:

> **Any record whose absence is interpreted as permission cannot be windowed.**

The benign case is already handled correctly and is the contrast that makes the rule clear: `projection.rs:591` drops a comment whose parent is not in the eligible set rather than orphaning or crashing. A missing post is invisible. A missing tombstone is a re-publication.

## The isolation invariant

`ensure_complete_sync_inventory` and `install_sync_inventory` enforce:

> the sync inventory must equal **exactly** every live id in the active namespace

Both are commented LOAD-BEARING for community isolation — this equality is what stops community A's entries reaching community B's peers. It is fundamentally incompatible with windowed sync: a device cannot hold "all entries" and also sync a slice.

The replacement preserves the goal in a stronger form:

> the offer is **derived from** an Area whose namespace is X

This is enforced by construction at the range scan, rather than by comparison after the fact — tighter *and* cheaper. `build_followed_site_offer` already demonstrates the pattern per namespace (and is currently `#[allow(dead_code)]`).

**This is the highest-risk part of the change.** Cross-community leakage needs adversarial testing as a first-class deliverable, not a follow-up.

## Work units

| # | Unit | Delivers | Risk |
|---|---|---|---|
| 1 | Area-shaped offer derivation; delete `sync_inventory` | per-write O(n) gone; `MAX_SYNC_IDS` can rise honestly | **High** — isolation invariant |
| 2 | Real windows: prefix + time range + count, with the moderation exemption | long-running communities become expressible | Medium — safety property |
| 3 | `AreaOfInterest` on the wire; paged `Summary`/`Request` frames | sync stops being single-shot | Medium — wire format |
| 4 | Fingerprint range reconciliation | O(differences); size stops mattering | High — likely upstream in willow25 |

Units 1–2 are ordinary refactors against well-understood code. Unit 3 requires reading the WGPS specification properly first; this document is grounded in the vendored crate and Riot's code, not in the spec, and an area-scoped wire protocol should not be designed by inference. Unit 4 is arguably a contribution to willow25 rather than Riot.

## Blast radius

- `prospective_sync_inventory` / `install_sync_inventory`: 15 call sites (14 in `mobile_state.rs`, 1 in `newswire_ffi.rs`)
- `ensure_complete_sync_inventory`: 2 call sites
- `ByteSyncSession::new(namespace_id, inventory)` → area-scoped
- `crates/riot-core/src/sync/{state,reconcile,wire,ffi_bridge}.rs`
- `EvidenceStore`: additive range-scan method alongside `entries_with_prefix_in_namespace`. **No schema migration, no path migration.**

## Testing requirements

Store-capacity and windowing bugs are invisible to tests that write two entries in one profile lifetime. The entire suite stayed green through a week-long outage for exactly that reason.

Mandatory:

1. **Fill the store.** 40+, 300+, and past-the-cap communities, on durable SQLite, not in-memory.
2. **Cross a process boundary.** Two profile lifetimes against one `db_path`. Same-session durable tests cannot see restoration bugs.
3. **Adversarial isolation.** A device in communities A and B must never offer an A entry to a B peer, under every area shape.
4. **The moderation exemption.** A device whose window excludes an old tombstone, but which holds the targeted post, must still render it redacted. This is the test that must never be allowed to fail.
5. **Orphan tolerance.** Windows that exclude parents degrade gracefully.

## Open questions for review

1. What is the default window? Time-based ("30 days"), count-based ("newest 500"), or size-based? Different answers for posts vs. reactions?
2. Who chooses — the app, or the person? Is "load older" a visible, deliberate act, consistent with Riot's preview-first import posture?
3. Do anchors/relays hold everything while phones hold windows? That is the natural split, but it makes relays load-bearing for history in a tool designed to survive their absence.
4. Should unit 4 be contributed upstream to willow25 rather than built in Riot?
5. Is there any *other* record type whose absence means permission? The exemption list must be proven exhaustive, not assumed. Capability revocations are the obvious candidate to check.
