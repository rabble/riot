# Store scaling: what we know, and the next two fixes

Date: 2026-08-03
Status: Living status doc. Three fixes landed; two named quadratics remain; the
caps must not move until they do.

## Why this exists

A week-long outage — every write in the macOS app failing with "Reactions
aren't available for this post" — turned out to be three unrelated causes
wearing one error string, and chasing them exposed a scaling story nobody had
written down. This is that story, with measurements rather than reasoning.

**The single most useful lesson: every prediction about where the cost was
turned out to be wrong, and every measurement found it somewhere else.** The
cause was assumed to be ed25519 verification (it was 6% and linear), then a
monotonic RAM index (it was 1 MB), then linear scans (they were free at the
current ceiling). Do not reason about this code's performance. Run the harness.

```
cargo test -p riot-core --test sqlite_open_cost -- --ignored --nocapture
```

## The ceilings, and what each is scoped to

Riot has at least six capacity ceilings in play simultaneously. Before claiming
any one of them is "the" limit, check which binds first for the operation in
question — several are easy to mistake for each other.

| ceiling | value | scope | failure |
|---|---|---|---|
| `MAX_SYNC_IDS` | 64 | **per namespace** | `SessionLimit`, refuses writes AND sync |
| `MAX_BUNDLE_ENTRIES` | 64 | per bundle | `TooManyEntries` on encode |
| `MAX_IMPORT_RECEIPTS` | 256 | per store | `CorruptDatabase` at open |
| `MAX_STORE_ENTRIES` | 1024 | **per store** | `StoreFull` at write — clean |
| `MAX_ACCEPTED_ENTRIES` / `MAX_LIVE_ENTRIES` | 1024 | **per store** | `CorruptDatabase` at open (backstop only) |
| `retained_store_budget_bytes` | 16 MB | per store | budget refusal |
| `MAX_BUNDLE_BYTES` | 8 MB | per bundle | ~27k records |

Two scoping facts that mislead people:

- `JoinState` holds **one merged live view across all namespaces**, so the 1024
  store caps are per *store*, not per namespace. Splitting a community into
  several namespaces does not relieve them.
- The write-time check is against `next_seen` — the **permanent** index of every
  entry ever accepted (`seen_index_charge_bytes` documents that it must never
  decrease). `forget_entry` removes from `live` and never touches `seen`. **No
  operation currently reclaims a seen slot.**

The count cap is ~32x tighter than the byte budgets beside it
(`retained_store_budget_bytes` 16 MB / `store_charge_entry_bytes` 512 implies
~32,700 entries; `store_encoded_entry_bytes` 8 MB at ~312 B/entry measured
implies ~26,800). Undocumented inference: 1024 was sized so the then-quadratic
dominance scan met the manifest's `inspection_target_seconds: 2` — a target
declared in `fixtures/manifest.json` and enforced nowhere in code.

`ceilings.store_entries` is a frozen contract value: declared in
`fixtures/manifest.json` (`riot-phase0a/1`, frozen 2026-07-10) and pinned again
in `crates/xtask/src/main.rs` as `EXPECTED_CEILINGS`, checked exactly by
`validate-contracts`. Raising it is a deliberate two-file amendment.

## Landed 2026-08-03

- `44fc0379` — `persist_transaction` wrote the **whole live set** on every
  commit (`DELETE FROM live_entries` + full reinsert), so the WAL estimate grew
  with store size and `admit_write` refused every write past ~44 entries. Now
  writes the delta. This was the actual outage.
- `50f7d762` — two all-pairs dominance scans (one per write in `import/join.rs`,
  one per receipt at open in `store/evidence.rs::replay_final_live`) replaced
  with ancestor-scoped lookups. Prefix pruning requires matching namespace,
  matching subspace, and a path-prefix relation, so a candidate's only possible
  pruners sit at its own path prefixes. Open at 1000 entries: 813 ms -> 183 ms;
  the scan itself 635 ms -> 11 ms.
- `3a5707be` — `seen` / `forgotten` / `union` / `final_ids` from linear `Vec`
  scans to `BTreeSet`. Bought nothing measurable at today's ceiling (740 ms ->
  737 ms, noise); a prerequisite for raising it, not a speedup.

## Measured, with the caps temporarily lifted

| entries | build | open | open/entry |
|---|---|---|---|
| 1,000 | 0.74 s | 0.18 s | 0.182 ms |
| 4,000 | 16.5 s | 2.03 s | 0.508 ms |
| 12,000 | 444 s | 17.2 s | 1.431 ms |

**The caps cannot be raised in this state.** A 12k-entry store takes seven
minutes to write and 17 seconds to open.

## The next two fixes

Both remaining sites are the same shape as the original bug: *rebuilding a
whole-store index per operation*.

### 1. Write path — `EvidenceStore::persist` re-reads the live set every commit

