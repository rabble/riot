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
Riot therefore defines a transport-independent authorization contract and
does not make WTP interoperability claims until its separate conformance gate
passes.

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
- Role templates, leases, renewals, delegation receipts, and signed
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

The module contains no `Moderator`, `AppApprover`, or UI concepts. Consumers
receive typed facts and stable rejection codes, not string parsing access to
cryptographic objects.

### Riot policy evaluator

The policy evaluator combines a cryptographically valid capability with:

- Space profile and linked-governance configuration.
- Signed issuance, renewal, revocation, and migration records.
- The locally known governance frontier.
- Role-template constraints such as accepted paths, lease limits, and maximum
  management-chain depth.
- App-version grants and local device consent.

Protocol support accepts arbitrary valid Meadowcap delegation chains. A Riot
Space profile may impose narrower operational limits without claiming that a
valid Meadowcap capability is malformed.

### Secure authority vault

The vault stores namespace root keys and delegated receiver keys behind a
minimal typed interface. Apple implementations use Keychain and Android
implementations use Keystore-backed secure storage. Secret keys and root
operations do not cross FFI as general byte arrays. Recovery export is
authenticated, encrypted, versioned, and explicitly initiated by the user.

Root keys are not used for daily entry signing. The root issues renewable,
time-bounded management capabilities to delegated receiver keys. Loss of all
root material without a recovery artifact is unrecoverable and must be
reported honestly.

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
8. Atomic commit of the complete accepted set.

Local writes pass through the same checks before persistence. No local API is
allowed to construct a privileged entry and insert it beneath this boundary.

### Replication read gate

Protected synchronization requires an encrypted, receiver-authenticated
session:

1. A serving peer sends a fresh random challenge.
2. The requester identifies one receiver public key and signs the challenge.
3. Every read capability presented in that session must name that receiver.
4. Each request or reconciliation range must be contained by the capability's
   namespace and granted area.
5. Payload responses are disclosed only inside the authenticated encrypted
   session.

The interface is transport-independent. The existing nearby-sync protocol
uses it first; later WTP and Confidential Sync adapters must satisfy the same
contract. Public areas are served according to their explicit Space
visibility policy rather than fabricating confidential read capabilities.

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

The broker re-evaluates authority at launch and for every bridge operation.
Revocation closes active sessions and rejects subsequent calls.

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

### Logical role templates

Wire path families are finalized in the implementation plan after inventorying
the existing object and app paths. The policy model must cover these distinct
areas without granting unrelated authority:

| Role | Logical authority |
| --- | --- |
| Contributor | Create submissions or ordinary content |
| Publisher/editor | Publish curated content and feature annotations |
| Moderator | Write moderation actions and reversals |
| Verifier | Write verification and correction annotations |
| Dispatcher | Update task, request, commitment, and handoff state |
| Directory curator | Publish or carry app-index records |
| App endorser | Write only its own endorsement slot |
| App approver | Approve or revoke exact app versions and permission subsets |
| Governance recorder | Publish governance proposals and decisions |
| Root custodian | Issue initial authority, recovery, and migration records |

Endorser-slot ownership must remain bound to the entry signer so one community
cannot overwrite another community's endorsement.

## Governance ledger

Governance records are ordinary signed Willow entries with strict canonical
schemas. They include:

- Role and app-grant proposals.
- Decisions and human-readable reasons.
- Capability issuance, attenuation, delegation, and renewal receipts.
- Revocations with effective time and target capability fingerprint.
- Moderation appeals and reversals.
- Root recovery declarations and namespace migrations.

The ledger does not make a capability cryptographically valid. It determines
whether Riot currently accepts use of an otherwise valid capability. Every
policy record has deterministic ordering and conflict rules. Exact ties choose
the more restrictive result; conflicting non-tied records select the newest
valid decision while preserving both in the audit view.

### Leases and revocation

Meadowcap has no instant global revocation primitive. Riot combines:

- Renewable, time-bounded management capabilities.
- Signed revocation records.
- Immediate rejection by peers that know the revocation.
- Deterministic convergence after disconnected peers exchange governance
  state.
- Audit classification for actions accepted before a peer learned of the
  revocation.

