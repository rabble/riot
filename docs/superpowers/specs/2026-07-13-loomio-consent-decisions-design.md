# Loomio-inspired consent decisions design

Date: 2026-07-13
Status: User-approved direction; design review round 3 pending

## Product decision

Replace Riot's one-current-question Quick Poll with an opinionated,
Loomio-inspired consent board for small-team democracy. A community can run
several decisions at once. Each decision keeps its proposal, focused
discussion, revisions, reasoned positions, objections, and recorded outcome
together.

The first release targets Riot's current **public communal communities**. It
does not promise private deliberation: anyone who joins or receives a copy of
the communal namespace can read the governance roster, discussion, positions, objections, and
outcomes. Moderator rules control who participates, not who can read the
underlying plaintext records. The product states this before setup and before a
person contributes.

This adopts Loomio's useful human pattern—discuss, take a reasoned position,
revise when an objection exposes risk, and record an explicit outcome—without
shipping Loomio's full poll catalog. It does not adopt Agora's tokens,
delegation, blockchain execution, or DAO machinery.

The interface remains a signed miniapp. Democratic validity, authenticated
authors, causal authority, conflict detection, exact outcome evidence, and
deterministic projection live in Rust. JavaScript cannot decide who is a
moderator, whether a position is eligible, or whether consent was reached.

## First adopter and use cases

The first adopter is a Riverside-like neighborhood mutual-aid working group of
5–12 active people coordinating a public communal community. Today they make
low-stakes operational decisions in chat or meetings and later struggle to
recover what was proposed, whose concern changed it, and what the group agreed.

- **An eligible neighbor wants to propose a six-week street or supply trial so
  the group can test a concrete action without requiring permanent consensus.**
- **A participant wants to explain Support, Stand aside, or Object when they
  cannot attend a meeting so their concern enters the same record as the
  proposal.**
- **A facilitator wants to revise a proposal in response to an objection so
  people reconfirm against the exact changed text.**
- **A moderator wants to set one default participation rule so every proposal
  starts from an inspectable process rather than improvised chat norms.**
- **A future participant wants to read the final outcome and evidence so they
  understand what the group decided and why.**
- **A public reader wants to distinguish a live tally from a completed decision
  so Riot never overstates legitimacy.**
- **A participant who made a harmful or mistaken public comment wants to append
  a correction or withdrawal so the normal view stops amplifying it without
  falsifying history.**

Seeded evaluation scenarios are: a low-stakes proposal that reaches consent, a
proposal revised after a concrete objection, and a proposal closed with no
consent.

## Current-state problem

`fixtures/apps/quick-poll` supports one replaceable question, two to four
choices, and one plurality vote per profile. It lacks discussion, proposal
history, objections, consent, explicit outcomes, concurrent decisions, or
moderator-owned rules.

The generic app-data bridge is not a governance boundary:

- it returns JSON without verified Willow author metadata;
- claimed profile IDs inside values are not bound to entry authors;
- it selects one recency winner per relative key across author subspaces;
- app data is scoped to a content-derived app ID; and
- current reconciliation rejects inventories above 64 live entries, while the
  in-memory relaunch path persists only app-data receipts.

The feature therefore depends on the approved Rust-owned multi-space SQLite
architecture, a bounded Decisions session, and generic paged reconciliation.
It must not be implemented as a front-end-only Quick Poll rewrite.

## Goals

- Move a small public working group from proposal through deliberation to an
  explicit consent or no-consent outcome.
- Run several decisions concurrently without mixing their discussions.
- Make objections actionable rather than losing votes.
- Let moderators set one signed community default.
- Preserve inspectable signed evidence through offline exchange and relaunch.
- Produce identical authoritative projections from identical valid record sets,
  independent of arrival order.
- Keep everyday language free of Willow, capability, epoch, and DAG jargon.
- Fail visibly and conservatively when evidence or authority conflicts.

## Consent MVP and deferred work

The MVP contains:

- organizer bootstrap of moderators;
- moderator-managed participant roster and one community policy;
- several proposals with local drafts;
- focused comments plus author correction/withdrawal and moderator hide
  annotations;
- append-only proposal revisions;
- causal Support, Stand aside, and Object positions;
- explicit close intent, consent/no-consent/withdrawn outcomes, and conservative
  conflict handling;
- stable SQLite persistence, paged sync, bounded APIs, and public-data warnings;
- iOS and macOS hosts for the first trial.

Deferred follow-ups are facilitator transfer, post-outcome review notes, legacy
Quick Poll migration, anonymous/hidden ballots, process templates, private-group
encryption, Android UI exposure, app-produced-record provenance, app handoff
records, and automatic archival. The fixed Quick Poll remains a separate legacy
tool and its plurality votes never become consent.

Android must not expose a partial Decisions tool. Rust/UniFFI contracts remain
portable, but Android parity is a separately planned slice after the Apple trial.

## Non-goals and residual risks

- No advice, consensus, score, allocation, ranking, election, or budgeting
  process catalog.
- No secret ballot, coercion resistance, or protection from compelled device
  inspection.
