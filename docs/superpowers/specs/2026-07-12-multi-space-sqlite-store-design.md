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

## Complete-design outcomes

Success requires all of the following:

- Creating or joining a second space preserves the first after process death.
- Owned, communal, and joined namespaces remain distinguishable.
- Space listing and switching do not replay all historical bundles.
- The same app ID has independent documents and approval state per namespace.
- A verified local write or import updates Willow bytes and its document
  projection in one transaction.
- Legacy migration is idempotent, preserves its source until verification, and
  never substitutes a blank database for unreadable data.

The slice fails if any test observes cross-namespace data, if a valid legacy
record disappears, if startup depends on Swift replaying receipt arrays, or if
corruption silently creates a fresh empty database.

Quick Poll, the other three apps, legacy migration execution, physical-phone
rehearsal, and TestFlight delivery have their own dependent slice gates below;
they are not required to declare the SQLite store slice complete.

## Conference-critical MVP boundary

The conference build must deliver one durable vertical slice, not every
production-hardening item in this document. Its release-blocking scope is:

- Open the local database without data loss. Legacy JSON migration is designed
  here but is post-conference because the new bundle cannot access the old
  development container.
- List, open, and switch between at least two retained spaces: one created on
  the device and one joined or communal space.
- Preserve each space's entries, app approval state, and app documents through
  process termination and relaunch.
- Round-trip one representative app document independently in each namespace.
- Sync the demonstrated spaces over Riot's existing confirmed nearby transport:
  CoreBluetooth discovery and framing, with its validated local Wi-Fi TCP
  handoff when available and BLE fallback otherwise. This makes no Confidential
  Sync interoperability claim.
- Import an existing-transport bundle into its exact namespace, reopen the
  database, and prove both namespaces and documents remain isolated.

For this build, destructive space deletion, leave, purge, database compaction,
SQLCipher, legacy JSON migration execution, background multi-namespace sync,
generalized JSON queries, FTS5, RTree, Android UI integration, four-app behavior,
physical-phone rehearsal, TestFlight packaging, and production-scale recovery
UX are deferred to their declared dependent slices.
Riot may hide/archive a space only by changing local metadata; doing so never
deletes its namespace, entries, capabilities, app approvals, or documents.

Provisional release budgets on a contemporary physical conference iPhone are:

- Cold launch to a usable space list with the demo database: under 2 seconds.
- List 50 spaces: under 100 milliseconds after database open.
- Switch to a cached space and show its initial document list: under 300
  milliseconds.
- Open a verified starter app and show its initial state: under 500
  milliseconds.
- Reflect a committed local app write in the visible WebView: under 200
  milliseconds after transaction commit.

Measurements include median and worst observed time across ten release-build
runs. A missed budget blocks performance claims but does not permit data loss or
namespace leakage as a shortcut.

## Architecture

### Ownership boundary

Rust owns persistence. Swift and Kotlin are clients of versioned UniFFI APIs;
they do not serialize the authoritative space database.

`RiotDatabase` owns the SQLite database, schema migrations, transactions, and
change feed. `SpaceSession` binds calls immutably to an exact namespace,
receiver/subspace signer, app ID when applicable, and approval generation.
`WillowRepository` verifies and persists canonical entries and authorization
material. `DocumentStore` exposes bounded JSON `put`, `get`, `list`, and
`watch` operations over accepted entries.

An app bridge is constructed only from `(SpaceSession, app_id)`. JavaScript
never supplies a namespace, SQL, filesystem path, capability, or arbitrary JSON
query expression.

### Physical database

One protected `riot.sqlite` contains:

| Table | Purpose |
| --- | --- |
| `schema_migrations` | Applied, ordered, transactional schema versions. |
| `spaces` | Full namespace ID, namespace kind, owned/joined relationship, display metadata, and local lifecycle state. |
| `space_identities` | Namespace-specific public signer, opaque secure-storage reference, relationship, and staged lifecycle; never secret bytes. |
| `namespace_roots` | Owned-root public key and opaque root-signer reference when this device is custodian; absent for joined roots. |
| `sealed_signers` | Domain-separated encrypted Ed25519 seed envelope, envelope version, public key, signer role, and secure wrapping-key reference. |
| `capabilities` | Canonical capability bytes, fingerprint, namespace, receiver, mode, area, lineage, and policy state. |
| `accepted_entries` | Permanent identity, coordinate, canonical entry/auth bytes, first receipt, arrival disposition, and provenance. |
| `live_entries` | Current non-pruned Willow view, retained payload, full coordinate, and accepted-entry reference. |
| `entry_path_prefixes` | Materialized exact path-prefix keys for indexed `Area` and prefix queries. |
| `import_receipts` | Immutable route, dispositions, provenance, and monotonic receipt sequence. |
| `forgotten_entries` | Durable local forget marker and generation for an accepted entry that is intentionally absent from the live view. |
| `documents` | Rebuildable namespace/subspace/path-scoped JSON projections referencing an exact live source entry. |
| `app_packages` | Global content-addressed manifest and bundle cache keyed by full app ID. |
| `space_app_state` | Namespace-specific app approval/trust/materialized availability. |
| `local_state` | Selected namespace and device-only preferences; never authority. |
| `change_log` | Monotonic durable sequence filtered by namespace/app/collection, with bounded retention and resumable watchers. |
| `migration_journal` | Source fingerprint, phase, counts, verification result, and completion marker. |
| `migration_quarantine` | Source kind/index/fingerprint, bounded retained record bytes, reason, retry state, and resolution. |

All identifiers are fixed-width blobs. Every space-scoped primary and foreign
key starts with `namespace_id`. Entry identity is the existing domain-separated
digest of canonical entry bytes. `accepted_entries` retains the permanent seen
index and provenance after pruning; `live_entries` alone participates in Willow
queries, projections, and sync. Pruning removes live payload/projection rows but
never the seen ID or immutable receipt. Local forgetting is recorded separately
from protocol pruning in `forgotten_entries`. Forgetting removes the live row,
payload, and projections but preserves accepted identity and receipts. Importing
the exact entry again—locally or from a peer—clears the marker and restores it
when it is not pruned, recording `RestoredAfterForget`; an ordinary live
duplicate remains `AlreadyPresent`. Restart reconstructs live state from
`live_entries` minus no implicit inference: every intentional absence is
durably accounted for.

Paths retain canonical components plus a deterministic component-boundary
encoding. `entry_path_prefixes` materializes every exact ancestor prefix as
`(namespace_id, entry_id, depth, prefix_bytes)`. Area queries use indexed prefix,
optional subspace, and timestamp bounds. Willow `u64` timestamps are stored as
eight-byte big-endian blobs so the complete unsigned range sorts correctly;
recency ties use digest and payload length exactly as Willow specifies. Required
indexes cover full coordinates, prefixes, namespace/subspace/time,
namespace/collection/document/path, capability receiver/fingerprint, and app ID.
Constraints enforce identifier widths, payload length versus actual blob length,
canonical entry/namespace consistency, immutable package hashes, and projection
foreign keys. Existing retained-store, path, payload, receipt, and namespace
ceilings remain blocking; moving to disk does not create unbounded acceptance.

The initial dependency is pinned `rusqlite =0.40.1` with default features off
and `bundled`, `backup`, `blob`, `hooks`, `limits`, and `serde_json` enabled.
Bundling gives iOS and Android one SQLite and JSON feature set. Work unit zero
must prove this exact configuration builds and opens a database for iOS arm64
and the arm64 simulator before schema implementation begins.

SQLite uses WAL mode, foreign keys, a bounded busy timeout, explicit
transactions, and controlled checkpoints. One thread-confined writer connection
belongs to a serialized Rust database worker; bounded read connections serve
snapshots. UniFFI sends typed commands rather than sharing a `Connection`, and
native callers work off the UI thread.