Revocation cannot retroactively erase accepted replicated entries. Product
views may hide superseded or unauthorized-after-reconciliation actions while
the audit view preserves them.

## App and directory management

### Manifest requests

Every content-derived app version declares all requested powers. The
vocabulary covers Willow read/write areas, own-versus-shared Space data,
camera, photos, location, microphone, notifications, nearby transport,
clipboard, network access, duration, and background behavior. Undeclared
powers are unavailable.

### Space approval

An app approver grants an exact `app_id` only a subset of its declared powers.
One holder of the `apps/approve` authority may approve or revoke in the first
release. Multiple holders are supported; quorum evaluation is deferred. A
Space approval cannot force a device owner to grant sensitive platform
permissions.

Changing any app content changes `app_id` and requires a new approval. Sharing
or carrying an app never enables it.

### Runtime enforcement

- The native host retains receiver keys and capabilities.
- App bridge keys are relative and normalized by the host.
- Willow operations are restricted to the effective granted area.
- Writes remain attributed to the person using the app.
- Network and remote script loading remain denied unless explicitly requested
  and approved.
- Permission state is checked for every operation, not cached for the lifetime
  of an app process.
- Revocation closes the active session and removes the app from the Space's
  Tools row without deleting previously synchronized app data.

### Plural directories

Directory governance preserves the separate roles of author, carrier,
curator, endorser, and Space approver. Dedicated directory Spaces use the same
open-or-managed choice as other Spaces. A global storefront remains a local
computation over valid app-index entries, trusted Space approvals,
endorsements, provenance, and starter apps. There is no canonical directory
database or privileged Riot catalog namespace.

## End-to-end flows

### Create a managed Space

1. Generate the owned-namespace root key in secure storage.
2. Create the initial Space profile and governance records.
3. Generate a separate daily manager receiver key.
4. Issue time-bounded management read/write capabilities to it.
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
commits or returns a stable diagnostic without partial insertion.

### Serve protected reads

The requester proves possession of the read-capability receiver key using a
fresh session challenge. The serving peer verifies every request against a
covering capability and sends only authorized entries inside the encrypted
session. Challenge responses cannot be replayed into another session.

### Export and import

Write authority is portable with each authorized entry. Read authority does
not make plaintext files confidential. Riot refuses plaintext export of a
protected area; it must be encrypted for a receiver or transported inside the
private Space's MLS-protected artifact. Import validates all write authority
before committing anything.

### Remove authority

Publish a signed revocation, stop renewal, close locally active sessions, and
reject new uses immediately. Offline peers may temporarily accept a still
cryptographically valid lease. After reconciliation, all peers compute the
same current policy and retain partition-era actions in the audit history.

### Replace a compromised root

Create a replacement owned namespace. Publish a migration record signed by
the old root when available and corroborated in the governance ledger. Clients
freeze new management actions under the old namespace, preserve its history,
and guide members to the replacement Space. A missing old root cannot provide
cryptographic continuity and the UI must say so.

## Management experience

Space creation presents the power model, not protocol vocabulary:

- **Open Space:** anyone can contribute; moderation is a chosen community
  lens.
- **Managed Space:** selected people manage roles, moderation, membership, and
  apps.

Managed Space settings provide people and roles, invitations, expiring
access, moderation and appeals, apps and permissions, renewals and
revocations, governance history, recovery, and migration. Primary UI uses
plain language such as "Can moderate reports until August 1." An advanced
inspector may display full namespace IDs, receiver IDs, paths, expiry,
delegation chains, signatures, and revocation state. IDs are never truncated.

## Error handling

All authorization failures are fail-closed. Stable internal diagnostics are
safe to test and log, while user messages avoid protected identifiers and
cryptographic jargon.

