# Multi-Community Open Newswire MVP Design

Date: 2026-07-13
Status: Approved in brainstorming; pending written-spec review and design review gate

## Purpose

Build the smallest useful Indymedia-style publishing system inside Riot:
open publishing, a collective editorial front page, transparent post-hoc
moderation, offline-first distribution, and no authoritative publishing server.

The product is not one global newswire. Riot contains many independently run
community spaces, such as Uganda, Germany, Uruguay, or Queers of Aotearoa. Each
space can carry a Newswire alongside the community's other signed local-first
apps.

This design narrows the Track A newswire in
`2026-07-10-riot-dual-mode-design.md`. It reuses the community shell from
`2026-07-13-community-first-navigation-design.md`, the signed app distribution
model from `2026-07-11-app-directory-design.md`, and the app isolation contract
from `2026-07-12-community-miniapp-suite-design.md`.

## Product Decision

A Riot space is a community container, not a synonym for a feed. Each public
community space has:

- its own identity, descriptive metadata, and fixed MVP admin-key list;
- an open Newswire anyone can publish to;
- a collective editorial front page derived from signed admin actions;
- a raw chronological wire and inspectable moderation history;
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
5. A configured admin can sign a feature, verification, correction, hide, or
   safety-tombstone action.
6. Every client derives the same collective front page, open wire, and
   moderation history from the same signed records.
7. A space exposes its approved Checklist, Needs & Offers, Events, Decisions,
   Chat, Dispatches, Wiki, Photo Wall, and future signed apps alongside
   Newswire.
8. A gateway can be rebuilt from signed public data without becoming the
   authority for a community.

## Scope

### Included

- Multiple independently administered public community spaces.
- A responsive Newswire experience on Riot and the public web.
- Persistent local pseudonymous signing identities by default.
- Clearly labeled one-off ephemeral publishing identities.
- Freeform reports with optional structured metadata.
- Required stricter fields for operational alerts or requests when those
  existing Riot object profiles are selected.
- A collective editorial front page plus a raw chronological wire.
- A fixed list of admin public keys per space.
- Independently signed, transparent post-hoc moderation actions.
- A replaceable web gateway for rendering, directory discovery, submission
  transport, and public sync.
- Existing per-space signed miniapp distribution, approval, and isolation.

### Explicitly deferred

- Quorum enforcement and threshold signatures.
- Admin rotation, recovery, and governance-management UI.
- Post editing or destructive deletion.
- Ranking algorithms, reputation scores, ratings, and personalized feeds.
- Competing or forked curation lenses within one named community.
- Cross-app activity aggregation.
- Private or connections-only spaces.
- A canonical global directory.
- A promise to remove data from devices that already copied it.

These are deferrals, not hidden placeholders. The MVP admin list is fixed when a
space descriptor is created. A community that must change that list before
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

- **Front page** presents posts featured by configured admin keys, including
  verification and correction state.
- **Open wire** presents every valid, non-expired post in signed creation order,
  subject to the display treatment of valid moderation actions.

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

The browser creates and retains a local signing key by default. A person may
explicitly use a one-off ephemeral identity when continuity is unsafe. Losing a
browser key is unrecoverable in the MVP; the product explains that a new key is
a new pseudonym.

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
merge behavior. UI code never decides whether an admin action is authoritative.

#### Newswire experience

Newswire is a first-party host surface backed by typed Riot-core commands and
projections. It is not executed through the JavaScript miniapp bridge in the
MVP. Mobile and desktop use native shell routes; the gateway uses a responsive
web renderer over the same canonical records. Security-critical validation and
admin recognition remain in core on every surface. A later extraction into a
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
- renders a directory, front page, open wire, post form, and moderation history;
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
silently substitute another name or admin list for the same community. It
contains:

- complete public namespace/space identifier;
- name and short description;
- languages;
- coarse geographic and topic tags;
- fixed ordered set of admin public keys;
- creation time and creator signature; and
- optional predecessor/successor space identifier.

The descriptor never contains secret keys, precise member location, a complete
membership list, or an authoritative gateway URL.

### News post

`NewsPostV1` contains:

- stable post identifier;
- author public key;
- headline and plain-text body;
- signed creation time;
- language;
- optional event time, coarse location, source claims, media references, and
  expiry;
- optional existing Riot operational-object profile; and
- the existing AI-assisted provenance flag.

