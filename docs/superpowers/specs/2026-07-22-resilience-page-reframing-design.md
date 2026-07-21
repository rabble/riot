# Riot Resilience Page Reframing Design

**Date:** 2026-07-22  
**Status:** Design review candidate, revision 3

**Scope:** Riot marketing site narrative and route structure only

## Purpose

Reframe Riot's current privacy page around the human capacity Riot exists to support: people making
useful, joyful, cooperative life together, especially when ordinary institutions and infrastructure
fail. Privacy remains an honest product boundary, but it is not Riot's primary promise.

The page should celebrate the society described in Rebecca Solnit's *A Paradise Built in Hell*:
people commonly respond to disruption with solidarity, improvisation, generosity, mutual aid, and
new forms of local civic life. Riot does not create that capacity or replace community. Riot builds
tools that help communities practice it every day and carry it through disruption; helping those
practices reach more people and places is the project's direction, not a current scale claim.

This is not collapse romanticism. Disaster is harmful. The hopeful claim is that cooperative civic
capacity already exists, and tools can help people cultivate and use it before, during, and after a
crisis.

## Audience and Use Cases

### Community member

A neighbor, organizer, cooperative member, local journalist, or mutual-aid participant wants to
understand what Riot is for, so that they can recognize it as shared civic infrastructure rather than
another social platform or privacy messenger. This matters during ordinary community life as well as
during outages, censorship, disaster, or institutional failure.

### Potential partner

A community organization, library, newsroom, cooperative, clinic, disaster-response group, or civic
institution wants to understand what Riot enables, so that they can imagine using or shaping tools
for meetings, publishing, coordination, shared knowledge, and community memory without surrendering
the work to one permanent provider.

### Technical visitor

A technically knowledgeable visitor wants an honest account of how Riot's architecture supports the
social purpose, so that they can distinguish present capability, tested prototype behavior, and
future direction without making privacy or censorship-resistance assumptions the project cannot
support.

## Editorial Thesis

The hero statement is:

> **People are the infrastructure.**

The supporting thesis is:

> Every day, people make a community through meals, meetings, stories, decisions, celebrations,
> care, and shared work. Riot helps them publish what they know, decide together, coordinate what
> needs doing, and carry their collective memory. When conditions change, the knowledge and tools
> already held by the community can remain useful instead of belonging only to a service.

The page speaks about people before protocols. Technology appears as enabling material, not the
protagonist. The page should feel celebratory, abundant, practical, and invitational: closer to a
festival, community kitchen, neighborhood assembly, or collectively built future than to a bunker,
security product, or catastrophe checklist.

## Framing Principles

1. **Human capacity first.** People already know how to cooperate, improvise, care, and organize.
   Riot helps that capacity travel between people and places. Supporting it at broader scale is a
   direction for the project, not a current deployment claim.
2. **Everyday life first.** Communities should practice shared publishing, meetings, decisions, and
   mutual aid before a crisis. Riot is not an emergency-only application.
3. **Celebration without romanticizing harm.** Disruption is real and often unjust. The hopeful
   subject is what people build together, not the disaster itself.
4. **Autonomy without isolation.** A community should be able to leave a provider or route around
   failed infrastructure without leaving one another.
5. **Tools, not technological salvation.** Riot does not manufacture trust, solidarity, legitimacy,
   or truth. It supports human processes and preserves their work.
6. **No privacy theater.** Public Riot spaces are public. Resilience is not anonymity, secrecy,
   invulnerability, or protection from a compromised device.
7. **No absolute claims.** Avoid “uncensorable,” “unstoppable,” “cannot be shut down,” and similar
   promises. Describe removal of single points of control and the availability of multiple paths.

## Narrative Structure and Copy Direction

### 1. Hero — People are the infrastructure

The hero establishes that people, relationships, knowledge, and cooperative practice are the real
network. It should name concrete activity: sharing information, holding meetings, making decisions,
organizing food and transport, publishing local knowledge, caring for neighbors, and remembering
what the community learned.

The hero must not lead with privacy, censorship, cryptography, protocols, offline mode, or product
limitations. A short status label may still say that Riot is a prototype built in the open.

### 2. A community is something people do

Show the positive civic world Riot supports:

- public conversations and community media;
- assemblies, meetings, proposals, and decisions;
- needs-and-offers boards, schedules, supplies, and shared work;
- local guides, histories, corrections, and collective memory;
- small community-made tools that fit local practice.

Lead with ordinary, joyful life: a block party, community kitchen, neighborhood publication,
cooperative meeting, festival, repair day, shared garden, or volunteer rota. Disruption examples
come later. Avoid reducing community life to emergency alerts or protest logistics.

### 3. Tools for the commons

Present the four human verbs here, while the page is still grounded in ordinary life:

- **Publish:** share signed public updates and community media.
- **Meet:** propose, discuss, decide, and keep the resulting record. Riot supports the artifacts and
  outcomes of meetings; it is not a live audio/video meeting service.
- **Coordinate:** use community-made checklists, boards, schedules, and shared local knowledge.
- **Carry:** keep useful community state and tools on participants' devices and move them through
  paths available to them.

Every verb must carry one of the exact status labels defined in the capability matrix below. The
copy must not collapse a working prototype, locally tested transport, active development, and
longer-term direction into one implied promise.

### 4. Build capacity before conditions change

Explain that resilient coordination comes from relationships and habits already in use. A tool used
for a neighborhood event, cooperative decision, local publication, or volunteer rota is also a tool
people understand when conditions become difficult.

This section rejects both “panic” mythology and disaster cosplay. Riot's intended contribution is
continuity: the community does not have to invent its information practices from scratch at the
worst moment. Ordinary use remains the subject; crisis resilience is a consequence of practiced
relationships and tools, not the page's emotional center.

### 5. A community should not depend on one service

Only after the social purpose is clear should the page explain the technical model:

- community data can be held by participants rather than only by a service;
- signed records let Riot clients check that a record came from a particular key independently;
  a valid signature does not establish a person's identity or make the content true or legitimate;
- local reading and writing can continue against data already held;
- nearby and portable exchange provide additional paths;
- public hosts and ordinary web mirrors can improve reach and discovery without becoming the sole
  owner of community identity or history.

Phrase this as graceful degradation and replaceability, not a guarantee that every path always
works. “A community can walk away from infrastructure without walking away from itself” is a design
aim, not a current or absolute availability claim.

Qualify continuity precisely: locally held data and installed tools may remain useful on a
functioning device; exchange requires a compatible peer or transport that is actually available.
Riot does not guarantee delivery, reachability, availability, persistence, recovery, or resistance
to blocking. Data a device never received cannot be recovered from local storage.

### 6. Honest boundaries — resilience is not secrecy

Keep this section compact but prominent enough to prevent a false safety inference:

- current public spaces should be treated as publishable;
- Riot does not currently ship private encrypted groups;
- pseudonymity is not anonymity;
- hosts, gateways, network operators, and nearby observers may see connection metadata;
- devices retain what they hold and compromised devices remain a risk;
- Riot is a prototype, not an audited hardened safety tool.

The section must also say that resilience depends on functioning devices, data already received,
and at least one suitable route when exchange is needed. It must not imply that Riot works without
any device, peer, transport, or previously held data.

Recommend established encrypted messengers for anything that must remain secret today. Link the
protocol field guide for detailed comparisons.

### 7. This website

Move the marketing-site privacy disclosure to a short final section. Preserve the verifiable facts:
no Riot analytics, cookies, accounts, remote fonts, third-party scripts, tracking pixels, or
fingerprinting; Cloudflare can observe ordinary request metadata while serving the site.

This disclosure supports the project's values without defining the whole project.

### 8. Invitation

End with an invitation to build and practice this future together. Link to the app release,
community/contribution page, and open-source repository. The call to action should invite
participation, experimentation, and local adaptation rather than consumption of a finished service.

## Route and Navigation Design

- Add `/resilience/` as the canonical route and label it **Resilience** in site navigation.
- The new page declares exactly `<link rel="canonical" href="/resilience/">`. The origin-relative
  canonical resolves correctly on the configured Workers origin and on any separately approved
  custom domain; this marketing-only change does not assume or mutate DNS, TLS, or route ownership.
- Preserve `/privacy/` as a static compatibility alias. Its HTML is byte-identical to
  `/resilience/`, including the canonical link to `/resilience/`; therefore an old bookmark loads
  the complete new page rather than a stale privacy essay or an unverified client-side redirect.
- All four files are byte-identical:
  `marketing/resilience/index.html`, `marketing/privacy/index.html`,
  `marketing/public/resilience/index.html`, and `marketing/public/privacy/index.html`.
- Add `/resilience/` to the sitemap and retain `/privacy/` while it remains a compatibility route.
- Reconcile all stale route, mirror, navigation, preview, and crawl-metadata statements in
  `marketing/README.md`. Its source and public inventories must include both `/releases/` and
  `/resilience/`; its navigation description must reflect the site-wide **Resilience** link; and its
  route, sitemap, and local-preview examples must describe the compatibility alias accurately.
- Site-wide navigation-label changes are mechanical scope. Do not rewrite unrelated page content.

The current editorial-page inventory for the navigation contract is exactly:

1. `marketing/index.html` (`/`)
2. `marketing/about/index.html` (`/about/`)
3. `marketing/privacy/index.html` (`/privacy/`, compatibility alias)
4. `marketing/resilience/index.html` (`/resilience/`)
5. `marketing/open-source/index.html` (`/open-source/`)
6. `marketing/community/index.html` (`/community/`)
7. `marketing/releases/index.html` (`/releases/`)
8. `marketing/protocols/index.html` (`/protocols/`)

The contract applies to each source file and its exact `marketing/public/` mirror. If another
editorial route is merged before implementation begins, the implementer must add that route to the
same explicit inventory rather than silently omitting it.

The duplicate compatibility page is intentional because the current static Workers-assets setup has
no checked-in request router or proven HTTP redirect mechanism. A future infrastructure change may
replace it with an HTTP redirect after that behavior is independently verified.

## Visual Design

Retain the existing Riot marketing visual system: poster typography, strong color fields, hard
borders, compact cards, visible focus states, reduced-motion support, and responsive single-column
layouts. This is a narrative reframe, not a visual rebrand.

Use warm, collective imagery in language and structure. Do not add stock disaster photography,
surveillance imagery, padlocks, shields, server diagrams, or heroic lone-user imagery. No new image
asset is required.

## Source Attribution

Credit Rebecca Solnit and *A Paradise Built in Hell* in a small “Further reading / intellectual
lineage” note. Paraphrase the book's thesis and link to the publisher or an authorized author
interview. Do not imply Solnit's endorsement of Riot and do not turn the page into a book summary.

The page may also express the Walkaway-compatible principle that people should be able to leave an
institution or provider while retaining their relationships and shared work, but it should not add a
Cory Doctorow attribution unless the final copy makes a specific, sourced claim.

## Accessibility and Failure Behavior

- Preserve semantic headings, landmark elements, skip link, keyboard-visible focus, and logical
  source order.
- Navigation labels must remain meaningful without visual context.
- External reading links use `rel="noopener"`.
- The page remains fully useful without JavaScript, remote assets, cookies, or analytics.
- On narrow screens, every card and call to action remains readable without horizontal page scroll.
- If `/resilience/` is missing from the deployment mirror, the contract test fails before deploy.
- If the source/deployment copies drift, the contract test fails before deploy.

## Product-status Contract

The page uses these four labels and no invented near-synonyms:

- **Available in the prototype:** exercised through a current Riot app or bundled tool, but not a
  claim of production readiness, audit, or broad field deployment.
- **Tested locally:** verified in automated tests, simulators, loopback, or same-machine rehearsal;
  not yet proven in the relevant physical multi-device/radio setting.
- **In development:** code or design work exists, but the end-to-end user promise is incomplete.
- **Direction, not shipped:** an intended capability that must not be relied on today.

The page's human verbs and infrastructure claims map to those labels as follows:

| Claim on the page | Required status | Permitted meaning and evidence boundary |
|---|---|---|
| Publish signed public updates and community media | Available in the prototype | Current open-newswire create/sign/read flows; still a prototype. |
| Meet through proposals, polls, discussion, decisions, and a shared record | Available in the prototype | Bundled tools and records support meeting work; no live audio/video claim. |
| Coordinate with checklists, boards, and shared local knowledge | Available in the prototype | Current bundled checklist, supply-board, roll-call, and quick-poll tools; do not promise every example named in the product brief. |
| Carry locally held state and installed tools for local use | Available in the prototype | Reading/writing data already on a functioning device; no delivery or recovery guarantee. |
| Move data by portable file, share link, or QR-assisted handoff | Available in the prototype | Current export/import and sharing surfaces; QR can encode a handoff/link and is not itself proof of radio transfer. |
| Exchange over nearby peer transport | Tested locally | Loopback/simulator/same-machine Bonjour evidence only; no claim of proven physical two-phone radio exchange. |
| Render public community material through an ordinary web gateway | Available in the prototype | Current gateway and smoke tests; browsers rely on the gateway's presentation and availability. |
| Discover and sync through replaceable public community anchors | In development | Anchor protocol/client/daemon work exists; do not describe a production anchor network or guaranteed discovery. |
| Use private encrypted groups | Direction, not shipped | Architectural direction only. Current public spaces are public. |
| Extend these practices “at scale” | Direction, not shipped | Product aspiration, never evidence of current deployment, capacity, or reach. |

The README and product brief describe private groups architecturally, while current marketing and
implementation status say they are not shipped. This page follows the current implementation status
and labels private encrypted groups **Direction, not shipped**; it does not silently convert the
architectural description into a product claim.

## Claim Safety

The following claims are forbidden both verbatim and in semantic equivalents:

- uncensorable, unstoppable, impossible to shut down, or “no one can block it”;
- always available, always online, or guaranteed to survive an outage;
- guaranteed delivery, discovery, reachability, synchronization, persistence, or recovery;
- preserves everything or recovers data that no participant already received;
- works without functioning devices, available peers/transports, or locally held data;
- anonymous, confidential, or private-by-default public spaces;
- production-ready, audited, field-proven, or operating at scale.

