# Loomio-inspired consent decisions design

Date: 2026-07-13
Status: User-approved; metaswarm design review pending

## Product decision

Replace Riot's one-current-question Quick Poll model with an opinionated,
Loomio-inspired consent board for small-team democracy. A community may run
several decisions at once. Each decision keeps its proposal, discussion,
revisions, reasoned positions, objections, and recorded outcome together.

This design adopts the useful human pattern from Loomio—focused discussion,
reasoned positions that may change as understanding changes, deliberate
closure, and a durable outcome record—without adopting Loomio's entire poll
catalog. It does not adopt Agora's token weighting, delegation, blockchain
execution, or DAO proposal machinery.

The Decisions interface remains a signed miniapp. Democratic authority,
authenticated record metadata, lifecycle validity, and deterministic
projection live in Rust below the miniapp. JavaScript never decides whether a
person is a moderator, whether a position is eligible, or whether consent was
reached.

## Current-state problem

The shipped `fixtures/apps/quick-poll` app supports one replaceable question,
two to four choices, and one changeable plurality vote per profile. It does not
support focused discussion, proposal history, objections, consent, explicit
outcomes, several simultaneous decisions, or moderator-owned community rules.

The generic app-data bridge is also insufficient for legitimate governance:

- `list` returns relative key and JSON value but not the verified Willow author
  or signed entry metadata;
- values can contain a claimed profile ID that is not bound to the entry author;
- reads select one winner per relative key across author subspaces; and
- app data is scoped to a content-derived app ID, so an ordinary app upgrade
  receives a different data scope.

Those properties are acceptable for the current demo poll but cannot establish
moderator authority, participant eligibility, or a durable consent outcome.
The redesign therefore requires a bounded Decisions capability and stable
collection before the new interface ships.

## Goals

- Help a small community move from a concrete proposal through deliberation to
  an explicit consent outcome.
- Let several decisions proceed concurrently without losing focus inside any
  one proposal.
- Make objections actionable rather than treating them as a losing vote.
- Let community moderators set one default decision policy.
- Preserve an inspectable, signed history across offline exchange and app
  upgrades.
- Keep the everyday interface legible to people who do not know Willow,
  namespaces, capabilities, or consensus algorithms.
- Fail visibly and conservatively when authority, eligibility, or outcome state
  cannot be established.

## Non-goals

- A catalog of advice, consensus, score, allocation, ranking, election, or
  participatory-budgeting processes.
- Anonymous or secret ballots, hidden intermediate results, or coercion-resistant
  voting.
- Token-weighted voting, vote delegation, quorum tokens, treasury actions, or
  blockchain execution.
- A general chat replacement. Discussion is scoped to one decision.
- Automatic claims that a decision is binding outside Riot.
- Automatic moderator selection or a universal community constitution engine.
- Retrofitting old plurality votes into consent positions.
- Deleting or rewriting signed discussion and decision history.

## People and authority

### Organizer

The existing recognized community organizer is the recovery/root authority for
the Decisions capability. The organizer approves the exact Decisions app
bundle, grants or revokes moderator status, and can resolve a moderator-policy
branch that moderators cannot reconcile. App trust does not by itself confer
moderator authority.

### Moderator

An active moderator may maintain the governance participant roster and publish
a successor community decision policy. Moderator grants are signed under the
community authority and evaluated by Rust; a profile ID inside JSON grants
nothing.

Each policy names its predecessor. Concurrent valid successors form a visible
policy conflict. Existing proposals continue under their snapshots, but new
proposals cannot open until a moderator publishes one successor referencing all
conflicting heads or the organizer resolves the conflict.

### Eligible participant

Eligibility comes from a moderator-maintained, signed governance participant
roster. It must not be inferred from Riot's Known Contributors view, because
observing an author's content does not prove membership. The default policy
allows every currently recognized governance participant to discuss and take a
position.

### Proposer and facilitator

The default policy allows any eligible participant to start a proposal. Its
author is the initial facilitator. A moderator may transfer facilitation to
another eligible participant through a signed transfer record. The current
facilitator or an active moderator may revise or close the proposal. Every
facilitator action is validated against the proposal's policy snapshot and
subsequent valid transfer records.

## Community decision policy

Moderators configure one current community default. A versioned policy record
contains:

- policy ID, schema version, predecessor policy IDs, and author;
- the governance participant roster revision used as its eligibility source;
- roles allowed to propose, facilitate, transfer facilitation, and close;
- default response-window duration;
- participation rule: all snapshotted participants or a fixed minimum response
  count;
- visible-position rule, fixed to visible for the first release; and
- the three position labels and meanings, fixed to Support, Stand aside, and
  Object for the first release.

