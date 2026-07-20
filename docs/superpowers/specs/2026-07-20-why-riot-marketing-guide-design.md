# Why Riot Marketing Guide Design

Date: 2026-07-20
Status: Approved in conversation; design review pending

## Product decision

Publish a standalone guide at `/why-riot/` on the existing Riot marketing site
at `riot.divine.video`.

The guide explains why Riot is special and what communities can do with it that
was not practical before. It uses one story told at three depths:

1. community members and organizers;
2. partners, funders, and journalists; and
3. builders and protocol readers.

The sections build on one another rather than repeating three independent
introductions. A reader can stop after the depth they need or continue into the
technical explanation.

The central promise is:

> Riot lets a community's information, tools, and memory travel with its people,
> even when the internet or a central service is unavailable.

The guide is public product communication, not an internal architecture note.
It uses plain language first, reserves protocol terminology for an expandable
technical subsection, and visibly separates current behavior from planned work.

## Audience and job

### Community members and organizers

The guide should let this reader answer:

- What can my community keep doing without internet?
- How do we share information nearby or through the web?
- Is Riot only chat, or can it carry useful tools and structured information?
- Can I participate in and manage more than one community?
- What control and privacy does Riot actually provide?

### Partners, funders, and journalists

The guide should let this reader answer:

- How is Riot structurally different from a conventional platform?
- What does it mean for a community to possess its infrastructure?
- Why are replaceable gateways and participant-held copies politically useful?
- What kinds of civic, mutual-aid, disaster, and media work does this enable?
- Which claims describe the current product and which are future direction?

### Builders and protocol readers

The guide should let this reader answer:

- What does Willow supply?
- What does Riot add?
- Why can the same update move online or offline?
- Where do community authority, app isolation, and reconciliation live?
- What privacy properties exist, and which require optional or future protocols?

## Editorial position

The page presents Riot as community infrastructure, not merely another social
network or messenger.

Most collaborative products bind authoritative state to a company account,
server, relay network, or hosted database. Riot uses Willow so community data
can remain useful on participants' devices and can travel over different
available paths. Internet gateways improve reach but do not become the sole
authority for the community's identity or existing data.

The page does not imply that all servers are absent or harmful. It explains the
more precise distinction: Riot can use servers without making one server the
canonical owner of the community.

The page follows four editorial rules:

1. **Lead with outcomes.** Say what a person can do before naming the mechanism.
2. **Use "Willow update" in public copy.** Do not introduce signatures,
   namespaces, entries, payload digests, or reconciliation in the main flow.
3. **Explain decentralization concretely.** Name who holds data and what a
   gateway can and cannot control instead of using "decentralized" alone.
4. **Separate proof from direction.** Never turn designed or partially verified
   behavior into a shipped privacy, transport, or reliability guarantee.

## Page title and opening

Working title:

> Community infrastructure that travels with people

The opening contrast is:

> Most community software works only while its company, server, and internet
> connection remain available. Riot keeps the useful parts — publishing,
> coordination, shared tools, and community memory — on people's own devices.

The opening then names the new abilities:

- continue reading, creating, and organizing while offline;
- exchange community updates directly with nearby devices;
- use replaceable internet gateways when connectivity exists;
- carry structured boards, guides, checklists, news, and approved mini-apps,
  not only chat messages;
- create, follow, switch, share, archive, and restore multiple communities; and
- keep the community useful across changes in transport and infrastructure.

## Information architecture

### 1. Hero

The poster-style hero contains:

- the working title;
- a two-sentence product explanation;
- a short status label that avoids production-readiness claims; and
- jump links for **For communities**, **For partners**, and **For builders**.

The hero does not begin with Willow, cryptography, or a protocol comparison.

### 2. For communities: "Your community should still work when the network doesn't"

This section uses short, concrete scenarios:

- a tenants union sharing alerts, meeting changes, rides, and checklists;
- disaster responders carrying shelter maps, supply requests, and verified
  updates into disconnected areas;
- a mutual-aid network coordinating needs, offers, schedules, and knowledge;
  and
- an independent publication continuing to circulate if a website is blocked
  or a gateway disappears.

It then explains the capabilities in plain language:

- **Keep working offline.** Reading, writing, organizing, and preparing updates
  happen on the device.
- **Share without internet.** Nearby devices can connect, review offered
  updates, and choose whether to add them.
- **Share with internet.** Public gateways can make community information
  reachable on the web without owning the community's identity.
- **Carry more than messages.** A community can carry structured information
  and community-approved tools.
- **Manage several communities.** People can create, follow, switch, share by
  link or QR, archive, and restore communities.
- **Keep human control.** External updates are reviewed before acceptance, and
  unapproved community tools do not execute.

The anchor line is:

> Riot turns every participating device into part of the community's library,
> newsroom, toolbox, and distribution network.

### 3. For partners: "Infrastructure communities can possess, not merely access"

