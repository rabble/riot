# Riot Human-Capacity Marketing Reframe

**Date:** 2026-07-22  
**Status:** Design review candidate, revision 13

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
- Every page footer contains the exact nine-route `allSitePaths` set, including its own route. The
  contract extracts `<footer\b[^>]*>[\s\S]*?</footer>` and compares normalized local href set
  equality; links in the body or primary navigation cannot satisfy this invariant.
- Update the explicit page inventories and navigation assertions in the marketing contract and
  `marketing/README.md`; the current README is stale and omits `/why-riot/`, `/guide/`, and
  `/releases/` in several places.
- Update every source-file, public-mirror, sitemap, and local-preview list in `marketing/README.md`
  to enumerate all nine routes, not only its `## Routes` summary.

The contract migration is explicit: retain `allSitePaths` as the nine-route footer and mirror
inventory; add `primaryNavPaths` with this exact ordered set:
`["/", "/why-riot/", "/guide/", "/about/", "/open-source/", "/community/", "/releases/",
"/protocols/"]`; replace the current
top-navigation loop over `allSitePaths` with a loop over `primaryNavPaths`; and add an assertion that
the set of local route hrefs extracted only from the nested
`<div class="sitenav-links">...</div>` block equals that exact
ordered list and set—no reordering, missing, or additional local route—and does not contain
`href="/privacy/"`. The separate `.sitenav-brand` root link is deliberately excluded, so it does not
duplicate the visible Home item. A local href is normalized with
`new URL(href, "https://local.invalid")` by
discarding query and fragment, converting a terminal `/index.html` to `/`, and ensuring every
non-root path has one trailing slash. The requirement
that the existing suite remains green means its intended coverage remains green after these obsolete
expectations are replaced—not that old and new navigation rules must both pass.

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
This intentionally produces a canonical URL relative to whichever approved host serves the static
bytes; selecting one preferred absolute production origin is a deployment decision outside scope.

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

“People are the infrastructure” is an explicitly user-approved creative requirement. The ordinary-
life thesis must appear immediately beneath it so “infrastructure” reads as relationships and shared
practice, not people treated as technical resources. Reviewers may critique execution of that
relationship but must not substitute a different H1.

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
- **Coordinate:** use the bundled checklist, supply board, roll call, and quick poll. Broader
  schedules, needs-and-offers workflows, and locally adapted tools are examples of direction unless
  separately evidenced.
- **Carry:** keep already-held community state and installed tools useful on a functioning device.
  A subordinate status list distinguishes portable handoff, nearby exchange, gateways, and anchors.

Status appears as a small text label on each card, not as the visual headline and never through color
alone. Publish, meeting artifacts, the four named bundled tools, and local use of already-held state
are **Available in the prototype**. The Carry card also lists: portable file/share-link/QR-assisted
handoff—**Available in the prototype**; nearby peer exchange—**Tested locally**; public gateway
rendering—**Available in the prototype**; public-anchor discovery and remote sync—**In
development**. The labels qualify software behavior, not the social practice.

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
- hosts can improve reach and discovery without becoming the sole authority for community identity
  or the sole holder of its history.

This section should be materially shorter than the current Why Riot builder and transport sections.
Link to `/protocols/` for the detailed model.

The central aspiration may be stated as: **A community should be able to leave a provider without
leaving one another.** Label it as an aim, not a guarantee.

### 6. Honest boundaries

Keep one compact boundary panel:

- public Riot spaces should be treated as publishable;
- current public Newswire content is plaintext, readable and copyable, and has no confidential
  public-read boundary;
- private encrypted groups are not shipped;
- pseudonymity is not anonymity;
- gateways, hosts, networks, nearby observers, and compromised devices remain risks;
- a signature proves control of a key, not identity, truth, or legitimacy;
- Riot is a prototype, not an audited hardened safety tool;
- local usefulness depends on a functioning device and data already received;
- exchange requires a compatible peer or transport that is actually available.