The initial default is:

- all recognized governance participants may propose and participate;
- the proposal author or an active moderator may facilitate and close;
- response review is due after five days;
- a moderator chooses a fixed minimum response count no greater than the
  current roster size; and
- positions and reasons are visible to snapshotted participants.

A proposal snapshots the exact policy record and exact eligible profile IDs
when it opens. Later policy, roster, or role changes apply to new proposals and
future facilitator actions as explicitly described below; they never silently
change who counted in an already-open proposal.

## Decision lifecycle

Each decision moves through:

```text
Draft -> Open -> Review due -> Consented | No consent | Withdrawn
```

`Review due` is a projection of an open decision whose response window has
elapsed. It is not an automatic closure and does not reject further positions.
Riot cannot assume every offline device exchanged before a wall-clock deadline.

### Draft

A local draft is private application state until its author explicitly opens
the proposal. It may contain a title, proposal text, and an optional response
window override permitted by the community policy. Draft storage is not synced
and creates no democratic record.

### Open

Opening creates a signed decision record containing an immutable decision ID,
initial facilitator, policy ID and digest, participant roster revision and
exact eligible profile IDs, initial revision ID and digest, open time, and
review-due time.

Eligible participants may add comments and take or update a position on the
current revision:

- **Support:** the proposal is safe enough to try;
- **Stand aside:** the participant does not support it but will not prevent it;
- **Object:** the participant identifies a concrete reason it is not safe
  enough to try.

Support and stand-aside reasons are optional. An objection requires a non-empty
reason. An objection stops consent from being recorded but is not a permanent
personal veto: the facilitator may revise the proposal or record a response.
If the participant does not withdraw the objection, the proposal may only close
as No consent reached or Withdrawn.

### Revision

A valid revision is append-only and references the current revision, its full
replacement proposal text, and a required summary of what changed. Publishing
it makes every prior position historical. Participants must take a position on
the new revision; positions never silently carry forward.

Concurrent valid revisions from the same current head create an explicit branch
conflict. Neither branch becomes the current proposal automatically. The
facilitator must publish a successor that references all conflicting revision
heads and states how they were reconciled. Until then, discussion remains
available but positions and closure are disabled.

### Closure

Closure is always deliberate. A signed outcome targets one exact unconflicted
revision and contains:

- outcome kind: Consented, No consent reached, or Withdrawn;
- a required outcome statement;
- exact policy and participant snapshots;
- the signed position record IDs/digests counted;
- support, stand-aside, object, and non-response counts;
- unresolved objection record IDs/digests;
- outcome author and time; and
- an optional review date and follow-up decision link.

Consented is valid only when the snapshotted participation rule is met and the
target revision has no active objections. Insufficient participation or any
active objection limits closure to No consent reached or Withdrawn.

Concurrent valid closure records create an Outcome conflict. Riot displays all
records and counts none as the canonical outcome until an authorized moderator
publishes a resolution that references every conflicting outcome. A resolution
may select one existing outcome or record No consent reached; it may not invent
positions or backdate participation.

Closed decisions are immutable. Further work begins as a linked proposal or an
append-only review note.

## Signed record model

The stable Decisions collection contains versioned, canonical records for:

| Record | Purpose |
| --- | --- |
| `ModeratorGrant` | Organizer-authorized moderator grant or revocation |
| `ParticipantGrant` | Moderator-authorized governance eligibility grant or revocation |
| `DecisionPolicy` | Community default and its predecessor heads |
| `DecisionOpened` | Immutable decision identity and policy/participant snapshot |
| `ProposalRevision` | Full proposal replacement plus predecessor(s) and change summary |
| `DiscussionComment` | Append-only contribution scoped to a decision/revision |
| `FacilitatorTransfer` | Signed handoff of facilitation |
| `Position` | One participant's current stance and reason on one revision |
| `DecisionOutcome` | Exact closure evidence and statement |
| `PolicyResolution` | Reconciles concurrent policy heads |
| `RevisionResolution` | Reconciles concurrent proposal heads |
| `OutcomeResolution` | Reconciles concurrent closure records |
| `ReviewNote` | Append-only post-outcome learning or review result |

Every record carries a schema version, stable record ID, community namespace,
collection ID, decision ID where applicable, verified author subspace, signed
Willow timestamp, payload digest, and causal predecessor IDs where applicable.
Profile and record IDs remain full length in storage, fixtures, diagnostics,
and accessibility values.

The core accepts identity-bearing fields only when they agree with verified
entry metadata and the applicable authorization snapshot. Malformed, spoofed,
unauthorized, oversized, or causally impossible records remain inspectable in
technical diagnostics but do not enter the democratic projection.

