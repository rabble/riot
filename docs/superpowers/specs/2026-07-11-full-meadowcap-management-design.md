# Full Meadowcap management design

## Status

Approved in product brainstorming on 2026-07-11. This design supersedes the
fixed-organizer authority workaround described in the signed JS apps design.
It does not supersede the open-versus-managed Space model, the plural app
directory, or MLS confidentiality for private groups.

Normative external references:

- [Meadowcap](https://willowprotocol.org/specs/meadowcap/index.html)
- [Read Access and Confidentiality](https://willowprotocol.org/specs/pio/index.html)
- [Willow Confidential Sync](https://willowprotocol.org/specs/confidential-sync/index.html)
- [Willow Transfer Protocol](https://willowprotocol.org/specs/wtp/index.html)

The Meadowcap specification is final as of 2025-11-21. WTP remains a sketch;
the read-confidentiality/Confidential Sync work is also an upstream proposal,
not made final merely because Meadowcap is final. Riot therefore defines a
transport-independent authorization contract and makes no WTP or Confidential
Sync interoperability claim until each separate conformance gate passes.

## Purpose

Riot needs Meadowcap to be its real authority layer rather than merely an
encoded field accepted in a narrow import format. People must be able to
choose an open or managed Space, assign limited community roles, remove those
roles, control which apps operate in a Space, and carry the resulting
authority across nearby sync and portable files without relying on a Riot
server.

Meadowcap answers a narrow security question: **may this receiver read or
write this Willow area?** Riot governance answers the product questions:
**which role should a person have, which app powers did a community approve,
and which decisions are currently in force?** Keeping these layers separate
prevents moderation policy, UI roles, and protocol validity from becoming one
unreviewable authorization mechanism.

## Use cases

1. **Space creator:** WHO is a person creating a Space; WANTS to choose open
   communal publishing or managed collective authority; SO THAT the Space's
   power model is explicit; WHEN the Space is created, before anyone joins.
2. **Collective organizer:** WHO manages an owned Space; WANTS to grant a
   moderator, verifier, publisher, dispatcher, or app approver only the paths
   and time range needed for that role; SO THAT compromise has a bounded blast
   radius; WHEN responsibilities are assigned or renewed.
3. **Open-space participant:** WHO contributes to a communal Space; WANTS to
   retain control of their own subspace while optionally following a
   community-run curation lens; SO THAT open publishing never silently gains a
   root administrator; WHEN choosing how to read and participate in the Space.
4. **Community removing an organizer:** WHO controls a managed Space; WANTS a
   removed manager's future actions rejected after revocation reaches a peer;
   SO THAT removal has enforceable effect without rewriting history; WHEN a
   role holder leaves, loses trust, or loses a device.
5. **Space member:** WHO synchronizes protected data; WANTS a peer to disclose
   entries only after proof of control of the receiver key of a covering read
   capability; SO THAT possession of copied capability bytes is insufficient;
   WHEN establishing a replication session.
6. **Space app approver:** WHO equips a Space with shared tools; WANTS to enable
   an exact app version with only a subset of its requested powers; SO THAT
   sharing an app never silently authorizes it; WHEN reviewing or updating an
   app.
7. **Device owner:** WHO runs a Space-approved app; WANTS camera, location,
   photos, microphone, and similar private powers to require local consent; SO
   THAT community approval cannot override personal privacy; WHEN the app first
   requests a sensitive platform permission.
8. **Root custodian:** WHO creates or recovers a managed Space; WANTS the root
   key protected by platform secure storage and an encrypted recovery artifact;
   SO THAT normal management does not expose the Space's highest authority;
   WHEN issuing daily authority, restoring a device, or responding to loss.
9. **Disaster-network courier:** WHO carries a portable Space update; WANTS
   write authority verified on import and protected content refused from
   plaintext export; SO THAT sneakernet does not bypass authorization or
   confidentiality; WHEN moving data between disconnected communities.

## Scope

This design includes:

- Communal and owned Meadowcap capabilities.
- Read and write access modes.
- Canonical encoding, exact decoding, creation, delegation, attenuation, and
  verification.
- Restrictions by namespace, subspace, path, timestamp, and receiver.
- Receiver-authenticated read enforcement in the replication layer.
- Open and managed Space creation.
- Linked owned governance for optional shared management of an open Space.
- Role templates, coordinate expiries, renewals, delegation receipts, and signed
  revocations.
- Secure root and delegated-key custody plus encrypted recovery export.
- Capability-derived moderation, directory, app approval, and app data access.
- Native mediation of device permissions; no raw capability or secret-key
  access from app JavaScript.
- Audit, recovery, and namespace-migration behavior.

This design does not include:

- A quorum or voting engine. Governance records are shaped so threshold
  policies can be added later, but one holder of the relevant approval
  capability can act in the first release.
- Root-key rotation in place. The namespace ID is the root public key, so root
  replacement is a signed migration to a new namespace.
- Treating read authorization as encryption. Private groups continue to use
  MLS-protected artifacts.
- Claiming WTP or Confidential Sync interoperability before their dedicated
  protocol conformance work lands.
- A globally privileged app directory or Riot-hosted authority service.

## Core principles

1. **One authorization decision everywhere.** Local writes, imports, nearby
   sync, portable drops, app bridge operations, and future Willow transports
   call the same authority engine.
2. **Protocol validity is not community policy.** Meadowcap validates
   capabilities; Riot policy evaluates roles, revocation records, app grants,
   and Space profiles.
3. **Open means no hidden root.** A communal namespace cannot gain authority
   over other participants through a UI convention.
4. **Moderation is signed curation.** Normal product moderation publishes
   inspectable actions and reversals rather than pretending replicated data
   can be globally deleted.
5. **Attenuation only.** Delegation can never increase authority.
6. **Fail closed without leaking.** Invalid authority reveals neither protected
   data nor protected path existence.
7. **Human attribution survives app use.** Apps operate through the host;
   Willow writes are signed by the person performing the action.
8. **Offline conflicts remain evidence.** Reconciliation produces one
   deterministic current policy without erasing partition history.

## Architecture

The authorization path is:

```text
Space profile + governance records
                |
                v
       Riot policy evaluator
                |
                v
      Meadowcap authority engine
                |
                v
 Willow write admission / replication read gate
```

### Meadowcap core

A focused `riot-core` module wraps the pinned `willow25` implementation and
owns all protocol-level operations:

- Construct communal and owned read/write capabilities.
- Canonically encode and exactly decode capability values.
- Delegate to a receiver while restricting an existing area.
- Inspect access mode, receiver, namespace, granted area, and chain depth.
- Verify every namespace and user signature in a delegation chain.
- Verify an entry authorization token against its entry.
- Verify that a read request is covered by a read capability.
- Return the capability receiver separately from the entry's subspace
  coordinate. In an owned namespace those identities are not interchangeable.

The module contains no `Moderator`, `AppApprover`, or UI concepts. Consumers
receive typed facts and stable rejection codes, not string parsing access to
cryptographic objects.

### Riot policy evaluator

The policy evaluator combines a cryptographically valid capability with:

- Space profile and linked-governance configuration.
- Signed issuance, renewal, revocation, and migration records.
- The locally known governance frontier.
- Role-template constraints such as accepted paths, coordinate limits, and maximum
  management-chain depth.
- App-version grants and local device consent.

Every policy decision consumes an immutable `PolicySnapshot` identified by its
governance-frontier hash. Local writes, import inspection, commit, sync, and
app calls either use that exact snapshot or fail stale; none may silently
re-evaluate against a different frontier halfway through an operation.
Mutation previews fail stale and must be regenerated. Protected-sync and app
execution sessions close on *any* frontier-hash change before the next response
or bridge result, even when an optimization believes the change is unrelated;
clients reauthenticate and bind a new snapshot. The repository generation is
checked atomically immediately before encrypting/sending a protected record, so
a known revocation cannot continue on an old session.

Protocol support accepts arbitrary valid Meadowcap delegation chains. A Riot
Space profile may impose narrower operational limits without claiming that a
valid Meadowcap capability is malformed.

### Secure authority vault

The vault stores namespace root keys and delegated receiver keys behind a
minimal typed interface. Apple implementations use Keychain and Android
implementations use Keystore-backed secure storage. Secret keys and root
operations do not cross FFI as general byte arrays. Recovery export is
authenticated, encrypted, versioned, and explicitly initiated by the user.

Root keys are not used for daily entry signing. The root issues management
capabilities to delegated per-device receiver keys. A stable `actor_id` names
the person or collective role in governance; signed actor-binding records map
each device receiver key to that actor. Entry coordinates, receiver keys, and
stable actors are persisted and exposed as three distinct facts.

Loss of all root material without a recovery artifact is unrecoverable and
must be reported honestly. The concrete recovery envelope and platform access
controls are specified below.

### Admission engine

All entry ingestion follows the existing inspect/plan/commit boundary. The
verification order is pinned:

1. Canonical entry decode and Riot path limits.
2. Canonical capability decode and chain structural limits.
3. Entry payload length and digest.
4. Meadowcap chain, access mode, receiver signature, namespace, and granted
   area.
5. Riot Space policy, revocation, and migration state.
6. Payload schema and path-to-payload binding.
7. Resource budgets.
8. Atomic commit of the user-selected eligible set plus its governance index
   update.

Frame failures remain item-local during inspection, matching Riot's existing
bundle behavior: an invalid sibling is ineligible and does not poison valid
siblings. Once the user or sync policy selects eligible items, commit is
atomic for that selected set; a failure commits none of the selected items.
Local writes pass through the same checks before persistence. No local API is
allowed to construct a privileged entry and insert it beneath this boundary.

### Replication read gate

Protected synchronization uses a new versioned protocol implementing Willow's
Private Interest Overlap and Confidential Sync security model. The current
conference-sync `/1` `Hello` and `Summary` frames disclose namespace and entry
identifiers before authentication and are therefore forbidden for protected
areas. They remain a legacy public-only codec.

The protected setup state machine is `fresh -> handshake -> authenticated ->
private-overlap -> capability-bound -> reconciling -> closed`:

1. Both peers generate 32-byte random nonces and ephemeral keypairs and run
   the Willow'25 authenticated Diffie-Hellman handshake suite.
2. The signed transcript is domain-separated with
   `org.riot.protected-sync/1` and contains protocol version, initiator and
   responder roles, both ephemeral keys, both nonces, both receiver keys,
   negotiated cipher suite, the handshake hash, and a derived session ID.
3. Each peer proves possession of its receiver secret within that transcript.
   All read and enumeration capabilities it later binds must name that
   receiver. A peer may use a session-ephemeral receiver delegated from its
   per-device read authority.
4. Ordered AEAD records use monotonically increasing 64-bit sequence numbers;
   replay, gap, reordering, transcript mismatch, unknown version, downgrade,
   or key mismatch closes the session without a distinguishable protected-data
   response.
5. Peers exchange only session-salted private-interest hashes before overlap
   is proven. Read capabilities use confidentiality-preserving relative
   encodings and are disclosed only under the PIO overlap rules. Awkward
   overlaps require a valid receiver-bound enumeration capability.
6. A bound reconciliation range must lie inside both peers' relevant granted
   areas. The pinned policy snapshot is checked when binding; immediately
   before every entry or payload response the repository generation must still
   equal that snapshot or the session closes and rebinds from a new handshake.

Riot adopts PIO's documented confidentiality levels rather than claiming
impossible zero leakage. Unauthorized peers receive no entries or payloads,
and namespace/path/capability material is protected according to the PIO
private-interest and overlap rules. A malicious online peer that already
guesses a PrivateInterest may confirm limited overlap/existence information
and, in protocol cases documented by PIO, limited count information. Sensitive
namespace and path components therefore use random high-entropy identifiers;
human labels never become secret path components. The threat model and tests
pin the specification's L0/L1/L2 disclosures for normal, guessed, nested, and
awkward interests. Frame count, ciphertext length, timing, and disconnect shape
are padded or bucketed where the Confidential Sync profile specifies them and
covered by traffic-shape tests.

This transport-independent state machine is implemented for nearby sync
before protected sync is exposed. Future WTP and Confidential Sync adapters
must satisfy the same contract and their own conformance gates. Public areas
are served according to explicit Space visibility policy rather than
fabricating confidential read capabilities.

### App permission broker

App JavaScript never talks to Meadowcap or secure storage directly. The native
host computes an effective grant and exposes only approved bridge operations.
The effective grant is:

```text
manifest request
intersection Space app approval
intersection current Meadowcap authority
intersection device-owner consent
intersection platform restrictions
```

Launching creates an opaque, revocable `AppExecutionSession` bound to profile,
content namespace, optional linked governance namespace, exact app ID and manifest digest, WebView instance and origin,
navigation generation, effective grant, actor, receiver key, and policy
snapshot. Bridge operations accept only that opaque handle and relative
operation data; they never accept a caller-selected app ID or Space ID. Any
navigation, origin change, policy change, revocation, or WebView destruction
closes the handle. The broker re-evaluates authority for every operation.
Grants are tagged with the namespace they cover; an Open Space session may use
communal own-data authority in the content namespace and separately provisioned
shared-data/app-policy authority in its linked owned governance namespace.

Every externally observable bridge effect is linearized against the authority
repository generation. A short generation guard is acquired immediately before
each network header/body chunk, redirect hop, platform-permission dispatch,
protected read return, or other native side effect; the effect occurs before a
concurrent frontier commit or is cancelled after it, never after a known
revocation. Durable writes condition their transaction on the same generation.
A frontier change signals and cancels in-flight work before further output and
zeroizes buffered protected data. If an OS permission sheet cannot be retracted
after dispatch, its result is discarded and no data or bridge result is
returned under the stale generation.

### Durable authority repository

`riot-core` owns an `AuthorityRepository` contract; platform storage supplies
transactional encrypted bytes but cannot declare policy valid. One transaction
persists the accepted Willow entries, governance journal, accepted frontier,
actor/receiver bindings, capability-lineage and revocation indexes, app grants,
audit classification, and relevant MLS epoch pointer. Content and policy
indexes cannot commit independently.

The journal is append-only and each derived snapshot is identified by the
hash of its complete frontier. Startup verifies the journal from the last
secure checkpoint and rebuilds every index before privileged admission or
protected reads begin. The secure vault stores the latest checkpoint hash and
monotonic generation; a database snapshot older than that checkpoint enters
rollback-recovery mode instead of re-enabling old authority. Missing parents,
invalid records, or an interrupted transaction remain quarantined and cannot
influence policy. Backup restore follows the same rebuild and rollback checks.

Capability verification may be cached only by canonical capability
fingerprint plus policy-frontier hash. Governance projections are indexed by
record type and target; no app bridge call scans the unbounded journal.

## Space profiles

### Open Space

- Uses a communal namespace.
- Every participant owns and may delegate within only their own subspace.
- There is no creator-controlled root over other participants.
- Moderation consists of signed curation, verification, correction, feature,
  mute, and hide-with-reason lenses.
- A linked owned governance namespace may control a shared default lens and
  shared app configuration.
- Following that governance namespace is voluntary; the raw communal Space
  and alternative lenses remain available.

### Managed Space

- Uses an owned namespace.
- The root namespace key issues initial read and write capabilities.
- Members and managers receive receiver-bound, area-restricted capabilities.
- Public managed Spaces may intentionally expose public content while
  retaining controlled writes.
- Private managed Spaces use MLS for confidentiality and Meadowcap for
  write-role separation inside the decrypted data plane.

### Riot authorization path profile v1

`**` below means a Meadowcap path-prefix area; angle-bracketed values are one
exact binary path component. A named role is a bundle of separate read/write
capabilities where a row contains disjoint prefixes. Custom roles may only
remove rows, narrow prefixes, or shorten timestamp areas. Meadowcap access mode
is immutable: replacing write with read means dropping the write capability
and independently issuing or attenuating a covering read capability.

| Role | Write-capability areas | Default protected read areas |
| --- | --- | --- |
| Member | `governance/v1/appeals/submissions/<actor_id>/**` | Only areas explicitly selected in the invitation |
| Contributor | `content/v1/submissions/<actor_id>/**` | The same actor submission prefix |
| Publisher/editor | `content/v1/published/**`; `annotations/v1/feature/**` | `content/v1/**`; `annotations/v1/**` |
| Moderator | `annotations/v1/moderation/**`; `governance/v1/appeals/resolutions/**` | `content/v1/**`; `annotations/v1/moderation/**`; `governance/v1/appeals/**` |
| Verifier | `annotations/v1/verification/**`; `annotations/v1/correction/**` | `content/v1/**`; those two annotation prefixes |
| Dispatcher | Separate prefixes below `workflow/v1/task/**`, `request/**`, `commitment/**`, and `handoff/**` | The same four workflow prefixes |
| Directory curator | Exact manifest and bundle slots below `app-index/<app_id>/` plus `governance/v1/directory/withdrawals/**`; never endorsement slots | `app-index/**`; `governance/v1/directory/**` |
| App endorser | Exact `app-index/<app_id>/endorsements-v2/<endorser_space_id>` path for each app | Exact `app-index/<app_id>/**` |
| App approver | `governance/v1/apps/approvals/**` and `governance/v1/apps/revocations/**` | `app-index/**`; `governance/v1/apps/**` |
| Governance proposer | `governance/v1/proposals/<actor_id>/**`; no decision authority | The same proposal prefix plus public governance decisions |
| Delegating administrator | Governance paths for `actors/**`, `roles/**`, `members/**`, `invitations/**`, `activations/**`, `capabilities/**`, and scoped `revocations/**`, plus explicit covering parent capabilities for each grantable role | The governance prefixes plus reads explicitly required by each grantable role |
| Root custodian | Full owned area, used only for genesis, initial delegation, recovery declarations, and migration | Full owned area |

Runtime app data retains the implemented `apps/<app_id>/<relative_key...>`
layout; the Willow namespace is the Space boundary and is never duplicated in
the path. App-index distribution retains its existing strict paths. Existing
open alert entries retain the implemented
`objects/alert/<object_id>/<revision_id>` layout. Managed Spaces use the versioned
`content/v1/` profile; translating an alert for managed publication produces a
new signed managed entry rather than rewriting a legacy one.

Meadowcap has no delegate-only access mode. A delegating administrator can
exercise every covering parent capability they can delegate. Its
`DelegatingAdminProfileV1` therefore lists exact `grantable_role_ids` and the
fingerprints of covering parent capabilities for those templates. The UI shows
only roles actually derivable from that set and states that the administrator
also holds those powers. A governance clerk without covering capabilities may
record proposals but cannot issue roles.

Every privileged role bundle also contains the exact
`governance/v1/actions/<actor_id>/**` prefix needed for its action receipts;
this receipt capability is delegated and revoked with the role, never granted
independently.

Public areas need no read capability. For protected data, the table's read
areas are separate Meadowcap read capabilities and are attenuated independently
from writes. Baseline managed membership grants only the explicitly selected
areas;
there is no implicit full-namespace read. Private membership normally grants
the MLS-protected Space data areas selected by the inviter.

Entry `subspace_id` remains a Willow coordinate. Human authorization and
attribution use the Meadowcap receiver plus its active actor binding.
Endorsement slots are keyed by stable `endorser_space_id` and are writable only
by a receiver currently authorized as that Space's app endorser; device keys
cannot create multiple endorsements for the same community.
Legacy communal endorsements retain
`app-index/<app_id>/endorsements/<endorser_subspace_id>`. Managed/stable-actor
endorsements use the distinct `endorsements-v2` component, so profile dispatch
can never reinterpret one 32-byte identity as the other.

## Governance ledger

Governance records are ordinary signed Willow entries with a canonical CBOR
`GovernanceRecordV1` containing: schema version, record kind, Space namespace,
sorted parent record IDs, stable actor ID, actual Meadowcap receiver key,
strictly increasing per-actor sequence, previous actor-record ID, authorizing
capability fingerprint, kind-specific body, and a display-only creation time.
`record_id` is a domain-separated hash of the canonical record. The entry path
must match the record kind and target under `governance/v1/`.

All literal components below are ASCII bytes. Actor, receiver, capability,
app, namespace, invite, action, role-instance, checkpoint, and record IDs are raw 32-byte
components; `sequence_be` is an 8-byte unsigned big-endian component. These are
the complete V1 governance kinds and exact paths; extra/missing components or a
path target that differs from the canonical body is ineligible.

| Record kind | Exact path | Required write authority |
| --- | --- | --- |
| `Genesis` | `governance/v1/genesis` | Root-issued owned capability for this exact path; receiver entry signature |
| `ActorBinding` | `governance/v1/actors/<actor_id>/bindings/<receiver_id>/<record_id>` | Delegating administrator `actors/**` |
| `MemberDecision` | `governance/v1/members/<actor_id>/<record_id>` | Delegating administrator `members/**` |
| `InviteManagerDecision` | `governance/v1/invitations/<invite_id>/manager/<record_id>` | Delegating administrator `invitations/**` |
| `InviteResponse` | `governance/v1/invitations/<invite_id>/responses/<receiver_id>/<record_id>` | Invitee's exact pre-activation response capability |
| `InviteActivation` | `governance/v1/activations/<invite_id>/<record_id>` | Delegating administrator `activations/**` |
| `RoleDecision` | `governance/v1/roles/<actor_id>/<role_instance_id>/<record_id>` | Delegating administrator `roles/**` plus the covering parent named in the body |
| `CapabilityIssued` | `governance/v1/capabilities/issued/<capability_fingerprint>/<record_id>` | Delegating administrator `capabilities/**` plus valid attenuation from the named covering parent |
| `CapabilityRenewed` | `governance/v1/capabilities/renewed/<capability_fingerprint>/<record_id>` | Same as issuance; renewed capability has a new fingerprint |
| `CapabilityRevoked` | `governance/v1/revocations/<capability_fingerprint>/<record_id>` | Root, self, ancestor receiver, or scoped revocation administrator as defined below |
| `Checkpoint` | `governance/v1/checkpoints/<checkpoint_id>` | Root or exact `checkpoints/**` capability |
| `ActionReceipt` | `governance/v1/actions/<actor_id>/<receiver_id>/<sequence_be>` | Receipt capability paired in the same role bundle |
| `Proposal` | `governance/v1/proposals/<actor_id>/<record_id>` | Actor's exact proposal prefix |
| `AppealSubmitted` | `governance/v1/appeals/submissions/<actor_id>/<action_id>/<record_id>` | Actor's exact member appeal prefix |
| `AppealResolved` | `governance/v1/appeals/resolutions/<action_id>/<record_id>` | Moderator resolution prefix |
| `AppApproved` | `governance/v1/apps/approvals/<app_id>/<record_id>` | App approver approval prefix |
| `AppRevoked` | `governance/v1/apps/revocations/<app_id>/<record_id>` | App approver revocation prefix |
| `AppProvisioned` | `governance/v1/apps/provisioning/<app_id>/<receiver_id>/<record_id>` | App-data issuer with covering per-app parent plus provisioning prefix |
| `DirectoryWithdrawn` | `governance/v1/directory/withdrawals/<app_id>/<record_id>` | Directory curator withdrawal prefix |
| `RecoveryDeclared` | `governance/v1/recovery/<record_id>` | Root only |
| `MigrationDeclared` | `governance/v1/migrations/<new_namespace_id>/<record_id>` | Healthy root only; compromised-root candidates are non-authoritative attestations |
| `LensSuccessor` | `governance/v1/lenses/successors/<new_namespace_id>/<record_id>` | Current lens root or delegated lens-successor authority |

A capability fingerprint is
`SHA-256("riot/meadowcap-fingerprint/v1" || canonical_capability_bytes)`; the
canonical bytes already bind capability type, access mode, namespace,
receiver, area, and every delegation signature. Fingerprints from another
codec or domain are never interchangeable.

The genesis record is an ordinary receiver-signed Willow entry authorized by a
root-issued owned write capability and has no parents. The namespace signature
anchors that owned capability; it is not used as the entry signature. Every
later record names an already accepted parent frontier; missing parents leave it pending. Records
form a hash DAG and are reduced topologically. Display timestamps never order
governance. A canonical checkpoint record merges a frontier that approaches
the 16-parent limit; only the root or a receiver with the exact
`governance/v1/checkpoints/**` authority may issue one.

Record authorization is kind-specific and evaluated against the named parent
frontier, before the new record can affect policy:

- Proposals require only the actor's proposal area and grant no authority.
- Actor/device bindings, membership, invitations, and role decisions require
  the delegating administrator's relevant governance capability.
- A role issuance or renewal receipt is valid only when it references an
  actually valid child capability delegated from the administrator's presented
  capability. Meadowcap attenuation proves the child is no broader; a record
  cannot manufacture or expand cryptographic authority.
- App approvals and revocations require the exact app-approver area.
- A capability may be revoked by the root, its current receiver (self-revoke),
  any receiver in its ancestor chain, or a root-issued revocation administrator
  whose signed `revocation_scope` contains the target capability's granted
  area. A general governance recorder cannot revoke or grant roles.
- Recovery declarations and ordinary root-authenticated migrations require the
  root. Compromised-root migration follows the separate trust ceremony below.

No record may authorize itself. Revocation of a capability fingerprint is
irreversible and invalidates its complete descendant delegation subtree,
including descendants received before their revoked ancestor. Restoring a
role requires a newly delegated capability with a new fingerprint.

Concurrent records converge by target-specific restrictive reducers: revoke
wins over grant; concurrent app permission approvals intersect; concurrent
role restrictions intersect; appeal resolution never restores revoked
cryptographic authority; and competing migration candidates remain an
explicit fork requiring human selection. All branches remain in the audit
journal.

### Leases and revocation

Meadowcap timestamp attenuation limits an entry's logical timestamp coordinate;
it does **not** prove when an offline entry was signed and is not treated as a
secure wall-clock lease. UI expiry dates drive renewal and bound coordinates,
but security removal uses governance revocation and an action-chain cutoff.

Every managed privileged write has a canonical `ActionReceiptV1` linking the
entry ID, capability fingerprint, actor/receiver, actor sequence, previous
action hash, and policy-frontier hash. A revocation body pins its parent
frontier and a cutoff map from each known `(actor_id, receiver_id)` in the
target subtree to that frontier's accepted action-head hash.

The post-revocation predicate is exact. Let `D` be every capability whose
canonical delegation chain contains the revoked fingerprint. An action using a
capability in `D` remains active only when its exact actor/receiver has a cutoff
head and its action hash is equal to or an ancestor of that head by repeated
`previous_action_hash` links. No cutoff entry means no active action for that
actor/receiver. A descendant discovered later is still in `D`; absent a cutoff
it is audit-only. Missing cutoff ancestors keep the dependent action pending;
a late ancestor may become active only by proving its chain to the pinned head.
Post-cutoff descendants and concurrent branches not ancestral to the head are
partition-era audit evidence and do not affect current views. Capabilities
outside `D` are unchanged. Read capabilities in `D` have no historical
grandfathering and close immediately when the revocation frontier is learned.

Because the revocation's parent frontier and cutoff map are immutable, every
arrival order computes the same result. This is the deliberate safety tradeoff
when removal races disconnection. The entry and receipt are inspected and
committed as one atomic selected unit; either missing half is ineligible.
`ActionReceiptV1` entries are authorization sidecars and are explicitly exempt
from generating another receipt, which is the recursion base case. Their own
entry signature, Meadowcap receipt-prefix capability, actor binding, canonical
path/body binding, action-entry hash, and previous-action hash are still fully
verified. A receipt cannot name itself or another receipt as its action.
The governance genesis is the only privileged action without a receipt: it is
validated against the empty frontier by its root-issued owned capability and
creates the first actor binding. Every other non-receipt privileged action must
have exactly one valid paired receipt, and one receipt may pair with exactly one
action.

Entry timestamps more than ten minutes ahead of a reliable local wall clock
are quarantined. An unavailable or rolled-back clock blocks issuance, renewal,
revocation, and migration, but record ordering still comes only from the DAG
and actor hash chains. Renewal delegates a new capability and records a new
fingerprint; it never mutates or un-revokes an old one.

Peers that know a revocation reject the entire descendant subtree immediately.
Disconnected peers may accept an old capability temporarily, then converge to
the same active policy and cutoff classification when governance synchronizes.
Revocation never deletes replicated bytes or audit history.

## App and directory management

### Manifest requests

`AppManifestV2` replaces free-form permission strings with a canonically sorted,
duplicate-free array of closed `AppPermissionV2` variants. Integer variant
tags and their parameter maps are versioned and unknown tags fail closed:

- `AppData { own|shared, read|write, relative_prefix, max_value_bytes }`.
  Apps never name a namespace, Space, subspace, or absolute host path.
- `CameraCapture`, `MicrophoneCapture`.
- `Photos { read_selected|add }`.
- `Location { approximate|precise, foreground|background }`.
- `Notifications { local|space_updates }`.
- `Nearby { scan|advertise|connect }`.
- `Clipboard { read|write }`.
- `Network { origins, methods }`, where each origin is one normalized HTTPS
  scheme/host/port tuple and methods are a closed enum. Wildcards, credentials,
  redirects to undeclared origins, IP literals, localhost, and private/link
  addresses are denied; a separately tagged `LocalNetwork` power is required
  for an explicitly approved local origin.
- `Background { task_kind, max_runtime_seconds }` with closed task kinds and a
  platform-capped duration.

Parameter subset rules are structural: an approval may remove variants, narrow
a relative prefix, lower byte/runtime bounds, reduce methods/origins, reduce
precision, or change background to foreground. It cannot substitute or widen.
The native permission renderer owns fixed plain-language and risk-category copy
for every variant; apps cannot supply the explanation. Compound grants that
combine shared Willow reads with network, clipboard-read, precise location, or
background execution require a separate high-risk confirmation on each device.

Legacy `AppManifestV1` remains canonically decodable and listable. Its arbitrary
strings grant no new V2 power and a V1 app cannot launch in a capability-managed
Space. Existing V1 apps may continue only in the legacy open-Space sandbox with
their historical own-app-data behavior. Repacking as V2 creates a new `app_id`
and requires fresh Space approval.

### Space approval

An `AppApprovalV1` records exact app ID, manifest digest, canonically sorted
granted V2 variants, approving actor/receiver, governance parents, and optional
display expiry. An app approver grants only a structural subset of the signed
manifest request.
One holder of the `apps/approve` authority may approve or revoke in the first
release. Multiple holders are supported; quorum evaluation is deferred. A
Space approval cannot force a device owner to grant sensitive platform
permissions.

Changing any app content changes `app_id` and requires a new approval. Sharing
or carrying an app never enables it.

Approval and data authority are two explicit phases. `AppProvisioningV1`
derives an exact per-app capability root for `apps/<app_id>/**` from the root or
from an app administrator that visibly holds a covering `apps/**` parent, then
delegates only the approved read/write subareas to each eligible current device
receiver. Future invitations include the selected approved-app children.
Meadowcap has no delegate-only power, so an app administrator with the broad
parent can exercise it; a policy-only app approver without that parent produces
`approved_waiting_authority` and cannot provision members.

An app is `available` on a device only after its exact capabilities and active
approval are both present. Other states are `shared_unapproved`, `approved_waiting_authority`,
`provisioning`, `available`, and `revoked`. Offline members remain provisioning
until capability delivery; the Tools row says access is still arriving rather
than launching a predictably failing app. Revoking the per-app root invalidates
all provisioned descendants. In a communal Open Space, own-subspace app data
uses each person's communal authority; shared app data requires corresponding
per-device authority from the linked owned governance Space.

### Runtime enforcement

- The native host retains receiver keys and capabilities.
- App bridge keys are relative and normalized by the host.
- Willow operations are restricted to the effective granted area.
- Writes remain attributed to the person using the app.
- Network and remote script loading remain denied unless explicitly requested
  and approved.
- Approved network requests are host-proxied without ambient cookies,
  credentials, platform proxy inheritance, or unrestricted redirects. Every
  request and redirect re-normalizes the origin, resolves DNS at connection
  time, rejects mixed public/private answers and loopback/link-local/private
  destinations, and pins the validated public address for that connection to
  prevent rebinding.
- Permission state is checked for every operation, not cached for the lifetime
  of an app process.
- Revocation closes the active session and removes the app from the Space's
  Tools row without deleting previously synchronized app data.
- Audit records retain both the human actor and mediated app ID so direct and
  app-initiated actions remain distinguishable.
- Each app/profile uses an isolated WebView data store/process where supported,
  a restrictive CSP with remote scripts disabled by default, and bridge
  suspension before navigation begins.
- Subresources, WebSockets, service workers, downloads, custom schemes, and
  top-level external navigation are denied unless represented by a typed grant
  and routed through the same host proxy and generation guard.

### Plural directories

Directory governance preserves the separate roles of author, carrier,
curator, endorser, and Space approver. Dedicated directory Spaces use the same
open-or-managed choice as other Spaces. A global storefront remains a local
computation over valid app-index entries, trusted Space approvals,
endorsements, provenance, and starter apps. There is no canonical directory
database or privileged Riot catalog namespace.

A curator publishes by carrying canonical manifest/bundle entries into its
authorized slots, updates by publishing a new content-derived app ID, withdraws
through a signed directory-withdrawal record without deleting bytes, and forks
by carrying selected app-index entries into another Space. None of these acts
approves the app for launch.

## Invitation, membership, and device lifecycle

Receiver keys are per device; `actor_id` is the stable person/role identity.
Adding a device creates a new receiver and a narrowly delegated child
capability. Receiver keys are never copied between devices.

An `InviteRequestV1` contains a random 256-bit request ID, Meadowcap receiver
signing key, separate X25519 HPKE encryption key, requested Space fingerprint,
request nonce, protocol version, and—when private—the MLS KeyPackage. The
receiver signs
`"riot/invite-request/v1" || canonical_request_without_signature`, binding the
encryption key and KeyPackage to the signing receiver and request context.

`InviteV1` is recipient-bound and contains a separate random 256-bit invite ID,
the complete request digest, Space/profile fingerprint, inviter actor and
receiver, requested actor binding, canonical child capabilities, expiry
coordinate, one-use nonce, and MLS reference. Its canonical clear header binds
invite ID, request ID/digest, both signing receivers, and protocol version; its
body is HPKE X25519/HKDF-SHA-256/ChaCha20-Poly1305 encrypted to the request's
encryption key with that header as AAD, then the inviter signs the header and
ciphertext digest. A recipient first shares the request by QR/file/nearby
exchange; managers cannot mint a secret-bearing invite to an unknown or
unbound encryption key.

The durable invitation state machine is `draft -> issued -> delivered ->
previewed -> accepted_pending -> active`, with terminal `rejected`, `cancelled`,
`expired`, and `failed` states. The recipient sees Space identity, inviter,
roles, exact readable/writable areas, expiry, and private/public consequences
before acceptance. Reuse of an invite ID/nonce, a changed request, cancellation,
or acceptance after expiry fails closed. Delivery and acceptance receipts make
retry idempotent; cancellation before activation publishes a cancellation
record, while cancellation after activation runs normal offboarding.

Every issued child fingerprint is recorded in `InviteLineageV1` under the
invite ID. Riot policy rejects use of that capability and every descendant at
every admission/read gate unless the evaluated frontier contains the matching
active activation record. Cryptographic Meadowcap validity alone is
insufficient. Cancellation, expiry, rejection, failed MLS completion, or
abandoned repair atomically consumes the invite nonce and transitively revokes
all pre-issued roots. Invite nonce consumption and the activation/revocation
transition commit in the same authority-repository transaction, so a crash
cannot reopen the invite.

The sole pre-activation exception is a separate exact-path response capability
for `governance/v1/invitations/<invite_id>/responses/<receiver_id>/**`. Policy
permits it only for canonical accept/reject responses bound to the request and
invite digests; it cannot authorize role, content, app, or activation writes.

Private managed invites use a recoverable two-phase flow:

1. Exchange receiver key and MLS KeyPackage; create and validate the attenuated
   Meadowcap grants.
2. Deliver an encrypted preflight and receive the invitee's signed acceptance.
3. Commit the MLS add and publish the activation record.
4. Deliver the MLS Welcome plus activation proof; activate local Space access
   only when both MLS and Meadowcap state validate.

If the MLS commit succeeds but delivery is interrupted, the state is
`accepted_pending` and the Welcome/grants are safely redelivered. If final
validation cannot succeed, the manager publishes an MLS removal/rekey and a
Meadowcap revocation; the UI says access setup needs repair and never claims an
atomic rollback. Removing a private member always combines capability-tree
revocation with an MLS remove/rekey. It stops future reads/writes after peers
learn the new state but cannot erase data already synchronized or decrypted.

Voluntary leave self-revokes the member's capability subtree, closes local
sessions, and publishes a leave request. A public managed Space needs no
further cryptographic step. A private Space becomes fully offboarded only when
a manager also commits the MLS removal/rekey; until then the UI distinguishes
`left on this device` from `removed from future private epochs`. Previously
synchronized data cannot be recalled.

Routine device replacement creates fresh receiver and encryption keys, binds
them to the same stable actor through the normal recipient-bound invitation,
activates the new device, and then revokes the retired receiver subtree. Secret
receiver keys are never copied between devices.

An open Space may have many competing governance lenses. Any participant may
create or share one; each client explicitly follows, unfollows, or selects its
default locally. A lens may sign a successor recommendation, but replacement
requires user confirmation and never changes the communal namespace.

## Typed management and FFI contract

`riot-core` defines versioned records/enums for `SpaceProfile`, `Actor`,
`DeviceReceiver`, `AuthorityArea`, `CapabilitySummary`, `RoleTemplate`,
`GovernanceRecord`, `AppPermission`, `AppApproval`, `OperationPreview`, and
`OperationStatus`. Public summaries expose full IDs and fingerprints but never
secret or raw capability bytes. Stable error enums distinguish malformed,
noncanonical, unauthorized, stale-policy, revoked, expired-coordinate,
receiver-mismatch, missing-parent, conflict, rollback-detected,
recovery-required, partial-private-invite, and resource-limit outcomes.

UniFFI exposes opaque lifecycle objects rather than secret byte arrays or
caller-selected identity parameters:

- `SpaceCreationSession`
- `SpaceManagementSession`
- `InviteAcceptanceSession`
- `ManagementOperation`
- `RecoverySession`
- `MigrationSession`
- `AppExecutionSession`

All durable operations have a random 128-bit `operation_id` stored in the
authority repository. `MobileProfile.reopen_management_operation(operation_id)`
reconstructs a typed `ManagementOperation`; specialized factories reopen
invite, recovery, and migration operations after validating the recorded kind.
Dropping or closing a UniFFI object releases only the process handle and never
cancels durable work.

| Object | Create/inspect | Read | Mutate | Terminal behavior |
| --- | --- | --- | --- | --- |
| `SpaceCreationSession` | `start_space_creation(input)` | `preview()`, `status()` | `commit(token)`, `cancel()` | Idempotent `close()`; reopen by operation ID |
| `SpaceManagementSession` | `open_space_management(space_id)` | Paged people/roles/apps/audit queries | Mutations return durable `ManagementOperation` values | Process-local query handle; dropping does not mutate |
| `InviteAcceptanceSession` | `inspect_invite(bytes)` or `reopen_invite(id)` | `preview()`, `status()` | `accept(token)`, `reject()`, `cancel_before_activation()`, `retry()` | Resumable until active or terminal |
| `ManagementOperation` | Returned by previewed grant/revoke/approve/provision operations or reopened | `preview()`, `status()`, `progress()` | `commit(token)`, `retry()`, `cancel_before_commit()` | Signed/committed operations cannot cancel |
| `RecoverySession` | `inspect_recovery(envelope)` or `reopen_recovery(id)` | `preview()`, `status()` | `unlock(recovery_key_handle)`, `restore(token)`, `cancel()` | Key handle is vault-owned; no raw key bytes |
| `MigrationSession` | `start/inspect/reopen_migration` | Candidate/fingerprint/impact preview and status | `confirm(token)`, `retry()`, `cancel_before_new_genesis()` | Old/new commit boundaries are explicit |
| `AppExecutionSession` | Native-host launch only | Permitted bridge reads | Permitted bridge writes | Process-local and non-resumable; closes on navigation, policy change, or process death |

Every mutation follows inspect/preview/commit. The preview contains a one-use
digest bound to operation inputs, authority delta, policy-snapshot hash, and
expiry; commit fails stale if any of those change. Operations report
`local_pending`, `distributed`, `effective`, `superseded`, `expired`,
`rejected`, `cancelled`, or `repair_required`. Long operations define progress,
idempotent retry, cancellation before the signing/commit boundary, and resume
after process restart. List APIs are deterministically sorted and use opaque
snapshot-bound pagination tokens.

The native host implements a `SecureSigner`/`SecureVault` adapter with opaque
key handles. `riot-core` requests domain-separated signatures through that
interface; general `Vec<u8>` wrapping keys and raw root/delegated secret values
are removed from the new management API. A compatibility-only legacy identity
API remains isolated until migration completes.

## Root custody, recovery, and migration

The root seed is sealed by a non-exportable platform wrapping key. On Apple,
the wrapping key is `ThisDeviceOnly`, available only while unlocked, requires
current user presence for root operations, and is excluded from iCloud Keychain
and backups. On Android, a non-exportable hardware-backed/StrongBox wrapping
key is preferred, requires user authentication per root operation, is excluded
from backup, and is invalidated according to the recorded biometric/passcode
policy. A software-backed fallback requires an explicit warning. Delegated
keys use separate handles and never authorize root operations. Reinstall or
secure-key invalidation requires recovery.

Recovery V1 uses a generated 256-bit random recovery key, displayed as a QR and
checksummed words; user-memorized passphrases are not accepted. HKDF-SHA-256
with a random 32-byte salt and domain `riot/root-recovery-key/v1` derives an
XChaCha20-Poly1305 key. The envelope has magic/version/suite, salt, random
24-byte nonce, namespace public key, ciphertext length, and ciphertext; AAD
binds magic, version, suite, namespace, and format parameters. Plaintext holds
only the versioned root seed and latest authority-checkpoint hash and is
zeroized after use. Any mismatch, truncation, rollback, wrong key, or unknown
version fails without partial restore.

Export requires user presence, a destination warning, and successful test
decrypt/namespace verification before completion. Riot never places the key or
plaintext on the clipboard, in logs, crash reports, or automatic cloud backup.
Recovery-key screens suppress app-switcher snapshots, screenshots/screen
recording, analytics capture, and notification overlays where platform APIs
permit; unsupported protections are disclosed before display.
Five failed UI imports close the recovery session; high-entropy key security,
not throttling, prevents offline guessing. A future passphrase mode would
require a separate review and at least Argon2id with 64 MiB, three iterations,
and parallelism one.

On import, Riot previews the full namespace fingerprint and checkpoint. An
existing identical namespace may merge only after journal/checkpoint
verification; conflicting local state is quarantined. If recovery export was
explicitly declined, every management screen shows a persistent
`ROOT NOT BACKED UP` state and Riot prevents deleting the final root-bearing
device without another explicit irreversible-loss confirmation.

A portable recovery artifact proves integrity only through its exported
checkpoint; on a fresh device it cannot prove that no later revocation exists.
Restore therefore enters `freshness_unknown` quarantine. The device may inspect
and re-export recovered material but may not issue capabilities, admit
privileged writes, serve protected reads, export protected data, invite members,
or approve apps. Freshness is established by synchronizing a governance journal
whose checkpoint descends from the artifact with a previously trusted device
or by importing an equivalently authenticated governance drop. With no such
source, the only safe offline override creates a visibly forked replacement
namespace through the manual fingerprint ceremony; it never resumes privileged
writes in the old namespace. The UI does not describe a stale artifact as a
current backup.

Healthy-root migration is authorized by the old root, previews every role,
app grant, governance lens, and private-membership change, and requires user
confirmation. Its signed record binds full old/new namespace fingerprints,
migration-manifest digest, selected old policy frontier, and new MLS group
identity when private. If the root is unavailable or suspected compromised, its
signature is not a trustworthy discriminator: Riot never auto-selects a
successor. Candidate Spaces are shown as a fork with full fingerprints and
manager/member attestations; every member chooses through an in-person QR or
other out-of-band fingerprint comparison. Roles and app grants are reissued
and reapproved rather than copied. Private migration creates a new MLS group.
History remains linked but no candidate can claim universal continuity.

## Backward compatibility

- Existing RIOTE1 bundles and zero-delegation communal entries remain readable
  and importable under a `LegacyCommunalV1` profile; they never gain owned or
  delegated authority implicitly.
- Existing `PublicSpace` values remain Open Spaces. Converting content to a
  Managed Space creates a new owned namespace and new signed entries.
- The protected-sync codec is a breaking version. Conference-sync `/1` remains
  public-only and cannot negotiate protected operation.
- Fixed organizer allowlists and profile-local trust markers are not accepted
  as Meadowcap authority. A recognized legacy organizer may preview and sign
  fresh app approvals into a newly created governance namespace; there is no
  silent translation.
- `AppManifestV1` remains listable with legacy own-data behavior as described
  above. V2 repackaging changes app identity and requires reapproval.
- Current sealed communal identities may be imported into the legacy profile.
  Owned roots and device receiver bindings use new versioned vault records.

## End-to-end flows

### Create a managed Space

1. Generate the owned-namespace root key in secure storage.
2. Create the initial Space profile and governance records.
3. Generate a separate daily manager receiver key.
4. Issue coordinate-bounded management read/write capabilities and a renewal
   schedule to it.
5. Require or explicitly decline encrypted recovery export.
6. Begin normal operation using only delegated authority.

### Delegate a role

1. Select a named role template and receiver.
2. Preview the concrete paths, modes, and expiry in plain language.
3. Attenuate an existing covering capability.
4. Sign the Meadowcap delegation.
5. Publish a governance receipt without secret material.
6. Deliver the capability through an authenticated invite or encrypted
   portable artifact.

### Admit a write

The writer supplies the canonical entry, complete write-capability chain, and
receiver signature. The shared admission engine performs the pinned checks and
atomically commits the selected eligible set or returns a stable diagnostic.

### Serve protected reads

The peers complete the protected-sync transcript, private-interest overlap,
confidential capability exchange, and receiver proof defined above. The serving
peer rechecks every bound range and response against the covering capabilities
and current policy snapshot. No legacy `Hello` or `Summary` frame is sent for a
protected area.

### Export and import

Write authority is portable with each authorized entry. Read authority does
not make plaintext files confidential. Riot refuses plaintext export of a
protected area.

`ProtectedDropV1` binds magic/version, Space namespace, exported-area
fingerprint, policy frontier, exporter actor/receiver, recipient encryption-key
IDs, optional MLS group/epoch, payload digest/length, and anti-replay envelope
ID. A random 256-bit content key encrypts the drop with XChaCha20-Poly1305; each
recipient gets an HPKE X25519/HKDF-SHA-256/ChaCha20-Poly1305 wrap bound to the
header as AAD. For every recipient the signed header also binds the fingerprint
of an active read capability covering the complete exported area and its active
actor/device binding at the named frontier. Export validates those predicates
before creating any wrap; an arbitrary bound encryption key without covering
read authority is ineligible. Device records carry encryption keys separately from Meadowcap
signature receivers; an actor-binding record signs the receiver and encryption
key together. The active exporter receiver signs
`"riot/protected-drop-signature/v1" || canonical_header || ciphertext_digest`.
Import verifies that signature, the actor/device binding, and covering export
read authority at the named policy frontier before unwrapping content. Before
unwrapping, the local recipient must possess the header-named active read
capability, its active receiver/encryption-key binding, and coverage of the full
area under a non-stale accepted frontier. Private-group exports additionally bind to the current MLS
group and epoch and include only current members; an old epoch or removed
member cannot receive a newly created export. Import rejects recipient,
namespace, epoch, digest, replay, version, and downgrade mismatches before
inspecting inner entries, then applies item-local inspection and atomic commit
of the selected eligible set. The envelope ID is consumed in that same
transaction, so a crash cannot replay a committed drop.

### Remove authority

Publish a signed transitive revocation with actor action-chain cutoffs, stop
renewal, close locally active sessions, and reject the capability subtree
immediately. Offline peers may temporarily accept actions under old policy.
After reconciliation, all peers compute the same current policy and retain
non-ancestral partition-era actions only in the audit history.

### Moderate and appeal

A moderator previews and signs a hide-with-reason annotation under the
moderation prefix. The affected actor or any permitted participant may publish
an appeal under
`governance/v1/appeals/submissions/<actor_id>/<action_id>/**`. A moderator with the
resolution prefix may affirm or reverse the lens decision, but cannot delete
the original content, action, or appeal and cannot restore a revoked
capability.

### Replace a compromised root

Create a replacement owned namespace. A healthy old root may sign the migration
after preview and confirmation. If it is missing or suspected compromised,
clients freeze automatic migration, present candidate forks and full
fingerprints, and require each member's out-of-band selection. The old Space
history remains available, but Riot makes no false claim of cryptographic
continuity.

## Management experience

Space creation presents the power model, not protocol vocabulary:

- **Open Space:** anyone can contribute; moderation is a chosen community
  lens.
- **Managed Space:** selected people manage roles, moderation, membership, and
  apps.

Managed Space settings provide people and roles, invitations, expiring
access, moderation and appeals, apps and permissions, renewals and
revocations, governance history, recovery, and migration. Primary UI uses
plain language such as `Moderator · renewal due August 1`; it never says that
calendar time alone removes offline authority. Removal status reads `Removed
on this device` or `Removal shared with known peers` until reconciliation. An advanced
inspector may display full namespace IDs, receiver IDs, paths, expiry,
delegation chains, signatures, and revocation state. IDs are never truncated.

Role grants, app approvals, recovery export/import, revocation, and migration
all use preview-and-confirm screens showing the concrete authority delta and
the point after which cancellation is impossible. Distributed operations say
`On this device`, `Shared with known peers`, or `Effective in the policy you
have received`; they never say globally complete. Partial private invitations
and migrations expose repair/resume actions. Permission risk is communicated
with text and accessibility labels, never color or icons alone. Custom
attenuation is shown as a named role plus readable exceptions, for example
`Moderator, except app approvals; renewal due August 1`. UX tests require
people to distinguish renewal reminders, Meadowcap entry-coordinate bounds,
and propagated revocation.

## Error handling

All authorization failures are fail-closed. Stable internal diagnostics are
safe to test and log, while user messages avoid protected identifiers and
cryptographic jargon.

| Condition | Behavior |
| --- | --- |
| Invalid or authority-expanding delegation | Reject; explain that the grant is invalid |
| Entry outside capability timestamp area | Reject; offer a newly delegated renewal where policy permits |
| Known-revoked capability | Reject and record a non-secret audit diagnostic |
| Partition-era action discovered later | Exclude from normal current views; retain in audit history |
| Read request without covering authority | Reveal neither entries nor hidden path existence |
| Root unavailable | Enter recovery; never silently create a replacement identity |
| Root compromised | Freeze automatic migration and guide explicit fingerprint-based fork selection |
| Unsafe device clock for authority change | Block the time-sensitive operation until corrected |
| App asks for an undeclared or unapproved power | Deny the bridge call with an actionable permission error |
| App permission revoked while running | Close the app session and preserve existing data |
| Conflicting policy records | Apply deterministic restrictive tie-break and show the conflict |
| Oversized chain, record, or request | Reject before expensive cryptographic or storage work |

Logs never contain secret keys, recovery material, full capability bytes, or
protected paths. Diagnostic correlation uses non-reversible fingerprints.

## Resource and abuse controls

The implementation plan must confirm or lower the pinned limits for capability byte size, delegation
depth, governance record size, capabilities per session, app grants per
Space, verification work per import, read ranges, payload bytes, and failed
challenge attempts. Limits apply before allocation or recursive verification
where possible. An authenticated peer exceeding a budget loses the session;
public gateways and nearby transports retain their existing byte/count/time
budgets.

Initial safety ceilings are a maximum delegation depth of 16, 64 KiB of
encoded capability data per capability, 128 bound capabilities and 64 active
read ranges per session, 16 KiB per governance record, 256 live app grants per
Space, and five failed receiver challenges before closing a session. The
implementation plan may lower a ceiling when measured valid fixtures need
less room, but raising one requires an explicit security review and updated
resource-exhaustion tests.

Verification uses a global token bucket as well as per-session/source buckets,
so rotating receiver keys cannot reset the CPU budget. A session may return at
most the existing Riot import byte budget and runs for at most the existing
sync time budget before resumable closure. Governance checkpoints compact
frontiers but never delete records needed to validate a live capability or
audit cutoff. On release-class mobile hardware, a depth-16 capability check
must remain below 25 ms p95, an indexed policy lookup below 2 ms p95, and a
cold rebuild of 10,000 governance records below two seconds; exceeding a budget
blocks release or lowers the corresponding ceiling.

## Testing strategy

TDD is mandatory. Each implementation work unit begins with a failing test and
does not complete until the new behavior and all existing behavior pass.
Coverage must meet `.coverage-thresholds.json` in lines, branches, functions,
and statements.

Every work unit documents a RED-GREEN-REFACTOR cycle: add the smallest failing
conformance, policy, or user-flow test; observe the expected failure; implement
only enough production behavior to pass; run the focused and workspace suites;
then refactor without changing the asserted contract. Shared test builders
construct deterministic namespace/user keypairs, capability chains, entries,
governance frontiers, encrypted sessions, and app grants. A fake secure vault
implements the same typed interface as Keychain/Keystore for core and FFI
tests; platform integration tests exercise the real storage adapters.

### Core conformance

- Canonical encode/decode for communal and owned read/write capabilities.
- Zero-, one-, and multi-hop delegation.
- Restrictions across namespace, subspace, path, timestamp, and receiver.
- Negative tests for every form of authority expansion.
- Tampered signatures, reordered chains, trailing bytes, non-canonical forms,
  wrong modes, wrong receivers, and wrong namespaces.
- Stable golden capability and authorization-token bytes.
- Differential tests against pinned `willow25` APIs.
- Property tests over generated valid and invalid capability trees.
- Capability-fingerprint domain/version separation and receiver-versus-entry
  subspace attribution.
- Versioned role-template golden tests pin every path/mode bundle above.
- Every closed GovernanceRecordV1 kind has exact path/body/authority golden and
  missing/extra/wrong-target negative fixtures.

### Admission and replication

- Identical decisions for local writes, imported drops, and synchronized
  entries.
- Invalid siblings remain item-local during inspection; commit of the selected
  eligible set is atomic.
- Fresh-challenge proof, challenge replay, receiver mismatch, stolen read
  capability, and cross-session reuse.
- No unauthorized entry or payload disclosure and no metadata disclosure
  beyond the pinned PIO L0/L1/L2 threat-model allowance.
- Public visibility policy does not accidentally expose protected areas.
- Protected portable exports require encryption.
- Full protected-sync state transitions, transcript mutation, downgrade,
  sequence replay/reordering, private-interest overlap, awkward enumeration,
  relative capability encoding, padding buckets, and constant-shape rejection.
- ProtectedDrop recipient/Space/epoch binding, removed-recipient, replay,
  downgrade, and corrupted-envelope vectors.

### Governance and partitions

- Coordinate-bound renewal, future-clock quarantine, and action-cutoff
  boundaries.
- Revocation before, during, and after disconnected operation.
- Different arrival orders produce the same effective policy.
- Restrictive causal conflict reducers with display timestamps ignored.
- Root recovery success/failure, compromise, and namespace migration.
- Audit history retains rejected and superseded decisions without activating
  them.
- Record-type authorization, no self-authorization, causal DAG ordering,
  checkpoint compaction, actor/action hash chains, transitive descendant
  revocation, cutoff classification, and concurrent restrictive reducers.
- Action-receipt base-case, missing-pair, self-reference, receipt-of-receipt,
  swapped-action, and tampered-action-hash rejection.
- Repository crash points between every journal/index/checkpoint write,
  deterministic rebuild, restored-backup rollback detection, and fail-closed
  startup.

### Apps and platforms

- Permission intersection across manifest, Space, Meadowcap, device consent,
  and platform restrictions.
- Path traversal and cross-app/Space access attempts.
- Raw keys and capability bytes never reach JavaScript.
- Runtime revocation closes sessions.
- iOS Keychain and Android secure-storage behavior under lock, reinstall,
  backup/restore, failed authentication, recovery, and deletion.
- Recovery-envelope golden and corruption vectors, wrong-key behavior,
  zeroization instrumentation, secure-vault rollback, and cloned-container
  restore.
- Stale recovery artifacts remain freshness-quarantined on fresh installs and
  cannot perform any forbidden privileged operation.
- Every invitation state and retry/cancel/expiry/replay transition, including
  private MLS partial failure and repair.
- Pre-activation invite capabilities fail every local/import/read path;
  activation and nonce consumption are crash-atomic; cancellation revokes all
  descendants.
- Invite request receiver/HPKE/KeyPackage binding, request-digest substitution,
  wrong-recipient decryption, and exact pre-activation response-capability
  confinement.
- Manifest V1/V2 decoding, V1 sandbox restrictions, canonical V2 permission
  subset algebra, network redirect/origin checks, and mandatory reapproval.
- Opaque app-session confused-deputy tests for caller-selected app/Space IDs,
  origin/navigation changes, stale policy snapshots, and destroyed WebViews.
- App approval/provisioning states, exact per-device app capabilities, future
  invite provisioning, unavailable Tools rows, and per-app subtree revocation.
- ProtectedDrop exporter-signature, actor/encryption-key binding, export-area
  authority, per-recipient covering read authority, and crash-atomic replay
  consumption.
- Any governance-frontier change closes protected-sync and app sessions before
  their next output; cutoff-map predicates cover late ancestors, absent
  descendants, and concurrent branches.
- Generation-guard races at every network chunk/redirect, native permission
  dispatch, protected return, and durable write linearize before revocation or
  cancel without further output.
- Exact current legacy paths and managed V1 paths are pinned in codec/schema
  dispatch tests.

### Compatibility

- RIOTE1 and existing communal identities remain readable under the legacy
  profile and cannot acquire delegated/owned authority.
- Existing trust markers never become V2 approvals without preview and a new
  authorized signature.
- Conference-sync `/1` rejects protected operation, and protected peers reject
  downgrade to `/1`.

### Blocking verification

```text
cargo test --workspace --all-features
cargo check --workspace --all-features
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
cargo tarpaulin --fail-under 100
```

Independent architecture, security, UX/API, product, and TDD-readiness reviews
must approve the design. Each implementation work unit receives independent
validation and adversarial review before commit.

## Acceptance criteria

1. A device can create communal and owned read/write capabilities, delegate
   them through multiple attenuation steps, encode/decode them canonically,
   and verify them using the shared core.
2. Riot admits valid owned and delegated writes and rejects invalid,
   authority-expanding, revoked, coordinate-expired, malformed, or policy-disallowed
   writes through the same local/import/sync path.
3. A person can create either an Open or Managed Space and the resulting
   namespace behavior matches the selected power model.
4. A managed Space can grant, renew, inspect, and revoke all defined roles
   without using the root key for daily writes.
5. An open Space can link an optional owned governance namespace without
   granting it write authority over communal participant subspaces.
6. Protected replication discloses no unauthorized entries or payloads and no
   metadata beyond PIO's explicitly pinned L0/L1/L2 allowance after receiver
   proof and valid private-overlap/capability exchange.
7. Protected areas cannot be exported as plaintext portable drops.
8. A Space can approve an exact app version with a permission subset; the app
   cannot exceed that subset or obtain raw authority material.
9. Sensitive device permissions still require local consent.
10. Known revocations take effect immediately, and partitioned peers converge
    deterministically while retaining an audit trail.
11. Root keys are secure-storage backed, recoverable only through the explicit
    encrypted recovery flow, and replaceable only by Space migration.
12. Managed invitations and device additions are recipient-bound, replay-safe,
    resumable, and active only after all required Meadowcap and MLS steps.
13. Governance state survives restart without reviving a revoked capability;
    backup/recovery restore remains quarantined until freshness is proven, and
    rollback or missing parents fail closed.
14. Revoking a capability invalidates every descendant and produces the same
    active action cutoff on every message ordering.
15. V2 app permissions are canonical, structurally comparable, destination
    scoped, and enforced through an opaque WebView-bound session.
16. Legacy communal data remains readable without being silently promoted to
    managed authority.
17. All blocking verification commands and the coverage gate pass.

## Success and failure measures

The feature is successful when every acceptance criterion has executable
coverage, cross-device tests demonstrate role grant and removal without a
server, protected sync tests show zero unauthorized entry/payload disclosure
and only the pinned PIO metadata allowance, and an app
cannot exceed any independently varied component of its effective grant.

Before release, automated conformance must accept 100% of maintained valid
golden fixtures and reject 100% of maintained invalid fixtures; protected-sync
tests must observe zero unauthorized entry/payload disclosure and exactly the
pinned PIO metadata leakage profile; all generated attenuation tests must show zero
authority expansions; and cross-device scenarios must converge to one
effective role/app policy for every tested message ordering. A field exercise
with at least three disconnected devices must complete Space creation, role
grant, protected sync, revocation, reconciliation, app approval, and recovery
without a server. Any unauthorized disclosure or authority expansion blocks
release regardless of aggregate pass rate.

In that field exercise, all three participants must correctly identify whether
the Space is Open or Managed, all must distinguish Space app approval from
personal device consent, and every operation must either complete or present a
recoverable next action. Any participant believing a merely local revocation
is globally complete, or losing a Space through an unexplained recovery state,
is a release-blocking UX failure.

The design has failed if Riot retains any feature-local organizer allowlist,
if local writes bypass imported-write checks, if a read capability works as a
bearer token without receiver proof, if communal Space governance can rewrite
participant subspaces, if a Space can force sensitive device consent, or if a
protected portable export can be produced in plaintext.

## Implementation slices and release boundary

The implementation plan decomposes this design into separately reviewed slices:

1. Meadowcap canonical codec, creation, delegation, inspection, verification,
   fingerprints, and conformance fixtures.
2. Versioned governance schemas, actor/device/action chains, durable repository,
   deterministic evaluator, and transitive revocation.
3. Shared contextual admission for local writes, imports, and synchronized
   entries, including legacy compatibility.
4. Protected-sync handshake, PIO/capability exchange, encrypted reconciliation,
   and ProtectedDrop V1.
5. Open/Managed Space creation, invitations, roles, app-independent membership,
   secure-vault adapters, recovery, and migration.
6. Manifest V2, permission algebra, approvals, directory role authority, and
   opaque app execution sessions.
7. iOS and Android management, consent, recovery, repair, accessibility, and
   audit experiences.
8. Cross-device conformance, partition, migration, field-exercise, performance,
   and security review.

No partial slice is marketed as full managed-Space security. The minimum
releasable journey requires all eight: create a managed Space, complete
recovery protection, invite a second device/person, grant a restricted role,
perform protected sync, approve an app subset, revoke the role offline, and
reconcile to the same policy without a server.

## Dependencies and sequencing constraints

- Keep the current app-index and app-data path validation; replace its
  authority source rather than weakening its schema checks.
- The capability core and shared admission engine precede role, governance,
  app, and native management UI work.
- Secure storage and recovery precede exposing managed-Space creation to
  users.
- Receiver-authenticated read gating precedes any claim that protected sync is
  available.
- Namespace migration must exist before describing root recovery as complete.
- The implementation plan must inventory current `willow25` APIs, map these
  pinned contracts to bounded file scopes, and identify any upstream gaps
  before coding.