The conference repository implements Riot's existing inspect/plan/commit and
payload-query surface over SQLite. It does not claim the complete pinned
`willow25::storage::Store` trait. Live-set behavior is differentially tested
against `willow25::MemoryStore` for insert, recency, pruning, multi-namespace
isolation, area/prefix queries, payload access, and forgetting. A complete
`Store` adapter, payload slicing, and Confidential Sync integration are a
separately gated follow-up; the schema must not require a second canonical path.

The production cutover is explicit and one-way within this slice:

1. Extract current canonical verification plus Willow join/pruning behavior into
   pure testable services without changing outcomes.
2. Implement `SqliteEvidenceStore` behind the existing inspect/plan/commit
   contract and make accepted/live/receipt updates one transaction.
3. Make each `SpaceSession` own its namespace-scoped signer, app runtime, and
   sync inventory.
4. Switch UniFFI/native callers to SQLite and run restart plus differential
   tests.
5. Remove the in-memory implementation from production construction; retain it
   only as a differential test oracle.

No production write commits to memory and later mirrors asynchronously to
SQLite. During development exactly one backend is selected at construction, and
the release configuration permits only SQLite.

### JSON projection

Canonical signed Willow entry bytes remain authoritative. JSON documents are
query projections and can be rebuilt. Native record families have deterministic
CBOR-to-JSON projections. App payloads must be valid bounded UTF-8 JSON.

System collections have closed versioned schemas. App collections accept any
valid JSON within host limits but remain restricted to the app's namespace and
app ID. Projection identity includes the full Willow coordinate—namespace,
subspace, and path—plus source entry ID. A typed resolver may derive a
product-level winner; storage never collapses different subspaces. JSON is text
initially for stable WebView interchange. SQLite JSONB may later be only a
derived optimization.

The accepted-write transaction is:

1. Decode canonically and enforce resource limits.
2. Verify canonical cryptography and bounded input outside the database lock.
3. Begin the serialized SQLite transaction and revalidate namespace, receiver,
   capability lineage, revocation/policy snapshot, source recency, and
   session/approval generation against the committed state being mutated.
4. Insert/update the canonical Willow entry and apply Willow recency/prefix
   pruning semantics within that namespace and subspace.
5. Update or remove deterministic document and app-state projections.
6. Append namespace/collection change records.
7. Commit, then publish notifications. Any failure rolls back every step.

### Spaces, identity, and capabilities

The space registry contains both spaces created locally and spaces joined from
others. Ownership is derived from namespace kind plus available root authority,
not from who selected or first displayed the space. The conference MVP proves
multiple created and joined communal spaces. The schema admits owned roots and
delegated receivers, but full owned Meadowcap creation/management remains behind
the separately approved conformance work.

There is no namespace-bound global `EvidenceAuthor`. Each space identity stores
its namespace, exact authoring subspace or receiver public key, role
(`communal-participant`, `owned-root-custodian`, or `delegated-member`), and an
opaque secure-storage signer reference. Owned roots have a separate root
reference; joined spaces never acquire it. Uniqueness is enforced over
`(namespace_id, receiver_or_subspace_id, signer_role)`.

The current 112-byte `sealedIdentity` (namespace plus encrypted subspace secret)
migrates only into the namespace it names and becomes that space's signer; it is
never promoted to a global identity. Keychain and SQLite changes use a staged
state machine with a random operation ID: create an inactive secure item, commit
a pending database reference, activate the item, mark the reference active,
then garbage-collect abandoned items. Startup reconciles incomplete stages and
fails closed when a referenced key is unavailable. If encrypted legacy identity
exists, a missing wrapping key enters recovery; `loadOrCreate` cannot generate
a replacement. Secret FFI buffers are zeroized.