Recommend established encrypted messengers for material that must remain secret today. Link to
`/privacy/` and `/protocols/` for detail. The actionable recommendation is: **For an ordinary
internet-connected conversation that must remain secret, use a purpose-built end-to-end encrypted
messenger such as Signal; choose any safety tool for your actual threat model.** Link “Signal” to
`https://signal.org/` with `rel="noopener"`; do not imply a blanket safety guarantee or Riot/Solnit
endorsement.

### 7. Invitation — Build it with us

End with participation, not purchase. Invite communities to experiment, adapt tools, contribute,
and practice the future together. Link to `/guide/`, `/community/`, `/releases/`, and the source
repository.

Add a small **Intellectual lineage** note crediting Rebecca Solnit and *A Paradise Built in Hell*.
Paraphrase rather than quote, link to the publisher or an authorized interview, and do not imply
Solnit endorses Riot.

## Privacy Page

The page remains at `/privacy/` and becomes a concise reference with this hierarchy:

1. **Public means public.** Current public Newswire content is plaintext, readable and copyable by
   recipients and infrastructure that handles it, and has no confidential public-read boundary.
   Riot does not currently ship private encrypted groups.
2. **What local-first changes—and what it does not.** Reduce mandatory centralized collection and
   explain local custody, while naming metadata, radio presence, device compromise, copied data,
   pseudonymity, and gateway-presentation risks.
3. **This website.** Preserve the verifiable disclosure: Riot's static page code sets no cookies and
   includes no analytics, accounts, remote fonts, third-party scripts, tracking pixels, or
   fingerprinting. Cloudflare can observe ordinary request metadata and controls edge response
   headers; do not claim that static source inspection can guarantee every hosting-layer behavior.
4. **Where to go next.** Link to Why Riot for purpose, Protocols for details, and Signal's official
   site with the same threat-model caveat for an ordinary internet-connected conversation that must
   remain secret.

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
| Export and import a Riot bundle file | Available in the prototype |
| Share a community reference by link or QR for onboarding | Available in the prototype |
| Nearby peer exchange | Tested locally |
| Public gateway rendering from exports | Available in the prototype |
| Replaceable public-anchor discovery and remote sync | In development |
| Private encrypted groups | Direction, not shipped |
| Production scale or field-proven resilience | Direction, not shipped |

The full matrix is an editorial and test contract. The rendered Why Riot page should show only the
labels needed beside the exact claims named above, not reproduce this table as a dominant technical
section. Site-wide prose outside those claims is audited for unsafe absolutes but does not need a
status badge on every sentence.

Status markup is deterministic. `#tools` contains four articles identified by
`data-capability="publish|meet|coordinate|carry"`. Visible status text uses exactly:

```html
<span class="chip" data-status="prototype">Available in the prototype</span>
<span class="chip" data-status="local">Tested locally</span>
<span class="chip" data-status="development">In development</span>
<span class="chip" data-status="direction">Direction, not shipped</span>
```

The exact card contract is:

- `article[data-capability="publish"]`: one `prototype` chip and the words “signed public updates
  and community media”.
- `article[data-capability="meet"]`: one `prototype` chip and the words “meeting artifacts, polls,
  discussion, decisions, and a shared record”; it also says “not live audio or video”.
- `article[data-capability="coordinate"]`: one `prototype` chip and the exact named tools
  “checklist, supply board, roll call, and quick poll”.
- `article[data-capability="carry"]`: no blanket card-level chip. Its six
  `li[data-carry-path]` rows are exactly `local-state`—`prototype`, `bundle-file`—`prototype`,
  `community-reference`—`prototype`, `nearby`—`local`, `gateway`—`prototype`, and
  `anchors`—`development`. `bundle-file` says “export and import a Riot bundle file”.
  `community-reference` says “share a community reference by link or QR” and explicitly says it is
  onboarding/reference, not proof that content moved by radio.

The contract extracts these bounded elements and asserts the exact chip value/text mapping plus the
required phrases. No other prose is classified as “material” by an automated status rule.

Presentation constraint: there is no separate rendered status table. The six Carry rows form one
compact secondary list inside the Carry card; each row is at most 18 visible words before its text
chip. Their type is no larger or heavier than body copy, and chips use the page's small label style.
The four verbs and ordinary-life examples remain the dominant scan path.

