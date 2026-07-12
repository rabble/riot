# Headless Multi-Node Test Network Design

Date: 2026-07-12
Status: Approved in brainstorming and design review gate

## Goal

Prove that several independent Riot app nodes can write signed public data and
app data, exchange it through the real mobile sync boundary, relay facts they
did not originate, and converge after a network partition heals.

The proof is deterministic and headless. It does not launch iOS/Android UI,
open real sockets, or add a production networking mode.

## Why the Existing Tests Are Not Enough

`core_sync.rs` proves the reconciliation state machine between two peers.
`mobile_contract.rs` proves two-profile alert sync and relay retention.
`apps_contract.rs` proves two-profile app-data sync. None constructs a network
of independently writing nodes, partitions that network, reconnects it through
a bridge, and proves application-level convergence everywhere.

## Chosen Boundary

The harness uses the public `riot_ffi` mobile API:

- `open_local_profile` for isolated nodes;
- `create_public_space` / `join_public_space`;
- `create_draft_alert` / `sign_draft` / `list_current_entries`;
- `app_runtime().install_app`, `app_data_put`, `app_data_get`, and
  `app_data_list`;
- `open_sync_session`, opaque outbound frame bytes, `receive_frame`, and
  `accept_import`.

This exercises the actual write, signing, inventory, preview/accept admission,
retention, relay, and read paths used by native applications. A lower-level
`ReconcileSession`-only harness is rejected because it would bypass application
admission and app-data reads. Separate localhost processes are rejected for the
first proof because scheduling and port behavior would add nondeterminism
without exercising a different Riot contract.

## Test-Only Components

New support lives only under `crates/riot-ffi/tests/support/`:

- `TestNode`: name, isolated `Arc<MobileProfile>`, shared `PublicSpace`, and the
  deterministic test app ID;
- `TestNetwork`: ordered nodes and deterministic edge schedules;
- `NodeSnapshot`: the normalized public read projection described below;
- `PairSyncReport`: stable endpoint labels, frames delivered in each direction,
  review bundles observed and accepted by each endpoint, protocol steps, and
  the two terminal outcome kinds;
- `sync_pair`: opens one sync session per endpoint, pumps only opaque bytes,
  accepts `ReviewImport` only for two named `TestNetwork` peers already known
  to have the same full namespace ID, and runs both directions to terminal
  outcomes;
- `run_until_quiescent`: runs an ordered edge list repeatedly until a complete
  round accepts zero imports, with a hard maximum of ten rounds.

No helper reaches into `MobileProfile.inner`, `ProfileState`, `ReconcileSession`,
or decoded `SyncFrame`. If the public mobile API cannot support the network,
the test fails rather than acquiring a test-only production bypass.
`support` is included only with `mod support` from the integration test, defines
no UniFFI exports, and requires no production module or feature changes.

The test-only contracts are:

```text
sync_pair(left: &TestNode, right: &TestNode) -> Result<PairSyncReport, SyncError>
// Test-crate-only primitive: sync_pair passes 16; the forced-cap test passes 1.
pub(crate) pump_pair(
    left: &TestNode,
    right: &TestNode,
    max_transitions: usize,
) -> Result<PairSyncReport, SyncError>
run_until_quiescent(
    network: &TestNetwork,
    edges: &[Edge],
    max_rounds: usize,
) -> Result<NetworkRunReport, SyncError>
```

`PairSyncReport` counts review bundles, not alert entries: an app-data review is
a real import even when `SyncOutcome.entries` is empty. `already_current` is a
derived predicate meaning that both endpoints observed zero review bundles and
accepted zero review bundles. Reports never retain frame bytes, import-bundle
bytes, signed payloads, app values, sealed identities, or wrapping keys.

