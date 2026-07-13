# Multi-Community Open Newswire MVP Design

Date: 2026-07-13
Status: Revised after design review round 1; pending gate re-review

## Purpose

Build the smallest useful Indymedia-style publishing system inside Riot:
open publishing, a collective editorial front page, transparent post-hoc
editorial stewardship, offline-first distribution, and no authoritative
publishing server.

The product is not one global newswire. Riot contains many independently
self-governed community spaces, such as Uganda, Germany, Uruguay, or Queers of
Aotearoa. Each
space can carry a Newswire alongside the community's other signed local-first
apps.

This design narrows the Track A newswire in
`2026-07-10-riot-dual-mode-design.md`. It reuses the community shell from
`2026-07-13-community-first-navigation-design.md`, the signed app distribution
model from `2026-07-11-app-directory-design.md`, and the app isolation contract
from `2026-07-12-community-miniapp-suite-design.md`.

## Political Commitments

Riot is a prefigurative political project: the infrastructure should practice
the social relations it hopes to make possible. It does not merely help people
post through a crisis. It helps communities build durable capacity to publish,
deliberate, coordinate, remember, and act without asking a state or corporate
platform for permission.

This MVP therefore treats:

- **communities as self-governing media institutions**, not audiences segmented
  by a platform;
- **editorial collectives as accountable participants**, not remote moderation
  teams administering someone else's service;
- **open publishing as a political commitment**, with curation happening after
  publication rather than through a bureaucratic permission gate;
- **signed editorial decisions as public acts**, so power remains legible,
  attributable, reversible, and open to criticism;
- **gateways as movement infrastructure**, replaceable utilities that serve a
  community rather than landlords that own its identity or archive;
- **replication as shared custody**, placing the means of distribution in the
  hands of readers, contributors, and collectives; and
- **community apps as instruments of collective agency**, moving people from
  consuming a feed to meeting needs, making decisions, preserving knowledge,
  and organizing together.

The product rejects opaque ranking, hidden enforcement, engagement extraction,
and the fiction that technical administration is politically neutral. The MVP
is deliberately small, but its boundaries must point toward democratic control
and community autonomy rather than recreating a centralized platform in
miniature.

## Product Decision

A Riot space is a community container, not a synonym for a feed. Each public
community space has:

- its own identity, descriptive metadata, and fixed MVP editorial-key roster;
- an open Newswire anyone can publish to;
- a collective editorial front page derived from signed editorial actions;
- a raw chronological wire and inspectable editorial-action history;
- a set of community-approved signed apps with isolated data alongside the
  first-party Newswire surface;
- a shareable link and QR code; and
- a listing in one or more replaceable gateway directories.

The MVP optimizes for an ongoing local or identity-based community newswire that
can continue operating during a crisis. Crisis resilience is a property of the
ordinary product rather than a separate incident-only mode.

## Definition of Done

The MVP is successful when:

1. A person can create or follow multiple public community spaces.
2. Riot reopens the last available community and can switch to another in one
   action.
3. A person can publish the same signed Newswire record from Riot or a web
   browser without a central account.
4. The post is readable locally before any gateway accepts it and later merges
   idempotently from nearby, file, or gateway exchange.
5. A recognized editor can sign a feature, verification, correction, hide, or
   safety-tombstone action in the name of the space's editorial collective.
6. Every client derives the same collective front page, open wire, and
   editorial-action history from the same signed records.
7. A space exposes its approved Checklist, Needs & Offers, Events, Decisions,
   Chat, Dispatches, Wiki, Photo Wall, and future signed apps alongside
   Newswire.
8. A gateway can be rebuilt from signed public data without becoming the
   authority for a community.

## Scope

### Included

- Multiple independently self-governed public community spaces.
- A responsive Newswire experience on Riot and the public web.
- Persistent local pseudonymous signing identities by default.
- Clearly labeled one-off ephemeral publishing identities.
- Freeform reports with optional structured metadata.
- Required stricter fields for operational alerts or requests when those
  existing Riot object profiles are selected.