## Authoritative Product Status

Executable behavior and current conformance/status pages are authoritative over aspirational product
brief language. Current code and the protocol field guide show public communities, local
create/read/import flows, bundled tools, tested local-network exchange, and gateway rendering; they
also state that private encrypted groups are not shipped.

The root `README.md` and `docs/product/product-brief.md` currently describe private groups as one of
two product modes without marking implementation status. They also use absolute “no server to raid”
or “no server to seize” language. This change updates both documents to label private groups
**Direction, not shipped** and qualifies those seizure-resistance claims: participant-held copies and
replaceable gateways can reduce dependence on one server, but Riot does not guarantee that a
complete reachable copy exists or that publishing, access, persistence, or censorship resistance
survives. No application behavior changes.

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

Automated tests use this exact case-insensitive pattern inventory across the nine editorial pages,
`README.md`, and `docs/product/product-brief.md`:

```text
\buncensorable\b
\bunstoppable\b
impossible to shut down
cannot be shut down
nothing (?:anyone|anybody|someone) can (?:seize|pressure|switch off)
\balways available\b
works? from zero signal
nothing (?:gets|is) (?:silently )?lost
\bpreserves? everything\b
\bguaranteed (?:delivery|discovery|synchroni[sz]ation|persistence|recovery|availability)\b
\banonymous by default\b
\bprivate by default\b
\bproduction[- ]ready\b
\bfield[- ]proven\b
\boperating at scale\b
```

Semantic equivalents remain a required human editorial check.

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
- Why Riot and Privacy contain no `<script>` elements. The homepage may retain its existing
  non-networked IntersectionObserver reveal script, but the contract continues to require that it
  makes no fetch, beacon, storage, cookie, analytics, or remote-resource call.
- No `javascript:` URLs, inline event-handler attributes, `ping` attributes, external SVG
  references, meta-refresh redirects, or forms/form actions.
- External links that use `target="_blank"` must also use `rel="noopener"`.

The static contract uses these exact case-insensitive predicates:

```text
Why Riot / Privacy only: <script\b
All pages: javascript:
All pages: \son[a-z]+\s*=
All pages: \sping\s*=
All pages: <meta\b[^>]*http-equiv\s*=\s*["']?refresh
All pages: <form\b
All pages: <base\b
All pages: \ssrcdoc\s*=
All pages: <(?:use|image|feImage)\b[^>]*(?:href|xlink:href)\s*=\s*["'](?:https?:)?//
All pages: <(?:script|link|img|iframe|audio|video|source|object|embed)\b[^>]*(?:src|srcset|href|data|poster)\s*=\s*["'](?:https?:)?//
All pages: @import\s+url|url\(\s*["']?(?:https?:)?//
All pages: <(?:script|link|img|iframe)[^>]+(?:plausible|google-analytics|googletagmanager|segment\.com|mixpanel|hotjar|clarity)
All pages: (?:plausible|google-analytics|googletagmanager|segment\.com|mixpanel|hotjar|clarity)\.[a-z0-9-]+/(?:[a-z0-9-]+\.js|analytics|track|beacon)
Homepage script: fetch\s*\(|sendBeacon\s*\(|XMLHttpRequest|WebSocket\s*\(|localStorage|sessionStorage|document\.cookie
```

Inline `<svg>` and `data:image/svg+xml` favicon links are explicitly allowed. Ordinary external `<a href>`
citations are allowed and are not runtime resource dependencies. For every `data:image/svg+xml`
attribute, the contract decodes percent encoding or base64, then applies the external-resource,
`javascript:`, inline-handler, `<script>`, `<foreignObject>`, and `<base>` predicates to the decoded
SVG. Decode failure is a test failure. For HTML attributes named `src`, `srcset`, `href`, `data`, or
`poster`, any `data:` value is rejected unless it is the `href` of a `<link rel="icon">` whose MIME
is exactly `image/svg+xml` and whose decoded SVG passes those checks. Existing CSS-embedded
`data:font/...` values remain allowed because they are font bytes, not active HTML documents.

