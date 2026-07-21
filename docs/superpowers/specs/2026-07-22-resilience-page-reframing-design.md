# Riot Human-Capacity Marketing Reframe

**Date:** 2026-07-22  
**Status:** Design review candidate, revision 4

**Scope:** Reframe `/why-riot/`, compact `/privacy/`, clarify the homepage hero, and reconcile
site-wide claims and navigation. No application, protocol, or deployment behavior changes.

## Decision Summary

Riot's central story is not perfect privacy or merely keeping software available during failure. It
is the human capacity to make useful, joyful, cooperative life together. Riot builds public tools
for that capacity: conversation, publishing, meetings, decisions, coordination, shared knowledge,
and collective memory that communities can hold and carry themselves.

The canonical human story belongs at the existing `/why-riot/` route. Do not create
`/resilience/`. The live site already uses `/why-riot/` for the product argument and `/guide/` for
task instructions; another manifesto route would duplicate both the homepage and Why Riot while
making an already crowded navigation worse.

Keep `/privacy/` as a short, factual reference. Public Riot communities are public, private
encrypted groups are not shipped, and the marketing site's limited data posture remains worth
stating. Privacy is a boundary, not the product's headline.

## Intellectual and Emotional Frame

The page draws from Rebecca Solnit's *A Paradise Built in Hell*: during disruption, people often
respond with solidarity, improvisation, generosity, and new forms of civic life. The page must not
romanticize disaster. Its subject is the cooperative capacity people already practice in ordinary
life—at meals, meetings, festivals, kitchens, gardens, publications, repair days, mutual-aid work,
and neighborhood assemblies.

Riot does not create community, trust, truth, or solidarity. It gives communities adaptable tools
with which to practice, record, and carry their own work. Technology is enabling material, never the
protagonist.

The desired register is collective optimism with practical specificity: abundant, handmade,
inviting, and a little punk. It must not sound like a platform promising “connection,” a corporate
business-continuity product, a bunker checklist, or disaster cosplay.

## Audience

- **Community participants and organizers** should recognize ordinary activities they already do
  and see Riot as a place to publish, decide, coordinate, remember, and make tools together.
- **Potential partners**—libraries, newsrooms, cooperatives, clinics, mutual-aid groups, and civic
  institutions—should understand the value of infrastructure a community can possess rather than
  merely access.
- **Builders and technically curious readers** should understand the bounded mechanism and current
  status without having protocols dominate the story.

## Site Architecture

The current editorial routes are:

1. `/`
2. `/why-riot/`
3. `/guide/`
4. `/about/`
5. `/privacy/`
6. `/open-source/`
7. `/community/`
8. `/releases/`
9. `/protocols/`

This change adds no route and no redirect.

### Navigation

- Keep **Why Riot** in primary navigation.
- Keep **Using Riot**, **About**, **Open source**, **Community**, **Get the app**, and
  **Protocol field guide** in primary navigation.
- Remove **Privacy** from primary navigation so it no longer reads as a peer of the core product
  story. Keep `/privacy/` linked in every footer and from relevant boundary sections.
- Preserve the existing rule that every page footer reaches every other editorial route, with a
  page permitted to omit its own self-link.
- Update the explicit page inventories and navigation assertions in the marketing contract and
  `marketing/README.md`; the current README is stale and omits `/why-riot/`, `/guide/`, and
  `/releases/` in several places.

### Route Roles

- `/`: concise product overview and entry point. It may demonstrate the app and architecture, but
  its hero must not duplicate Why Riot.
- `/why-riot/`: canonical social purpose and human-capacity story.
- `/guide/`: task-oriented instructions for the current prototype.
- `/about/`: lineage, project history, and builder.
- `/privacy/`: compact factual privacy and safety reference.
- `/protocols/`: technical comparisons, source ledger, and deeper trust boundaries.

