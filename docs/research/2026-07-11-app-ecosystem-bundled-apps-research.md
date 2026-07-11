# Apps as Bundles on a Local-First Network: DXOS, Web Bundles, Beaker, and Automerge Precedent

Date: 2026-07-11
Method: Deep-research workflow — 6 search angles, 24 sources fetched, 91 claims extracted, top 25 adversarially verified (3 independent verification votes per claim), synthesized to 7 findings. 21 of 25 votes confirmed, 4 refuted.

## Purpose

The project owner asked what other app types (beyond newswire) an offline-first, peer-synced network like Riot's should support — todo lists, wikis, calendars were named — and asked for research into a named protocol given as "DFOS." A quick live check during this session found no real protocol by that name and surfaced **DXOS** (dxos.org) as the likely intended subject; this pass researched DXOS on that assumption. Mid-pass, the actual DFOS protocol was identified from a URL the project owner supplied (`protocol.dfos.com`) — a different, narrower cryptographic identity/proof protocol, covered in a separate research doc (`docs/research/2026-07-11-dfos-protocol-research.md`). This document's DXOS findings stand on their own merit regardless of the naming mix-up: DXOS is a real, directly comparable local-first application platform worth understanding alongside Web Bundles and Beaker Browser as prior art for "apps distributed as bundles over a network."

## Summary

DXOS's ECHO layer is a CRDT database (built on Automerge) where "spaces" are the sync and access-control boundary — objects added to a space replicate to all members via a tamper-proof append-only control feed, offline writes merge automatically on reconnect, and no central server holds data. HALO is architecturally separate from ECHO's data layer, handling public-key identity, device invitations, and revocation. Web Bundles (.wbn) is real and technically capable of exactly what SneakerWeb/.snk already does, but its browser adoption has stalled — Chrome shipped subresource-loading support, Firefox and Safari have given "no signal" of intent to implement, and W3C TAG review closed "ambivalent" — a real cautionary precedent against building Riot's distribution layer on a spec that never reached cross-browser consensus. Beaker Browser is strong firsthand evidence that a peer-to-peer "apps as sites" model can work technically at small scale but fails on load-time/connection-reliability/scalability (a roughly 100k-user ceiling) and, more importantly, on product-market fit — its own creator's postmortem attributes the project's end to never crossing an "80-90% easy" adoption threshold despite genuine early enthusiasm. For the "one substrate, many app shapes" question specifically, the strongest verified precedent is not DXOS itself but **Ink & Switch/Automerge**: three structurally distinct apps (a Kanban board, collaborative drawing, and a mixed-media canvas) were built on one generic, schema-less CRDT document model plus one network-agnostic sync protocol, with no per-app-type protocol required — direct support for a design where Riot's Willow object vocabulary grows by adding new object *shapes*, not new sync protocols.

Critically, evidence on the human/organizational side of the question — whether mutual-aid, disaster-response, or activist groups actually need purpose-built collaborative wikis, calendars, task tools, or rosters rather than generic docs/spreadsheets — was not found in this pass. That's an open gap, not a verified null result, and it's the half of the question most directly relevant to deciding *which* app types are worth Riot actually building.

## Verified Findings

### DXOS architecture

- **ECHO is a CRDT-based (Automerge) peer-to-peer database with no central server; "spaces" are the fundamental sync/access-control boundary.** A space is "an instance of an ECHO database which can be replicated by a number of peers," and "spaces serve as sync boundaries — objects added to a space replicate across all members." ECHO is "based on the Automerge CRDT, so both real time and offline collaboration is possible without manual conflict resolution." [3-0] *Refuted, do not repeat:* calling ECHO a "graph database" [1-2] — the accurate description is a CRDT-based object/document store.
- **HALO is a distinct, separate identity/access-control protocol, not folded into ECHO's data layer.** Public keys per user with per-device keys; admission and device invitations are recorded in a tamper-proof, append-only control feed replicated among space members; guests prove authorization by signing invitation challenges; individual devices are revocable at any time. [3-0] *Refuted, do not repeat:* that HALO's key-pair auth is integrated directly into ECHO rather than being a separate protocol [1-2]. This separable-authorization-layer design is directly comparable to how Meadowcap sits alongside (not inside) Willow's data model in Riot's own architecture.
- **No concrete evidence found that DXOS itself has shipped multiple distinct app types (task list, wiki, etc.) packaged and distributed as self-contained bundles on its own substrate.** This is a genuine coverage hole in DXOS's own product history, not a confirmed absence — none of the verified claims address a DXOS-specific app-packaging/distribution mechanism analogous to Riot's `.snk`-style bundles. [low confidence, absence of evidence]