Resource URL validation is allowlist-based, not regex-only. Extract resource-bearing values from
`script[src]`, `link[href]`, `img[src|srcset]`, `iframe[src|srcdoc]`, `audio[src]`, `video[src|poster]`,
`source[src|srcset]`, `object[data]`, `embed[src]`, and SVG `use|image|feImage[href|xlink:href]`.
Permit only origin-local relative/root-relative URLs with no URI scheme and no `//` prefix. The sole
HTML-attribute exception is the decoded and validated `data:image/svg+xml` favicon described above.
For CSS `url(...)`, permit origin-local relative/root-relative URLs and existing
`data:font/woff2;base64,...` font bytes; reject every other `data:` MIME, protocol-relative value, or
URI scheme including `http:`, `https:`, `ftp:`, `file:`, and `blob:`. Ordinary external anchor
citations are outside this resource allowlist and remain permitted.

Parse every `srcset` as a comma-separated candidate list according to its URL-plus-descriptor shape;
validate every candidate URL independently against the same allowlist, reject an empty or malformed
candidate, and never validate only the first token or the unsplit attribute string.

Reject raw resource-attribute/CSS URL values containing backslashes, ASCII control characters, or
invalid percent escapes. Parse HTML with Chromium on the loopback preview so character references
and browser URL normalization are applied; inspect resolved DOM resource properties rather than
trusting raw regex output. The browser-level network gate below is authoritative if a static parser
and Chromium resolution differ.

## TDD and Acceptance Criteria

Extend `scripts/marketing/protocol-page-contracts.mjs` first and run:

```sh
node scripts/marketing/protocol-page-contracts.mjs
```

The new assertions must fail before HTML implementation. After implementation they must verify:

1. all nine source pages have byte-identical `marketing/public/` mirrors;
2. `stat()` returns `ENOENT` for both `marketing/resilience` and
   `marketing/public/resilience`; normalized sitemap paths and all internal hrefs also omit
   `/resilience/`;
3. Why Riot and Privacy have their exact origin-relative canonical links;
4. normalized local route hrefs extracted from every source and mirror primary-navigation block have
   exact ordered-array and set equality with `primaryNavPaths`; Privacy is absent there and retained
   in `allSitePaths` footer checks;
5. local hrefs extracted from every `<footer>` have exact set equality with all nine
   `allSitePaths`, including Privacy and the current page's self-link;
6. sitemap and `marketing/README.md` contain the exact nine-route inventory; sitemap `<loc>` path
   count is nine and its normalized path set equals `allSitePaths` with neither missing nor extra
   routes. For README, extract the text between `## Routes` and the next `##` heading, then collect
   only list entries matching ``^- `([^`]+)` ``; the resulting ordered array and set must equal
   `allSitePaths` exactly;
7. homepage hero is distinct from Why Riot and links prominently to `/why-riot/`;
8. Why Riot contains the exact H1, ordinary-life section, four human verbs, practice section,
   compact mechanism and boundary sections, Solnit attribution, and participation links;
9. the code-native illustration is present, accessible, local, and dependency-free;
10. Privacy begins with public-space truth, keeps app/device/metadata boundaries, puts website
    disclosure later, and links back to purpose and technical detail;
11. the four human-verb cards and Carry sub-list use the exact deterministic status markup and
    label/text pairings defined above;
12. every exact forbidden-claim pattern in the Site-Wide Claim Audit is absent across all nine
    editorial pages, `README.md`, and `docs/product/product-brief.md`;
13. changed pages include no remote runtime or asset dependency;
14. the exact static-content predicates above pass; external blank-target links include
    `rel="noopener"`;
15. the existing marketing contract suite remains green after these legacy assertions are retired:
    the old homepage headline `Community infrastructure that travels with people`; the old Why Riot
    audience labels `Depth one`, `Depth two`, and `Depth three`; and the old required phrases
    `Privacy through control, not secrecy`, `One update, different paths`, and
    `Direction being built or still unverified`. Their replacement assertions are criteria 7–12.
16. `README.md` and `docs/product/product-brief.md` label private encrypted groups
    **Direction, not shipped**, and replace “no server to raid/seize” absolutes with the bounded
    participant-copy, replaceable-gateway, and no-guarantee language defined above.
17. local HTTP/browser checks find no `Set-Cookie`, no stored browser cookie, and no resource request
    outside the loopback preview origin on all nine editorial routes.

The legacy-test migration replaces four complete regions in
`scripts/marketing/protocol-page-contracts.mjs`, rather than deleting individual assertions ad hoc:

1. Replace the entire block beginning
   `// ---------- reframed homepage (offline-guides design, 2026-07-20) ----------` through the
   `/guide/` homepage-link loop immediately before the `for (const name of ["apps", ...])` screenshot
   assertions. Keep the screenshot, builder, source, field-guide, and remaining homepage contracts.