Add `<link rel="canonical" href="/why-riot/">` to Why Riot and
`<link rel="canonical" href="/privacy/">` to Privacy. Origin-relative links work on the configured
Workers origin and any separately approved custom domain without making DNS or TLS assumptions.

## Homepage Changes

The homepage remains a product overview rather than becoming the full Solnit essay.

Replace its duplicate hero with:

> **Community tools that travel with people.**

Supporting direction:

> Riot is a home for public conversation, community decisions, shared tools, and collective
> memory—carried by the people who make it matter.

The hero should link prominently to `/why-riot/` with an invitation such as **Why Riot exists**.
Keep the current app screenshots and product demonstrations. Make only targeted copy changes beyond
the hero: remove absolute availability or preservation claims and avoid presenting shutdown as the
only reason communities need Riot.

## Why Riot Narrative

### 1. Hero — People are the infrastructure

The H1 is:

> **People are the infrastructure.**

The supporting thesis is:

> Every day, people make a community through meals, meetings, stories, decisions, celebrations,
> care, and shared work. Riot helps them publish what they know, decide together, coordinate what
> needs doing, and carry their collective memory.

The hero may carry one quiet label: **Prototype, built in the open**. It must not lead with outages,
privacy, Willow, cryptography, servers, censorship, or product limitations.

### 2. A community is something people do

Show ordinary collective life before disruption:

- a block party or festival;
- a community kitchen, garden, or repair day;
- a cooperative or neighborhood meeting;
- a local publication, history, or guide;
- needs-and-offers boards, rides, schedules, and shared work;
- proposals, decisions, corrections, and community memory.

Use an original, code-native inline illustration or collage in the established poster palette to
make the social world visible. It should suggest several people cooking, meeting, publishing,
gardening, making music, or sharing work—not a heroic individual, phone network, server diagram,
padlock, protest confrontation, or disaster scene. The illustration must be meaningful decoration
with concise accessible text or `aria-hidden="true"` when adjacent prose carries the meaning. It
must add no remote asset or runtime dependency.

### 3. Tools for the commons

Use four human verbs:

- **Publish:** public updates, community media, and local knowledge.
- **Meet:** proposals, discussion, polls, decisions, and the resulting record. Riot is not a live
  audio/video meeting service.
- **Coordinate:** checklists, boards, schedules, needs, offers, and shared work.
- **Carry:** keep useful community state and installed tools on participants' devices and move them
  through paths available to them.

Status appears as a small label on each card, not as the visual headline. Publish, Meet, Coordinate,
and local Carry are **Available in the prototype**. The labels qualify the software, not the social
practice.

### 4. The future is a practice

Explain that communities become resilient by using shared habits and tools in ordinary life. A tool
used for a festival rota, cooperative decision, neighborhood publication, or community meal is
already familiar when conditions become difficult.

Disruption enters here, after the positive world is established. Disaster is harmful; the hopeful
subject is what people already know how to build together.

### 5. More than one path

Explain the mechanism briefly and in plain language:

- participants can hold community data instead of only accessing a service;
- signed records let Riot verify that a record came from a particular key, not that it is true;
- already-held data and installed tools can remain locally useful on functioning devices;
- files, QR-assisted handoffs, nearby exchange, public gateways, and anchors provide different
  possible paths with different current status;
- hosts improve reach and discovery without owning community identity or history.

This section should be materially shorter than the current Why Riot builder and transport sections.
Link to `/protocols/` for the detailed model.

The central aspiration may be stated as: **A community should be able to leave a provider without
leaving one another.** Label it as an aim, not a guarantee.

### 6. Honest boundaries

Keep one compact boundary panel:

- public Riot spaces should be treated as publishable;
- private encrypted groups are not shipped;
- pseudonymity is not anonymity;
- gateways, hosts, networks, nearby observers, and compromised devices remain risks;
- a signature proves control of a key, not identity, truth, or legitimacy;
- Riot is a prototype, not an audited hardened safety tool;
- local usefulness depends on a functioning device and data already received;
- exchange requires a compatible peer or transport that is actually available.