- No token weighting, delegation, treasury action, or blockchain execution.
- No assertion that a Riot outcome is legally binding.
- No deletion of exchanged signed history.
- No organizer-key rotation or recovery in this release. Key loss leaves
  governance read-only; compromise is a documented residual risk.
- No confidentiality in public communal communities. Encrypted private-group
  governance requires the separate protected-sync design.

## Authority and causal epochs

### Organizer and moderator roster

The recognized community organizer is the root authority. The initial
`ModeratorRoster` contains the organizer. Only the organizer may publish a
successor moderator roster or Decisions app-access record. Each successor names
every current moderator and all predecessor heads it resolves.

Concurrent moderator-roster successors are a conflict. Until the organizer
publishes one successor referencing all heads, new policy, roster, proposal,
revision, close, and resolution commands fail closed. Existing signed records
remain inspectable.

### Moderator authority epochs

Every privileged record references an exact moderator-roster head. Timestamps
never grant or order authority. A roster successor closes the preceding epoch
and includes the canonical heads of every privileged action stream it observed;
those heads commit their causal ancestors.

An action under a closed epoch remains valid only when it is a named frontier
head or a causal ancestor of one. An unseen action under the old epoch that
arrives later is
`StaleAuthority`; it remains inspectable but cannot change policy or a decision.
Concurrent revocation therefore fails closed even if it rejects an honest
offline action. This conservative loss is preferable to allowing a revoked
moderator to backdate authority.

### Participant roster

An active moderator publishes a complete `ParticipantRoster` successor that
references the current moderator epoch and every participant-roster head it
resolves. Concurrent participant rosters block new proposals until a moderator
publishes one successor referencing all heads.

A proposal snapshots one unconflicted participant-roster record and the exact
full profile IDs it contains. Later participant removal does not rewrite an
open proposal. A removed participant remains eligible for that snapshotted
decision; moderators may withdraw it and open a replacement under a new roster
when that is unsafe.

Known Contributors is never used as membership evidence. Adding a participant
requires deliberate full-key verification or an in-person verified exchange;
display names and short tags are non-authoritative.

### Proposer and facilitator

Any snapshotted participant may propose. The proposal author remains its
facilitator for the MVP. The facilitator may revise and begin closure. An active
moderator may also revise, withdraw, or resolve authorized conflict types by
referencing the current moderator epoch. Facilitator transfer is deferred.

If the facilitator becomes unavailable, a moderator may continue the process.
If no valid moderator exists, the decision is readable but cannot be closed as
consent until the organizer repairs the moderator roster.

## Community default policy

An active moderator publishes a `DecisionPolicy` successor referencing the
current moderator epoch, one unconflicted participant-roster head, and every
policy head it resolves. Concurrent policies block new proposal opening; open
proposals retain their immutable snapshots.

The v1 policy fixes the process to visible consent and configures:

- who may propose: all snapshotted participants;
- who may facilitate and close: proposal author or active moderator;
- normal response window, default five days;
- participation rule: all participants or a fixed minimum response count; and
- the public-data disclosure shown before contribution.

The moderator-selected minimum must be between 2 and the roster size. Policy
changes affect new proposals only.

## Lifecycle

The authoritative signed lifecycle is:

```text
Open -> Close pending -> Consented | No consent | Withdrawn
  ^          |                 |
  |          v                 v
  +---- Reopened <------ Outcome conflict
```

Draft is host-local state. **Review due** is non-authoritative presentation:
`observed_now >= review_due_time` for an injected local observation time. It is
excluded from authorization, causal ordering, convergence claims, and signed
state.

### Draft and proposal creation

From the board, **Start a proposal** opens a host-owned local draft containing:

- title, maximum 120 UTF-8 bytes;
- proposal/context text, maximum 4,096 UTF-8 bytes;
- response window within 1–30 days; and
- the policy and participant heads last previewed.

Drafts are stored in `local_state` by namespace, Decisions collection, profile,
and draft ID. They are never synced. Each edit autosaves transactionally.
Leaving keeps the draft; Discard requires confirmation. Each profile may retain
8 drafts and 64 KiB of draft JSON per community. `draftsPage(cursor, limit)` is
capped at 20; reaching the quota asks the person to open or discard a draft and
never discards one automatically.

Before opening, Review shows the exact proposal, facilitator identity,
participant count, response rule, review date, and this warning:

> This is a public community record. Anyone who joins or receives a copy of
> this community can read the proposal, discussion, names, positions,
> objections, and outcome.

Opening rechecks the policy and roster heads. If stale, the draft remains and
Review refreshes before confirmation. A successful command atomically commits
`DecisionOpened` plus revision 1, then deletes the local draft.

### Discussion and contribution remedies

Snapshotted participants may add plain-text comments of at most 2,048 UTF-8
bytes. Authors may append one correction/withdrawal chain per comment.
Withdrawal hides the body in the normal timeline but preserves a deliberate
technical-history disclosure.

An active moderator may append a hide-with-reason annotation for harassment,
sensitive personal data, or unsafe content. It does not erase bytes already
exchanged. Public permanence is stated before submission.

Required text rejects empty or whitespace-only values. Core rejects control,
bidi-override, and unsafe zero-width code points; rendering uses text nodes.