- A collective editorial front page plus a raw chronological wire.
- A fixed roster of editorial public keys per space.
- Individually signed, transparent post-hoc editorial actions.
- A replaceable web gateway for rendering, directory discovery, submission
  transport, and public sync.
- Existing per-space signed miniapp distribution, approval, and isolation.

### Explicitly deferred

- Quorum enforcement and threshold signatures.
- Editorial-roster rotation, recovery, and collective-governance UI.
- Post editing or destructive deletion.
- Ranking algorithms, reputation scores, ratings, and personalized feeds.
- Competing or forked curation lenses within one named community.
- Cross-app activity aggregation.
- Private or connections-only spaces.
- A canonical global directory.
- A promise to remove data from devices that already copied it.

These are deferrals, not hidden placeholders. The MVP editorial roster is fixed
when a space descriptor is created. A community that must change that list before
governance management exists creates a successor descriptor and links it from
the old space where possible.

## User Experience

### Level 1: Your communities

The mobile community chooser lists followed spaces with name, relationship,
unread activity, available tool count, and sync freshness. Create and Find are
separate actions. Returning users open the last available community directly;
the chooser remains one action away through the community name in the header.

The web directory lists spaces known to that gateway. Gateway featuring or
delisting changes discovery only. A direct space link, QR code, or synced copy
continues to work independently of that directory decision.

### Level 2: Community Home

Home answers two questions:

1. What is happening here?
2. What can we do together?

It shows:

- the community name, summary, languages, and sync state;
- the current collective Newswire feature;
- **Open wire** and **Post update** actions;
- four deterministic shortcuts from the approved app catalog; and
- access to the full Tools, People, and Nearby destinations.

On iPhone, Home, Tools, People, and Nearby remain the bottom destinations from
the approved community-first navigation design. Newswire opens as a focused
full-screen route retaining the community name and Back. On wider web and
desktop surfaces, the community list, primary content, and live-wire preview may
appear together.

### Newswire reading

Newswire has two explicit views:

- **Front page** presents posts featured by the editorial collective's
  recognized keys, including
  verification and correction state.
- **Open wire** presents every valid, non-expired post in signed creation order,
  subject to the display treatment of valid editorial actions.

There is no opaque blended ranking. Readers always know whether they are seeing
the collective's editorial selection or the open chronological wire.

### Publishing

Both Riot and the browser use the same review-before-signing flow:

1. Enter a headline and report body.
2. Optionally add event time, coarse location, sources, media references,
   language, or expiry.
3. For an operational alert or request, complete the stricter fields required
   by that selected object profile.
4. Review the exact destination community, content, and acting identity.
5. Sign and commit locally.
6. Show local success plus Pending exchange until at least one selected
   transport accepts the record.

The default Newswire composer creates a freeform `NewsPostV1`; only headline
and body are required. Choosing an existing operational alert or request
profile switches the composer to that profile's stricter required fields. For
this Newswire route, this rule supersedes the default **Post an update** field
requirements in the earlier community-navigation and local-first PWA designs.

The browser creates and retains a local signing key by default. A person may
explicitly use a one-off ephemeral identity when continuity is unsafe. Losing a
browser key is unrecoverable in the MVP; the product explains that a new key is
a new pseudonym.

The browser persists signed posts and retry state in a durable local outbox
before attempting gateway submission. Reloading or restarting the browser MUST
not discard a locally successful pending post. A persistent browser pseudonym
is protected only as well as the JavaScript delivered by its serving origin;
the review screen states that limitation. Gateway-delivered web code MUST NOT
generate, import, retain, or use an editorial-roster key. Editorial actions are
signed only in Riot's native trusted code in this MVP.

### Editorial action flow

