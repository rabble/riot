# Riot Protocol Comparison Marketing Design

Date: 2026-07-13
Status: Approved direction; written-spec review pending

## Product decision

Publish a dedicated `/protocols/` page explaining where Riot fits relative to
AT Protocol, ActivityPub, DFOS, Nostr, Farcaster, Bitchat, Secure Scuttlebutt,
Briar, and Matrix. The page is for technically curious readers evaluating
Riot, not the primary audience encountering Riot for the first time.

The existing homepage remains centered on the human outcome: communities can
create, verify, carry, and use information without dependable infrastructure.
It gains only two quiet links to `/protocols/`: one inside “For the technically
curious” and one in the footer. The homepage hero, primary calls to action, and
main navigation do not become a protocol comparison.

The comparison is one long-form page rather than a collection of child routes.
It combines the three useful formats Rabble selected:

1. a compact orientation matrix;
2. a dedicated field guide explaining the important differences;
3. standardized deep-dive sections for each neighboring protocol.

This keeps the comparison easy to scan while avoiding a premature publishing
system or a maintenance-heavy series of separate pages.

## Audience and job

The primary reader is a protocol designer, local-first practitioner, potential
contributor, funder, security reviewer, or technically sophisticated organizer
who already understands Riot's value proposition and now wants to evaluate its
architecture.

The page must let that reader answer:

- Is Riot another federated social network, relay protocol, or messenger?
- What does Willow supply, and what does Riot add?
- What still works when there is no reachable server or internet path?
- Who can alter data, read plaintext, observe metadata, or deny availability?
- Does the system distribute posts, messages, mergeable data, or executable
  applications?
- Which claims describe verified Riot behavior and which remain planned?

## Editorial position

The comparison is not a leaderboard and does not declare a universal winner.
These systems occupy different layers and optimize for different situations.
Each is first described on its own terms, then compared along shared axes.

The article uses four editorial rules:

1. **Name the trust boundary.** “Decentralized,” “private,” and “local-first”
   never substitute for saying who holds plaintext and who controls access.
2. **Separate protocol from product.** For example, AT Protocol is not Bluesky,
   ActivityPub is not Mastodon, Willow is not Riot, and the DFOS protocol is not
   the complete Dark Forest OS product.
3. **Separate proof from roadmap.** Riot's current public communal spaces,
   signed miniapp runtime, and headless nearby-sync evidence are described as
   current. Encrypted groups, production multi-space persistence, relay
   backhaul, and unverified physical-iPhone BLE behavior are labeled separately.
4. **Use primary sources.** Every protocol profile links its governing
   specification or maintainer documentation. The page carries a visible
   “Checked 2026-07-13” date because protocol behavior and maturity can change.

## Page structure

### 1. Hero: “Where Riot fits”

The opening says plainly that Riot is a community application runtime built on
Willow, not a new universal social protocol. It introduces Riot's distinctive
combination:

- canonical mergeable data on each device;
- operation without a reachable server;
- transport-independent carriage between nearby devices;
- per-community, signed miniapps using isolated data paths;
- human approval before an app may execute for a community.

The hero includes a short status badge: “Prototype · claims separated by
evidence.” It does not claim production readiness, audited security, private
groups, or verified two-phone BLE.

### 2. “These systems are not the same layer”

A compact taxonomy prevents the matrix from creating a false equivalence:

- **social publishing and federation:** AT Protocol, ActivityPub;
- **signed relay/hub social networks:** Nostr, Farcaster;
- **verifiable identity and content substrate:** DFOS;
- **offline and nearby messaging:** Bitchat, Briar;
- **offline-first replicated social feeds:** Secure Scuttlebutt;
- **federated messaging rooms:** Matrix;
- **mergeable data substrate:** Willow;
- **community product and signed app runtime:** Riot.

### 3. Orientation by situation

Before the technical matrix, five short situation cards state which design
center matters:

- public social publishing across the internet;
- portable identity across service providers;
- encrypted messaging with a known person or group;
- nearby communication with no internet;
- a community carrying shared state and its own small applications.

The cards link to the relevant profiles but do not recommend one protocol as a
drop-in replacement for another.

### 4. Comparison matrix

The matrix uses rows for systems and these columns:

- design center;
- canonical data unit and conflict/history model;
- identity and authority;
- replication and transport;
- useful behavior without internet or a home server;
- server, relay, or hub role;
- plaintext and metadata visibility;
- application/extension model;
- current maturity or deployment context.

Dense details stay out of cells. Each row links to its deep-dive anchor. On
small screens the table lives in an explicitly labeled horizontal scroll
region; the first column remains visually distinct. The deep dives repeat the
meaningful content in a phone-readable format, so the table is not the only
accessible representation.

### 5. Standardized protocol profiles

Each profile follows the same order:

1. **What it is for** — its stated design center in one sentence.
2. **What is canonical** — repository, activity object, event, message, chain,
   feed, room DAG, or Willow entry.
3. **How data moves** — federation, relay/hub, gossip, nearby mesh, or generic
   reconciliation.
4. **Who is trusted** — identity authority, hosting operator, relay, hub,
   membership controller, or device.
5. **Privacy boundary** — who can read content and which metadata remains
   observable.
6. **Offline behavior** — what can be created, read, and exchanged without an
   internet path.
7. **Extension model** — schemas, clients, NIPs, frames/miniapps, bots, or no
   general app runtime.
8. **Relationship to Riot** — one thing Riot learns from it and one deliberate
   difference.
9. **Primary sources** — direct links plus the checked date.

The profiles remain concise. They explain architectural distinctions rather
than recounting project histories or scoring communities.

### 6. “Why Willow?”

This is the page's central technical explainer. It distinguishes Willow's
responsibility from Riot's:

**Willow provides:**

