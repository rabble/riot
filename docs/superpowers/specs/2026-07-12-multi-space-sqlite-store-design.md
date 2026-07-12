# Riot multi-space SQLite store design

## Status and purpose

Approved in product brainstorming on 2026-07-12. This replaces Riot's
single-space `profile.json` reconstruction model. Riot is inherently a client
for many Willow namespaces: spaces a person creates or owns, communal spaces
they participate in, and managed spaces they join through delegated
capabilities. Selecting a space is a UI/query operation and must never decide
which spaces survive on disk.

This is the first of three conference-recovery slices:

1. Multi-space SQLite store (this design).
2. Four working space apps and space navigation.
3. Internal TestFlight delivery for `riot.protest.net` under Verse
   Communications.

The store is the blocking foundation. App and release work may proceed only
where it does not preserve or deepen the single-space model.

Normative protocol references:

- [Willow Data Model](https://willowprotocol.org/specs/data-model/)
- [Meadowcap](https://willowprotocol.org/specs/meadowcap/)
- [Willow Confidential Sync](https://willowprotocol.org/specs/confidential-sync/)

Willow defines a logical store over one namespace. A Riot device maintains a
local database containing many such logical stores, keyed by full
`NamespaceId`. The current `willow25` reference stores namespaces in separate
keyspaces; Riot preserves those semantics while using SQLite as its physical
mobile database and JSON document-query layer.

## Use cases

1. **Space owner:** WHO creates several spaces; WANTS every owned namespace,
   root-key reference, entry, app approval, and app document to survive space
   switches and relaunches; SO THAT creating another group never destroys the
   first; WHEN creating, opening, or reopening Riot.
2. **Space member:** WHO joins spaces run by other people; WANTS joined
   namespaces and delegated capabilities stored alongside owned spaces; SO THAT
   participation is durable and does not imply ownership; WHEN accepting an
   invite, syncing, or reopening Riot.
3. **App user:** WHO uses the same app in two spaces; WANTS documents and votes
   isolated by namespace; SO THAT one group's data never appears in another;
   WHEN reading, writing, watching, or switching spaces.
4. **Offline peer:** WHO receives entries during nearby sync; WANTS verified
   entries and their JSON projections committed atomically; SO THAT a crash
   cannot produce a UI state different from the Willow state; WHEN importing
   or reconciling data.
5. **Existing tester:** WHO already has Riot data in `profile.json`; WANTS it
   migrated without silent loss; SO THAT upgrading does not erase spaces,
   alerts, apps, or app data; WHEN opening the SQLite-backed build first.

## Success and failure criteria

Success requires all of the following:

- Creating or joining a second space preserves the first after process death.
- Owned, communal, and joined namespaces remain distinguishable.
- Space listing and switching do not replay all historical bundles.
- The same app ID has independent documents and approval state per namespace.
- A verified local write or import updates Willow bytes and its document
  projection in one transaction.
- Quick Poll can create a poll, accept one movable vote per identity, display
  results, sync those results between two peers, and retain them after restart.
- Legacy migration is idempotent, preserves its source until verification, and
  never substitutes a blank database for unreadable data.

The slice fails if any test observes cross-namespace data, if a valid legacy
record disappears, if startup depends on Swift replaying receipt arrays, or if
corruption silently creates a fresh empty database.

## Architecture

### Ownership boundary

Rust owns persistence. Swift and Kotlin are clients of versioned UniFFI APIs;
they do not serialize the authoritative space database.

`RiotDatabase` owns the SQLite database, schema migrations, transactions, and
change feed. `SpaceSession` binds calls to an exact namespace and the locally
available capability/key references. `WillowRepository` verifies and persists
canonical entries and authorization material. `DocumentStore` exposes bounded
JSON `put`, `get`, `list`, and `watch` operations over accepted entries.

An app bridge is constructed only from `(SpaceSession, app_id)`. JavaScript
never supplies a namespace, SQL, filesystem path, capability, or arbitrary JSON
query expression.

### Physical database

One protected `riot.sqlite` contains:

| Table | Purpose |
| --- | --- |
| `schema_migrations` | Applied, ordered, transactional schema versions. |
| `spaces` | Full namespace ID, namespace kind, owned/joined relationship, display metadata, and local lifecycle state. |
| `key_references` | Opaque secure-storage references for namespace, subspace, and receiver keys; never secret bytes. |
| `capabilities` | Canonical capability bytes, fingerprint, namespace, receiver, mode, area, lineage, and policy state. |
| `willow_entries` | Namespace, subspace, path, timestamp, digest, length, canonical entry, authorization token/signature, and payload. |
| `documents` | Namespace-scoped deterministic JSON projections with collection, document ID, author, source entry, and current/pruned state. |
| `app_packages` | Global content-addressed manifest and bundle cache keyed by full app ID. |
| `space_app_state` | Namespace-specific app approval/trust/materialized availability. |
| `local_state` | Selected namespace and device-only preferences; never authority. |
| `migration_journal` | Source fingerprint, phase, counts, verification result, and completion marker. |

All identifiers are fixed-width blobs. Every space-scoped primary key starts
with `namespace_id`. Required indexes cover namespace/path coordinates,
namespace/collection/document, recency, capability receiver/fingerprint, and
app ID. FTS5 and RTree are deferred until a measured query needs text or
geospatial indexing.

SQLite uses WAL mode, foreign keys, a bounded busy timeout, explicit
transactions, and controlled checkpoints. A serialized Rust writer owns
mutation; read connections serve indexed snapshots. Native callers perform
database work off the UI thread.

### JSON projection

Canonical signed Willow entry bytes remain authoritative. JSON documents are
query projections and can be rebuilt. Native record families have deterministic
CBOR-to-JSON projections. App payloads must be valid bounded UTF-8 JSON.

System collections have closed versioned schemas. App collections accept any
valid JSON within host limits but remain restricted to the app's namespace and
app ID. JSON is stored as text initially for stable interchange with WebViews.
SQLite JSONB may later be used only as a derived optimization.

The accepted-write transaction is:

1. Decode canonically and enforce resource limits.
2. Verify namespace, Meadowcap chain/token, receiver, path/body binding, and
   applicable Riot policy.
3. Begin the SQLite write transaction.
4. Insert/update the canonical Willow entry and apply Willow recency/prefix
   pruning semantics within that namespace and subspace.
5. Update or remove deterministic document and app-state projections.
6. Append namespace/collection change records.
7. Commit, then publish notifications. Any failure rolls back every step.

### Spaces, identity, and capabilities

The space registry contains both spaces created locally and spaces joined from
others. Ownership is derived from namespace kind plus available root authority,
not from who selected or first displayed the space.

Secure storage holds or wraps secret material. SQLite stores opaque key
references, public keys, capability bytes, and policy metadata. Communal
participation and managed-space membership may use space-specific subspace or
receiver keys; no global-identity assumption is built into the schema.

Space selection changes only the active `SpaceSession` and visible queries.
It does not create, copy, re-import, or delete namespace data. Sync binds areas
and capabilities per namespace and may reconcile several shared namespaces in
one peer relationship when the transport supports it.

### Apps

App packages are globally content-addressed cached bytes. Whether an app is
approved, available, or revoked is namespace-specific. App data paths retain
their Willow `apps/<app_id>/...` shape inside the selected namespace, which
naturally isolates identical app IDs between spaces.

The conference starter catalog contains four real tools: Checklist, Supply
Board, Roll Call, and Quick Poll. Each must use the same bounded document API,
persist across restart, and update after remote sync. Quick Poll stores the
question under a unique poll ID and one vote document per `(poll_id,
voter_receiver_id)`; changing a vote replaces that voter's document rather
than adding a ballot.

## Legacy migration

Migration recognizes the current protected `profile.json`, fingerprints it,
and writes into a new temporary SQLite database. It imports the sealed identity
reference, current space, signed alert bundles, verified carried app packages,
trust decisions, profiles, and signed app-data receipts through the normal
verification/projection pipeline.

The migration transaction records expected and actual counts. Riot reopens and
queries the completed database before atomically selecting it. The JSON source
is retained as a recovery backup until a later explicit cleanup release.
Interruption is safe: an incomplete database is discarded or resumed according
to the journal, and re-running with the same source fingerprint is idempotent.
Malformed individual records are reported and block automatic source retirement;
they are never silently skipped while presenting migration as complete.

## Failure handling and protection

- Database open or integrity-check failure shows a recovery state and preserves
  the file; Riot never overwrites it with an empty database.
- SQL statements are static and parameterized. JavaScript inputs cannot become
  identifiers or query fragments.
- File protection applies to the database, WAL, SHM, backups, and migration
  files. Secrets remain in Keychain/secure storage.
- JSON, paths, identifiers, bundles, and payloads retain existing hard ceilings.
- A capability or policy failure mutates nothing and reveals no protected
  document contents.
- Watch notifications are emitted only after commit and include namespace plus
  collection so inactive spaces are not repainted.

## TDD and verification

Implementation follows red-green-refactor. The initial failing integration
tests establish the contract before the database code exists:

1. `second_space_survives_restart`.
2. `owned_and_joined_spaces_coexist`.
3. `same_app_is_isolated_between_namespaces`.
4. `failed_import_rolls_back_entry_and_projection`.
5. `sync_for_one_namespace_cannot_mutate_another`.
6. `legacy_migration_is_lossless_idempotent_and_interruptible`.
7. `corrupt_database_is_preserved_and_fails_closed`.
8. `quick_poll_create_vote_move_tally_sync_restart`.

Unit tests cover schema constraints, projection determinism, prefix pruning,
JSON limits, static parameterized queries, change filtering, and migration
journaling. Property tests generate namespace/document combinations to prove no
query or mutation crosses a namespace boundary. Native contract tests prove
the same APIs through UniFFI. Existing Rust, Swift, packaging, and app tests
remain green, with 100% coverage enforced by `.coverage-thresholds.json`.

Performance fixtures measure cold open, listing dozens of spaces, opening a
space with thousands of documents, an indexed app list, and a bounded bulk
import. Exact thresholds are fixed in the implementation plan after recording
the current simulator/device baseline; no unmeasured speed claim is a release
gate.

## Deferred work

- Full Confidential Sync interoperability and PIO remain separately gated.
- FTS5, RTree, SQLCipher, and JSONB are measured follow-ups, not initial
  dependencies.
- Governance records replace migrated local trust markers in the full
  Meadowcap management implementation.
- Android UI parity is not required for the first internal iOS TestFlight
  build, but the Rust database and UniFFI contracts must remain portable.