Every post detail exposes **Editorial history** to every reader. In Riot,
recognized editorial keys additionally expose **Editorial action**. The action
form conditionally requires the reason and correction text defined by the
record contract. Its immutable review shows the complete target entry ID,
community, acting editor key, action, reason, and replacement text before
signing. A successful signature commits locally and shows Pending exchange;
failure preserves the draft. Non-editors never receive an enabled editorial
control, and UI visibility is never an authorization check. `correct` is
labelled **Editorial correction** so it cannot be mistaken for an author edit.

An empty Newswire says that no reports have arrived. If the Open wire has posts
but none is currently featured, the Front page says that the collective has
not selected a feature and links to the Open wire. Offline/stale state and
projection failure are distinct from both empty states.

### Community tools

Newswire is the prominent first-party publishing surface, not the definition of
the space and not an ordinary JavaScript miniapp in this MVP. Tools lists every
app approved by that community. Joining a space brings its trust markers and
app bundles through ordinary sync; there is no per-person install step.

The MVP reuses the existing signed-app contract:

- app integrity is content-addressed;
- organizer-signed trust markers determine whether an app may execute;
- each app has isolated Willow data;
- apps have no network access or cross-app reads; and
- unapproved or incomplete packages may be carried but never executed.

## Architecture

```text
 Riot mobile / desktop                 Web browser
 durable local key + store             local browser key
          |                                  |
          +-------- signed records ----------+
                             |
                  Public community space
          descriptor + Newswire records + app index
                + isolated per-app data
                             |
              +--------------+--------------+
              |                             |
      nearby / file exchange       replaceable gateway
                                      web renderer
                                      directory view
                                      submission relay
                                      public sync source
```

The signed community data is authoritative. A gateway is a cache, validator,
renderer, index, and transport. Seizing or losing one gateway does not seize the
community identity or its existing copies.

### Components

#### Riot core

Riot core owns canonical record encoding, signing and verification, schema and
budget validation, deterministic Newswire projections, Willow admission, and
merge behavior. UI code never decides whether an editorial action is
authoritative.

#### Newswire experience

Newswire is a first-party host surface backed by typed Riot-core commands and
projections. It is not executed through the JavaScript miniapp bridge in the
MVP. Mobile and desktop use native shell routes; the gateway uses a responsive
web renderer over the same canonical records. Security-critical validation and
editorial-key recognition remain in core on every surface. A later extraction into a
portable app requires a separate reviewed capability design.

#### Community shell and app runtime

The existing native shell owns community selection, Home, Tools, People,
Nearby, identity context, and app launching. Existing app manifests, bundles,
trust markers, and app-scoped storage are reused without a parallel newswire app
store.

#### Gateway and directory

A gateway:

- accepts bounded public signed records;
- validates and deduplicates before caching;
- renders a directory, front page, open wire, post form, and editorial-action
  history;
- serves direct links, QR join data, and public exchange artifacts; and
- can rebuild all community views from signed records.

Anyone may create a public space. A gateway decides which submitted descriptors
it lists or features. That local catalog policy is not written into the space
and does not revoke direct access.

## Signed Data Contracts

The implementation plan may reuse existing Riot envelopes and object profiles,
but the logical MVP contract has four record families.

### Space descriptor

`SpaceDescriptorV1` is an immutable bootstrap record. Its canonical digest is
bound into the complete join/share reference, so a relay or gateway cannot
silently substitute another name or editorial roster for the same community. It
contains:

- complete public namespace/space identifier;
- name and short description;
- languages;
- coarse geographic and topic tags;
- fixed ordered roster of editorial public keys;
- creation time; and
- optional predecessor/successor space identifier.

The descriptor never contains secret keys, precise member location, a complete
membership list, or an authoritative gateway URL.

For an MVP-created space, the descriptor is a signed Willow entry at
`newswire/v1/descriptors/<random-16-byte-object-id>`. Its entry namespace MUST
equal its declared complete namespace ID, and its verified entry subspace ID
MUST equal that namespace ID. This bound signer is the founding organizer used
by Riot's existing app-trust rules. The descriptor's complete canonical
32-byte `EntryId` is the `space_descriptor_entry_id` pinned by join references
and every Newswire record. A second descriptor in the same communal namespace
is a different space definition, never an update or substitute for the pinned
descriptor.