### Web Bundles (.wbn) format maturity

- **Web Bundles package multiple HTTP resources (HTML/JS/images/CSS) into a single CBOR-encoded file for offline, instant-load distribution of a whole site or app** — directly comparable in intent to Riot's SneakerWeb `.snk` packets. [3-0, Chrome developer docs and WICG/webpackage README fetched directly]
- **Cross-browser adoption has stalled.** The original 2019 Internet-Draft was never IETF-endorsed and was superseded; spec work relocated from WICG/webpackage to a dedicated IETF `wpack-wg` repo; Chrome shipped subresource-loading support by default in M104, but Firefox and Safari have given "no signal" of intent to implement, and W3C TAG review closed without consensus ("ambivalent") at a July 2023 meeting. [3-0] *Refuted, do not repeat:* that removing Chrome's navigation-to-bundle feature in Feb 2023 "effectively ended the primary browser implementation path" [0-3] — subresource loading shipped separately and persists via Chrome's Isolated Web Apps; also refuted: that the relocated spec is "still incomplete, only a PR" [0-3] — the `wpack-wg` repo is an active, more developed working-group draft. Note: the underlying chromestatus snapshot is dated May 2023 — "unresolved" should be read as "as of last update," not freshly reconfirmed for 2026.

### Beaker Browser / Dat-Hypercore precedent

- **Beaker Browser — a real, shipped peer-to-peer browser built on Dat/Hypercore letting users create and share p2p "hyperdrive" sites/apps, including lightweight todo-list/wiki-style examples — ended official development in December 2022** when creator Paul Frazee archived the repo after moving to Bluesky. His firsthand postmortem attributes the end to two compounding failures: **technical** (slow initial connection/time-to-first-paint, unreliable/randomly-failing peer connections, scalability breaking down past roughly 100k users, inaccessibility from mainstream browsers, no mobile browser) and **product** (rapid MVP and strong early demo feedback never converted into user retention — his own framework put Beaker around "50% easy" against an 80-90% threshold he says successful projects need). [3-0, direct fetch of the archive notice, corroborated by an independent Hacker News thread and a second firsthand farewell post]

### One substrate, many app shapes — the real precedent

- **Ink & Switch built three structurally distinct apps — Trellis (Kanban/task tool), Pixelpusher (collaborative drawing), and PushPin (a mixed-media canvas) — all sharing the same Automerge CRDT data layer**, and Automerge itself provides a generic JSON-like, schema-less document model plus one general-purpose network sync protocol. App-type diversity (task list vs. wiki vs. calendar) lives in the *shape of data* built atop the CRDT, not in the sync protocol itself — confirmed further by a real shipped multi-app ecosystem (PushPin, Trellis, Capstone, TodoMVC-Automerge: corkboard, task board, notes, and todo-list apps all on one generic document type). [3-0]

## Cross-Cutting Patterns