### Position causal state

For each `(decision, revision, participant)`, `Position` forms a causal stream.
The first position references no position predecessor; every update references
all current heads known to the author. Each record also references the exact
decision phase head and proposal revision.

Concurrent position heads are a **Position conflict**. The participant resolves
it by publishing one successor referencing all heads and choosing one stance
and reason. Until resolved, the participant counts as responded but the
decision cannot close as consent. No arrival-time or last-write-wins choice is
allowed. A moderator can inspect or annotate the conflict but cannot resolve,
suppress, or choose another person's stance.

- **Support:** safe enough to try.
- **Stand aside:** not supported, but not prevented.
- **Object:** a concrete reason the proposal is not safe enough to try; a
  non-whitespace reason is required.

Only the objector can remove their objection by publishing a successor position.
A facilitator response annotates the concern but does not resolve it.

### Revision causal state

A revision replaces the full proposal text, references every current revision
head, and includes a required change summary. It creates a new phase head and
makes all earlier positions historical.

Concurrent revisions are a Revision conflict. Discussion stays available;
position and closure commands are disabled. A facilitator or active moderator
publishes one successor referencing all heads and explains the reconciliation.

### Close intent and exact evidence frontier

Closure is two-step and never inferred from a tally.

`CloseIntent` references:

- the exact unconflicted policy, participant roster, moderator epoch, decision
  phase, and proposal revision heads;
- every eligible participant ID;
- for each participant, every current position head known or an explicit
  `NoPosition` marker;
- the requested outcome and required statement; and
- the exact evidence-set digest.

Core verifies that the named position heads exist, belong to their participant,
target the revision, have no unresolved conflict, and meet the participation
rule. Consent additionally requires zero Object heads. The signed timestamp is
display metadata only.

`DecisionOutcome` repeats the evidence digest and counts. It can be committed
only from a valid close intent. The normal UI labels it **Recorded outcome**,
not an absolute statement that every disconnected device had synchronized.

After `beginClose`, the phase is **Close pending**. Position, revision, and new
close commands are disabled; discussion remains readable. The board shows
**Finish recording outcome** to the facilitator and active moderators after
relaunch. `recordOutcome` revalidates the close intent and current local
evidence. If new unincluded evidence is already present it returns `Conflict`
without committing. `cancelClose` publishes a branch resolution referencing
the close-intent head and a successor phase head that returns the unchanged
revision to Open.

Any subsequently learned valid position on the closed revision that was not in
the close frontier is **Unincluded evidence**, never “authored after closure.”
It creates Outcome conflict. If it is Object, the consent outcome cannot be
selected during resolution.

`resolveOutcomeConflict` is one atomic Rust transaction and has three typed
modes, each referencing all conflicting close intents, outcomes, and unincluded
position heads:

- **Record no consent:** commits `BranchResolution`, a successor `CloseIntent`
  with the expanded evidence frontier, and a No consent `DecisionOutcome`.
- **Replace recorded outcome:** allowed only when expanded evidence has no
  Object or position conflict; commits the same three-record successor chain
  and may preserve Consented.
- **Reopen with revision:** commits `BranchResolution` plus a new
  `ProposalRevision` whose predecessor phase is the conflict and whose change
  summary explains the response. The resulting phase is Open and every
  participant must position on the new revision.

Because every successor descends from all conflict heads, it is not concurrent
with the records it resolves. An unincluded Object permits only Record no
consent or Reopen with revision. `Withdrawn` may be recorded immediately by the
facilitator or active moderator without meeting the participation minimum;
No consent may also record insufficient participation. Only Consented requires
the configured minimum.

Concurrent close intents or outcomes also create Outcome conflict. No outcome
is canonical while conflict exists.

## Canonical records and Willow paths

All protocol payloads use canonical integer-key CBOR maps. Decoders reject
duplicates, indefinite lengths, unknown required enum values, unsorted head
arrays, and any value whose canonical re-encoding differs. Head arrays sort by
full 32-byte head ID.

The collection ID is
`SHA256("riot/decisions-collection/v1" || namespace_id)`. A payload has a stable
logical record ID, but its immutable coordinate ends in a digest-bound variant:

```text
decisions / v1 / <collection-id> / <kind> / <object-id> / <variant-id>
```

`payload_digest` is SHA-256 of canonical payload bytes. `variant_id` is
`SHA256("riot/decisions-variant/v1" || kind || object_id || logical_record_id ||
author_id || payload_digest)`. `head_id` is
`SHA256("riot/decisions-head/v1" || collection_id || kind || object_id ||
logical_record_id || author_id || variant_id || payload_digest)`.
The evidence-set digest uses
`SHA256("riot/decisions-evidence/v1" || canonical_cbor(authority_heads,
phase_head, revision_head, sorted_participant_ids,
sorted(participant_id -> position_heads | NoPosition), requested_outcome,
outcome_statement))` and is always recomputed by core.

Different semantic payload variants necessarily occupy distinct Willow paths,
so Willow pruning cannot remove an equivocation branch. More than one variant for the
same `(kind, object, logical ID, author)` **poisons that logical stream** and
blocks dependent authority or consent until an authorized successor references
every variant head. Equivocation is never discarded as merely invalid and is
never resolved by timestamp.