### News post

`NewsPostV1` contains:

- pinned `space_descriptor_entry_id`;
- headline and plain-text body;
- language;
- optional event time, coarse location, source claims, media references, and
  expiry;
- optional existing Riot operational-object profile; and
- the existing AI-assisted provenance flag.

Posts are append-only. Author-authored correction linkage is deferred with post
editing; the MVP does not overwrite or delete the original.
`EditorialActionV1.correct` is an editorial correction, not an author revision.

A post entry uses
`newswire/v1/<space-descriptor-entry-id>/posts/<random-16-byte-object-id>`.
Its authoritative post identifier is the complete canonical 32-byte Willow
`EntryId`, not the 16-byte object ID. The verified entry subspace ID is the
author key and the checked Willow entry timestamp is the signed creation time.
If an encoding duplicates either value in its payload, admission MUST require
exact equality with the verified entry envelope.

An optional `MediaRefV1` contains only a content digest, declared byte length,
and allow-listed media type. Core verifies all three against bounded bytes
before storage or rendering. A media reference MUST NOT be an arbitrary URL,
and neither a client nor a gateway automatically fetches a publisher-supplied
URL. Source URLs, when present as claims, render as inert text unless a person
explicitly chooses to navigate to them; gateways never fetch them.

### Editorial action

`EditorialActionV1` contains:

- pinned `space_descriptor_entry_id`;
- complete target post `EntryId`, or complete target editorial-action `EntryId` for a
  retraction;
- action: `feature`, `verify`, `correct`, `hide`, `tombstone`, or `retract`;
- required human-readable reason for `correct`, `hide`, `tombstone`, and
  `retract`; and
- replacement or correction text where applicable.

An action affects the collective projection only when its signature is valid
and its signer occurs in that space descriptor's fixed editorial roster. Any
recognized editor acts independently in the MVP. `retract` targets one prior
editorial action and does not erase it. Every valid action and reversal remains
inspectable as an act of the collective and an attributable act of its signer.

An editorial-action entry uses
`newswire/v1/<space-descriptor-entry-id>/actions/<random-16-byte-object-id>`.
Its authoritative action identifier is its complete canonical 32-byte Willow
`EntryId`; its verified entry subspace ID is the editor key; and its checked
Willow timestamp is the signed creation time. Payload duplicates, if any, MUST
exactly equal those verified envelope values. A target outside the pinned
descriptor, of the wrong record family, or absent from the accepted record set
has no projection effect.

### Existing app records

App manifests, bundles, app-index records, organizer trust markers, and
app-scoped data retain their approved schemas. This design introduces no
cross-app read privilege or generic native bridge.

App execution authority and Newswire editorial authority are independent. For
MVP-created spaces, app-index `organizer_subspace_ids` is derived from the
pinned descriptor entry's verified founding organizer (which equals the
namespace ID), never caller-local configuration. Existing signed organizer
trust markers authorize app execution; the descriptor's editorial roster
authorizes Newswire projection. Neither grants the other authority.

### Envelope and scope invariants

Admission MUST establish all of the following before a record can affect a
projection:

1. The Willow entry, capability, canonical encoding, and complete `EntryId`
   verify.
2. The canonical path has the record-family form above and contains the exact
   pinned descriptor `EntryId` where required.
3. The payload's pinned descriptor `EntryId` equals the path binding and the
   descriptor in the complete join reference.
4. Any duplicated namespace, actor, record ID, or timestamp exactly equals the
   authoritative verified entry-envelope value.
5. An editorial action's actor occurs in the pinned descriptor's fixed roster.

Exact duplicate `EntryId` values are one record. Object-ID reuse never
identifies or targets a record and cannot make a target ambiguous.

## Projection Rules