2. Replace the entire `// --- Unified footer nav across all pages` block through its closing loop
   with footer-block extraction and exact nine-route equality, including self-links.
3. Replace the entire `// --- Site-wide top nav` block through its closing loop with extraction of
   `.sitenav-links` and exact `primaryNavPaths` ordered-array/set equality.
4. Replace the Why Riot portion of `// --- Guide pages: paired-story depths + honest boundaries`
   from that heading through `assert.doesNotMatch(whyRiot, /ecosystem/i, ...)`. Keep the following
   Using Riot guide assertions. The replacement is acceptance criteria 8–12 and the exact status
   element contract.

These region replacements are exhaustive for legacy exact-copy conflicts. Existing assertions
outside the four named regions remain unless the new finite security/route checks strictly subsume
them. The RED run occurs after the replacement assertions are written and before HTML changes.

Add `"test:marketing": "node scripts/marketing/protocol-page-contracts.mjs"` to `package.json` and
run it as a distinct blocking step in the existing CI web job after `npm run test:web:unit`.

Implementation verification also includes:

```sh
npm run test:web:unit
npm run test:marketing
```

Before Playwright review, verify `npx playwright --version`; if Chromium is unavailable, run
`npx playwright install chromium`. Do not install other browsers.

Then serve `marketing/public/` locally and visually review `/`, `/why-riot/`, and `/privacy/` at
1456×900 and 390×844. Verify navigation wrapping, hierarchy, contrast, illustration behavior, lack
of horizontal overflow, and that technical/status material remains subordinate. Record the six
screenshots under `/tmp/visual-review/riot-human-capacity/`. At 390 px, use Playwright evaluation to
require `document.documentElement.scrollWidth <= document.documentElement.clientWidth`. Capture a
forced-colors screenshot where Chromium supports the emulation; otherwise record the unsupported
check explicitly. Inspect computed foreground/background pairs with the existing palette and record
WCAG AA results in the committed
`docs/marketing/2026-07-22-human-capacity-implementation-review.md`. That report records each
screenshot path, SHA-256, viewport, overflow result, forced-colors support/outcome, inspected color
pairs and ratios, and any issue found. Screenshots remain reproducible `/tmp` artifacts rather than
large committed binaries; the committed report and exact capture commands preserve the evidence
needed to repeat them.

For each of the nine locally served routes, attach request and response listeners before navigation,
load the page, scroll through the complete document to trigger lazy resources, wait for network idle,
and capture every response header, request URL, and `performance.getEntriesByType("resource")` entry.
Require no
`Set-Cookie` response header, an empty Playwright browser-context cookie jar before and after the
visit, and no request origin other than the chosen loopback preview origin. Also retain the static
`document.cookie`, storage, beacon, and network-call predicates. These checks prove the built static
artifact's behavior. The copy deliberately does not promise that an independently configured edge
can never add headers or cookies; live post-deploy verification remains a separate deployment gate.
Write the ordered response-header and request-origin evidence into the committed implementation
report and include its SHA-256 alongside the screenshot evidence.