Posts are append-only. An author corrects a post through another signed record;
the MVP does not overwrite or delete the original.

### Admin annotation

`AdminAnnotationV1` contains:

- stable annotation identifier;
- target post identifier, or target annotation identifier for a retraction;
- admin public key;
- signed creation time;
- action: `feature`, `verify`, `correct`, `hide`, `tombstone`, or `retract`;
- required human-readable reason for `correct`, `hide`, `tombstone`, and
  `retract`; and
- replacement or correction text where applicable.

An annotation affects the collective projection only when its signature is
valid and its signer occurs in that space descriptor's fixed admin list. Any
configured admin acts independently in the MVP. `retract` targets one prior
admin annotation and does not erase it. All valid actions and reversals remain
inspectable.

### Existing app records

App manifests, bundles, app-index records, organizer trust markers, and
app-scoped data retain their approved schemas. This design introduces no
cross-app read privilege or generic native bridge.

## Projection Rules

- **Open wire:** valid posts ordered by signed creation time descending, with a
  deterministic full-ID tie-breaker. Expired posts move into an Earlier view.
- **Front page:** non-expired posts with at least one current valid `feature`
  annotation, ordered by feature time and full annotation ID.
- **Verification:** display every current valid verification and its signer; do
  not collapse several human attestations into an unexplained score.
- **Correction:** show the original plus the signed correction and reason.
- **Ordinary hide:** remove the body from default lists but provide a warning
  interstitial through which a reader can inspect the original and signed
  action.
- **Safety tombstone:** suppress the body and payload references from gateway and
  ordinary client projections. Retain only post ID, author key, timestamps,
  action signer, reason, and action history.
- **Retraction:** a valid `retract` removes its target annotation from the
  current collective projection while preserving both records in history.
- **Unknown or invalid admin action:** retain only as unauthoritative raw data if
  local forensic policy permits; never alter the collective projection.

The projection must define deterministic precedence for multiple actions and
reversals in executable tests before implementation. No client may use arrival
order as authority.

## Data Flows

### Create and discover a space

1. Riot creates and signs an immutable `SpaceDescriptorV1` locally and binds its
   canonical digest into the complete space reference.
2. The creator chooses initial admin public keys and approved starter apps.
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

1. An admin chooses an action, supplies the required reason, and reviews the
   target and acting identity.
2. Core or the web signer creates an `AdminAnnotationV1`.
3. Clients and gateways verify both its signature and membership in the fixed
   admin list.
4. Every implementation recomputes the same front page and wire projection.

## Failure Handling

| Failure | Required behavior |
| --- | --- |
| Offline or gateway unavailable | Commit locally, show Pending exchange, retain retry and nearby/file paths |
| Duplicate record | Treat as idempotent success |
| Invalid signature or malformed schema | Reject that record without poisoning valid siblings |
| Path, size, count, or media budget exceeded | Reject before commit with a stable user-facing error |
| Annotation signed by an unknown admin | Ignore for collective projection |
| Conflicting valid annotations | Preserve all; apply deterministic precedence and show history |
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
- Moderator authority comes only from the space descriptor's full public keys.
- No Nostr, Willow, post, annotation, or app identifier is truncated in
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
- Unknown-admin and forged-signature behavior.
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
  open wire, publishing, moderation history, and Tools.
- Keyboard, screen-reader, focus, touch-target, contrast, narrow-width, dynamic
  type, reduced-motion, and offline/error-state checks.
- Coverage enforcement uses `.coverage-thresholds.json` as the sole threshold
  source before task completion or PR creation.

## MVP Delivery Slices

The implementation plan should prefer end-to-end proof over subsystem breadth:

1. **Canonical Newswire records and deterministic projection** in Riot core.
2. **One-space Riot flow** for open publish, front page, wire, and admin action.
3. **Gateway/web flow** using the same vectors and records.
4. **Multiple-community selection and directory discovery.**
5. **Existing signed-app integration** on the approved Home and Tools surfaces.

Each slice requires tests first, independent validation, and the project's
mandatory design, plan, coverage, and adversarial review gates. The plan may
reorder slices only when dependency evidence demonstrates a smaller executable
vertical path.

## Non-Goals

This MVP does not solve every governance problem or promise universal
censorship resistance. It proves a simpler claim: communities can openly
publish, collectively curate, transparently moderate, carry useful local apps,
and keep functioning when any one web server disappears.