This section explains the strategic and political difference:

- signed community data, rather than a domain name or gateway database, carries
  authority;
- participant devices provide shared custody of existing community information;
- gateways are replaceable caches, renderers, indexes, and transports;
- communities choose their tools and editorial practices;
- public newswires can combine open publishing with transparent editorial
  actions rather than invisible ranking; and
- ordinary community infrastructure can degrade gracefully during outages,
  shutdowns, or censorship.

The opportunity is framed through:

- community media without one publishing server to seize;
- local coordination that remains useful when infrastructure fails;
- community-specific software ecosystems;
- continuity between field exchange and ordinary web publishing; and
- shared foundations for mutual aid, tenant organizing, clinics, cooperatives,
  disaster response, protests, and independent media.

This section says Riot establishes an architecture for community autonomy. It
does not claim production readiness, completed security audits, guaranteed
availability, or confidentiality for the public Newswire.

### 4. Visual: "One update, two paths"

The page includes one accessible, site-native HTML/CSS illustration rather than
a decorative bitmap.

The diagram follows this plain-language sequence:

```text
You post or update something
              |
Riot publishes a Willow update
              |
       +------+------+
       |             |
 internet gateway   nearby devices
       |             |
       +------+------+
              |
the same shared community space
```

Supporting copy:

> The update stays the same whichever route it takes. Internet available? A
> gateway can help distribute it widely. No internet? Riot devices can exchange
> it directly nearby. When connections return, the community's copies can come
> back together.

The illustration must remain understandable in text order without CSS, expose
meaningful labels to assistive technology, and avoid implying that the current
gateway is already a complete global relay or that physical-phone Bluetooth has
been fully verified.

### 5. For builders: "Willow separates shared data from the network carrying it"

The first explanation remains non-technical:

> Riot publishes your update into the community's Willow space. It can then
> travel through an internet server or directly between devices offline. Either
> way, it becomes part of the same shared community space.

The key insight is:

> Offline and online are not separate versions of the community. They are
> different ways of carrying the same shared state.

An expandable **Under the hood** subsection may then use precise language.

Willow provides:

- independent namespaces;
- subspaces, paths, timestamps, and arbitrary payloads;
- deterministic store joins for partial replicas;
- a data model independent of any one network transport; and
- optional Meadowcap capability and Willow synchronization protocols.

Riot adds:

- community and profile semantics;
- community relationships and per-community identities;
- public Newswire records and editorial actions;
- community-approved, content-addressed mini-apps;
- app-scoped data and hardened native execution;
- preview, validation, and acceptance policy;
- durable multi-community management;
- nearby and gateway product flows; and
- native interfaces for people who should not need to understand Willow.

The guide must state that Willow itself does not define Riot profiles,
communities, newswires, moderation, mini-apps, or native sandbox behavior.

### 6. Privacy: "Privacy through control, not secrecy"

The public Newswire is intentionally semi-public and plaintext. Alerts, mutual
aid requests, public reporting, and community publications are often meant to
circulate. The page must not imply that widely shared public information is
secret.

Approved public framing:

> Riot is privacy-respecting, not secret by default. Public community updates
> are meant to circulate. Privacy comes from reducing centralized collection,
> keeping community data on participants' devices, supporting separate
> community identities, limiting what tools can access, and letting people
> exchange information without always exposing their activity to internet
> infrastructure.

> Riot cannot promise anonymity, conceal public posts, or erase every copy after
> information has spread. Encrypted private groups are planned separately.

The concise boundary is:

> You control your participation and your local data — not every copy of public
> information once it has been shared.

#### Privacy claim boundaries

The page may say:

- Willow spaces can live on participants' hardware without a central authority.
- Willow is end-to-end encryptable.
- Meadowcap can scope read and write authority.
- Riot supports separate pseudonymous data identities between joined
  communities.
- Riot asks before accepting a nearby connection or adding offered updates.
- Community mini-apps are isolated to their own Riot data and run behind strong
  network restrictions.
- A gateway is replaceable and is not the protocol authority for the community.

The page must also say or preserve these boundaries:

- Willow and Riot public data are not automatically encrypted.
- Riot's current public Newswire is plaintext.
- A public publisher cannot control later redistribution.
- Read capabilities cannot recall data a recipient already copied.
- Pseudonymity is not an anonymity guarantee; names, behavior, proximity, and
  network metadata can correlate people.
- A gateway sees the public content sent through it and may observe ordinary
  connection metadata.
- Decentralized storage cannot guarantee deletion from every raw replica.
- Encrypted private groups and their stronger leakage boundaries are not built.

### 7. "What works today / What comes next"

The guide closes with a visible status block.

Current, code-backed capabilities:

- durable creation, following, switching, archiving, restoring, and reopening
  of multiple communities;
