# Riot conference incremental sync

Status: implementation under review for the conference demo.

Riot conference sync is a small, transport-independent reconciliation layer.
BLE, local IP streams, QR, and files carry the same canonical frame bytes; none
of those transports may bypass the existing inspect, preview, plan, and commit
admission boundary.

## Protocol shape

Every canonical CBOR frame carries the fixed codec ID, one frame kind, and one
bounded body. The one-shot, bidirectional exchange is:

1. `Hello(namespace)`
2. `Summary(namespace, sorted entry IDs)`
3. the initiator sends `Request(namespace, missing entry IDs)` and receives
   `Entries(namespace, canonical Riot evidence bundle)`, when needed
4. after preview/plan/commit accepts those bytes, the initiator sends its union
   `Summary`
5. the responder requests and imports anything it is missing
6. the peers finish with `Complete`, or either side terminates with `Reject`

Summaries and requests contain at most 64 full 32-byte IDs. Entries reuse the
8 MiB evidence-bundle ceiling, and the enclosing frame has only a fixed 128-byte
allowance above it. Duplicate or unsorted IDs, unknown requests, foreign
namespaces, malformed/noncanonical bytes, oversized frames, and out-of-sequence
messages are rejected before the state advances.

## TDD evidence

The first focused test failed because `riot_core::sync` did not exist. The
current suite covers deterministic codec round trips, overlap suppression,
duplicate and ceiling failures, identical-set completion without transfer, a
two-author shared-namespace exchange that imports only the missing entry, and
state preservation after replay, sequence, namespace, or requested-fact
errors. A divergent-inventory test proves both peers transfer their unique
entry before the exchange completes.

## Inventory boundary

`ReconcileSession` accepts canonical `SignedWillowEntry` public export facts,
not private signer state. The current `EvidenceStore` intentionally retains
the live entry and accounting identities, not a capability, signature, or
payload export. Native persistence therefore must retain accepted canonical
public bundles alongside its store snapshot and rebuild the bounded sync
inventory from those bytes. Task 4 owns that adapter and its encrypted-at-rest
lifecycle; this state machine rejects any local inventory the canonical bundle
encoder would not export.

Run:

```bash
cargo test -p riot-core --test core_sync --locked
cargo clippy -p riot-core --test core_sync --locked -- -D warnings
```

Physical transport and two-phone radio-off evidence remain Task 5; this module
deliberately owns no radio, socket, persistence, or background-service code.