`SyncError` preserves a structured cause (`MobileError` for public-API
failures, or a harness invariant/transition-limit cause), round/edge/step and
endpoint context, and the partial metadata-only `PairSyncReport`. The namespace
test can therefore match the underlying `MobileError::InvalidInput` and prove
from partial counters that no review or accept occurred. Checks inside the pump
return `SyncError`; they do not panic before the cleanup guard runs.

`NodeSnapshot` is built exclusively from `list_current_entries()` and
`app_data_list(app_id, "items/")`. It contains:

```text
alerts: sorted [(entry_id, namespace_id, signer_id, headline)]
app_items: sorted [(key, value)]
```

Alerts sort by full entry ID and app items sort by exact key. Snapshot equality
means exact equality of those two normalized vectors; it deliberately excludes
random local identity metadata and app-install bookkeeping. Every listed app
item is also checked with `app_data_get`, and every expected alert is checked
against its recorded full entry ID and signer ID.

### Pair Pump and Cleanup

The left endpoint is the sole initiator and calls `begin()` exactly once; that
call consumes transition 1, so `pump_pair(..., 1)` deterministically takes the
transition-limit cleanup path before delivering the initial queued frame. The
harness forwards only bytes returned by `take_outbound_frame()`, alternates the
endpoint that owns the next queued frame, and immediately calls
`accept_import()` only after that same endpoint returns `ReviewImport`. It
continues until both endpoints are terminal, including delivery of any queued
terminal frame. Missing queued frames, unexpected outcome/terminal
combinations, API errors, or more than 16 protocol transitions are harness
errors with round, edge, step, endpoint label, and outcome kind.

A transition is one successful `begin`, `receive_frame`, or `accept_import`
call; `take_outbound_frame` and cleanup calls do not consume the transition
budget. Terminal handling follows both `outcome.terminal` and queued-frame
draining: a terminal `FrameReady` still has its final frame delivered, while a
terminal `Complete` may already have removed its session.

Session cleanup is mandatory rather than dependent on handle drop. A cleanup
guard tracks whether each endpoint reached a closed terminal state. On every
other exit—including namespace mismatch, malformed input, assertion/reporting
failure, and the 16-step cap—it calls `cancel()` on both sessions best-effort.
The primary pump error is preserved and any cleanup errors are appended. Error
path tests then prove that each profile can immediately open and cancel a fresh
session. Successful pairs likewise prove that a fresh session can be opened
after terminal completion.

## Five-Node Scenario

1. Node A creates one public space; B–E join it. Every node has a distinct
   signing identity inside that namespace.
2. The scenario creates one checklist manifest/bundle with Riot's existing
   codecs, once, then clones those exact byte vectors into every node. Its
   generated app author makes the bytes and app ID dynamic between test runs,
   but all scenario participants record the same runtime app ID without a fake
   app-data path.
3. Each node writes exactly one alert using `valid_from = None`,
   `expires_at = u64::MAX - 1`, language `en`, Immediate/Severe/Observed,
   `ai_assisted = false`, headline `Network alert from node X`, description
   `Test-network write from node X`, and source claim `node-X`. Each also writes
   UTF-8 bytes `checklist value from node X` at the lowercase key
   `items/node-x`, for X = A through E.
4. The initial partition has two components:
   - A ↔ B ↔ C
   - D ↔ E
5. Each component syncs until quiescent. A/B/C must read exactly three alerts
   and three checklist values; D/E must read exactly two. No node may see a fact
   from the other component.
6. The bridge C ↔ D is added. The connected edge schedule
   `[A-B, B-C, C-D, D-E]` repeats until quiescent.
7. Every node must read all five alerts and all five checklist key/value pairs.
8. One more complete round must observe and accept zero review bundles and
   leave every normalized snapshot exactly identical.

## Assertions

The test proves:

- five full, distinct signing-key IDs authored the five alerts;
- every expected alert entry ID is present exactly once on every node;
- every expected app key returns its exact value on every node;
- list results agree with point reads;
- relay is origin-specific: A's alert and app item reach E, and E's alert and
  app item reach A, despite neither endpoint syncing directly;