The six standard files are `home-desktop.png`, `home-mobile.png`, `why-riot-desktop.png`,
`why-riot-mobile.png`, `privacy-desktop.png`, and `privacy-mobile.png`. Forced-colors captures are
additional files named `why-riot-forced-colors.png` and `privacy-forced-colors.png` at 1456×900.
Contrast evaluation walks every visible text element and interactive control on all three routes at
both standard viewports, resolves its computed foreground against the nearest non-transparent flat
background, and records every unique pair. Required ratio is 4.5:1 for normal text and 3:1 for text
at least 24 CSS px or at least 18.66 CSS px and bold. Any unresolved background is recorded and
manually inspected rather than assumed to pass.

After visual verification, compute the SHA-256 of `marketing/public/why-riot/index.html`, record it in
the report, and run a first-read editorial gate with three fresh, mutually independent review
sessions. Each receives only that exact file plus one of the prompts below—not this specification,
prior review answers, or another reviewer's context. Assign one declared reader role per session:

- **Community participant/organizer**
- **Potential partner/institution**
- **Builder/technical reader**

Every reviewer answers these four shared questions:

1. What kind of ordinary community life is Riot trying to support?
2. What four kinds of work does Riot make easier?
3. Why might the same tools matter when conditions become difficult?
4. What is not currently guaranteed or private?

The partner also answers: **What can a community possess rather than merely access, and why does
that matter?** The builder also answers: **What bounded mechanism and current-status distinctions
make the claim plausible?**

Scoring is deterministic, one point per required element:

- Q1: names at least two ordinary-life examples and people/community—not software—as the subject.
- Q2: names Publish, Meet, Coordinate, and Carry or unmistakable equivalents.
- Q3: says familiar, already-used relationships/tools/data remain useful; does not claim guaranteed
  operation.
- Q4: names public plaintext/non-confidential content plus at least two of no anonymity guarantee,
  device/metadata exposure, incomplete transports, unaudited prototype, or no delivery/persistence
  guarantee.
- Partner Q5: identifies participant-held data/tools/community memory and reduced dependence on one
  provider, without claiming total independence.
- Builder Q5: identifies local replicas/signed records/multiple possible paths plus at least two
  distinct statuses from prototype, locally tested, in development, or not shipped.

Passing threshold: the community reviewer scores 4/4; partner and builder score 5/5; no reviewer
describes Riot primarily as a privacy messenger, disaster-survival product, or protocol project.
Commit each role, verbatim answer, element-by-element score, verdict, session identifier, and prompt
hash in `docs/marketing/2026-07-22-human-capacity-implementation-review.md`.

Use this exact shared prompt prefix, appending the role-specific line and assigned questions above:

```text
Read only the attached rendered Why Riot HTML. Do not inspect the repository, design documents,
prior reviews, or external sources. You are an independent first-time reader in the role: <ROLE>.
Answer each assigned question in plain language using only what the page communicates. Return JSON
with role, answers (keyed Q1–Q5 as applicable), and primary_impression. Do not score yourself.
```

The orchestrator, not the reviewer, applies the element rubric. A session is fresh when it has a new
tool thread/session identifier and receives no earlier reviewer output. The report includes the
complete rendered-file hash, exact prompt, its SHA-256, returned JSON verbatim, orchestrator scores,
and session identifier so another reviewer can repeat the procedure.

A fourth fresh editorial-auditor session receives these exact ordered public-mirror files and two
product documents plus this
exact
prompt, but not implementation commentary:

```text
marketing/public/index.html
marketing/public/why-riot/index.html
marketing/public/guide/index.html
marketing/public/about/index.html
marketing/public/privacy/index.html
marketing/public/open-source/index.html
marketing/public/community/index.html
marketing/public/releases/index.html
marketing/public/protocols/index.html
README.md
docs/product/product-brief.md
```

```text
Review only the attached nine rendered marketing HTML files and two product Markdown documents. Find present-tense or absolute claims
that mean any of: impossible to censor or shut down; always available; guaranteed delivery,
discovery, synchronization, persistence, or recovery; nothing can be lost; anonymous,
confidential, or private-by-default public spaces; audited, field-proven, production-ready, or
operating at scale. A bounded statement naming prerequisites or clearly labeled aspiration is not a
finding. Return JSON with verdict PASS or FAIL and findings containing route, exact excerpt, and
which category it violates.
```