Re-proving the identical canonical semantic payload at the same variant path is
not a new head; the current live Willow proof is interchangeable. Outcomes
reference semantic head IDs, not a timestamp-bearing entry proof, so pruning an
identical duplicate cannot erase the named decision evidence.

MVP records are:

| Record | Purpose |
| --- | --- |
| `ModeratorRoster` | Organizer-authored moderator epoch and closure frontier |
| `DecisionAppAccess` | Organizer-authored exact app grant/revoke successor used only for local session access |
| `ParticipantRoster` | Complete governance participant successor |
| `DecisionPolicy` | One community default successor |
| `DecisionOpened` | Immutable policy, roster, participants, and initial phase |
| `ProposalRevision` | Full proposal successor and change summary |
| `DiscussionComment` | Append-only focused contribution |
| `CommentAnnotation` | Author correction/withdrawal or moderator hide annotation |
| `Position` | Participant's causal stance stream on one revision |
| `CloseIntent` | Exact participant/position evidence frontier |
| `DecisionOutcome` | Recorded outcome tied to one close intent |
| `BranchResolution` | Typed resolution for app access, moderator roster, participant roster, policy, revision, close, or outcome heads; never another person's position |

Every record contains schema version, record and collection IDs, verified full
author ID, predecessor/authority/phase heads, payload digest, and signed Willow
timestamp. Identity-bearing fields must match verified entry metadata.

## Deterministic validity rules

- Timestamps affect display ordering and Review due only.
- Every mutable logical stream uses explicit predecessor heads.
- Missing predecessors produce `MissingEvidence`, not partial validity.
- Concurrent authority, roster, policy, revision, position, close, or outcome
  heads disable any action that would overstate consent.
- Only a `Position` successor signed by the verified participant may resolve
  that participant's position variants or heads; an Object remains blocking
  until that successor changes it.
- Branch resolution must reference every current head. A resolution that loses
  a race returns `StaleHead` and does not commit.
- Projection validity is a pure function of the accepted signed record set and
  explicit observation time only for non-authoritative reminder fields.
- Invalid records remain in redacted technical diagnostics but never consume a
  participant response or authority slot.

## Persistence prerequisite

Decisions do not ship on the current in-memory `EvidenceStore`. They depend on
the approved `RiotDatabase`/`SpaceSession` architecture in
`2026-07-12-multi-space-sqlite-store-design.md`:

- canonical entries and proofs persist in `accepted_entries`;
- live immutable records remain addressable in `live_entries`;
- Decisions projections are rebuildable SQLite tables keyed by namespace and
  collection;
- commands commit signed entry, projection changes, and monotonic `change_log`
  sequence in one Rust-owned transaction;
- local drafts use `local_state` in a separate transaction and never enter
  Willow sync;
- startup rebuilds/quarantines per record, so one corrupt record cannot blank a
  community; and
- schema migration is ordered and transactional under `schema_migrations`.

For SQLite-backed Decisions paths, this design normatively supersedes that
design's sentence retaining the prototype's 1,024-entry/16-MiB whole-store
ceiling. The legacy in-memory store keeps its existing ceiling; Decisions uses
the per-space/global accounting below. The implementation prerequisite updates
the checked-in manifest, limit constants, and SQLite design contract together
before any Decisions record is admitted.

Swift does not maintain a parallel receipt array. If the database commit
succeeds but UI acknowledgement is interrupted, retrying the same command ID
returns the committed result.

## Generic paged reconciliation prerequisite

The existing `org.riot.conference-sync/1` 64-ID whole-namespace inventory is
insufficient. Decisions depends on a generic sync v2 before product exposure:

- peers negotiate v1 or v2; v1 behavior remains unchanged for legacy peers;
- v2 summarizes one namespace/area snapshot in sorted pages of at most 64 IDs;
- every page carries snapshot ID, area, cursor, and completion flag;
- requests and entry bundles remain capped at 64 records and existing byte
  limits;
- a stable snapshot cannot mix pages from different inventory generations;
- disconnect/retry resumes by snapshot/cursor or restarts safely;
- collection-prefix areas permit bounded Decisions reconciliation without
  hiding other namespace records; and
- received entries still pass the existing canonical authorization and import
  boundary before Decisions projection.

This is a generic Riot sync improvement, not a Decisions-only transport.

The sender materializes each advertised sorted ID list in SQLite. A snapshot
pins IDs, not extra payload copies, for 10 minutes; at most 8 snapshots and
16,384 IDs per namespace are retained. Requested immutable entries remain in
`accepted_entries`. Expired or evicted snapshots return `SnapshotExpired`, and
the receiver discards that partial inventory and restarts from page one.
Snapshot rows survive process restart until expiry; cleanup never removes
accepted evidence.

## Capacity, denial of service, and performance

The first release supports and tests these semantic limits:

- 32 governance participants;
- 8 simultaneously open decisions and 2 open proposals per author;
- 8 revisions per decision including revision 1;
- 4 total Position records per participant per revision, including the initial
  position and three successors;