Recommend established encrypted messengers for material that must remain secret today. Link to
`/privacy/` and `/protocols/` for detail.

### 7. Invitation — Build it with us

End with participation, not purchase. Invite communities to experiment, adapt tools, contribute,
and practice the future together. Link to `/guide/`, `/community/`, `/releases/`, and the source
repository.

Add a small **Intellectual lineage** note crediting Rebecca Solnit and *A Paradise Built in Hell*.
Paraphrase rather than quote, link to the publisher or an authorized interview, and do not imply
Solnit endorses Riot.

## Privacy Page

The page remains at `/privacy/` and becomes a concise reference with this hierarchy:

1. **Public means public.** Public community content is meant to circulate. Riot does not currently
   ship private encrypted groups.
2. **What local-first changes—and what it does not.** Reduce mandatory centralized collection and
   explain local custody, while naming metadata, radio presence, device compromise, copied data,
   pseudonymity, and gateway-presentation risks.
3. **This website.** Preserve the verifiable disclosure: no Riot analytics, cookies, accounts,
   remote fonts, third-party scripts, tracking pixels, or fingerprinting; Cloudflare can observe
   ordinary request metadata while serving the site.
4. **Where to go next.** Link to Why Riot for purpose, Protocols for details, and an established
   encrypted-messenger recommendation for secrets.

Remove the current defensive hero, the large website-first section, duplicated product manifesto,
and repeated capability tables. The page should remain easy to cite when someone asks a precise
privacy question.

## Product-Status Contract

Use these labels consistently across the changed pages:

- **Available in the prototype:** exercised through the current app or bundled tool; no production,
  audit, or deployment-scale claim.
- **Tested locally:** verified through tests, simulator, loopback, or same-machine rehearsal; not
  proven in the relevant physical multi-device/radio setting.
- **In development:** code or design exists, but the end-to-end promise is incomplete.
- **Direction, not shipped:** intended capability that must not be relied on today.

| Claim | Required status |
|---|---|
| Publish signed public updates and community media | Available in the prototype |
| Meeting artifacts, polls, discussion, decisions, and shared records | Available in the prototype |
| Bundled checklists, supply board, roll call, and quick poll | Available in the prototype |
| Local use of already-held state and installed tools | Available in the prototype |
| Portable file, share-link, or QR-assisted handoff | Available in the prototype |
| Nearby peer exchange | Tested locally |
| Public gateway rendering from exports | Available in the prototype |
| Replaceable public-anchor discovery and remote sync | In development |
| Private encrypted groups | Direction, not shipped |
| Production scale or field-proven resilience | Direction, not shipped |

The full matrix is an editorial and test contract. The rendered Why Riot page should show only the
labels needed beside claims, not reproduce this table as a dominant technical section.

## Site-Wide Claim Audit

Audit all nine editorial source pages and mirrors for unsafe absolutes. Remove or qualify claims
equivalent to:

- uncensorable, unstoppable, impossible to shut down, or nothing anyone can switch off;
- always available, guaranteed to work offline, or works without any prerequisite;
- nothing is ever lost, preserves everything, or recovers unseen data;
- guaranteed delivery, discovery, synchronization, persistence, or recovery;
- anonymous, confidential, or private-by-default public spaces;
- production-ready, audited, field-proven, or operating at scale.

Positive claims name their mechanism and prerequisite. Already-held data may remain useful on a
functioning device. Exchange requires an available compatible path. A lost gateway need not erase
copies participants already hold, but Riot does not guarantee that any complete copy exists.

Automated tests cover a finite, case-insensitive pattern set. Semantic equivalents remain a required
human editorial check.

## Visual and Accessibility Requirements

- Retain the existing poster typography, flat color fields, hard borders, visible focus, and
  responsive card system.