The pure reducer is `project(accepted_records, as_of)`. Given identical
accepted records and the same `as_of`, every implementation MUST return the
same result. `MAX_FUTURE_SKEW_SECONDS` is the protocol constant `600`. A post or
action whose checked entry time is greater than `as_of + 600 seconds` remains
inspectable in a future-dated quarantine and has no current projection effect
until it becomes eligible. Expiry and future eligibility use the supplied
`as_of`, never arrival time.

Eligible posts and actions have the total ascending order `(checked entry
time, complete EntryId)`. Exact duplicate EntryIds are deduplicated. An eligible
`retract` has an effect only when it targets an eligible, earlier,
non-`retract` editorial action for the same pinned descriptor. Retractions
cannot target retractions. A non-retraction action is **active** unless at least
one eligible, valid, later retraction targets it. Missing, later, wrong-family,
or wrong-space targets have no effect. Arrival order is never an input.

For each eligible post:

- **Open wire:** order posts by `(checked entry time, complete post EntryId)`
  descending. A post whose expiry is less than or equal to `as_of` moves into
  Earlier using the same order.
- **Safety tombstone:** if any active `tombstone` exists, suppress body and
  payload references from gateway and ordinary client projections. Retain only
  complete post ID, author key, timestamps, action signer, reason, and action
  history.
- **Ordinary hide:** otherwise, if any active `hide` exists, remove the body
  from default lists but provide a warning interstitial through which a reader
  can inspect the original and signed actions.
- **Feature:** otherwise, for a non-expired post with one or more active
  `feature` actions, use the greatest action by the total order as that post's
  current feature key. The Front page orders posts by current feature key
  descending. A post with no active feature is absent from the Front page.
- **Verification:** display every active verification in total order with its
  signer; never collapse attestations into a score.
- **Correction:** show the immutable original plus every active editorial
  correction and reason in total order. List cards may summarize the greatest
  active correction but MUST link to the complete history.

Every valid action and retraction remains in Editorial history even when it no
longer affects the current projection. Unknown or invalid actions may be kept
as unauthoritative forensic data under local policy but never alter the
collective projection.

## Data Flows

### Create and discover a space

1. Riot creates and signs an immutable `SpaceDescriptorV1` locally and binds its
   canonical digest into the complete space reference.
2. The founding organizer key remains the existing app-trust organizer; the
   founding collective chooses its separate initial editorial public keys and
   approved starter apps.
3. The descriptor travels through nearby exchange, files, or a gateway.
4. A gateway may list or feature it after local catalog review.
5. Direct links and QR codes carry the complete space identifier and gateway
   hints without making a hint authoritative.

### Publish from Riot

1. The person reviews a draft and identity.
2. Core validates, signs, and commits it to the local Willow store.
3. The UI immediately renders the local post with Pending exchange state.
4. Nearby, file, and gateway transports exchange the identical signed bytes.
5. Receivers validate and merge idempotently.

### Publish from the web

1. The browser creates or loads a local signing identity.
2. The browser renders the same review fields and signs the canonical post.
3. A gateway validates and caches the record, then relays it to other sources.
4. Riot clients receive and verify the same bytes through normal public sync.

Gateway rejection never invalidates the person's signed record; it means only
that this gateway declined to carry it.

### Moderate

1. An editor chooses an action, supplies the required reason, and reviews the
   target, collective, and acting identity.
2. Riot native core creates and signs an `EditorialActionV1`;
   gateway-delivered web code cannot hold or use an editorial-roster key in the
   MVP.
3. Clients and gateways verify both its signature and membership in the fixed
   editorial roster.
4. Every implementation recomputes the same front page and wire projection.

## Failure Handling