1. **Every real p2p "apps as sites/bundles" precedent found (Beaker, and by inference DXOS's own thin app-story) struggles more on adoption/product-market-fit and cross-platform reach than on core sync technology.** The technology mostly worked; getting it in front of ordinary users reliably didn't.
2. **Separating identity/authorization from the data-sync layer is a recurring, validated architecture choice** — DXOS's HALO-separate-from-ECHO mirrors Riot's own Meadowcap-separate-from-Willow design, independently arrived at.
3. **The strongest technical answer to "one substrate, many app shapes" comes from a schema-less, generic CRDT document model with app-specific shape layered on top, not from a purpose-built multi-app platform.** DXOS, despite branding itself as an apps platform, doesn't have better-verified evidence for this pattern than Ink & Switch's own Automerge work does.
4. **Standards-track distribution formats (Web Bundles) can stall for years on cross-browser politics even after one vendor ships them** — a real risk to weigh against Riot's existing choice to control its own bundle format (Drop Format / `.snk`-style, not standards-dependent).

## Design Implications for Riot

- **Grow the Willow object vocabulary by adding new object *shapes*, not new sync protocols**, following the Automerge/Ink & Switch precedent directly — a todo list or wiki page is a new typed object (or a profile of `document`/`task`) inside the existing Willow data model, not a reason to bolt on a second sync mechanism.
- **Riot's existing Willow/Meadowcap separation already matches the validated pattern** (HALO separate from ECHO) — no architecture change indicated here, just confirmation the current split is sound.
- **Do not bet Riot's primary distribution format on Web Bundles.** It's technically capable of what SneakerWeb/.snk already do, but its years-long cross-browser stall is a concrete cautionary data point supporting the existing decision to keep a Riot-controlled bundle format rather than standards-track browser features that may never ship everywhere.
- **Treat Beaker's failure mode as the one to design against, not just a historical footnote.** Its technical problems (slow connections, unreliable peers, ~100k-user ceiling, no mobile browser) map onto real risks for any Riot local-transport design; its product problem (never crossing an "80-90% easy" threshold) is a reminder that a bundled-apps platform's hardest problem may be onboarding/UX, not protocol design.
- **Before committing to specific new app types (wiki, calendar, task list) as product priorities, close the evidence gap on question 4** — this pass did not find (and did not conclusively rule out) documented evidence that mutual-aid/disaster/activist groups need purpose-built collaborative versions of these tools versus generic docs/spreadsheets. Recommend a dedicated follow-up pass before investing design effort in any specific new app type beyond newswire/mutual-aid, which already have strong grounding from prior research.

## Coverage Holes and Open Questions

- **Unresearched**: whether mutual-aid, disaster-response, or activist groups have documented, specific need for purpose-built collaborative wikis, shared calendars, group task/checklist tools, roster/contact directories, or resource-mapping tools, as opposed to generic docs/spreadsheets. This is the most product-relevant open question from the original brief and needs its own pass.
- Does DXOS itself have a concrete app-packaging/distribution story — a real shipped example of multiple distinct app types distributed as self-contained bundles on its own substrate?
- How does HALO's control-feed + device-invitation + revocation model compare, mechanism-by-mechanism, to Meadowcap — is Meadowcap's design closer, more expressive, or more constrained for offline/disconnected admission scenarios?
- What is the current (2026) status of Web Bundles / Isolated Web Apps specifically — has Firefox/Safari's "no signal" changed since the May-2023 snapshot underlying these claims?

### Sourcing caveats

DXOS claims are near-exclusively sourced from DXOS's own docs/blog — primary but vendor-authored, with no independent security/scalability audit found in this pass. Beaker's postmortem is a single firsthand account (its own creator), though independently corroborated by outside commentary from the time. Web Bundles adoption data is a May-2023 snapshot and should be treated as such, not as current.

## Primary Sources

- DXOS ECHO introduction — https://docs.dxos.org/echo/introduction/
- DXOS HALO introduction — https://docs.dxos.org/halo/introduction/
- DXOS documentation home — https://docs.dxos.org/
- DXOS blog, "How local-first multiplayer works in DXOS apps" — https://blog.dxos.org/how-local-first-multiplayer-works-in-dxos-apps/
- Chrome developer docs, Web Bundles — https://developer.chrome.com/docs/web-platform/web-bundles
- WICG/webpackage repository — https://github.com/WICG/webpackage
- IETF datatracker, draft-yasskin-wpack-bundled-exchanges-01 — https://datatracker.ietf.org/doc/html/draft-yasskin-wpack-bundled-exchanges-01
- Chrome Platform Status, Web Bundles subresource loading — https://chromestatus.com/feature/5710618575241216
- CDN77 blog, Web Bundles overview — https://www.cdn77.com/blog/web-bundles
- Beaker Browser archive notice (Paul Frazee) — https://github.com/beakerbrowser/beaker/blob/master/archive-notice.md
- Wikipedia, "Beaker (web browser)" — https://en.wikipedia.org/wiki/Beaker_(web_browser)
- Hacker News discussion on Beaker's archival — https://news.ycombinator.com/item?id=41453713
- Beaker Browser GitHub discussion — https://github.com/beakerbrowser/beaker/discussions/1944
- IndieWeb wiki, "Beaker" — https://indieweb.org/Beaker
- Mutual Aid NYC, tools page — https://mutualaidny.org/tools.html
- Mutual Aid Disaster Relief, apps/tech page — https://mutualaiddisasterrelief.org/apps-tech/
- Ink & Switch, "Local-first software" essay — https://www.inkandswitch.com/local-first.html
- Automerge repository — https://github.com/automerge/automerge
- NextGraph docs, CRDTs — https://docs.nextgraph.org/en/framework/crdts/