The executable conference signer contract uses exportable Ed25519 seeds wrapped
at rest, because iOS Keychain generic-password records do not sign and Secure
Enclave does not provide Ed25519. Keychain stores a random 32-byte wrapping key
with `WhenUnlockedThisDeviceOnly`; SQLite stores only the versioned authenticated
sealed seed envelope and its wrapping-key reference. For a signing command,
native code loads the wrapping key into mutable memory and passes that key—not
the Ed25519 seed—through UniFFI to the Rust database worker. Rust authenticates
and unwraps the selected space envelope into a `Zeroizing<[u8; 32]>`, constructs
a transient Ed25519 signer, signs, and drops/zeroizes both signer and wrapping
key before returning. No plaintext signer seed crosses FFI or persists in Rust,
Swift, SQLite, logs, or temporary files. Locked/unavailable Keychain returns a
typed `KeyUnavailable` and performs no write.
The envelope's associated data binds format version, namespace, public key,
signer role, and operation record so ciphertext cannot be transplanted between
spaces or roles. Swift overwrites its mutable wrapping-key buffer on every
success and error path immediately after UniFFI returns.

Legacy conversion authenticates and opens the current envelope in Rust and
reseals it under the new domain/version in the same bounded operation; plaintext
exists only in zeroizing Rust memory. Communal subspace signing uses this
contract now. Future owned-root and delegated-receiver signing uses distinct
sealed envelopes and roles through the same service, but feasibility of a truly
non-exportable platform signer remains part of the full-management conformance
spike rather than a conference claim.

Space selection changes only the active `SpaceSession` and visible queries.
It does not create, copy, re-import, or delete namespace data. Sync binds areas
and capabilities per namespace and may reconcile several shared namespaces in
one peer relationship when the transport supports it.
The launch-space list reads only `spaces` plus `local_state`; it never joins
entries, documents, or payloads, preserving the 50-space startup budget.

### Apps

App packages are globally content-addressed cached bytes. Whether an app is
approved, available, or revoked is namespace-specific. App data paths retain
their Willow `apps/<app_id>/...` shape inside the selected namespace, which
naturally isolates identical app IDs between spaces.
Starter packages are held globally and appear as cards in every space, but they
do not auto-approve or auto-mount. Each namespace gets an explicit
`space_app_state` approval before its bridge can open.

The conference starter catalog contains four real tools: Checklist, Supply
Board, Roll Call, and Quick Poll. Each must use the same bounded document API,
persist across restart, and update after remote sync. Quick Poll stores the
question under a unique poll ID and one vote document per `(poll_id, verified
authoring_subspace)`. The host derives and appends the voter component from the
authenticated `SpaceSession`; JavaScript cannot nominate another voter. A
path/payload claim that differs from the verified entry subspace is rejected.
Changing a vote replaces that author's live vote by Willow recency; tallies
count one resolved live vote per authoring subspace. This is one-key-one-vote,
not proof of one biological human.

Every app operation presents an opaque session generation bound to namespace,
app ID, receiver/subspace, and approval generation. The transaction rechecks
it. Space switch, revocation, logout, or closure invalidates the generation,
closes the WebView bridge, and cancels namespace/app/collection-filtered watches.
No bridge resolves through a mutable global selected-space value.

Minimum functional contracts for the dependent app/release slice are:

- Checklist adds an item, toggles it, attributes the current actor by stable ID,
  and preserves/syncs the resulting state.
- Supply Board posts a need or offer, claims it, releases it, resolves actor IDs
  at render time, and preserves/syncs the resulting state.
- Roll Call maintains one current check-in per receiver, updates rather than
  duplicates a repeated check-in, resolves names at render time, and
  preserves/syncs the resulting state.

### Versioned native and app API

UniFFI exposes opaque handles and versioned DTOs, never SQL or mutable global
selection:

- `RiotDatabase.open(path, wrapping_key) -> DatabaseOpenResult` returns exactly
  one launch/recovery state below.
- `list_spaces(include_archived) -> SpacePage` uses opaque full IDs,
  relationship (`created`, `joined`, `communal`, with protocol kind separately),
  availability/key/sync status, last-change sequence, and a bounded cursor.
- `create_communal_space(title)`, `import_join(bytes)`, `archive_space(id)`, and
  `unarchive_space(id)` return typed outcomes. No destructive delete API ships
  in the conference build.