Passing requires `PASS` with zero findings. The report stores the complete prompt and SHA-256,
ordered SHA-256 list for the eleven reviewed files, returned JSON verbatim, and fresh session
identifier. This is the reproducible human half of the site-wide claim audit.

Deployment is outside scope. Do not mutate production or claim the live site changed.

## Scope Boundaries

This work changes marketing HTML, its exact public mirrors, sitemap, marketing documentation,
contract tests, package scripts, and the existing CI web job. It does not change Riot protocols,
application behavior, cryptography, privacy guarantees, anchor behavior, sync transports,
deployment configuration, DNS, TLS, telemetry, or production state.

Documentation scope also includes the narrow private-group status clarification in `README.md` and
`docs/product/product-brief.md`, plus the committed implementation-review report.

## Review History

Revisions 1–3 proposed a new `/resilience/` route and a `/privacy/` compatibility alias. The design
gate approved that version before `/why-riot/` and `/guide/` were merged into the current site.
Comparison with the deployed site showed that the route would duplicate the existing canonical
product argument, worsen crowded mobile navigation, and leave site-wide claim conflicts untouched.

Revision 4 follows the approved comparison: reframe `/why-riot/`, keep `/privacy/` concise and
factual, add no route, clarify the homepage hero, make ordinary collective life visible, and audit
claims across the current nine-page site.

The first review of revision 4 approved its architecture and security direction, then requested
deterministic migration of legacy contract assertions, narrower status claims for Coordinate and
Carry, explicit plaintext/readable/copyable privacy language, a finite forbidden-claim pattern set,
static-content injection checks, and an observable first-read comprehension gate. Revision 5 adds
those contracts.

The second review approved the narrative and route architecture, then requested exact primary-nav
and sitemap set equality, deterministic no-tracking predicates, and reproducible audience-specific
reader and semantic-audit evidence. Revision 6 defines each one and names the committed review
artifact.

The third review approved architecture and UX, then found incomplete footer extraction, literal
status, remote-resource, reader-evidence, and private-group documentation contracts. Revision 7
defines each exact boundary and reconciles the aspirational docs with current implementation status.

The fourth review approved product and architecture but requested exact absence/inventory checks,
`<base>` and decoded data-SVG safety, ordered auditor inputs, and a status-density constraint.
Revision 8 adds them. Its request to replace the H1 was rejected because “People are the
infrastructure” is explicitly user-approved; the required adjacent ordinary-life thesis addresses
the stated concern without overriding the user's creative decision.

The fifth review approved product and UX, then requested `.sitenav-links` extraction, exhaustive
legacy assertion-region replacement, and rejection of active HTML data URLs and `srcdoc`. Revision 9
adds those last implementation contracts.

The sixth review approved architecture, UX, product, and implementation readiness. Security
requested that the seizure-resistance audit cover the two product documents and that resource URLs
use a strict scheme allowlist. Revision 10 adds both without changing product behavior.

The seventh security pass requested an HTTP/browser test for the no-cookie disclosure. Revision 11
limits the claim to Riot's static code, names Cloudflare's edge boundary, and adds local response,
cookie-jar, and request-origin verification.

The eighth security pass requested candidate-by-candidate `srcset` validation and durable request/
header evidence. Revision 12 adds both.

The ninth security pass requested browser-equivalent URL normalization and all-route HTTP coverage.
Revision 13 makes Chromium's resolved DOM and observed network behavior the authoritative backstop
and exercises every editorial route.

## Primary Sources

- Rebecca Solnit, *A Paradise Built in Hell*, publisher description:
  <https://www.penguinrandomhouse.com/books/301070/a-paradise-built-in-hell-by-rebecca-solnit/>
- Rebecca Solnit interview on disaster, community, and everyday civic confidence:
  <https://www.aarp.org/advocacy/the-author-speaks-disaster-strikes-people-shine-2010/>
- Riot product grounding: `README.md`, `docs/product/product-brief.md`,
  `docs/architecture/willow-architecture.md`, and the current marketing pages.
