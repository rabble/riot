# Headless Multi-Node Test Network Design

Date: 2026-07-12
Status: Approved in brainstorming; pending design review gate

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
- `PairSyncReport`: frames exchanged, imports accepted on each side, and
  whether the pair was already current;
- `sync_pair`: opens one sync session per endpoint, pumps only opaque bytes,
  automatically accepts every `ReviewImport`, and runs both directions to a
  terminal outcome;
- `run_until_quiescent`: runs an ordered edge list repeatedly until a complete
  round accepts zero imports, with a hard maximum of ten rounds.

No helper reaches into `MobileProfile.inner`, `ProfileState`, `ReconcileSession`,
or decoded `SyncFrame`. If the public mobile API cannot support the network,
the test fails rather than acquiring a test-only production bypass.

## Five-Node Scenario

1. Node A creates one public space; B–E join it. Every node has a distinct
   signing identity inside that namespace.
2. All five nodes install the exact same deterministic checklist app bundle,
   producing one common app ID without a fake app-data path.
3. Each node writes one signed alert with a node-specific headline and one app
   value at `items/node-a` through `items/node-e`, containing a matching value.
4. The initial partition has two components:
   - A ↔ B ↔ C
   - D ↔ E
5. Each component syncs until quiescent. A/B/C must read exactly three alerts
   and three checklist values; D/E must read exactly two. No node may see a fact
   from the other component.
6. The bridge C ↔ D is added. The connected edge schedule
   `[A-B, B-C, C-D, D-E]` repeats until quiescent.
7. Every node must read all five alerts and all five checklist key/value pairs.
8. One more complete round must accept zero imports and leave every snapshot
   byte-for-byte/equality identical.

## Assertions

The test proves:

- five full, distinct signing-key IDs authored the five alerts;
- every expected alert entry ID is present exactly once on every node;
- every expected app key returns its exact value on every node;
- list results agree with point reads;
- B and D successfully relay data they did not author;
- partitioned components cannot read one another's facts before C ↔ D;
- the convergence result is independent of whether the connected edge schedule
  runs forward or in reverse in alternating rounds;
- a final already-current round transfers no import bundle;
- each session closes, leaving nodes able to write or open another session.

## Namespace Isolation

A separate negative test creates a sixth profile in a foreign public namespace.
Attempting to sync it with a network node must fail with namespace rejection and
must not change either side's alert or app-data snapshot. The harness does not
silently join or switch namespaces to make the test pass.

## Determinism and Bounds

- Node names, edge order, app bundle, keys, and expected values are fixed.
- Cryptographic identities remain randomly generated, but assertions compare
  complete runtime identities rather than snapshots with hard-coded keys.
- The quiescence cap is ten rounds; exhausting it fails with the last per-edge
  reports and node snapshot counts.
- A pair session has a frame-step cap derived from current protocol bounds;
  exceeding it fails instead of hanging.
- No sleeps, wall-clock ordering assumptions, sockets, threads, or network
  access are used.

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
3. RED: add foreign-namespace isolation and already-current assertions.
4. GREEN: add bounded error/report handling.
5. REFACTOR: extract only duplicated fixture construction; rerun focused and
   full workspace gates.

## Definition of Done

- `cargo test -p riot-ffi --test multi_node_network -- --nocapture` passes and
  prints deterministic per-round convergence summaries without secret material.
- Both public alerts and checklist app data converge across five isolated nodes.
- Partition isolation, relay propagation, namespace rejection, and quiescent
  no-op resync are asserted.
- The harness uses only public mobile APIs and test-only files.
- `cargo test --workspace --all-features`, strict Clippy, formatting, and the
  configured coverage command pass before completion.
