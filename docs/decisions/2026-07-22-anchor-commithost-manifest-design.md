# Design: Serve `CommitHost` on `riot/anchor/1` + persist committed-manifest state (WU-BC)

Closes the two GATE STATUS known-risks in the anchor sync2-serving plan
(`docs/superpowers/plans/2026-07-22-anchor-sync2-serving.md`): the control plane
cannot commit, and no committed-manifest state exists for the ReadCommitted
equality check. Branch context: `feat/anchor-sync2-serving` after wu019
hardening — `CURRENT_SCHEMA_VERSION = 2`, so **migration 3 is claimed here and
the plan's Task D1 `anchor_secrets` migration renumbers to 4**.

## 1. Production `HostingAuthority::resolve_manifest` — the data flow

Three artifacts converge at CommitHost; none is operator say-so:

1. **The root-signed ticket** arrives once, at PrepareHost
   (`PrepareHostV1.root_signed_ticket_core: RootSignedTicketCoreEnvelopeV2`,
   riot-anchor-protocol control.rs:1467-1474). Its `PublicSiteTicketV2Core`
   (records.rs:121-146) carries `root_id`, the O/C/W namespace ids,
   `manifest_digest`, `manifest_version`, `manifest_required_transport`,
   `transport_floor`, `transport_epoch`, `expiry_unix_seconds`.