Positive claims must name the bounded mechanism: local usefulness of already-held data, independent
signature checks that prove only control of a particular key, more than one possible exchange path,
and replaceable hosts. The page may describe the future Riot is trying to build, but aspirations use
“being built,” “aim,” or the exact status labels above rather than present-tense guarantees.

## Verification and Acceptance Criteria

TDD uses the existing contract file. First extend
`scripts/marketing/protocol-page-contracts.mjs` with the new assertions and run
`node scripts/marketing/protocol-page-contracts.mjs`; it must fail before the HTML work exists.
After implementation, the same command must pass.

Add `"test:marketing": "node scripts/marketing/protocol-page-contracts.mjs"` to `package.json` and
add a distinct `npm run test:marketing` step to the `web` job in `.github/workflows/ci.yml`, after
`npm run test:web:unit`. This makes the marketing contract a blocking pull-request check without
changing dependencies.

Automated contracts must verify:

1. byte identity across all four canonical/compatibility source/public files;
2. exactly one origin-relative canonical link, `<link rel="canonical" href="/resilience/">`, in that
   content;
3. `/resilience/` appears in the sitemap;
4. all eight explicitly inventoried source pages and all eight public mirrors include the
   **Resilience** label linked to `/resilience/` in primary navigation;
5. the resilience page contains the required hero, human-activity sections, honest-boundaries
   section, website disclosure, source attribution, and participation links;
6. every capability in the matrix appears with its required exact status label, and the explicit
   device/data/peer/transport limitations appear;
7. a bounded, case-insensitive set of forbidden claim patterns covers at least `uncensorable`,
   `unstoppable`, `cannot be shut down`, `impossible to shut down`, `always available`, `always
   online`, `guaranteed delivery`, `guaranteed discovery`, `guaranteed persistence`, `guaranteed
   recovery`, `anonymous`, `private by default`, `production-ready`, `field-proven`, and `operating
   at scale`; semantic equivalents that evade those finite patterns remain a required human
   editorial-review check rather than a falsely comprehensive automated test;
8. the signed-record copy says that signatures prove control of a key, not identity, truth, or
   legitimacy;
9. the page contains no remote runtime or asset dependency, including remote scripts, stylesheets,
   fonts, images, media, iframes, analytics, beacons, cookies, or tracking endpoints;
10. the full existing marketing contract suite remains green through both
   `node scripts/marketing/protocol-page-contracts.mjs` and `npm run test:web:unit`.

Editorial review succeeds when a first-time reader can answer, from the page alone:

- What kind of society and community activity is Riot trying to support?
- What does Riot make easier in ordinary life and during disruption?
- What currently works, what is prototype/direction, and what privacy should not be assumed?

Failure means the page still reads primarily as a privacy disclaimer, a disaster-survival product, a
technical architecture explainer, or a claim that software itself creates community.

## Scope Boundaries

This change does not alter Riot protocols, application behavior, privacy guarantees, deployment
architecture, analytics policy, or product status. It does not implement encrypted groups, anchors,
new sync transports, redirects, or telemetry. It changes marketing structure and copy, adds the
canonical resilience route plus a static compatibility route, updates mirrors/navigation/sitemap,
and strengthens automated editorial contracts and their existing CI job. Deployment and post-deploy
checks are outside this change; no production mutation is authorized by this design.

## Design Review History

Revision 1 established the human-first thesis and page structure. The first five-role review found
the narrative sound but requested: exact compatibility-route behavior, an explicit capability/status
matrix, stronger availability and censorship-resistance boundaries, ordinary joyful life ahead of
crisis language, an exact page inventory, and a named test/CI gate. Revision 2 incorporates each of
those requests. The second review approved the product and security framing, then found two blockers:
an accidental present-tense “scale” implication and an absolute canonical host outside the static
site's configuration. Revision 3 makes scale explicitly directional, uses an origin-relative
canonical route, reconciles every stale README inventory statement, bounds automated forbidden-claim
patterns, rejects signature-as-truth implications, and tests every form of remote runtime asset.

## Primary Sources

- Rebecca Solnit, *A Paradise Built in Hell*, publisher description:
  <https://www.penguinrandomhouse.com/books/301070/a-paradise-built-in-hell-by-rebecca-solnit/>
- Rebecca Solnit interview on disaster, community, and everyday civic confidence:
  <https://www.aarp.org/advocacy/the-author-speaks-disaster-strikes-people-shine-2010/>
- Riot product grounding: `docs/product/product-brief.md`, `README.md`, and
  `docs/architecture/willow-architecture.md`.
