# Riot Resilience Page Reframing Design

**Date:** 2026-07-22  
**Status:** Approved direction; pending design review gate  
**Scope:** Riot marketing site narrative and route structure only

## Purpose

Reframe Riot's current privacy page around the human capacity Riot exists to support: people making
useful, joyful, cooperative life together, especially when ordinary institutions and infrastructure
fail. Privacy remains an honest product boundary, but it is not Riot's primary promise.

The page should celebrate the society described in Rebecca Solnit's *A Paradise Built in Hell*:
people commonly respond to disruption with solidarity, improvisation, generosity, mutual aid, and
new forms of local civic life. Riot does not create that capacity or replace community. Riot builds
tools that help communities practice it every day, carry it through disruption, and extend it across
more people and places.

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

> When ordinary systems fail, people build systems of care. Riot helps communities publish what
> they know, meet and decide, coordinate shared work, and carry their collective memory—with tools
> they can keep using when a platform, server, domain, or network path disappears.

The page speaks about people before protocols. Technology appears as enabling material, not the
protagonist. The page should feel celebratory, abundant, practical, and invitational: closer to a
festival, community kitchen, neighborhood assembly, or collectively built future than to a bunker,
security product, or catastrophe checklist.

## Framing Principles

1. **Human capacity first.** People already know how to cooperate, improvise, care, and organize.
   Riot helps that capacity travel and scale.
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

### 2. What people build together

Show the positive civic world Riot supports:

- public conversations and community media;
- assemblies, meetings, proposals, and decisions;
- needs-and-offers boards, schedules, supplies, and shared work;
- local guides, histories, corrections, and collective memory;
- small community-made tools that fit local practice.

Examples should span ordinary life and disruption. Avoid reducing community life to emergency
alerts or protest logistics.

### 3. Build the capacity before the crisis

Explain that resilient coordination comes from relationships and habits already in use. A tool used
for a neighborhood event, cooperative decision, local publication, or volunteer rota is also a tool
people understand when conditions become difficult.

This section rejects both “panic” mythology and disaster cosplay. Riot's contribution is continuity:
the community does not have to invent its information practices from scratch at the worst moment.

### 4. What Riot makes easier

Describe the product through four human verbs rather than components:

- **Publish:** share signed public updates and community media.
- **Meet:** propose, discuss, decide, and keep a durable record.
- **Coordinate:** manage needs, offers, schedules, checklists, and local knowledge.
- **Carry:** keep useful community state and tools on participants' devices and move them through
  available paths.

Current status must remain explicit. Working behavior, tested prototypes, and future direction
cannot be presented as equivalent. The exact status language should follow the existing marketing
site's “working now / tested prototype / direction” convention.

### 5. More than one path; no single off switch

Only after the social purpose is clear should the page explain the technical model:

- community data can be held by participants rather than only by a service;
- signed records let Riot clients check authorship independently;
- local reading and writing can continue against data already held;
- nearby and portable exchange provide additional paths;
- public hosts and ordinary web mirrors can improve reach and discovery without becoming the sole
  owner of community identity or history.

Phrase this as graceful continuity and replaceability, not a guarantee that every path always works.
The central line is: a community can walk away from infrastructure without walking away from itself.

### 6. Honest boundaries — resilience is not secrecy

Keep this section compact but prominent enough to prevent a false safety inference:

- current public spaces should be treated as publishable;
- Riot does not currently ship private encrypted groups;
- pseudonymity is not anonymity;
- hosts, gateways, network operators, and nearby observers may see connection metadata;
- devices retain what they hold and compromised devices remain a risk;
- Riot is a prototype, not an audited hardened safety tool.

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
- Preserve `/privacy/` as a compatibility route containing the same current content and a canonical
  link to `/resilience/`; do not break existing bookmarks or external links.
- Keep source and `marketing/public/` deployment mirrors byte-identical for each route.
- Add `/resilience/` to the sitemap and retain `/privacy/` while it remains a compatibility route.
- Update `marketing/README.md` so route ownership and the compatibility behavior are explicit.
- Site-wide navigation-label changes are mechanical scope. Do not rewrite unrelated page content.

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

## Verification and Acceptance Criteria

Automated contracts must verify:

1. source/public byte identity for `/resilience/` and `/privacy/`;
2. the canonical relationship from the compatibility page to `/resilience/`;
3. `/resilience/` appears in the sitemap;
4. every marketing page's primary navigation includes the **Resilience** label and canonical route;
5. the resilience page contains the required hero, human-activity sections, honest-boundaries
   section, website disclosure, source attribution, and participation links;
6. forbidden absolute claims such as “uncensorable” and “unstoppable” do not appear;
7. the page contains no remote script, analytics, cookie, or tracking dependency;
8. the full existing marketing contract suite remains green.

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
and strengthens automated editorial contracts.

## Primary Sources

- Rebecca Solnit, *A Paradise Built in Hell*, publisher description:
  <https://www.penguinrandomhouse.com/books/301070/a-paradise-built-in-hell-by-rebecca-solnit/>
- Rebecca Solnit interview on disaster, community, and everyday civic confidence:
  <https://www.aarp.org/advocacy/the-author-speaks-disaster-strikes-people-shine-2010/>
- Riot product grounding: `docs/product/product-brief.md`, `README.md`, and
  `docs/architecture/willow-architecture.md`.