- namespaces, subspaces, paths, timestamps, and payload digests;
- deterministic entry ordering and reconciliation over partial replicas;
- a data model independent of any particular network transport;
- Meadowcap capability scoping alongside the data model.

**Riot provides:**

- community and profile semantics;
- organizer authority and per-space app approval;
- signed, content-addressed HTML/CSS/JavaScript bundles;
- hardened native WebViews with no network access;
- the app-scoped `get`, `put`, `list`, `watch`, `whoami`, and `profile` bridge;
- nearby transport adapters, preview/accept policy, persistence, and native UX.

The article explicitly says Willow itself does not define profiles, feeds,
moderation, group membership, a miniapp store, or Riot's execution sandbox.

### 7. One concrete Riot data path

A small diagram follows one checklist mutation:

1. a person checks an item inside a trusted miniapp;
2. the WebView bridge validates the app-relative key and JSON value;
3. Riot writes a signed Willow entry under the app's path in the community
   namespace;
4. another device reconciles the entry over an available transport;
5. Willow selects the current value for that path;
6. Riot notifies the already-open miniapp and it redraws.

The caption separates evidence from assumption: this path is proven through
the native simulator runtime and headless loopback/Bonjour sync tests; the same
flow over BLE between two physical iPhones remains unverified.

### 8. “What Riot does not provide yet”

An honest limitations block prevents the comparison from turning planned work
into present-tense marketing. It lists:

- encrypted private groups are designed but not shipped;
- the current public-space model is not confidential;
- nearby physical BLE lacks two-iPhone verification;
- multi-community durable storage is not complete;
- Riot is not yet an interoperable implementation of every Willow draft;
- the miniapp catalog/runtime is a prototype, not an audited public app store.

### 9. Sources and maintenance

The page ends with a source ledger grouped by protocol. Sources must be primary:

- the W3C ActivityPub Recommendation and official Matrix specifications;
- official AT Protocol architecture/specification documents;
- DFOS protocol, content model, credential, relay, and DID documents;
- the canonical Nostr NIPs repository/specification;
- official Farcaster protocol documentation;
- the official Bitchat project repository and its protocol/whitepaper documents;
- official Briar documentation;
- canonical Secure Scuttlebutt protocol documentation;
- Willow Data Model, Meadowcap, Willow'25, Confidential Sync, WTP, and changes
  pages;
- Riot's checked-in architecture and measured test evidence.

Claims that cannot be supported by those sources are omitted or labeled as an
inference. Every future material edit updates the checked date.

## Visual design

The page reuses the marketing site's Anton, Work Sans, Space Mono, paper/ink,
pink, and electric-blue language. It remains a reading page rather than a
dashboard:

- oversized poster title followed by restrained long-form typography;
- black rules and hard-bordered cards for hierarchy;
- pink labels for trust-boundary warnings;
- blue for links and data-flow arrows;
- no decorative protocol logos that imply endorsement or require image assets;
- no remote fonts, scripts, analytics, or other runtime requests.

The matrix is only one section. Long-form profiles use a responsive single
column with a sticky in-page table of contents on wide screens and a normal
contents list on phones. Source links are descriptive and keyboard accessible.
Motion is limited to the homepage's existing reveal behavior and is disabled by
`prefers-reduced-motion`.

## Routes and files

The implementation follows the current static-site mirror convention:

- source page: `marketing/protocols/index.html`;
- deployed copy: `marketing/public/protocols/index.html`;
- source homepage link: `marketing/index.html`;
- deployed homepage link: `marketing/public/index.html`;
- marketing documentation: `marketing/README.md`;
- structural contract test: `scripts/marketing/protocol-page-contracts.mjs`.

The two homepage files remain byte-identical, as do the two protocol-page
files. Cloudflare's existing static asset configuration serves
`/protocols/index.html`; the page also uses a canonical `/protocols/` link.
No framework, generator, server code, tracking, or content-management system is
introduced.

## Failure and maintenance behavior

- The page remains fully readable when JavaScript is disabled.
- External source links open normally but no external resource is required to
  render the article.
- A missing or unreachable source does not break the page; it is a normal link,
  not fetched client-side.
- Unknown or ambiguous protocol behavior is described as unknown rather than
  normalized into a misleading yes/no cell.
- Protocol maturity is dated, not presented as timeless.
- Planned Riot capabilities are visually and textually distinct from verified
  behavior.

## Verification contract

Implementation is complete only when:

1. the source and deployed mirror pairs compare byte-for-byte;
2. the page contains every named protocol and every comparison axis;
3. every protocol profile has at least one direct primary-source link;
4. privacy copy distinguishes content confidentiality, operator access,
   metadata visibility, integrity, authorization, and availability;
5. the Riot section names physical BLE and encrypted groups as unproven or
   planned rather than shipped;
6. the page makes no network requests when rendered except navigation after a
   person deliberately follows an external source link;
7. keyboard navigation, heading order, link text, table labeling, contrast,
   reduced motion, and a 320-pixel viewport pass focused checks;
8. Playwright screenshots at 390×844 and 1280×800 show no clipped copy,
   overlapping navigation, or page-level horizontal overflow;
9. the existing homepage's primary hierarchy and calls to action remain
   unchanged apart from the two quiet protocol links;
10. local structural contracts and the existing marketing deployment command
    complete successfully.

## Out of scope

- changing Riot's protocol, runtime, privacy model, or application behavior;
- changing the homepage's primary value proposition;
- declaring a protocol winner or publishing numeric rankings;
- protocol logos, trademarks, screenshots, or externally hosted assets;
- comments, subscriptions, analytics, feeds, or a blog engine;
- separate child pages for every protocol in this first release;
- automatic scraping or freshness monitoring of third-party documentation;
- deploying to production without the existing deployment owner and release
  process.
