# Willow Architecture for Riot

## Priority

Use Willow in this order:

1. Data Model
2. Drop Format
3. Meadowcap
4. Willow Transfer Protocol
5. Encrypted Willow
6. Confidential Sync

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
- `resource_location`
- `route_status`
- `need`
- `offer`
- `checklist`
- `announcement`
- `translation`
- `correction`
- `field_report`

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

### Phase 1: Drops

Use Willow Drop Format as the initial exchange artifact.

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
- future relay transport when internet is available.

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
