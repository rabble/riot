# Riot Dual-Mode Design: Open Newswire + Private Groups

Date: 2026-07-10
Status: Approved in brainstorming; object vocabulary revised per research addendum (see end of doc).

## Purpose

Riot is an offline-first activist app for creating, publishing, and sharing information when the internet is shut down, censored, or untrusted. This spec extends the earlier product brief and Willow architecture docs with a decided product shape: **two parallel subsystems** — an open emergency-publishing newswire and private encrypted group sharing — joined only by an explicit bridge.

Lineage: indymedia.org (open publishing + editorial curation), protest.net (structured activist events), TxtMob (broadcast alerts during actions), Odeo/divine.video (syndicated media). The through-line is publishing infrastructure, not chat. Exact usage is deliberately open-ended; the design favors a general runtime over baked-in use cases.

## Decisions Made

1. **Architecture: two parallel subsystems** (newswire module, groups module) with separate stores and exchange paths, plus a small shared kernel. Chosen over a unified "space" abstraction. Rationale: the separation is a safety property — newswire code cannot leak group data it never touches — and each module ships independently.
2. **Privacy bar for groups: unlinkable + encrypted.** Group data is encrypted at rest and in drops; group membership is not provable from anything a non-member intercepts. Encrypted-Willow-style techniques are on the critical path, not deferred.
3. **Bridge: two-way, always deliberate.** Content crosses between modules only as explicit, signed user acts. Never automatic.
4. **Joining groups: both doors at launch.** In-person QR/NFC verification and portable encrypted invite artifacts.
5. **Open side organization: per-incident/community spaces + a global directory.** Anyone can create a space; a tiny well-known directory namespace carries only pointers and opaque rendezvous records.
6. **Build order: both modules in parallel**, after the shared kernel is frozen.
7. **Public web gateway** for discovery and onboarding, serving newswire content only.

## System Shape

```
+---------------------+          +---------------------+
|  Newswire module    |  bridge  |  Groups module      |
|  (open, plaintext,  |<-------->|  (owned, encrypted, |
|   communal + owned  | explicit |   unlinkable        |
|   public namespaces)|  signed  |   namespaces)       |
+---------------------+   acts   +---------------------+
          |                                |
          +----------- shared kernel ------+
          | identity/signing, object types,|
          | renderer, provenance display   |
          +--------------------------------+
                     |
          +---------------------+
          |  Web gateway        |  (newswire content only)
          +---------------------+
```

All state is Willow: namespaces, subspaces, paths, timestamped signed entries, mergeable stores. Exchange starts with Willow Drop Format files (sneakernet-first); Willow Transfer Protocol adds live local sync later.

## Newswire Module

Open emergency publishing and durable movement media.

### Space profiles

- **Open space** (communal Willow namespace): anyone holding the namespace ID can read everything and publish under their own subspace (keypair identity, no accounts). The classic open newswire: publishing is frictionless.
- **Publication space** (owned Willow namespace, plaintext, publicly readable): only Meadowcap capability-holders write. The publisher is a pseudonymous collective identity — a signing key, not a server or named people. Example target: an indymedia.de-style collective facing a state ban publishes a news space; subscribers' devices are the distribution network; there is nothing to raid. A collective can run both, linked: an open space for submissions, a publication space for edited output.

### Curation, not gatekeeping

Every open space has a `/features/` path writable only by curation-capability holders. The space creator holds the root capability and can delegate or hand off, so spaces outlive founders. Readers get two lenses on the same data: raw newswire (everything, newest first, signer visible) and curated view. Curation never deletes: `correction` objects flag entries and clients down-rank them. Blocking is reader-side (local subspace mutes). Trust is a lens applied at read time, not a gate at write time.

### Objects, pages, media

Entries are typed objects (see Shared Kernel) plus static-site paths per SneakerWeb conventions (`/site/index.html`, path = URL, `sneakerweb.html` previews), so a space is simultaneously a structured feed and a browsable offline website. Media payloads are content-addressed and travel separately from entries: sync a space's index cheaply, pull large payloads (audio, video) opportunistically — the podcast feed/enclosure split rebuilt for offline.

### Exchange

Willow Drop Format files: export whole space, selection, or changes-since-timestamp. Import is always preview-first (manifest, signers, entry counts, size shown before ingest). Subscribing to a space = holding its namespace ID and accepting newer entries from any peer or drop that carries them. WTP over local transports (Bonjour, MultipeerConnectivity, future BLE) in a later phase; drops remain the permanent fallback.

### Directory

One well-known namespace, hardcoded in the app. Carries only two record types, both size-capped: space pointers (namespace ID + manifest digest + optional region tag) and opaque group rendezvous records. Tiny, so it syncs aggressively on every peer contact.

## Groups Module

Private encrypted sharing for affinity groups, coops, crews, collectives.