- canonical community share links and iOS QR codes;
- local Newswire creation and durable display;
- nearby connection confirmation and preview-before-accept import;
- tested local-network/Bonjour peer exchange;
- public gateway rendering from exported community data;
- community-approved mini-app packages;
- isolated per-app Willow data and hardened app execution; and
- fresh per-community author identities when joining another community.

Capabilities that must remain labeled as planned, partial, or unverified:

- encrypted private groups;
- confidentiality for current public communities;
- full deletion from devices that already copied public content;
- a complete production global gateway/backhaul network;
- production scale, audited security, or guaranteed availability;
- full interoperability with every Willow draft and transport; and
- physical two-iPhone Bluetooth exchange until it has been rehearsed and
  verified on real devices.

## Willow source alignment

The guide's Willow explanation is grounded in Willow's own current materials:

- the Willow homepage describes independent digital spaces stored on users'
  hardware, explicit receipt, offline operation, and transport through paths
  such as the internet or a USB key;
- the Willow Data Model describes payloads addressed by paths, timestamps,
  subspaces, and namespaces, with deterministic joins between stores;
- Meadowcap describes fine-grained, delegable read/write authority without a
  required central authority;
- Willow Confidential Sync describes private interest overlap and partial
  synchronization, but Riot's public Newswire must not inherit its
  confidentiality claims; and
- Willow Drop Format demonstrates asynchronous movement through improvised
  channels, but Riot must not claim interoperable Drop Format support until its
  own conformance bar is met.

Primary sources:

- <https://willowprotocol.org/>
- <https://willowprotocol.org/specs/data-model/>
- <https://willowprotocol.org/specs/meadowcap/>
- <https://willowprotocol.org/specs/confidential-sync/>
- <https://willowprotocol.org/specs/drop-format/>

The rendered guide should link these primary sources from the technical section
and carry a visible checked date.

## Visual design

The page reuses the current marketing site's self-contained poster language:

- Anton display type, Work Sans body text, and Space Mono labels;
- paper, ink, electric blue, and pink colors;
- hard borders, stamped labels, and restrained zine-like composition;
- existing light and dark theme behavior;
- no remote fonts, analytics, scripts, or runtime dependencies; and
- motion disabled under `prefers-reduced-motion`.

The page is a readable long-form guide rather than a dashboard. The three
audience depths receive clear visual transitions. The two-path diagram is the
only required explanatory visual and must be built from semantic HTML and CSS.
No generated illustration or stock photography is required.

## Marketing-site integration

The implementation follows the static source/deployment mirror convention:

- source guide: `marketing/why-riot/index.html`;
- deployment mirror: `marketing/public/why-riot/index.html`;
- source homepage integration: `marketing/index.html`;
- deployment homepage integration: `marketing/public/index.html`;
- route documentation: `marketing/README.md`; and
- focused structural contract coverage under `scripts/marketing/`.

The homepage gains:

- a primary **Why Riot** navigation link;
- a prominent **Why Riot is different** callout; and
- a footer link.

The existing `/protocols/` field guide remains available as the deeper protocol
comparison. The new page should link to it from the builder section rather than
reproduce its comparison matrix.

Responsive navigation must keep **Why Riot** discoverable on small screens
without making the existing Protocols route inaccessible.

## Accessibility and failure behavior

- The full article remains readable with JavaScript disabled.
- The two-path diagram has a linear text equivalent in document order.
- Jump links land on correctly ordered headings.
- Focus states, link text, contrast, and touch targets match or improve the
  existing site.
- No page-level horizontal overflow appears at 320 CSS pixels.
- External Willow sources are normal links; their failure never breaks the page.
- Planned and current claims remain distinguishable without relying on color.
- Reduced-motion users receive no reveal or decorative transition movement.

## Verification contract

Implementation is complete only when:

1. source and deployment mirror files compare byte-for-byte;
2. `/why-riot/` includes the three approved audience sections;
3. the page uses "Willow update" in primary public copy and reserves protocol
   terms for the technical disclosure;
4. the same-update/two-path visual remains comprehensible without CSS or
   JavaScript;
5. the privacy section states that public Newswire content is plaintext and
   does not promise anonymity, recall, universal deletion, or completed private
   groups;
6. current and planned capabilities are visibly separated;
7. every Willow technical claim links to a current primary source;
8. the page makes no runtime network requests except deliberate navigation;
9. heading order, keyboard navigation, contrast, reduced motion, and a
   320-pixel viewport pass focused checks;
10. Playwright screenshots at phone and desktop widths show no clipped or
    overlapping content;
11. the homepage, guide, protocol page, and footer provide a coherent navigation
    path; and
12. the existing marketing and repository checks remain green.

## Out of scope

- Implementing new Riot application behavior.
- Claiming or implementing encrypted private groups.
- Changing Willow protocol behavior.
- Building a CMS, framework, analytics pipeline, or remote asset service.
- Replacing the existing protocol comparison.
- Adding decorative AI-generated imagery.
- Deploying before the design and implementation quality gates pass.
