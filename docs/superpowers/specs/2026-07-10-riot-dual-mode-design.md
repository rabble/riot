# Riot Dual-Mode Design: Open Newswire + Private Groups

Date: 2026-07-10
Status: Approved in brainstorming; amended by both research addenda and the evidence-sprint design.

## Purpose

Riot is an offline-first activist app for creating, publishing, and sharing information when the internet is shut down, censored, or untrusted. This spec extends the earlier product brief and Willow architecture docs with a decided product shape: **two parallel subsystems** — an open emergency-publishing newswire and private encrypted group sharing — joined only by an explicit bridge.

Lineage: indymedia.org (open publishing + editorial curation), protest.net (structured activist events), TxtMob (broadcast alerts during actions), Odeo/divine.video (syndicated media). The through-line is publishing infrastructure, not chat. Exact usage is deliberately open-ended; the design favors a general runtime over baked-in use cases.

## Research Amendments

Two addenda are authoritative where they refine this document:

- `docs/research/2026-07-10-mutual-aid-coordination-research.md` grounds coordination workflows, roles, paper interoperability, governance, and runbooks in historical practice.
- `docs/research/2026-07-10-dual-mode-research-addendum.md` checks the design against current Willow, Meadowcap, MLS, platform APIs, and emergency-data standards.
- `docs/research/2026-07-10-willow-implementation-audit.md` supersedes earlier implementation-status assumptions for Phase 0A dependency pins, canonical encodings, timestamp/path mapping, and store-join semantics.

Phase 0 is a sequence of separately designed and reviewed evidence sprints. The executable Phase 0A public-kernel contract is `docs/superpowers/specs/2026-07-10-riot-evidence-sprint-design.md`; private groups and the bridge require their own Phase 0B and 0C contracts.

## Decisions Made

1. **Architecture: two parallel subsystems** (newswire module, groups module) with separate stores and exchange paths, plus a small shared kernel. Chosen over a unified "space" abstraction. Rationale: the separation is a safety property — newswire code cannot leak group data it never touches — and each module ships independently.
2. **Privacy bar for groups: encrypted with an explicit leakage boundary.** Group data is encrypted at rest and complete group drops are opaque and padded. The design targets confidentiality for group identifiers, membership material, Willow metadata, and content inside a drop; it does not claim to hide artifact existence, timing, channel, or padded size. MLS is the candidate membership control plane.
3. **Bridge: two-way, always deliberate.** Content crosses between modules only as explicit, signed user acts. Never automatic.
4. **Joining groups: both doors at launch.** In-person QR/NFC verification and portable encrypted invite artifacts.
5. **Open side organization: per-incident/community spaces + plural directory feeds.** Anyone can create a space or signed directory feed. Readers apply expiry, byte/count budgets, and feed trust locally; there is no canonical globally writable directory store.
6. **Build order: both modules in parallel**, after the shared kernel is frozen.
7. **Public web gateway** for discovery and onboarding, serving newswire content only.

## System Shape