- 16 comments per author and 128 comments total per decision;
- 4 annotations per comment;
- 8 close intents, 8 outcomes, and 16 branch resolutions per decision;
- 64 app-access records, 64 moderator-roster records, 128 participant-roster
  records, and 128 policy records per community;
- at most 8 unresolved heads for each authority/configuration stream and 32
  unresolved heads for a decision stream; and
- 256 closed decisions retained and synchronizable.

Limits are checked by Rust for local commands and deterministic projection of
imports. Per-author quotas prevent one participant consuming another's
allowance. Duplicate variants cost no second quota. Position and authority
limits follow causal ordinal. Comments and annotations reference the author's
current per-decision contribution heads; concurrent records at the last
available ordinal are admitted by full head-ID order up to remaining quota and
the rest are deterministically `OverQuota`, independent of arrival order.

SQLite physically allocates each public community 72 MiB: 14,336 ordinary
canonical records/56 MiB, 2,048 protected canonical records/8 MiB, a
1,024-digest/4-MiB rejected-record quarantine, and 4 MiB of indexes/metadata.
The global database supports 8 full-capacity communities: 131,072 canonical
records and 576 MiB including quarantine/metadata. Additional communities may
exist only while both their per-space and the global limits remain available.

The protected partition admits only semantically authorized app access,
moderator/participant roster, policy, branch resolution, moderator hide,
close-intent, and outcome records within the exact stream quotas above.
Ordinary comments, revisions, and position churn cannot consume it. Decisions
path admission performs canonical Willow verification and domain semantic/quota
preflight in one Rust-owned transaction before storing full bytes.
Unauthorized, malformed-domain, or `OverQuota` variants store only a bounded
digest, author, kind, and reason in quarantine. After 64 digests per author or
1,024 per community, Riot increments an aggregate rejection counter and retains
no attacker payload/proof. Reimport may be re-evaluated but cannot consume the
protected partition.

A quarantined digest that collides on logical stream identity with an admitted
variant still marks that stream `Poisoned`; it can never make an Object,
revocation, or authority branch disappear from consent accounting. Repair
requires an authorized successor referencing every retained full head and every
quarantined variant digest. If required bytes are unavailable, the stream stays
fail-closed.

A canonical record whose authorization cannot yet be decided solely because a
predecessor is missing uses a separate pending-evidence slice within the same
4-MiB quarantine: at most 128 full records/author and 512/community. Arrival of
a predecessor re-evaluates it transactionally. Eviction is full-head-ID order;
sync may reoffer an evicted record, and no pending record enters the projection
or protected quota before authorization succeeds.

At the ordinary or global ceiling Riot refuses new proposals with **This
community's decision record is full**; it never deletes evidence automatically.
The protected partition remains available for repair and closure until its own
stream/global cap. Export/archive and raising limits are follow-up work. All
conflict-head queries page at 32 and return total count plus `hasMore`, never an
unbounded head array.

Release-build budgets, measured over ten runs at the supported maximum, are:

- open board first page under 500 ms;
- proposal header and first 50 timeline items under 500 ms;
- committed local command visible under 200 ms after transaction commit; and
- each 64-record projection batch under 100 ms excluding transport time.

Missing a budget blocks the performance claim and trial release.

## App trust and upgrades

Tool availability is one composed gate, never two competing authorities. The
host constructs a Decisions session only when:

1. the exact installed package has a currently accepted organizer TrustMarker
   under the existing app runtime contract;
2. its manifest declares machine capability `org.riot.decisions.v1`; and
3. the same app ID is named by one unconflicted `DecisionAppAccess` head.

The session approval generation is the tuple `(trust_marker_entry_id,
decision_app_access_head, database_generation)`. Every query and command
rechecks the tuple transactionally. A change to either authority invalidates
the session, removes/recomputes the Tools listing, closes an already-open tool,
and prevents immediate reopen until all three conditions are true again.

App-access successors are organizer-authored, reference all current access
heads, and never use timestamp ordering.

This requires manifest schema v2 to add a bounded machine-readable
`capabilities` array. Existing `permissions` remains plain-language review copy
and cannot grant native access. Schema-v1 apps cannot request Decisions.

App access is a **local execution boundary**, not remote record provenance. A
human-signed Decisions record remains valid or invalid based on human/community
authority; it does not claim which app produced it.

Revocation removes both read and command access, closes the tool, and returns to
Tools as required by the community-navigation contract. It never leaves a
revoked app in a read-only collection view.

A successor app ID may access the same deterministic collection only after a
new exact TrustMarker review and an app-access successor that names it. No claim about
which app authored synchronized human records is made. A malicious
organizer-approved app remains a residual risk; bounded native commands,
confirmation for policy/roster/outcome actions, and exact bundle review reduce
but cannot eliminate it.

Initial access is not circular. A native host-owned
`CommunityGovernanceAdminV1.bootstrapDecisions` operation is available from
Community settings after the exact TrustMarker is accepted but before a
`DecisionSessionV1` exists. The organizer must first acknowledge the fixed
public-data disclosure in trusted native UI. The operation takes app ID,
expected TrustMarker entry ID, expected empty app-access/moderator heads, and an
idempotency ID; it atomically commits the initial `DecisionAppAccess` and
`ModeratorRoster` containing the organizer. Retry returns the original commit;
non-empty/stale heads fail without mutation. Successors use the corresponding
native `updateDecisionsAccess` operation.