## Stable collection and app upgrades

Decisions use a host-owned stable collection rather than the content-derived
app-data path of one bundle. The organizer grants an exact app ID a bounded
capability to read the Decisions projection and submit Decisions commands.

An app upgrade has a new content-derived app ID and no automatic collection
access. The organizer reviews it and signs a handoff from the currently
authorized app ID to the successor. Revoking the handoff makes the old app
read-only immediately on devices that carry the revocation; concurrent offline
use may still produce signed records, which core evaluates against the
authority state applicable to each command and surfaces as stale-authority
records rather than silently accepting them.

The initial Quick Poll remains a separate legacy app-data collection. A
deliberate migration tool may create a new Decisions proposal containing the
old question and choices as quoted context and link to the legacy record. Old
plurality votes remain legacy votes; they do not become consent positions or an
outcome.

## Components

### Rust core

`riot-core::decisions` owns canonical codecs, collection capability checks,
role and roster projections, policy validation, lifecycle validation, conflict
detection, consent calculations, and deterministic read models. Identical
valid signed input sets must produce identical projections regardless of
arrival order.

### FFI and native host

A bounded Decisions session exposes commands and projections rather than raw
identity-bearing JSON writes. Commands include:

- maintain moderator and participant grants;
- publish or resolve policy;
- open, discuss, revise, or resolve a decision;
- take or update a position;
- transfer facilitation;
- record or resolve an outcome; and
- append a review note.

The native bridge supplies the current authenticated identity. JavaScript
cannot select an author, moderator, or signer. The session returns typed error
codes with plain-language host mappings and never leaks raw codec or signature
text into the interface.

### Signed Decisions miniapp

The miniapp renders four states:

1. **Needs you:** open proposals where the current profile has not positioned
   on the current revision or must reconfirm after revision.
2. **Open:** all active proposals, including Review due and conflicted items.
3. **Closed:** immutable outcome records and post-outcome review notes.
4. **How we decide:** the current policy for everyone; editing controls only for
   profiles whose core projection grants them authority.

The proposal page contains the exact current revision, revision history,
participation summary, active objections, focused discussion, and a primary
Take or update your position action. The interface uses labels, text, shape,
and accessibility state rather than color alone.

### Existing sync

Nearby exchange carries the same signed Willow records without a Decisions-
specific network protocol. Import triggers deterministic reprojection and a
live miniapp update. No server, account service, blockchain, or network request
is introduced.

## Data flow

```text
person acts
  -> bounded native Decisions command with authenticated identity
  -> Rust validates capability, role, snapshots, and current causal heads
  -> Rust signs and commits a versioned Willow record
  -> deterministic projection updates
  -> miniapp rerenders
  -> existing nearby exchange carries the signed record
  -> receiving Rust core validates and projects the same evidence
```

No user-visible success is shown before the local signed record commits.

## Interface design

The Decisions home uses compact cards rather than a results dashboard. Default
ordering is:

1. current profile needs to reconfirm after a revision;
2. current profile has not taken a position;
3. an active objection needs facilitator attention;
4. Review due;
5. remaining open decisions by most recent valid activity.

Each card shows title, lifecycle state, facilitator, response-review timing,
support/stand-aside/object/non-response counts, and whether the proposal is
revised or conflicted. It never labels a live tally as the community's decision.

The proposal detail presents proposal text before charts. Active objections
appear with their reasons and related facilitator responses. Discussion and
revision events share one chronological record, while position changes are
summarized without flooding the thread. A closed page leads with the outcome
statement and then exposes the exact evidence counted.

The moderator policy screen uses plain labels:

- Who takes part
- Who may start a proposal
- Who facilitates and closes
- Responses needed
- Normal response window

Every open proposal can show the snapshotted rules it uses. Editing the current
default explicitly states that open proposals will not change.

## Offline and error behavior

- A successful command reports **Saved on this device** and may show **Waiting
  for nearby exchange**.
- Drafts survive validation failures, commit failures, app closing, and relaunch.
- A passed response time yields Review due and accepts positions until closure.
- An outcome counts only the exact signed position records it names. A position
  arriving later remains visible as **Arrived after this outcome; not counted**.
- Missing policy, roster, capability, or profile evidence makes affected
  actions read-only with a concrete explanation and retry path.
- One malformed or unauthorized record cannot blank a decision or the board.
- Policy, revision, and outcome branches are visible conflicts, never silently
  resolved by last-write-wins.
- Losing facilitator or moderator authority prevents new privileged actions but
  does not erase records that were valid when authored.