2. **The manifest itself** is a signed Willow entry at the reserved path
   `/manifest` in the O namespace (riot-core site/validate.rs:162-169,
   `MANIFEST_COMPONENT` re-export willow/mod.rs:40). The host pushes it via the
   sync/2 `HostReconcileStaged` staged data, so at commit time it is in
   `staged_entries` (or already-committed `entries` on a refresh). Matches spec
   §Composite transaction ordering ("O is read and the manifest admitted before
   exact C and W routing", spec:1246-1248) and hosting.rs:10-11's module
   contract.
3. **`CommitHostV1` carries neither** — only `operation_id` + three digests
   (protocol control.rs:1478-1483; spec:1402-1407).

Consequence: **the ticket must be persisted at PrepareHost time** — nothing
else can supply it at commit, and re-accepting a client-supplied ticket at
commit would let a host swap tickets (epoch-floor/expiry bypass). Today it is
dropped after the epoch-floor read (control.rs:387-388, :491-494).

**Migration 3** (forward-only): nullable column on `operations`:

```sql
ALTER TABLE operations ADD COLUMN ticket_envelope_bytes BLOB
    CHECK (ticket_envelope_bytes IS NULL OR
           (length(ticket_envelope_bytes) > 0 AND length(ticket_envelope_bytes) <= 896));
```

(896 = `MAX_TICKET_CORE_BYTES` + 128, mirroring listing.rs:428-432.)
`NewPreparedOperation`/`StoredOperation` gain the field; `insert_operation` /
`load_operation` read/write it; `handle_prepare_host` stores
`encode_canonical()` bytes in its existing transaction. NULL at commit
(pre-migration row) fails closed as `invalid_operation_authority`.

### resolve_manifest steps

Trait signature changes to receive the open transaction read-only (TOCTOU
otherwise; the current trait is barred from the store yet both inputs live
there):

```rust
fn resolve_manifest(
    &self,
    tx: &RepoTransaction<'_>,
    plan: &HostPlanView,
    observed_at: u64,
) -> Result<ManifestAuthorization, ControlRefusal>;
```

Production impl `TicketManifestAuthority` (new, hosting.rs, default build):
1. `tx.load_operation` → `ticket_envelope_bytes` → `decode_canonical::<RootSignedTicketCoreEnvelopeV2>`; None/failure → `InvalidOperationAuthority`.
2. Locate `/manifest` candidates in staged ∪ committed O: filter
   `tx.staged_entries` by `path_bytes == manifest_path_bytes()`
   (deterministic encoding, sync_service.rs:245-254) plus a new
   `tx.committed_entries_by_path(namespace_id, path_bytes)` query
   (`entries.path_bytes` column exists, schema.rs:138).
3. For each candidate: `verify_anchor_item_parts` (sync_service.rs:180-210 —
   binds payload↔entry) then `validate_site_manifest(&signed, &plan.community_root)`
   (riot-core validate.rs:146-222 — zero-delegation keystone). Select the one
   whose `manifest_coordinates(..).manifest_digest == core.manifest_digest`
   (authority.rs:174-207). No match → `CommitManifestMismatch`.
4. **Canonical gate with manifest attached**: `admit_public_site_ticket(&envelope,
   Some(&validated), &TransportFloor::RequireNone, &TicketFloor { root_id,
   highest_transport_epoch: tx.highest_ticket_transport_epoch(..)? }, observed_at)`
   — mirror of the sibling PrepareHost call (daemon.rs:288-298) but with
   `Some(manifest)`; gate step 7 (authority.rs:295-313) is the ONLY legal place
   for ticket↔manifest coordinate/transport equality.
5. **Manifest rollback floor**: `manifest_floors` (schema.rs:109-114); floor
   above `core.manifest_version` → `ManifestEquivocation`.
6. Return `ManifestAuthorization { community_id: core.o_namespace_id,
   full_site_root: core.root_id, manifest_digest, manifest_version,
   ordered_namespaces: [o,c,w], manifest_bytes: <validated canonical payload> }`
   (new `manifest_bytes` field). Existing routing assertion hosting.rs:315
   closes the loop. Identity note: `manifest.root == O id` (validate.rs:196)
   and `core.root_id == manifest.root` (authority.rs:304).

Refusal mapping (closed CommitHost matrix, spec:586-598):

| Failure | ControlRefusal |
|---|---|
| ticket missing/undecodable on operation row | `InvalidOperationAuthority` |
| no digest-matched validated `/manifest` | `CommitManifestMismatch` |
| `validate_site_manifest` failure | `InvalidOperationAuthority` |
| `AuthorityError::ExpiredTicket` | `TicketExpired` |
| `AuthorityError::ManifestTransportMismatch` | `ManifestTransportMismatch` |
| `AuthorityError::ManifestMismatch` | `CommitManifestMismatch` |
| other `AuthorityError` | `InvalidOperationAuthority` |
| floor rollback | `ManifestEquivocation` |

All are terminal-cleanup refusals (hosting.rs:300-313, :533-574).
`commit_capacity` stays `Ok(())` (parity with deferred capacity accounting).

## 2. Committed-manifest persistence — decision: populate `manifests` (+ `manifest_floors`) inside the composite commit transaction

- No migration needed for the write path — both tables exist since migration 1
  (schema.rs:100-107, :109-114) and are the spec's named tables (spec:2342).
  FK to `communities` satisfied because `commit_generation_cas` writes the
  community row earlier in the SAME transaction (repository.rs:1759-1767) —
  ordering: CAS (hosting.rs step 7) before manifest rows.
- Crash-atomic with promotion/receipt/token-invalidation/idempotency
  (hosting.rs:441-469, module contract :30-34).
- Single-writer preserved (actor thread only); ReadCommitted does a point read.
- Rejected: column on `communities` (extra migration, loses bytes needed by
  SubmitListing equality spec:1427-1430 and Slice-4/5); decoding
  `hosting_receipts` (no latest-per-community key, couples reads to receipt
  retention).

New repository methods: `upsert_manifest`, `advance_manifest_floor` (monotonic,
mirrors advance_ticket_transport_epoch), `manifest_floor`, `committed_manifest`
(latest by generation), `committed_entries_by_path`. Optionally also write
`public_site_tickets` in the same commit transaction (community row now
exists) — pre-provisions SubmitListing.

## 3. Control-plane CommitHost arm

Dispatch currently falls through at control.rs `_ => ProtocolFailure(Unsupported)`
(pinned by control_edges.rs:424-446 — that pin is REPLACED).

- Composition: `AnchorControlService<P, S>` gains field
  `hosting: CommitHostService<TicketManifestAuthority, S>` (concrete authority,
  no third type param; `CommitHostService<A, S>` stays generic for unit tests).
  Needs `S: Clone` (derive Clone on `Ed25519OperatorSigner` — SigningKey is
  Clone — and test `TestSigner`). `CommitHostContext` projected from
  `AnchorControlContext`.
- **Descriptor-epoch coherence (load-bearing)**: `install_persisted_descriptor`
  mutates epoch/digest after construction (control.rs:213-237). Add
  `CommitHostService::set_descriptor(epoch, digest)` and call it from
  `install_persisted_descriptor`, else receipts stamp stale coordinates.
- The arm: pre-claim `control_request_digest` (as PrepareHost, control.rs:281),
  call `self.hosting.commit(repo, &idempotency_key, body,
  &control_request_digest, now, entropy, &mut no_failpoint)`; map
  `CommitError::{Repository, Codec}` to `ControlError`; `MalformedPlan` →
  `Codec(NonCanonical)` fail-closed. Idempotency, transaction boundaries, and
  the refusal dispositions are already fully implemented inside
  `CommitHostService::commit` (replay/conflict/novel via idempotency.rs;
  one `repo.begin()` per disposition; StaleBase/SnapshotMismatch/
  OperationExpired/OperationNotFound routing) — the arm adds no new machinery.
- Wiring: `assemble_service` threads retention; `hosting_common::control_service()`
  updates; other unknown ops keep `Unsupported`.

## 4. Canonical-gate compliance — reuse list + named traps

MUST reuse: `admit_public_site_ticket` **with `Some(manifest)`** (mirror
daemon.rs:288-298); `validate_site_manifest`; `verify_anchor_item_parts`
(candidate binding) and `verify_anchor_item` (promotion, hosting.rs:488);
existing frame decode steps; idempotency.rs classify/claim.

Traps (defect-class #76/#90 — adversarial review must check each):
1. **"Digest equality is enough"** — skipping `validate_site_manifest` skips the
   zero-delegation keystone (validate.rs:62-65, :179-183): a delegated owned cap
   covering `/manifest` passes ordinary admission but must never authorize the
   manifest (delegated editor seizes C/W routing).
2. **"validate_site_manifest verifies enough"** — it does NOT bind payload bytes
   to the entry's payload digest (validate.rs:150-155 bounds lengths only).
   Only `verify_anchor_item_parts` binds payload↔entry. Both, in that order.
3. **"Let the client re-present the ticket at commit"** — breaks the wire
   contract and enables ticket substitution. Source is EXCLUSIVELY the
   operation row persisted at PrepareHost.
4. **Hand-comparing ticket↔manifest coordinates** instead of `Some(manifest)`
   into the gate — a 6-field compare silently drops the
   `manifest_required_transport` check.
5. **Skipping expiry/epoch re-check at commit** — the gate call with `now`
   re-enforces both; `ticket_expired` is a legal CommitHost refusal (spec:596).
6. **Omitting the manifest floor** — allows replaying an older root-signed
   manifest+ticket pair to roll the site back (spec:2342 floors).

## 5. TDD test plan

New fixture `make_site_fixture()` in hosting_common: real owned site
(`OwnedRoot`/`OwnedMasthead`, willow/mod.rs:38-39), `/manifest` entry signed
via `owner_write_capability`/`authorise_owner_entry` (masthead.rs:63/:183),
`encode_item`, `manifest_coordinates`, root-signed matching
`RootSignedTicketCoreEnvelopeV2` (pattern: authority_records.rs:532-575).
`insert_prepared_operation` gains `ticket_envelope_bytes: Option<Vec<u8>>`.

Schema/repository units:
1. `version_two_database_upgrades_to_operation_ticket_column`
2. `prepared_operation_round_trips_ticket_envelope_bytes` (incl. NULL)
3. `manifest_floor_is_monotonic`
4. `committed_manifest_returns_the_highest_generation`

Hosting units:
5. `commit_resolves_manifest_from_staged_o_entry_and_promotes` (happy path; receipt fields; manifests+floors rows)
6. `commit_refuses_when_no_manifest_entry_is_staged_or_committed` → CommitManifestMismatch + terminal cleanup
7. `commit_refuses_a_delegated_signer_manifest` → InvalidOperationAuthority (TRAP 1)
8. `commit_refuses_a_payload_swapped_manifest_item` (TRAP 2)
9. `commit_refuses_an_expired_ticket_at_commit_time` → TicketExpired (TRAP 5)
10. `commit_refuses_manifest_version_rollback_via_the_floor` → ManifestEquivocation (TRAP 6)
11. `commit_without_a_persisted_ticket_fails_closed` → InvalidOperationAuthority
12. `commit_replay_returns_byte_identical_receipt_without_new_manifest_rows`
13. `commit_failpoint_before_manifest_row_leaves_no_partial_state` (failpoint label "manifest")

Control routing:
14. `commit_host_routes_to_the_composite_service` (replaces the Unsupported pin)
15. `prepare_host_persists_the_ticket_envelope`
16. `commit_receipt_reflects_the_persisted_descriptor_epoch` (pins set_descriptor)

E2E: folds into Task C2's push→commit→pull test — the pushed O set includes the
fixture `/manifest`; plus a fail-closed leg: push WITHOUT a manifest →
CommitHost refuses `commit_manifest_mismatch` over the wire, subsequent
ReadCommitted serves nothing. Task C0's manifest-equality sub-case (e) reads
`committed_manifest`.

## 6. File scope + size

| File | Change | ~LOC |
|---|---|---|
| schema.rs | MIGRATION_THREE, version 3, test | 50 |
| repository.rs | ticket column plumbing; manifest methods; tests | 180 |
| hosting.rs | trait tx param; manifest_bytes; TicketManifestAuthority; persistence in commit(); set_descriptor | 260 |
| control.rs | CommitHost arm; hosting field; prepare-time ticket persistence | 70 |
| config.rs + daemon.rs | retention threading; Clone on signer; alias | 25 |
| tests/hosting_common/mod.rs | make_site_fixture; TestAuthority/TestSigner updates | 170 |
| tests/* | tests 5-16 + mechanical updates | 450 |
| tests/daemon_e2e.rs | manifest in C2 fixture + fail-closed leg | 60 |

No changes to riot-anchor-protocol, riot-core, riot-transport, Cargo.lock.
Sequencing: schema/repository → hosting trait + authority → control arm → e2e.
**Task D1 renumbers its migration to 4 / version 4.**