| Failure | Required behavior |
| --- | --- |
| Offline or gateway unavailable | Commit locally, show Pending exchange, retain retry and nearby/file paths |
| Duplicate record | Treat as idempotent success |
| Invalid signature or malformed schema | Reject that record without poisoning valid siblings |
| Path, size, count, or media budget exceeded | Reject before commit with a stable user-facing error |
| Editorial action signed by an unknown editor | Ignore for collective projection |
| Conflicting valid editorial actions | Preserve all; apply deterministic precedence and show history |
| Future-dated signed record | Keep inspectable in quarantine; exclude until `as_of + MAX_FUTURE_SKEW_SECONDS` admits it |
| Incomplete or unapproved app | Never execute; offer organizer review only when authority permits |
| Lost browser key | Explain unrecoverable pseudonym loss and create a new identity only with consent |
| Gateway delists a space | Remove from that directory only; direct access and other copies remain |
| Safety tombstone arrives after content copied | Suppress future ordinary display and redistribution without claiming remote erasure |

Malformed user strings render only as text. Web and miniapp surfaces enforce the
existing no-network, navigation, CSP, path, and payload constraints.

## Security and Abuse Boundaries

- Open publishing is intentionally permissionless; display and carrying are
  bounded independently.
- Gateways and clients enforce per-record and per-source byte/count/rate budgets
  without treating those local policies as global deletion.
- Signatures establish authorship, not truth. Verification is a named human
  attestation, not an automated fact score.
- Editorial authority comes only from the full public keys in the space
  descriptor's fixed collective roster.
- Existing app execution authority comes only from organizer trust markers
  signed by the pinned descriptor's verified founding organizer; it is not
  editorial authority.
- No Nostr, Willow, post, editorial-action, or app identifier is truncated in
  protocol data, fixtures, diagnostics, or security decisions.
- Public Newswire content is plaintext by design. Private-group state never
  enters a Newswire gateway.
- AI assistance may draft or translate, but cannot sign, publish, moderate, or
  change authority. Its provenance flag survives publication.

## Verification Strategy

Implementation follows TDD and the repository's coverage gate.

### Canonical contract tests

- Shared golden vectors prove Rust, browser, iOS, and Android encode, sign,
  verify, and decode identical complete records.
- Damage, non-canonical encoding, unknown versions, and full-identifier mismatch
  fail closed.

### Projection tests

- Front page and open-wire ordering.
- Feature, verification, correction, ordinary hide, tombstone, reversal, and
  conflicting-action precedence.
- Unknown-editor and forged-signature behavior.
- Expiry and deterministic full-ID tie-breaking.

### End-to-end tests

- Create and switch among multiple spaces.
- Publish from Riot while offline, then merge through nearby and gateway paths.
- Publish from a browser and read the same signed record in Riot.
- Rebuild a gateway's views from signed data.
- Carry approved apps with a space, execute them only after valid trust, and
  prove app-data isolation.

### Security, visual, and accessibility tests

- XSS, unsafe navigation, forged records, malicious packages, path traversal,
  and byte/count/rate limits.
- Mobile and web screenshots for community chooser, Home, Newswire front page,
  open wire, publishing, editorial-action history, and Tools.
- Keyboard, screen-reader, focus, touch-target, contrast, narrow-width, dynamic
  type, reduced-motion, and offline/error-state checks.
- Coverage enforcement uses `.coverage-thresholds.json` as the sole threshold
  source before task completion or PR creation.

## MVP Delivery Slices

The implementation plan should prefer end-to-end proof over subsystem breadth:

1. **Canonical Newswire records and deterministic projection** in Riot core.
2. **One-space Riot flow** for open publish, front page, wire, and editorial
   action.
3. **Gateway/web flow** using the same vectors and records.
4. **Multiple-community selection and directory discovery.**
5. **Existing signed-app integration** on the approved Home and Tools surfaces.

Each slice requires tests first, independent validation, and the project's
mandatory design, plan, coverage, and adversarial review gates. The plan may
reorder slices only when dependency evidence demonstrates a smaller executable
vertical path.

## Non-Goals

This MVP does not solve every governance problem or promise universal
censorship resistance. It proves a simpler political and technical claim:
communities can own the means to publish, collectively curate, make editorial
power transparent, carry useful local apps, and keep organizing when any one web
server disappears.