## Bounded API contract

`DecisionSessionV1` is immutably bound to namespace, signer, app ID, collection
ID, the composed approval-generation tuple, and database generation. JavaScript
supplies none of those values.

Queries are cursor-bounded:

- `bootstrapView()` returns role, setup state, current causal heads, public-data
  disclosure, schema version, and supported limits;
- `boardPage(filter, cursor, limit, observedNow)` returns at most 50 summaries;
- `decisionView(decisionId, cursor, limit, observedNow)` returns header plus at
  most 50 timeline records;
- `conflictHeadsPage(streamId, cursor, limit)` returns at most 32 full heads;
- `changes(afterSequence, limit)` returns durable invalidations; host push is a
  reload hint, and missed pushes resume from sequence; and
- `draftsPage(cursor, limit)`, `loadDraft`, `saveDraft`, and `discardDraft`
  operate only on host-local SQLite state with the 20-row page and draft quotas.

Commands include `publishModeratorRoster`, `publishParticipantRoster`,
`publishPolicy`, `openDecision`, `appendComment`, `annotateComment`,
`publishRevision`, `setPosition`, `beginClose`, `recordOutcome`, and
`cancelClose`, `resolveBranch`, and `resolveOutcomeConflict`. `setPosition` is
the only position-resolution command and core requires the signer to equal the
position participant.

Every command carries a caller-generated idempotency ID plus expected causal
heads. Core derives record IDs and author. Repeating the same command ID and
payload returns the original `Committed { recordId, changeSequence }`; the same
ID with different bytes returns `InvalidCommand`. A head mismatch returns
the head count and first bounded page without committing.

Roster, policy, moderator revision, close, outcome, access, and branch-
resolution commands do not proceed from a JavaScript confirmation alone. The
host renders trusted native review from the exact canonical command bytes,
acting full identity, expected heads, and public consequence; confirmation then
revalidates those same bytes and heads at commit. A changed byte/head cancels
the signature request and returns `StaleHead`.

Stable error codes and recovery are:

| Code | Recovery |
| --- | --- |
| `NotAuthorized` | Remove action; explain which role is required |
| `PublicDisclosureRequired` | Return to Review and require acknowledgement |
| `StaleHead` | Reload current heads while preserving draft/input |
| `Conflict` | Open participant or moderator conflict view |
| `MissingEvidence` | Keep read-only and retry sync/import |
| `Validation` | Focus the named field; preserve draft |
| `Capacity` | Explain the exact reached limit |
| `SnapshotExpired` | Reset the page to the first snapshot and preserve focus/filter |
| `AppRevoked` | Close tool and return to Tools |
| `UnsupportedVersion` | Block mutation and offer app update |
| `Storage` | Preserve local input and offer retry |

Requests and responses are versioned UniFFI records. The JavaScript shim maps
them to camelCase and exposes no arbitrary key/value escape hatch.

## First-use and moderator flows

Setup lives in **Community settings > Decisions**; everyday policy inspection
lives in the tool's **How we decide** view.

1. Organizer approves the exact Decisions app.
2. Trusted native setup shows the fixed public-data disclosure; no identity-
   bearing Decisions record exists until the organizer acknowledges it.
3. The native bootstrap transaction commits app access and a moderator roster
   containing the organizer.
4. Organizer verifies and adds at least one other participant by full key.
5. A moderator publishes the participant roster.
6. A moderator chooses minimum responses and response window, reviews the same
   disclosure, and publishes the first policy.
7. The Decisions tool becomes writable.

Before completion, members see **Decisions isn't set up yet** and the missing
step. Only the authorized actor sees the repair action. Zero participants,
minimum above roster size, conflicting setup heads, or no active moderator all
remain read-only with a precise recovery path. If the organizer key is lost,
Riot truthfully says setup cannot be repaired in this release.

Roster removal and policy edits show the exact successor, affected future
proposals, public-data warning, and confirmation. Open proposals retain their
snapshots.

Each profile acknowledges public contribution once per policy head in
host-local state. A policy successor resets acknowledgement. Every composer
still shows one-line public permanence copy; only the first contribution under
that policy opens the full acknowledgement. It is never inferred from app use.

## Board, proposal, and conflict UX

The default landing filter is **Needs you**, followed by **Open** and **Closed**.
Needs you is a filtered subset of Open, not a lifecycle state. Empty states say
whether there are no decisions, no decisions needing this person, or setup is
incomplete.

Board ordering is: reconfirm after revision, no current position, active
objection needing facilitator attention, locally Review due, then recent valid
activity. The total sort key is `(priority, signed_activity_timestamp desc,
activity_head_id asc, decision_id asc)`. Timestamps do not affect validity.
Timeline order is causal depth, signed timestamp, then full head ID. Concurrent
records therefore have one deterministic presentation order. Cards show compact
participation; full counts live on detail.