- `open_space(id) -> SpaceSession` returns an immutable namespace/generation
  handle or a typed unavailable/importing/key-locked error.
- `SpaceSession.list_apps(cursor, limit)`, `approve_app(app_id, version)`, and
  `open_app_session(app_id) -> AppSession` keep approval namespace-local.
- `AppSession.get_document`, `list_documents(collection, prefix, cursor, limit)`,
  and `put_document(collection, document_id, json, precondition)` use closed
  collection names plus the existing canonical path-segment grammar.
  Preconditions are `Any`, `IfAbsent`, or `IfSequence(u64)`.
- A committed write returns `CommittedDocument { document_id, key, json,
  author_id, source_entry_id, sequence }`, allowing immediate paint without an
  optimistic guess.
- `changes(after_sequence, collection, prefix, limit) -> ChangePage` returns a
  durable resume sequence and reset-required marker when bounded retention has
  passed the caller. Watches are cancelable and invalidated with the session.

All pages have a fixed maximum size and opaque stable cursors bound to database
generation, namespace, query, and snapshot sequence. FFI database calls execute
on the Rust worker; Swift invokes them from an async/background task and publishes
typed results on the main actor. WebView promises resolve through structured
serialization, never interpolated JavaScript strings.

Errors are a closed enum: `InvalidInput`, `NotFound`, `Unauthorized`,
`SessionStale`, `KeyUnavailable`, `SpaceUnavailable`, `Importing`, `Conflict`,
`BusyRetryable`, `StorageFull`, `CorruptDatabase`, `MigrationRequired`, and
`Internal`. Native and app bridges map them to fixed product copy and a recovery
class (`retry`, `unlock`, `reopen`, `review`, or `support/export`); raw SQLite,
Willow, cryptographic, capability, path, and identifier details never enter app
UI.

### Space and recovery experience

Launch has an explicit state machine:

```text
opening -> migrating -> ready
                     -> ready_with_quarantine
                     -> recovery_interrupted
opening/migrating    -> recovery_missing_key
                     -> recovery_corrupt_database
retry                -> migrating | ready | ready_with_quarantine | retry_failed
```

`ready_with_quarantine` is permitted only when all identity, namespace,
capability, and accepted live records for at least one space verified; the
affected records/spaces remain unavailable and a persistent recovery summary
shows migrated, quarantined, and failed counts. `Retry` reprocesses unchanged
source bytes idempotently. `Export` creates a bounded diagnostic/recovery
artifact without secrets. `Continue` opens only verified spaces and never marks
quarantine resolved. `Reset` and destructive discard do not exist in this
release. Missing key or corrupt database blocks affected data behind recovery
and preserves every source file. Interrupted migration resumes/restarts from its
journal; retry failure returns to the same recoverable state.

The persistent space switcher lists created and joined spaces, relationship,
protocol kind, archived state, sync status, and key availability without opening
payload tables. Create and join/import append spaces. Archive/unarchive is local
visibility metadata only. Opening an importing or missing-key space shows its
typed status without changing selection. If the selected space is archived or
unavailable, Riot chooses the most recently opened available space, otherwise
the chooser.

Switching while a WebView is open first dismisses it to the destination space
home and invalidates the old bridge. Conference apps commit on explicit submit
or tap; unsubmitted form text is ephemeral. The app host prompts before leaving
when the page has registered a dirty form, otherwise closes immediately. It
never silently reopens the same app in another namespace. Revocation closes the
app immediately and explains that the tool is no longer enabled here.

Loading uses progress with accessible labels; empty states distinguish no
spaces, no approved tools, and no documents. Offline is normal status, not an
error. Non-destructive sync failure keeps last verified content visible.
Validation/conflict offers edit-and-retry, busy retries with a bound, key-locked
offers unlock, permission denial returns to app review, and corruption/migration
errors enter recovery rather than showing an empty board.