- **Identity:** keypairs generated locally; multiple unlinked personas per device (newswire persona and group membership never need to share a key).
- **Group = owned encrypted namespace:** entries and payloads encrypted, paths obfuscated. An intercepted drop or a non-member's seized device reveals nothing — not topic, not membership, not size. Group state merges like any Willow store; members who meet rarely still converge.
- **Joining (both at launch):**
  - *In-person:* QR/NFC exchange with a member holding invite capability; mutual key verification face-to-face; zero internet required.
  - *Invite artifact:* an encrypted single-use file transportable over any channel, redeemable at next contact with a member; revocable until redeemed.
- **Roles via Meadowcap:** admins delegate read/write/invite capabilities, restrictable by path (medics write `/medical/`, all read) and by expiry (natural offboarding).
- **Rendezvous:** a group may publish a blinded record to the public directory — indistinguishable from noise without the invite secret — so invite-holders can locate current group sync material offline. Groups can opt out and stay fully dark.
- **Panic:** per-group wipe and full-device wipe; keys destroyed before data.

## Bridge

The only integration points between modules. All are explicit, user-initiated, signed acts. Implementation is deliberately dumb: copy-with-re-signing between two stores. No shared storage, no live cross-boundary references — a "link" from group content to a public entry is a copy, so group reading behavior never leaks.

1. **Group → newswire (publish out):** a member drafts from group content; the group's *publishing identity* (a keypair distinct from any member's personal key, held via capability by authorized members) signs it into a public space. The published entry carries no group metadata beyond that public identity.
2. **Newswire → group (clip in):** copy a public entry into the group with original signature and source-space provenance intact, so the group can privately assess public claims.
3. **Group → directory (rendezvous):** the blinded record described above.

## Web Gateway

A hosted, stateless renderer for newswire content: any open or publication space browsable at a normal URL. Purpose: discovery, shareable links on the existing web, search indexing, and onboarding before a crisis (the iOS install-boundary problem).

- **Ban-resistance preserved:** a gateway holds no canonical state; it mirrors signed data whose authority is the publisher's key, not the domain. Anyone can run a gateway from any synced copy (the indymedia mirror tradition, formalized). Seizing a gateway seizes a cache; the space keeps propagating peer-to-peer and any subscriber can stand up a new mirror.
- **On-ramp:** every page carries "open in Riot" plus the space's namespace ID as a QR code, converting web readers into offline carriers. Gateways also serve drop files over HTTP, doubling as sync sources whenever internet is available.
- **Hard boundary:** gateways serve newswire content only. Private groups never render through a gateway; at most the public directory (including opaque rendezvous records) syncs through it as bytes.
- **Scope:** a small third deliverable — static renderer + Willow store as a boring web service. Cheapest first demo.

## Shared Kernel

The only code both modules and the gateway share. Defined first, test-heavy, frozen early — it is where parallel tracks would otherwise drift.

- **Identity & signing:** keypair generation, unlinked personas, signing/verification, Meadowcap capability handling.
- **Object vocabulary (revised per research addendum):** `alert`, `event`, `need`, `offer`, `task`, `verification`, `moderation_action`, `resource_location`, `route_status`, `checklist`, `announcement`, `correction`, `field_report`, `translation`. Common envelope: stable id, author subspace, created time, expiry (required for operational types), language, confidence, source note, affected area, supersedes/corrects references, AI-assisted flag. `need`/`offer`/`task` carry a claim/fulfillment lifecycle. See the addendum for grounding.
- **Renderer:** sandboxed web view + native object views. No external network requests, no native bridges, local/offline status visible, signer and freshness shown outside the web content. Identical rendering in groups, newswire, and gateway.
- **Provenance display:** one consistent presentation of who signed what, when, where it was imported from, and how it crossed the bridge.

## Local LLM

A field editor, never an authority. Drafts typed objects from rough notes, translates approved text, summarizes deltas since last sync, extracts structured `need`/`offer` objects from freeform text, formats to a packet's house style.

Constraints:

- Runs **per-module with no cross-module memory**: a session that read group content never touches newswire drafting. No accidental leak channel.
- Output is marked `ai-assisted: true` and the flag survives the bridge — published collective output discloses machine help.
- Cannot sign, publish, import, delete, change trust policy, or impersonate a signer. A human reviews and signs everything.

## Threat Model Summary

| Threat | Answer |
| --- | --- |
| Server seizure / domain ban | No canonical servers. Gateways are disposable mirrors; publisher identity is a key, not infrastructure. |
| Internet shutdown | Everything works from local store + drops; sneakernet is a first-class transport. |
| Traffic interception | Group drops encrypted and unlinkable. Newswire drops are signed plaintext by design (they are publications). |
| Device seizure (non-member) | Reveals nothing about groups the holder isn't in; rendezvous records look like noise. |
| Device seizure (member) | **Residual risk, stated honestly:** exposes that group. Mitigated by per-group panic wipe, capability expiry, unlinked personas, small-group practice. |
| Flooding / disinformation | Curation lens, corrections, reader-side mutes; per-space blast radius, no global feed to poison. |
| Malicious packet content | Sandboxed renderer, no network, no native bridge, preview-before-import, byte/count/path limits. |