- Preserve skip links, semantic landmarks, logical heading order, keyboard navigation,
  reduced-motion behavior, and readable narrow-screen layouts.
- Verify text and interactive-element contrast against WCAG AA and inspect forced-colors behavior.
- The inline illustration must not create horizontal overflow at 390 px or obscure content when CSS
  is unavailable.
- Keep capability labels visually subordinate to the human narrative.
- No JavaScript is required for meaning or navigation.
- No remote scripts, stylesheets, fonts, images, media, iframes, analytics, beacons, cookies, or
  tracking endpoints.

## TDD and Acceptance Criteria

Extend `scripts/marketing/protocol-page-contracts.mjs` first and run:

```sh
node scripts/marketing/protocol-page-contracts.mjs
```

The new assertions must fail before HTML implementation. After implementation they must verify:

1. all nine source pages have byte-identical `marketing/public/` mirrors;
2. no `/resilience/` source or public route is introduced;
3. Why Riot and Privacy have their exact origin-relative canonical links;
4. all nine source pages and mirrors include Why Riot in primary navigation and omit Privacy from
   primary navigation;
5. every footer preserves reachability to all other routes, including Privacy;
6. sitemap and `marketing/README.md` contain the exact nine-route inventory;
7. homepage hero is distinct from Why Riot and links prominently to `/why-riot/`;
8. Why Riot contains the exact H1, ordinary-life section, four human verbs, practice section,
   compact mechanism and boundary sections, Solnit attribution, and participation links;
9. the code-native illustration is present, accessible, local, and dependency-free;
10. Privacy begins with public-space truth, keeps app/device/metadata boundaries, puts website
    disclosure later, and links back to purpose and technical detail;
11. every material capability claim carries the required status language;
12. the bounded forbidden-claim patterns do not occur across any editorial page;
13. changed pages include no remote runtime or asset dependency;
14. the existing marketing contract suite remains green.

Add `"test:marketing": "node scripts/marketing/protocol-page-contracts.mjs"` to `package.json` and
run it as a distinct blocking step in the existing CI web job after `npm run test:web:unit`.

Implementation verification also includes:

```sh
npm run test:web:unit
npm run test:marketing
```

Then serve `marketing/public/` locally and visually review `/`, `/why-riot/`, and `/privacy/` at
1456×900 and 390×844. Verify navigation wrapping, hierarchy, contrast, illustration behavior, lack
of horizontal overflow, and that technical/status material remains subordinate.

Deployment is outside scope. Do not mutate production or claim the live site changed.

## Scope Boundaries

This work changes marketing HTML, its exact public mirrors, sitemap, marketing documentation,
contract tests, package scripts, and the existing CI web job. It does not change Riot protocols,
application behavior, cryptography, privacy guarantees, anchor behavior, sync transports,
deployment configuration, DNS, TLS, telemetry, or production state.

## Review History

Revisions 1–3 proposed a new `/resilience/` route and a `/privacy/` compatibility alias. The design
gate approved that version before `/why-riot/` and `/guide/` were merged into the current site.
Comparison with the deployed site showed that the route would duplicate the existing canonical
product argument, worsen crowded mobile navigation, and leave site-wide claim conflicts untouched.

Revision 4 follows the approved comparison: reframe `/why-riot/`, keep `/privacy/` concise and
factual, add no route, clarify the homepage hero, make ordinary collective life visible, and audit
claims across the current nine-page site.

## Primary Sources

- Rebecca Solnit, *A Paradise Built in Hell*, publisher description:
  <https://www.penguinrandomhouse.com/books/301070/a-paradise-built-in-hell-by-rebecca-solnit/>
- Rebecca Solnit interview on disaster, community, and everyday civic confidence:
  <https://www.aarp.org/advocacy/the-author-speaks-disaster-strikes-people-shine-2010/>
- Riot product grounding: `README.md`, `docs/product/product-brief.md`,
  `docs/architecture/willow-architecture.md`, and the current marketing pages.