For the conference MVP, app approval is explicitly a local per-device,
per-space, exact-app-version decision; it is not presented as shared organizer
governance. The review sheet shows the fixed manifest permissions, Enable and
Not now. Denial leaves the card disabled. Phone A's demo fixture contains
previously persisted local approvals for all four starter versions; Phone B
manually enables Quick Poll during rehearsal/demo. Shared signed governance
approval remains part of the full Meadowcap management implementation.

## Legacy migration

Migration recognizes the current protected `profile.json`, enforces byte/depth
limits, fingerprints the exact bytes, and writes a temporary SQLite database.
Its mapping is deterministic:

| Legacy field | Destination and rule |
| --- | --- |
| `space` | One `spaces` row; it does not imply owned-root authority. |
| `sealedIdentity` | One pending space signer for the namespace embedded in the envelope; missing/wrong key blocks activation. |
| `alerts[].bundle` | Verify every entry, derive namespace from canonical bytes, and import accepted/live state plus receipt. |
| `appDataBundles[]` | Verify every entry and derive namespace/app ID from it; order is retained only as receipt provenance. |
| `carriedApps[]` | Reverify manifest/bundle association and deduplicate immutable packages by full app ID. |
| `trustedAppIDs[]` | Scope only to `space.namespaceID`; without a valid space, quarantine rather than infer. |
| `demoBundle` | Verify and derive its namespaces; disagreement with `space` adds registered namespaces rather than rewriting IDs. |

Records that verify but cannot be assigned safely stay in migration quarantine
with source kind/index, source-file fingerprint, a fingerprint of the extracted
record, bounded retained bytes, reason, retry count/state, and resolution. A
unique constraint on `(source_fingerprint, source_kind, source_index)` makes
retries idempotent. Expected/actual accounting includes accepted and quarantined
records. The post-conference migration UI shows a recovery summary and supports
retry/export; fine-grained record editing is deferred. Migration is not reported
complete until the recovery state accounts for every source record.

The migration records expected/actual counts. It builds in the destination
directory, checkpoints WAL, closes every connection, syncs where supported,
applies and verifies protection on database and sidecars, atomically renames the
closed main database, reopens it, and verifies counts plus `quick_check`. The
JSON source remains a recovery backup until a later explicit cleanup release.
The checkpoint uses `TRUNCATE`, and cutover verifies no required transaction
remains only in WAL before the rename.
Interruption is safe: an incomplete database is discarded or resumed according
to the journal, and re-running with the same source fingerprint is idempotent.
Malformed individual records are reported and block automatic source retirement;
they are never silently skipped while presenting migration as complete.

After a successful migration Riot selects the last valid selected namespace; if
none exists it selects the migrated legacy namespace; if neither exists it shows
the space chooser. Migration failure leaves the source untouched and shows a
retryable recovery state, never an empty space list.

TestFlight uses bundle ID `riot.protest.net`; development builds used
`net.protest.riot`. They have different containers and Keychain access groups,
so there is no false automatic cross-bundle migration claim. In-place migration
applies only to data accessible inside the new container. Moving old development
data requires explicit signed export/import or a previously provisioned shared
App Group/access group; neither exists today. Conference fixtures enter the new
app through normal verified import.

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
- A namespace-scoped session rejects mixed-namespace input. A multi-namespace
  transport splits input into explicit namespace transactions so failure in one
  cannot partially mutate or disclose another.
- Backups use SQLite's backup API, not live-file copying. An authenticated
  manifest records schema/database generation. Restored approvals and
  capability policy stay quarantined until revalidated, preventing rollback
  from silently resurrecting revoked authority.

## TDD and verification

Implementation follows red-green-refactor. The initial failing store-slice
tests establish the conference contract before database code exists:

Work unit zero first verifies or installs `cargo-tarpaulin` and
`cargo-llvm-cov`; absence is a blocking toolchain prerequisite, not permission
to skip the configured gates.