```
+---------------------+          +---------------------+
|  Newswire module    |  bridge  |  Groups module      |
|  (open, plaintext,  |<-------->|  (MLS membership +  |
|   communal + owned  | explicit |   opaque drops)     |
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

Application content is Willow: namespaces, subspaces, paths, timestamped signed entries, and mergeable stores. Private-group membership epochs are separate MLS control state. Exchange starts with files through a codec boundary; current Willow Drop Format compatibility and live WTP sync activate only after their conformance gates pass.

## Newswire Module

Open emergency publishing and durable movement media.

### Space profiles

- **Open space** (communal Willow namespace): anyone holding the namespace ID can read everything and publish under their own subspace (keypair identity, no accounts). The classic open newswire: publishing is frictionless.
- **Publication space** (owned Willow namespace, plaintext, publicly readable): only Meadowcap capability-holders write. The namespace key is the pseudonymous collective identity; delegated purpose-specific subspace keys remain visible as the actual signers. Example target: an indymedia.de-style collective facing a state ban publishes a news space; subscribers' devices are the distribution network; there is no canonical publishing server to raid. A collective can run both, linked: an open space for submissions and a publication space for edited output.

### Curation, not gatekeeping

An open communal namespace has no creator-controlled root authority: each author controls only their own subspace. Curation therefore lives in one or more linked owned namespaces whose capability-holders publish signed feature, verification, correction, moderation, and governance annotations targeting open entries. Readers can use the raw newswire or choose one or more curation lenses. Curation never deletes; blocking remains reader-side through local subspace mutes. Trust is applied at read time, not as a gate at write time.

### Objects, pages, media

Entries are typed objects (see Shared Kernel) plus static-site paths per SneakerWeb conventions (`/site/index.html`, path = URL, `sneakerweb.html` previews), so a space is simultaneously a structured feed and a browsable offline website. Media payloads are content-addressed and travel separately from entries: sync a space's index cheaply, pull large payloads (audio, video) opportunistically — the podcast feed/enclosure split rebuilt for offline.

### Exchange

Willow Drop Format files remain the target format for exporting a whole space, a selection, or changes since a timestamp. Import is always preview-first (manifest, signers, entry counts, size shown before ingest). An alpha upstream implementation now exists, but current payload-import limitations, test posture, and vector coverage do not satisfy Riot's conformance bar. Phase 0 therefore uses a visibly non-interoperable development codec behind `DropCodec`. WTP and live transports remain later work. File drops are the permanent fallback.

### Directory

Riot standardizes directory record schemas, not one global namespace. Directory feeds are ordinary owned public namespaces; the app may ship removable seed feeds, and users may add or share alternatives. Devices retain records under expiry, byte/count, region, and feed-trust budgets. Group rendezvous is a separate privacy/abuse research track and is not assumed to be invisible merely because its content is pseudorandom.

## Groups Module

Private encrypted sharing for affinity groups, coops, crews, collectives.

- **Identity:** keypairs generated locally; multiple unlinked personas per device (newswire persona and group membership never need to share a key).
- **Group = MLS control plane + Willow data plane:** MLS orders membership epochs. Complete Willow group drops are encrypted and padded as opaque artifacts; members decrypt before validating and merging ordinary Willow entries locally. Non-members may carry opaque blobs but cannot inspect or partially merge them.
- **Joining (both at launch):**
  - *In-person:* QR/NFC exchanges a one-time MLS KeyPackage, commits the add, and returns the Welcome while participants verify keys face-to-face.
  - *Invite artifact:* an expiring voucher and invitee-bound redemption request transportable over any channel. One canonical MLS commit redeems it; copied files are not assumed to disappear, and concurrent redemption is an explicit conflict.
- **Roles via Meadowcap:** all members in one MLS group may read the decrypted data plane; Meadowcap restricts write authority by path and expiry. Workflows requiring different read sets use separate groups until a reviewed subgroup construction exists.
- **Rendezvous:** deferred to a separate design. Any future record must specify content, size, timing, publisher, and traffic leakage rather than claiming blanket indistinguishability.
- **Panic:** per-group wipe and full-device wipe; keys destroyed before data.

## Bridge

The only integration points between modules. All are explicit, user-initiated, signed acts. The implementation is a typed declassification boundary, not a storage-copy API. No shared storage and no live cross-boundary references exist.

1. **Group → newswire (publish out):** the group module produces an allowlisted draft that excludes all group identifiers, private signers, capabilities, receipts, and private relations while preserving the AI-assistance taint. After human review, a purpose-specific delegated public signer creates a new object in the collective's publication namespace.
2. **Newswire → group (clip in):** the complete original public entry, payload, signature, capability, and source namespace remain intact. The clipping member adds a private signed annotation.
3. **Group → directory (rendezvous):** deferred until the separate rendezvous leakage and abuse design is approved.

## Web Gateway

A hosted, stateless renderer for newswire content: any open or publication space browsable at a normal URL. Purpose: discovery, shareable links on the existing web, search indexing, and onboarding before a crisis (the iOS install-boundary problem).

- **Ban-resistance preserved:** a gateway holds no canonical state; it mirrors signed data whose authority is the publisher's key, not the domain. Anyone can run a gateway from any synced copy (the indymedia mirror tradition, formalized). Seizing a gateway seizes a cache; the space keeps propagating peer-to-peer and any subscriber can stand up a new mirror.
- **On-ramp:** every page carries "open in Riot" plus the space's namespace ID as a QR code, converting web readers into offline carriers. Gateways also serve drop files over HTTP, doubling as sync sources whenever internet is available.
- **Hard boundary:** gateways serve newswire content and selected public directory feeds only. Private groups and private rendezvous material never render through a gateway.
- **Scope:** a small third deliverable — static renderer + Willow store as a boring web service. Cheapest first demo.

## Shared Kernel

The only code both modules and the gateway share. Defined first, test-heavy, frozen early — it is where parallel tracks would otherwise drift.

- **Identity & signing:** keypair generation, unlinked personas, signing/verification, Meadowcap capability handling.
- **Object vocabulary (reconciled across both research addenda):** ten durable wire kinds: `alert`, `observation`, `event`, `resource`, `request`, `offer`, `commitment`, `task`, `document`, and `annotation`. Product terms map to profiles: `need` labels a request; route status and field report are observations; checklist, announcement, and runbook are documents; verification, moderation action, correction, translation, feature, fulfillment, and task state are annotations. The signed content envelope carries schema/object/revision IDs, created and validity times, language, typed body, relations, source claims, and the AI-assisted flag. Willow carries author/capability/digest data; local receipts carry import and trust provenance.
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
| Traffic interception | Group-drop contents and inner identifiers are encrypted and padded; artifact existence, timing, channel, and padded size remain visible. Newswire drops are signed plaintext by design. |
| Device seizure (non-member carrier) | Reveals opaque artifact count and padded sizes but no plaintext group identifiers or content, subject to correct envelope and key handling. |
| Device seizure (member) | **Residual risk, stated honestly:** exposes that group. Mitigated by per-group panic wipe, capability expiry, unlinked personas, small-group practice. |
| Flooding / disinformation | Curation lens, corrections, reader-side mutes; per-space blast radius, no global feed to poison. |
| Malicious packet content | Sandboxed renderer, no network, no native bridge, preview-before-import, byte/count/path limits. |

## Build Phasing

- **Phase 0 — Separately gated evidence sprints.** Phase 0A executes `docs/superpowers/specs/2026-07-10-riot-evidence-sprint-design.md` and proves one public alert through the shared Rust core, Willow authority, preview-first atomic import, generated Swift/Kotlin bindings, and a two-way iOS Simulator↔Android emulator artifact handoff. Phase 0B will test MLS/private-envelope/invite claims under its own reviewed threat model and agent-hour budget. Phase 0C will test the declassification bridge under its own reviewed information-flow contract and budget. The earlier Swift prototype remains historical product-flow scaffolding, not the execution plan. *Product owner confirmed 2026-07-10: run Phase 0A as specified; platform pivot to shared Rust core with native iOS and Android shells confirmed.*
- **Parallel demo track (non-gating, confirmed 2026-07-10).** A minimal gateway/reader — a small web service rendering a hand-authored space as a browsable site — is built alongside Phase 0A. It restores the "cheapest first demo," gives organizers something concrete to react to, and collects vocabulary/workflow feedback before schemas freeze. It makes no protocol claims, shares no evidence budget, and its content can be hand-authored fixtures until the kernel exists. It never gates or is gated by the Phase 0 evidence sprints.
- **Phase 1 — Parallel tracks.**
  - Track A: newswire module — spaces, authoring, drops, directory.
  - Track B: groups module — encrypted store, QR + invite joins, group sync via drops.
  - Track C: web gateway serving Track A's format (early demo).
- **Phase 2 — Integration.** Bridge (all three crossings), WTP live sync over local transports, local LLM.
- **Phase 3 — Reach.** Confidential Sync if partial/private sync demands it, bitchat/BLE transport adapters, relay transports for when internet exists.

## Open Questions

- MLS/mobile viability, canonical concurrent-commit handling, long-offline recovery, and independent cryptographic review before Track B release.
- Rendezvous format and its content, size, timing, publisher, and traffic leakage, plus directory abuse controls.
- Production private-drop envelope construction and padding policy after the evidence format is reviewed.
- Whether `.snk` compatibility with SneakerWeb is a hard goal or a convention to follow loosely.
- The stabilization path for Willow Drop Format, authoritative cross-implementation vectors, and whether Riot should contribute missing import/conformance work.
- Membership vetting and infiltration defense practices in real activist groups (research coverage hole; directly relevant to invite design).

## Addendum: Research-Grounded Revisions (2026-07-10)

Source: `docs/research/2026-07-10-mutual-aid-coordination-research.md` — an adversarially verified study of how mutual aid and grassroots networks coordinate (Occupy Sandy, Verificado 19S, TXTMob, Indymedia, NYC COVID mutual aid). Changes it drives:

**Workflow findings and their reconciled wire mappings.**

- `task` remains a core dispatch object; signed annotations carry claim, handoff, state, and completion so offline conflicts remain visible.
- `verification` is an annotation profile recording its method (eyewitness, N independent sources), grounded in Verificado 19S's two-source rule and the NYC Comms Collective's trusted-broadcast layer.
- `moderation_action` is an annotation profile for inspectable hide-with-reason decisions, grounded in IMC UK's hide-not-delete practice.
- `need` remains the user-facing label for `request`; `commitment` plus fulfillment annotations form the shared editable ledger linking requests and offers.

**Structural confirmations and additions.**

- The TXTMob 2×2 matrix (public/private × moderated/unmoderated) independently validates the dual-mode architecture: communal submissions are unmoderated public; owned curation lenses and publication spaces are moderated public; private groups are encrypted; path-restricted write capabilities provide role-specific private workflows.
- **Roles as capability templates**: intake, dispatcher, field verifier, moderator/curator become named Meadowcap capability bundles.
- **Governance meta-channel**: each curation/publication namespace gets governance paths separate from content, mirroring Indymedia's rule that moderation disputes stay off the newswire. A communal submission namespace does not gain a creator root through this convention.
- **Paper interop is a requirement**: printable forms and QR round-trips for intake and distribution; flyer/zine export in multiple languages. Analog channels are how networks reach their most vulnerable members and how data moves when power is out.
- **Runbooks as first-class content**: seedable, user-editable "how this hub works" documents (the checklist type extended), addressing the verified tacit-knowledge failure mode.
- **Onboarding assumes existing groups**: import-your-crew flows take priority over stranger discovery — networks bootstrap from pre-existing channels, never cold.

**Failure modes Riot's architecture already answers** (worth stating for reviewers): carrier/platform chokepoints (T-Mobile blocked TXTMob at the 2004 RNC; COVID groups depended wholly on Slack/Airtable/Venmo) and identifiable-operator arrest (why radio comms gave way to SMS). No carrier, no canonical server, pseudonymous keys.

## Relationship to Existing Docs

- Extends `docs/product/product-brief.md`: adds the dual-mode shape, publication spaces, gateway, and softens trust-as-gate to trust-as-curation for non-operational content. Operational object types keep required expiry and source notes.
- Extends `docs/architecture/willow-architecture.md`: Willow remains the canonical data model. Opaque whole-drop encryption now precedes property-preserving Encrypted Willow; the latter is deferred until untrusted relays need entry-level partial sync.
- The Swift-only prototype plan is retained as historical product-flow scaffolding. The cross-platform evidence-sprint sequence supersedes it as Phase 0.