Board and timeline cursors bind a projection change-sequence snapshot and the
last total sort key. Any relevant mutation between pages returns
`SnapshotExpired`; the client restarts at page one while preserving filter and
focus. It never combines two projection generations.

Proposal detail leads with exact current text, then participation, active
objections, and the chronological discussion/revision timeline. Helper copy
defines Stand aside and asks an objector: **What harm or risk must change before
this is safe enough to try?** Every composer repeats concise public permanence
copy before submission.

Participants encountering a policy, roster, revision, position, or outcome
conflict see a banner, can inspect all branches, and retain read/discussion where
safe. Actions that could overstate consent are disabled.

For a Position conflict, only that participant sees **Choose your current
position**. The view compares their variants, requires a fresh stance/reason,
and signs a Position successor referencing every head. Moderators can inspect
and annotate but have no resolution action.

For other conflicts, the actor authorized for that record type opens **Resolve
conflict**, compares full authors and records in columns on wide screens or
stacked cards on narrow screens, composes the permitted successor, reviews
consequences, and confirms. If another resolution wins first, `StaleHead`
reloads comparison without discarding composed text. An Object in unincluded
evidence forbids selecting the prior consent outcome.

Close pending appears on board and detail with the exact evidence summary.
Facilitator or active moderator can **Finish recording outcome** or **Cancel
close and return to discussion** after relaunch. If unincluded evidence arrives
before outcome commit, Finish becomes Resolve outcome conflict; no stranded
close intent is silently finalized.

## Offline and error behavior

- Success says **Saved on this device** and may say **Waiting for nearby
  exchange**.
- Review due is a local reminder, never automatic closure.
- Unincluded evidence is named exactly; Riot never claims it was authored after
  closure.
- One malformed, over-quota, spoofed, or unauthorized record cannot blank a
  board or decision.
- Missing predecessors or profiles keep affected actions read-only and retryable.
- Profile names resolve at render time; authorization uses full verified IDs.
- Full IDs appear only in deliberate Technical details and verification flows,
  not ordinary VoiceOver labels.
- Revoked app access closes the tool rather than leaving a stale view.

## Accessibility and visual requirements

- 44-point touch targets, visible keyboard focus, WCAG AA contrast, and no
  color-only meaning.
- Semantic headings, labels, validation messages, and status announcements.
- Dynamic Type/zoom and 320-point widths without clipping reasons or outcomes.
- Reduced motion.
- Board/detail routes, composers, confirmation dialogs, and branch comparison
  have defined initial focus, Escape/Back behavior, dirty-draft confirmation,
  and focus restoration.
- Sync updates announce without stealing focus.
- Normal identity labels use current display name plus key-derived tag; full IDs
  require deliberate verification disclosure.
- Visual review at 390x844 and 1280x800 covers setup, Needs you, Open, proposal
  creation/recovery, revision, objection, each conflict type, policy, empty,
  error, revocation, and closed outcome.

## Security and privacy

- Every namespace holder can inspect plaintext Decisions records. Participation
  gating is not confidentiality.
- JavaScript receives no signing key, capability secret, namespace selector, or
  author selector.
- Authority uses causal heads/frontiers, never device time.
- Full IDs or in-person exchange verify role changes; names and short tags do not.
- Per-author quotas and reserved authority capacity mitigate valid-record spam.
- Diagnostics are local, structured, payload-redacted, and never automatically
  export roster lists, proposal text, reasons, signatures, or capability bytes.
- Plain text is byte-bounded; unsafe control/bidi characters are rejected and
  stored HTML is never executed.
- Existing CSP, blocked navigation/network, canonical import, and item-isolated
  diagnostics remain mandatory.
- Visible named positions permit social coercion and retaliation. The warning
  is explicit; coercion resistance requires a different future process.

## Executable TDD contracts

Planning must create tests before implementation for these minimum contracts:

| Given | When | Then |
| --- | --- | --- |
| Accepted exact TrustMarker and no Decisions heads | Organizer acknowledges disclosure and bootstraps | App access plus initial moderator roster commit atomically and retry is idempotent |
| Two record permutations contain identical valid evidence | Project both | Authoritative views are byte-identical |
| Same logical position ID has two digest-bound variants | Either arrives first | Both immutable heads remain and the position stream is poisoned/blocking |
| A moderator epoch closes without an offline action in its frontier | That action arrives | It is `StaleAuthority` and changes no projection |
| One participant creates concurrent Support and Object heads | Either arrives first | Position conflict blocks consent until that participant resolves it |
| A revision commits | Prior positions exist | They remain historical and Needs you requests reconfirmation |
| Close intent omits a known position head | Core validates | Command fails without commit |
| An unincluded offline Object arrives after consent is recorded | Core reprojects | Outcome conflict appears and consent cannot be selected |
| A public draft has unsaved/failed open | App relaunches | Draft and focusable field values recover locally |
| App approval generation changes while open | Next query/command runs | Session returns `AppRevoked` and host closes the tool |
| Sync inventory exceeds 64 records | Peers reconcile with v2 | All pages converge without mixed snapshots |
| A participant exceeds comment quota | Another participant comments | Only the offender is rejected; reserved authority commands still commit |
| Unauthorized communal authors fill ordinary/quarantine limits | Organizer repairs authority | Full repair bytes still commit in the protected partition |
| A cursor's projection sequence changes between pages | Next page is requested | `SnapshotExpired` resets pagination without mixing generations |