`persist` calls `read_connection(live_entry_keys)`, a full
`SELECT namespace_id, entry_id FROM live_entries`, on every commit; the
transaction then scans `live_entries` again to find departed rows. Two
full-table reads per write, so O(n^2) across a store's life. Introduced by
`44fc0379` — that commit removed an O(store) *write* and left an O(store)
*read* in its place.

**Take:** the join already knows the answer and should hand it over. `JoinState`
holds both the previous `live` and the next `live` in memory, so the delta is a
set difference computed in RAM with no database read at all. Change
`EvidenceMutation` to carry `entered: Vec<LiveJoinEntry>` + `departed:
Vec<EntryId>` instead of `live: <the entire live set>`, and let `persist` charge
and write exactly those.

The safety question is whether `persist` can trust a caller-supplied delta
rather than re-deriving it. It can: `persist_transaction` already opens with a
generation-guarded update
(`UPDATE evidence_meta SET ... WHERE singleton = 1 AND generation = ?4`,
`BusyRetryable` if it does not match exactly one row), so a delta computed
against a stale view cannot commit. That guard is what makes this safe, and it
should be called out in the code, not rediscovered.

Expected: removes both full scans; write cost becomes proportional to the batch.
Confidence: high — mechanism is clear and the guard already exists. Verify with
the build column, which is where it will show.

### 2. Open path — `replay_final_live` rebuilds its bucket index per receipt

`validate_relational_state` loops over every import receipt and calls
`replay_final_live` each time; that function builds its `by_path` bucket index
over the whole union on every call. `50f7d762` made the inner comparison cheap
and left the index build at O(n) per receipt, so open is still O(n^2).

**Take:** hoist it. Build `by_path` **once** over `accepted` before the receipt
loop and pass it in; per receipt, look up the candidate's ancestor buckets and
intersect against that receipt's union (a `BTreeSet` membership test). Index
build becomes O(total) once instead of O(n) per receipt.

Bigger structural option, worth doing separately rather than instead:
**watermark the audit.** Persist "verified through generation N" and replay only
receipts after it, so a steady-state open is O(new since last open) rather than
O(every receipt ever). The hoist is the small, safe change; the watermark is the
one that makes open cost stop depending on history length at all.

Confidence: high on the hoist, medium on the watermark's blast radius — it
changes what a profile open actually guarantees, so it wants its own review.

### Then, and only then

Re-measure, find where the byte budgets actually bind, and amend
`fixtures/manifest.json` + `crates/xtask/src/main.rs` together with the
measurement in the commit message. Keep the failure at the boundary clean:
`StoreFull` is correct behaviour and must not regress into anything quieter.

## Beyond the caps

- **The reaction path scheme is wrong.** Reactions live at
  `newswire/v1/<descriptor>/reactions/<timestamp>/<digest>`, so every reaction —
  and every toggle-off — is a distinct sibling path that nothing ever prunes.
  Tapping a reaction on, off, and on again leaves three permanent live records
  while the UI shows one; `projection.rs` does latest-wins in code at read time.
  Putting identity coordinates in the path instead
  (`.../reactions/<parent_entry_id>/<kind>`, with the author already covered by
  the subspace half of the prune predicate) would make Willow's own prefix
  pruning do that supersession, turning reaction cost from O(taps) into
  O(opinions). This is a record-family migration and needs a design.
- **Nothing reclaims a seen slot.** Even with the caps raised, `accepted_entries`
  grows forever (~312 B/record measured; a million records is ~312 MB). The
  proposal on the table is **one namespace, many local shards** — shard the
  *store* by date span while the namespace, and therefore community identity,
  capabilities and signatures, stay unchanged. Peers see one community; sharding
  is purely local, needs no wire change, and gives a unit that can be archived
  or dropped wholesale. `open_stores_per_session = 1` is the constraint to
  design around.
- **Sync is the one thing that genuinely needs protocol work.** `MAX_SYNC_IDS`
  is 64 because `org.riot.conference-sync/1` puts the complete entry-id list in
  one `Summary` frame with no paging, under an unversioned codec. See
  `docs/superpowers/specs/2026-08-03-area-scoped-windowed-sync-design.md` — that
  design went through the review gate and came back NEEDS_REVISION from all five
  reviewers; read the corrections before building from it.

## Rules for whoever picks this up

1. **Do not raise any cap** until fixes 1 and 2 land and the table above is
   re-measured.
2. **Do not trust the UI copy when diagnosing.** `ReactionFailure` collapses
   `Internal`, `InvalidInput`, `StalePreview` and `AppRejected` into one string;
   three different root causes wore it for a week. Read
   `warn!(target: "riot::newswire")` instead — in tests via
   `#[tracing_test::traced_test]` with `--nocapture`.
3. **Test across two profile lifetimes and with a full store.** Every durable
   test built its community and wrote into it inside one profile lifetime, with
   a handful of entries. That shape is structurally blind to both the outage and
   the capacity cliff, which is why the suite stayed green throughout.
4. **Measure, don't predict.** See the top of this document.