## Build Phasing

- **Phase 0 — Shared kernel.** Identity, object vocabulary, renderer, provenance. The existing prototype plan (`docs/superpowers/plans/2026-07-10-riot-prototype.md`) maps onto this almost 1:1 and remains the first execution target, with the JSON store still mirroring the Willow mapping.
- **Phase 1 — Parallel tracks.**
  - Track A: newswire module — spaces, authoring, drops, directory.
  - Track B: groups module — encrypted store, QR + invite joins, group sync via drops.
  - Track C: web gateway serving Track A's format (early demo).
- **Phase 2 — Integration.** Bridge (all three crossings), WTP live sync over local transports, local LLM.
- **Phase 3 — Reach.** Confidential Sync if partial/private sync demands it, bitchat/BLE transport adapters, relay transports for when internet exists.

## Open Questions

- Exact Encrypted Willow construction for groups (path obfuscation scheme, key rotation on member removal) — needs a dedicated crypto design doc before Track B implementation.
- Blinded rendezvous record format and what "indistinguishable from noise" requires concretely.
- Drop encryption envelope for group drops (the Drop Format spec recommends encrypting drops; pick the construction).
- Whether `.snk` compatibility with SneakerWeb is a hard goal or a convention to follow loosely.
- Membership vetting and infiltration defense practices in real activist groups (research coverage hole; directly relevant to invite design).

## Addendum: Research-Grounded Revisions (2026-07-10)

Source: `docs/research/2026-07-10-mutual-aid-coordination-research.md` — an adversarially verified study of how mutual aid and grassroots networks coordinate (Occupy Sandy, Verificado 19S, TXTMob, Indymedia, NYC COVID mutual aid). Changes it drives:

**Object vocabulary additions.**

- `task` — a dispatch ticket with an open → claimed → done lifecycle and explicit handoff. This is the verified core coordination object across contexts: Occupy Sandy's spreadsheet-row-plus-index-card dispatch, COVID mutual aid's intake/dispatch pipeline (the one group that formalized it avoided the burnout everyone else hit).
- `verification` — a signed attestation attached to another object, recording method (eyewitness, N independent sources). Grounded in Verificado 19S's two-source rule and the NYC Comms Collective's trusted-broadcast layer.
- `moderation_action` — hide-with-reason, publicly inspectable, never a delete. Grounded in IMC UK's hide-not-delete practice with the "View all posts" transparency page.
- `need`/`offer` gain claim/fulfillment status so a space functions as the shared editable ledger the research found at the center of every operation.

**Structural confirmations and additions.**

- The TXTMob 2×2 matrix (public/private × moderated/unmoderated) independently validates the dual-mode architecture: open space = unmoderated public; `/features/` = moderated public; publication space = moderated public (writer-gated); private group = unmoderated private; path-restricted group capabilities = moderated private (the medic-dispatch shape).
- **Roles as capability templates**: intake, dispatcher, field verifier, moderator/curator become named Meadowcap capability bundles.
- **Governance meta-channel**: each space gets a governance path separate from content, mirroring Indymedia's rule that moderation disputes stay off the newswire.
- **Paper interop is a requirement**: printable forms and QR round-trips for intake and distribution; flyer/zine export in multiple languages. Analog channels are how networks reach their most vulnerable members and how data moves when power is out.
- **Runbooks as first-class content**: seedable, user-editable "how this hub works" documents (the checklist type extended), addressing the verified tacit-knowledge failure mode.
- **Onboarding assumes existing groups**: import-your-crew flows take priority over stranger discovery — networks bootstrap from pre-existing channels, never cold.

**Failure modes Riot's architecture already answers** (worth stating for reviewers): carrier/platform chokepoints (T-Mobile blocked TXTMob at the 2004 RNC; COVID groups depended wholly on Slack/Airtable/Venmo) and identifiable-operator arrest (why radio comms gave way to SMS). No carrier, no canonical server, pseudonymous keys.

## Relationship to Existing Docs

- Extends `docs/product/product-brief.md`: adds the dual-mode shape, publication spaces, gateway, and softens trust-as-gate to trust-as-curation for non-operational content. Operational object types keep required expiry and source notes.
- Extends `docs/architecture/willow-architecture.md`: same Willow priority order, with Encrypted Willow pulled earlier (Track B critical path) and `event` added to object types.
- The prototype plan remains valid as Phase 0.