1. `second_space_survives_restart`.
2. `created_and_joined_spaces_coexist`.
3. `same_app_is_isolated_between_namespaces`.
4. `failed_import_rolls_back_entry_and_projection`.
5. `sync_for_one_namespace_cannot_mutate_another`.
6. `stale_app_session_fails_after_space_switch_or_revocation`.
7. `live_and_seen_match_memory_store_after_pruning_and_forgetting`.
8. `mixed_namespace_input_never_partially_commits`.

Post-conference/dependent-slice RED tests include
`legacy_migration_is_lossless_idempotent_and_interruptible`,
`corrupt_database_is_preserved_and_fails_closed`,
`keychain_failure_at_each_staged_transition_recovers_or_fails_closed`, and
`quick_poll_create_vote_move_tally_sync_restart`.

Unit tests cover schema constraints, projections, live-versus-seen provenance,
prefix pruning, JSON limits, static queries, signer binding, change filtering,
secure-item stages, backup rollback quarantine, and migration journaling.
Property tests generate namespace/subspace/path/document combinations proving
no query or mutation crosses namespaces. Differential tests compare live state
against `willow25::MemoryStore`. Native tests prove the UniFFI APIs. Existing
Rust, Swift, packaging, and app tests remain green. Each work unit records the
failing RED command/output before implementation and the GREEN output after.
The blocking Rust coverage source of truth remains `.coverage-thresholds.json`:

```sh
cargo test --workspace --all-features
cargo tarpaulin --fail-under 100
cargo llvm-cov --workspace --all-features --branch \
  --fail-under-lines 100 --fail-under-functions 100 \
  --fail-under-regions 100 --fail-under-branches 100
```

`cargo tarpaulin` remains mandatory even though it does not measure Swift.
RiotKit repository/bridge tests run separately with coverage enabled:

```sh
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -derivedDataPath build/ios-derived -enableCodeCoverage YES
xcrun xccov view --report build/ios-derived/Logs/Test/*.xcresult
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme Riot \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -derivedDataPath build/ios-ui-derived
```

Cargo coverage is never represented as Swift or packaging coverage. Packaging
is verified by the native artifact checks plus Release archive/link tests.

Performance fixtures measure cold open, listing dozens of spaces, opening a
space with thousands of documents, an indexed app list, and a bounded bulk
import. The conference budgets above are the initial thresholds; recorded
device evidence may tighten them but may not silently relax them.

### Store-slice conference gate

The SQLite slice passes when a Release-configured integration fixture creates
two communal namespaces, commits a different representative app document in
each, imports an existing-transport bundle into the matching namespace, closes
and reopens the database, and observes both spaces/documents with zero
cross-namespace rows. This gate does not require all four app UIs, physical
phones, migration, or TestFlight.

### Dependent app/release two-phone gate

After the app, navigation, and TestFlight slices land, the combined release
candidate passes only when this sequence succeeds on two physical iPhones using
Release builds:

1. Phone A launches with an existing locally created communal space and creates
   or joins a second space without losing the first. Phone B joins the
   demonstrated shared space.
2. Both phones list both applicable retained spaces and all four starter-app
   cards. Switching spaces shows distinct alerts and app documents.
3. In the shared space, Checklist add/toggle, Supply Board post/claim/release,
   Roll Call check-in/update, and Quick Poll create/vote/change/tally all work.
4. The phones confirm a nearby connection. App and content changes synchronize;
   no entry or app document appears in the other namespace.
5. Both apps are force-terminated and relaunched. Spaces, selected-space
   fallback, approvals, all four app states, and the final poll tally remain.
6. Airplane mode remains enabled with only the radios required for the confirmed
   nearby transport. No server or internet fallback participates.

## Deferred work

- Full Confidential Sync interoperability and PIO remain separately gated.
- FTS5, RTree, SQLCipher, and JSONB are measured follow-ups, not initial
  dependencies.
- Governance records replace migrated local trust markers in the full
  Meadowcap management implementation.
- Android UI parity is not required for the first internal iOS TestFlight
  build, but the Rust database and UniFFI contracts must remain portable.