- partitioned components cannot read one another's facts before C ↔ D;
- the deterministic connected schedule alternates forward and reverse edge
  order by round and converges under that schedule;
- a final already-current round transfers no import bundle;
- each session closes, leaving nodes able to write or open another session.

## Namespace Isolation

A separate negative test creates a sixth profile F in a foreign public
namespace. F installs the same scenario-cloned app bytes, signs its own
non-expiring `Network alert from node F`, and writes `checklist value from node
F` at `items/node-f`, so both sides have non-empty alert and app-data reads.
Attempting to sync it with a network node must fail through the current public
surface as `MobileError::InvalidInput`. Before and after the attempt, the test
compares both normalized list snapshots and point reads on both profiles. It
also proves that no `ReviewImport` occurred, the auto-accept path was never
entered, both sessions were explicitly canceled, and both profiles can
immediately open and cancel a fresh session. The harness does not silently join
or switch namespaces to make the test pass.

## Determinism and Bounds

- Node names, edge order, key/value templates, and alert fields are fixed. The
  app fixture is generated once per scenario and cloned byte-for-byte; its
  runtime app ID is recorded rather than printed or hard-coded.
- Cryptographic identities remain randomly generated, but assertions compare
  complete runtime identities rather than snapshots with hard-coded keys.
- The quiescence cap is ten rounds; exhausting it fails with the last per-edge
  reports and node snapshot counts.
- A pair session has a fixed 16-transition cap; exceeding it fails instead of
  hanging and exercises the same mandatory cleanup path.
- No sleeps, wall-clock ordering assumptions, sockets, threads, or network
  access are used.
- Routine summaries contain only stable node/edge labels, round and step
  numbers, frame/review/accept counts, outcome kinds, terminal flags, and
  snapshot counts. They omit random identities and all content bytes. If a
  public ID is needed in a failing assertion, it is printed in full, never
  truncated.

## TDD and File Scope

Files:

- create `crates/riot-ffi/tests/support/mod.rs`;
- create `crates/riot-ffi/tests/support/test_network.rs`;
- create `crates/riot-ffi/tests/multi_node_network.rs`.

Existing integration tests remain unchanged. The small deterministic app fixture
is duplicated in the new support module to keep this work isolated and avoid
refactoring already released assertions.

TDD sequence:

1. RED: write the five-node partition test against the wished-for harness and
   observe missing types/functions.
2. GREEN: implement node creation, writes, pair pumping, and quiescence with the
   minimum public-API-only code.
3. RED: add already-current and origin-specific relay assertions.
4. GREEN: add deterministic reports and normalized snapshots.
5. RED: add foreign-namespace and forced-cap cleanup/recovery tests. The cap
   test calls private `pump_pair(..., 1)`; the namespace test matches the
   structured underlying `MobileError::InvalidInput` and partial
   zero-review/zero-accept counters.
6. GREEN: add the two-sided cleanup guard and bounded error handling.
7. REFACTOR: extract only duplicated fixture construction; rerun focused and
   full workspace gates.

## Definition of Done

- `cargo test -p riot-ffi --test multi_node_network -- --nocapture` passes and
  prints deterministic per-round convergence summaries without secret material.
- Both public alerts and checklist app data converge across five isolated nodes.
- Partition isolation, relay propagation, namespace rejection, and quiescent
  no-op resync are asserted.
- Namespace mismatch and forced-cap failures mutate no reads and release both
  profiles for an immediate new session.
- The harness uses only public mobile APIs and test-only files.
- `cargo test --workspace --all-features`, strict Clippy, formatting, and the
  configured coverage command `cargo tarpaulin --fail-under 100` from
  `.coverage-thresholds.json` pass before completion. Success,
  namespace-error, and forced-cap helper branches are exercised explicitly
  even if Tarpaulin excludes integration-test support source.
