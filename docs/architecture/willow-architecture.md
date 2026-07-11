# Willow Architecture for Riot

## Priority

Use Willow in this order after the 2026-07-10 implementation audit:

1. Data Model
2. Meadowcap
3. Riot's visibly non-interoperable Phase 0A evidence bundle around canonical Willow components
4. Re-evaluated Willow Drop Format after upstream payload-import and CI gaps close
5. Encrypted Willow (critical path for the groups module — see the dual-mode spec)
6. Willow Transfer Protocol
7. Confidential Sync

The dual-mode design (`docs/superpowers/specs/2026-07-10-riot-dual-mode-design.md`) splits Riot into a plaintext newswire module and an encrypted groups module built in parallel; Encrypted Willow moves ahead of WTP because the groups module cannot ship without it, while both modules can ship on drops alone before live sync exists.

## Core Mapping

Riot should treat Willow as the canonical data model, not as an import/export detail.

| Riot concept | Willow concept |
| --- | --- |
| Packet / incident / community space | `namespace_id` |
| Site / signer / organization / device | `subspace_id` |
| Page, object, asset, index, translation | `path` |
| Update ordering | `timestamp` |
| HTML, JSON, image, manifest, model prompt | `payload` |
| Authority to read/write | Meadowcap capability |
| Offline file exchange | Drop Format |
| Live local request/response sync | Willow Transfer Protocol |

## Packet Structure

At the app level, a packet can be represented as:

```text
packet.json
site/
  index.html
  assets/
schema/
  alert.json
  resource-location.json
  checklist.json
  correction.json
prompts/
  draft-alert.md
  translate.md
  summarize-changes.md
trust/
  signers.json
  import-policy.json
seed/
  rights.md
  first-aid.md
  contacts.json
```

In Willow, these become entries such as:

```text
/packet/manifest.json
/site/index.html
/site/assets/style.css
/schema/alert.json
/prompts/draft-alert.md
/trust/signers.json
/seed/first-aid.md
/updates/2026-07-10/<id>.json
/i18n/es/updates/<id>.json
```

## Object Types

The first product version should avoid arbitrary page generation and use typed update objects:

- `alert`
- `event`
- `resource_location`
- `route_status`
- `need`
- `offer`
- `task`
- `verification`
- `moderation_action`
- `checklist`
- `announcement`
- `translation`
- `correction`
- `field_report`

`need`, `offer`, and `task` carry a claim/fulfillment lifecycle (open, claimed, done) so a space can serve as a shared dispatch ledger. `verification` and `moderation_action` attach to other objects by reference. Grounding for these types is in `docs/research/2026-07-10-mutual-aid-coordination-research.md`.

`verification` also covers **optional media authenticity** (ProofMode capture proofs, C2PA manifests) as a detachable annotation on a media payload, never embedded in the payload itself. The default publish path redacts GPS and device-identifying assertions before a `verification` annotation crosses the Group → newswire bridge; full unredacted provenance may be kept privately. The annotation records what was redacted and by whom, not just a pass/fail validator result — a C2PA-conformant "valid" check does not cover data that was excluded from the signature. Grounding: `docs/research/2026-07-11-proofmode-c2pa-media-authenticity-research.md`.

Every object should include:

- stable id,
- title,
- body,
- category,
- author subspace,
- signer key,
- created time,
- expiry time when operational,
- source note,
- confidence,
- affected area,
- language,
- supersedes or corrects references,
- AI-assisted flag.

## Rendering

Riot should render packets through two layers:

1. Native views for structured packet objects.
2. Sandboxed web rendering for static packet sites.

The web renderer should:

- block external network requests by default,
- block arbitrary native bridges,
- make local/offline status visible,
- expose signer and freshness metadata outside the web content,
- support `sneakerweb.html`-style previews for packet cards.

## Exchange

### Phase 0A: Evidence bundle

Use `.riot-evidence` only to prove canonical Willow entry/capability bytes, corrected WILLIAM3, Meadowcap authority, bounded import, and native handoff. It is not `.snk`, Drop Format, or a WTP stream.

### Phase 1: Drops

Re-evaluate Willow Drop Format as the first interoperable exchange artifact only after canonical upstream issue #51 (payload imports) and issue #54 (hosted CI) improve and Riot has authoritative cross-implementation vectors. Do not infer Drop compatibility from Phase 0A.

Operations:

- export entire packet,
- export selected packet,
- export changes since timestamp,
- import drop preview,
- import selected entries,
- block domains/namespaces/signers,
- carry a trusted drop onward.

### Phase 2: WTP

Add Willow Transfer Protocol over local transports once drop import/export works.

Candidate transports:

- local Wi-Fi / Bonjour,
- MultipeerConnectivity,
- nearby desktop companion,
- future BLE transport,
- future bitchat bridge,
- future relay transport when internet is available — research now grounds this as an *opportunistic backhaul*, not a special-cased path: the same reconciliation logic that runs over local transports should run over the relay, following Briar's and Secure Scuttlebutt's proven pattern of one merge function that never special-cases which transport delivered an entry. Candidate relay mechanism: Arti (Tor Project's Rust Tor reimplementation) embedded in-process for outbound onion-service hosting; inbound relay/bridge hosting is not yet viable on Arti, and the mobile FFI boundary — not Arti's core protocol — is the highest-risk surface in any such integration, per audit evidence. Grounding: `docs/research/2026-07-11-hybrid-gossip-backhaul-research.md`, `docs/research/2026-07-11-arti-tor-backhaul-research.md`.
- physical data-mule / courier sync — a periodically-refreshed Drop Format package carried by a scheduled courier, transit route, or sneakernet, in the pattern of Cuba's El Paquete Semanal and DakNet's vehicle-mounted relays — as a first-class transport, not a fallback. It is the strongest deployed evidence found for any offline distribution mechanism researched so far. Grounding: `docs/research/2026-07-11-shutdown-resistant-distribution-research.md`.

WTP's own request/response model already leans toward the demand-driven retrieval pattern recent research argues is more resilient than the blind flood/epidemic routing used by most deployed mesh-messaging apps; keep it that way rather than adding broadcast relay by default.

### Phase 3: Confidential Sync

Add Confidential Sync only after the app needs private interest overlap or more sophisticated partial sync. It is powerful but significantly more complex than WTP.

## Local LLM Boundary

The LLM can read:

- packet prompts,
- packet schemas,
- trusted seed docs,
- user-provided notes,
- selected imported updates.

The LLM can produce:

- draft text,
- translations,
- summaries,
- structured object candidates,
- static page candidates.

The LLM cannot:

- sign,
- publish,
- change trust policy,
- invent sources,
- auto-import,
- auto-delete,
- impersonate a signer.

All LLM output must be reviewed by a user and signed by a human-controlled key.

## Security Notes

- Treat imported packets as hostile until accepted.
- Keep parsing separate from ingest.
- Enforce byte, object count, path length, and render limits.
- Keep operational updates expirable.
- Preserve provenance through re-export.
- Support panic wipe for keys, local stores, imports, and caches.
- Avoid storing secrets in packet content.