Required reusable test infrastructure is `DeterministicIdentityFactory`,
`TempRiotDatabase`, `FakeObservationClock`, `ProjectionPermutationHarness`,
`TwoNodePagedSyncHarness`, `DecisionSessionFake`, and native/JavaScript bridge
contract fixtures generated from the same UniFFI schema.

Each work unit follows RED (contract test fails), GREEN (minimum behavior), then
REFACTOR with the full affected suite. The implementation plan must enumerate
those cycles method by method, including every stable error code and head-race.

Before production code, work unit 0 extends `.coverage-thresholds.json`—the
single source of truth—to invoke checked-in `scripts/coverage-rust.sh`,
`scripts/coverage-miniapps.mjs`, and `scripts/coverage-apple.sh`. They enforce
line/branch/function/statement coverage for Rust and Istanbul-instrumented
miniapp JavaScript, plus LLVM/Xcode line/function/branch coverage for changed
Swift modules. Each available configured metric is 100 percent. Generated
UniFFI bindings may be excluded only by an explicit checked-in path exemption.
PR creation and task completion remain blocked until every configured command
passes.

Final verification also runs workspace tests, strict formatting and Clippy,
SQLite migration/recovery tests, UniFFI contract tests, iOS/macOS builds and
tests, Playwright behavior/accessibility/visual review, two-node and multi-node
paged sync, hostile codecs/rendering, supported-capacity performance runs, and
the repository green script.

## Product trial and failure criteria

Within 14 calendar days of a release-candidate build, run a moderated trial with
5–9 people matching the neighborhood mutual-aid persona and the three seeded
scenarios. The task script also samples organizer setup/recovery, two concurrent
decisions, and comment correction/withdrawal. Percentages below use all assigned
task attempts as the denominator; comprehension uses all scored question
responses.

Release requires:

- at least 80 percent complete proposal-to-position and outcome-retrieval tasks
  without facilitator intervention;
- at least 90 percent correctly identify the current revision, whether consent
  was reached, and why;
- every facilitator correctly handles the objection/revision/reconfirmation
  scenario without recording false consent;
- at least 80 percent rate confidence in understanding the recorded outcome at
  4 or 5 on a five-point scale; and
- zero participants incorrectly report that contributions are private.

Any unauthorized action, lost committed record, false consent, hidden branch,
or mistaken privacy belief pauses release. Missing a percentage threshold also
pauses release; findings are incorporated and the same scenario is rerun before
expanding beyond the trial.

## Delivery dependencies and work units

The implementation plan must order these reviewable units:

0. multi-surface 100-percent coverage enforcement;
1. multi-space SQLite prerequisite, superseding capacity manifest, physical
   admission partitions, and persistent generic signed records;
2. generic paged reconciliation v2 and stable snapshot cursors;
3. canonical Decisions codecs, digest-bound paths, causal authority, exact
   quotas, and deterministic projection;
4. composed app approval/bootstrap, bounded UniFFI/native session, local drafts,
   stable errors, and watches;
5. organizer setup, roster, policy, native confirmations, and authority conflict
   UX;
6. board, proposal creation, discussion, correction/withdrawal, revision, and
   causal positions;
7. close intent, outcomes, unincluded-evidence conflicts, and resolution UX;
8. accessibility, visual review, hostile tests, capacity/performance, and the
   Apple product trial.

No UI work unit may simulate authority, use generic app-data writes, or call a
live tally consent. Decisions remain unavailable until SQLite and paged sync
prerequisites pass their own gates.

## Definition of done

- Public-data boundaries are disclosed truthfully before setup and contribution.
- Several decisions can be open and independently discussed.
- Organizer, moderator, participant, policy, facilitator, and app boundaries are
  enforced from verified causal evidence in Rust.
- Positions and every mutable authority stream have explicit conflict semantics.
- Revisions preserve history and require reconfirmation.
- Consent cannot be recorded with insufficient participation, an active Object,
  unresolved heads, or omitted known evidence.
- Causally concurrent unincluded objections force Outcome conflict.
- SQLite relaunch and paged sync preserve exact outcome evidence beyond 64
  records.
- Quotas isolate abusive authors and reserve governance repair capacity.
- The typed bridge, setup, proposal, conflict, failure, and revocation flows are
  implementable from this contract.
- All functional, security, accessibility, visual, performance, product-trial,
  and configured 100-percent coverage gates pass.

## Product references

- Loomio, “Proposals”: <https://help.loomio.com/en/user_manual/polls/proposals/>
- Loomio, “Proposals and polls”: <https://help.loomio.com/en/user_manual/polls/intro_to_decisions/>
- Loomio, “Outcome”: <https://help.loomio.com/en/user_manual/polls/outcomes/>
- Loomio, “Discussions”: <https://help.loomio.com/en/user_manual/threads/intro_to_threads/>
- Agora, “Governance Technical Overview” (contrast only):
  <https://docs.agora.xyz/governance-technical-overview>