| Condition | Behavior |
| --- | --- |
| Invalid or authority-expanding delegation | Reject; explain that the grant is invalid |
| Expired capability | Reject and offer renewal to an authorized manager |
| Known-revoked capability | Reject and record a non-secret audit diagnostic |
| Partition-era action discovered later | Exclude from normal current views; retain in audit history |
| Read request without covering authority | Reveal neither entries nor hidden path existence |
| Root unavailable | Enter recovery; never silently create a replacement identity |
| Root compromised | Guide a signed migration to a replacement namespace |
| Unsafe device clock for authority change | Block the time-sensitive operation until corrected |
| App asks for an undeclared or unapproved power | Deny the bridge call with an actionable permission error |
| App permission revoked while running | Close the app session and preserve existing data |
| Conflicting policy records | Apply deterministic restrictive tie-break and show the conflict |
| Oversized chain, record, or request | Reject before expensive cryptographic or storage work |

Logs never contain secret keys, recovery material, full capability bytes, or
protected paths. Diagnostic correlation uses non-reversible fingerprints.

## Resource and abuse controls

The implementation plan must pin limits for capability byte size, delegation
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

### Admission and replication

- Identical decisions for local writes, imported drops, and synchronized
  entries.
- No partial commits when one item in a batch fails.
- Fresh-challenge proof, challenge replay, receiver mismatch, stolen read
  capability, and cross-session reuse.
- No namespace, subspace, path, entry, count, or payload disclosure before
  authorization.
- Public visibility policy does not accidentally expose protected areas.
- Protected portable exports require encryption.

### Governance and partitions

- Lease renewal and expiry boundaries.
- Revocation before, during, and after disconnected operation.
- Different arrival orders produce the same effective policy.
- Restrictive exact-timestamp tie-breaks.
- Root recovery success/failure, compromise, and namespace migration.
- Audit history retains rejected and superseded decisions without activating
  them.

### Apps and platforms

- Permission intersection across manifest, Space, Meadowcap, device consent,
  and platform restrictions.
- Path traversal and cross-app/Space access attempts.
- Raw keys and capability bytes never reach JavaScript.
- Runtime revocation closes sessions.
- iOS Keychain and Android secure-storage behavior under lock, reinstall,
  backup/restore, failed authentication, recovery, and deletion.

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
   authority-expanding, revoked, expired, malformed, or policy-disallowed
   writes through the same local/import/sync path.
3. A person can create either an Open or Managed Space and the resulting
   namespace behavior matches the selected power model.
4. A managed Space can grant, renew, inspect, and revoke all defined roles
   without using the root key for daily writes.
5. An open Space can link an optional owned governance namespace without
   granting it write authority over communal participant subspaces.
6. Protected replication discloses no entries until the requester proves
   possession of a covering read-capability receiver key.
7. Protected areas cannot be exported as plaintext portable drops.
8. A Space can approve an exact app version with a permission subset; the app
   cannot exceed that subset or obtain raw authority material.
9. Sensitive device permissions still require local consent.
10. Known revocations take effect immediately, and partitioned peers converge
    deterministically while retaining an audit trail.
11. Root keys are secure-storage backed, recoverable only through the explicit
    encrypted recovery flow, and replaceable only by Space migration.
12. All blocking verification commands and the coverage gate pass.

## Success and failure measures

The feature is successful when every acceptance criterion has executable
coverage, cross-device tests demonstrate role grant and removal without a
server, protected sync tests show zero unauthorized disclosure, and an app
cannot exceed any independently varied component of its effective grant.

Before release, automated conformance must accept 100% of maintained valid
golden fixtures and reject 100% of maintained invalid fixtures; protected-sync
tests must observe zero identifier, path, count, entry, or payload disclosures
across every unauthorized case; all generated attenuation tests must show zero
authority expansions; and cross-device scenarios must converge to one
effective role/app policy for every tested message ordering. A field exercise
with at least three disconnected devices must complete Space creation, role
grant, protected sync, revocation, reconciliation, app approval, and recovery
without a server. Any unauthorized disclosure or authority expansion blocks
release regardless of aggregate pass rate.

The design has failed if Riot retains any feature-local organizer allowlist,
if local writes bypass imported-write checks, if a read capability works as a
bearer token without receiver proof, if communal Space governance can rewrite
participant subspaces, if a Space can force sensitive device consent, or if a
protected portable export can be produced in plaintext.

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
- The implementation plan must inventory current `willow25` APIs and pin all
  wire schemas, resource limits, and file scopes before coding.