- Closed decisions reject mutation. Follow-up uses linked append-only records.
- Oversized content is rejected before signing; the draft and focus remain.
- Profile names are resolved at render time from full stored IDs and may update
  without rewriting decision records.

## Accessibility and visual requirements

- Minimum 44-point touch targets and visible keyboard focus.
- WCAG AA contrast; position and conflict states never rely on color alone.
- Semantic headings, form labels, status announcements, and named controls.
- Dynamic Type/zoom and 320-point widths without clipped IDs, reasons, or
  outcome statements.
- Reduced-motion support.
- Focus returns to the invoking card after leaving a proposal.
- Revision invalidation, saved state, new synced activity, conflicts, and
  outcome closure are announced without stealing focus.
- Visual review at 390x844 and 1280x800 for Needs you, Open, proposal detail,
  revision, active objection, conflict, moderator policy, empty, error, and
  closed outcome states.

## Security and privacy

- JavaScript receives no raw signing key, capability secret, or authority to
  choose its signer.
- Organizer, moderator, participant, facilitator, and outcome authority are
  evaluated from verified signed records in Rust.
- A display name or key-derived tag is never an authorization input.
- The app gets no network, contacts, cross-app reads, or arbitrary native bridge.
- Positions and reasons are visible to eligible participants in the first
  release. The interface states this before a participant submits.
- Records are durable once exchanged. The composer warns that comments,
  positions, and outcomes become part of the community record.
- Content is rendered with text nodes and bounded plain text; no stored HTML is
  executed.
- Technical diagnostics expose full IDs for verification but never secrets.

## Verification

Implementation follows repository-mandated TDD and the thresholds in
`.coverage-thresholds.json`, including 100 percent line, branch, function, and
statement coverage when those are the configured thresholds.

Required test layers:

- canonical codec round trips and hostile decoder fixtures for every record;
- authorization tests for organizer, moderator, participant, facilitator, and
  app collection capability boundaries;
- property tests proving arrival-order-independent projection;
- policy, roster, revision, position, facilitation, closure, and branch-
  resolution state-machine tests;
- tests proving revision invalidation and exact outcome evidence accounting;
- hostile tests for forged JSON IDs, wrong authors, stale capabilities,
  unauthorized policy changes, key collisions, oversized records, and malformed
  imported entries;
- two-device and multi-device offline convergence, late-arrival, and concurrent
  branch scenarios;
- app upgrade handoff, revocation, and stable collection continuity;
- migration tests proving legacy plurality votes never become consent;
- bridge contract and typed-error mapping tests on every native host;
- browser behavior tests for every primary action, draft recovery, read-only
  states, malformed-record isolation, and conflict resolution;
- accessibility and visual review for the required phone and desktop states;
- full Rust workspace tests, strict formatting and Clippy, binding tests, Apple
  builds/tests, relevant Android checks, and the blocking coverage command.

## Delivery boundary

The feature is one product design but implementation should be decomposed into
reviewable work units after planning:

1. canonical Decisions records, authority, and deterministic projection;
2. stable collection capability, upgrade handoff, FFI, and native bridge;
3. moderator roster and community default policy interface;
4. decision board, proposal discussion, revision, and position flows;
5. closure, conflicts, late-arrival evidence, migration, accessibility, and
   cross-device verification.

No work unit may ship a front-end-only moderator check or label a plurality
tally as consent. Until the authority and collection units are complete, the
current Quick Poll remains visibly distinct rather than pretending to implement
this design.

## Definition of done

- Several consent decisions can be open and independently discussed.
- Moderators set one signed community default and all participants can inspect
  the exact snapshot governing a proposal.
- Eligible people can Support, Stand aside, or Object with authenticated,
  reasoned positions.
- Revisions preserve history and require reconfirmation.
- Consent cannot be recorded with an active objection or insufficient
  participation.
- Every outcome names the exact revision and signed positions counted.
- Offline peers converge on the same projection from the same valid evidence;
  concurrent branches remain visible until explicitly resolved.
- Decision history survives an organizer-approved app upgrade without granting
  an unapproved bundle access.
- The interface passes functional, security, accessibility, visual, cross-
  device, and configured 100-percent coverage gates.

## Product references

- Loomio, “Proposals”: <https://help.loomio.com/en/user_manual/polls/proposals/>
- Loomio, “Proposals and polls”: <https://help.loomio.com/en/user_manual/polls/intro_to_decisions/>
- Loomio, “Outcome”: <https://help.loomio.com/en/user_manual/polls/outcomes/>
- Loomio, “Discussions”: <https://help.loomio.com/en/user_manual/threads/intro_to_threads/>
- Agora, “Governance Technical Overview” (contrast only):
  <https://docs.agora.xyz/governance-technical-overview>
