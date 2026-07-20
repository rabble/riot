# Riot Marketing and Offline Guides Design

Date: 2026-07-20
Status: Approved by the design review gate

## Product decision

Ship two first-class Riot guides:

1. **Why Riot** — the product, political, privacy, and Willow explanation; and
2. **Using Riot** — practical, current-product instructions and recovery help.

Both guides are published on the existing marketing site at
`riot.divine.video` and bundled inside the iOS, macOS, and Android
applications. Every essential explanation and instruction must remain readable
with no internet connection.

The web routes are:

- `/why-riot/`
- `/guide/`

The app presents both documents through a **Help & Guides** surface available
before a person joins a community and from the ordinary in-community
experience.

The website is one presentation of the content, not its only home. The
canonical guide bundle is a release artifact in its own right.

## Core story

The central promise is:

> Riot lets a community's information, tools, and memory travel with its people,
> even when the internet or a central service is unavailable.

Most collaborative products bind their authoritative state to a company
account, server, relay network, or hosted database. Riot uses Willow so
community data can remain useful on participants' devices and can be carried
over different available paths.

The guide must distinguish three internet concepts:

- a **public web gateway** renders an exported view for ordinary web readers;
- an **internet seed or sync server** carries Willow updates between remote Riot
  devices; and
- a **nearby transport** carries Willow updates directly between local devices.

The current prototype supports local creation, local persistence, tested
Bonjour/local-network exchange, and public web rendering from exports. The
server-mediated Riot-device sync path is architectural direction with anchor
work in progress, not an integrated current product capability.

## Audiences and jobs

### Community members and organizers

The material should let this reader answer:

- What can my community keep doing without internet?
- How do we share information nearby?
- What can we publish on the public web today?
- Is Riot only chat, or can it carry tools and structured information?
- How do I create, join, switch, share, archive, or restore a community on my
  platform?
- What control and privacy does Riot actually provide?
- What should I do when a connection, permission, or import fails?

### Partners, funders, and journalists

The material should let this reader answer:

- How is Riot structurally different from a conventional platform?
- What does it mean for a community to possess its infrastructure?
- Why are replaceable web views and participant-held copies politically useful?
- What kinds of civic, mutual-aid, disaster, and media work does this enable?
- Which claims describe the current prototype and which describe direction?

### Builders and protocol readers

The material should let this reader answer:

- What does Willow supply?
- What does Riot add?
- Why can the same update move through different transports?
- Where do community authority, app isolation, and reconciliation live?
- What privacy properties exist, and which require optional or future protocols?
- What is the trust boundary of the current public web renderer?

## Editorial rules

1. **Lead with outcomes.** Say what a person can do before naming the mechanism.
2. **Use "Willow update" in public copy.** Do not introduce signatures,
   namespaces, entries, payload digests, or reconciliation in the main flow.
3. **Explain decentralization concretely.** Name who holds data and what a
   gateway, seed, or peer can and cannot control.
4. **Separate proof from direction where the claim first appears.** Do not rely
   on a status block at the end to correct an earlier impression.
5. **Describe authorship, not truth.** Do not use "verified update" when the
   evidence only establishes who authored or approved it.
6. **Write for offline use.** External links provide provenance and optional
   depth; they never contain instructions required to use the app.
7. **Document only current UI.** When platform behavior differs, name the
   platform. Never imply feature parity that the code does not provide.

## Guide 1: Why Riot

### One story at three depths

The guide uses one story told progressively for:

1. community members and organizers;
2. partners, funders, and journalists; and
3. builders and protocol readers.

A reader can stop after the depth they need or continue into the technical
explanation. A persistent audience label or strong visual transition identifies
each depth, including when a reader arrives through a jump link.

### Hero: "Community infrastructure that travels with people"

The opening contrast is:

> Most community software works only while its company, server, and internet
> connection remain available. Riot keeps the useful parts — publishing,
> coordination, shared tools, and community memory — on people's own devices.

The hero contains:

- the title;
- a two-sentence product explanation;
- a visible **Prototype** label;
- a compact before/after comparison; and
- jump links for **For communities**, **For partners**, and **For builders**.

The before/after comparison makes the requested difference explicit:

| Conventional platform | Riot's direction |
| --- | --- |
| The service holds the canonical community database | Participants carry community state |
| Offline means waiting for the service to return | Local reading and writing continue |
| A server outage cuts off the product | Nearby exchange can continue locally |
| Extensions belong to the platform | Communities approve and carry their own tools |
| A domain and hosted account anchor identity | Willow data carries community continuity |

Rows describing incomplete end-to-end field behavior carry a visible prototype
or direction label beside the row.

### For communities: "Your community should still work when the network doesn't"

This section uses examples of what Riot is designed to enable:

- a tenants union carrying alerts, meeting changes, rides, and checklists;
- disaster responders carrying shelter maps and supply requests into
  disconnected areas;
- a mutual-aid network coordinating needs, offers, schedules, and knowledge;
  and
- an independent publication retaining and re-sharing its existing material if
  a website is blocked or a gateway disappears.

The scenarios are explicitly examples, not claims that Riot has completed
field validation for each organization type.

It explains the capabilities in plain language:

- **Keep working locally.** Reading, writing, organizing, and preparing updates
  happen on the device.
- **Exchange without internet.** Nearby devices can connect, review offered
  updates, and choose whether to add them. Current evidence is prototype
  Bonjour/local-network testing; physical two-iPhone Bluetooth remains
  unverified.
- **Publish a public web view.** The current reference gateway can render
  exported public community content. It is not yet a live sync server for Riot
  devices.
- **Carry more than messages.** A community can carry structured information
  and community-approved tools.
- **Manage several communities.** Current native code supports creation,
  following, switching, archiving, restoring, and reopening; the Using Riot
  guide names which UI is present on each platform.
- **Keep human control.** External updates are previewed before acceptance, and
  unapproved community tools do not execute.

The anchor line is:

> Riot turns every participating device into part of the community's library,
> newsroom, toolbox, and distribution network.

### For partners: "Infrastructure communities can possess, not merely access"

This section explains:

- the community's Willow data carries its own continuity rather than deriving
  it from a domain name or hosted account;
- participant devices provide shared custody of existing community information;
- public gateways are replaceable views over exported public data;
- communities choose their tools and editorial practices;
- public Newswires can combine open publishing with transparent editorial
  actions rather than invisible ranking; and
- ordinary community infrastructure can remain locally useful during outages,
  shutdowns, or censorship.

The opportunity is framed through:

- community media without one canonical publishing server;
- local coordination that remains useful when infrastructure fails;
- community-specific software ecosystems;
- future continuity between field exchange and remote internet sync; and
- shared foundations for mutual aid, tenant organizing, clinics, cooperatives,
  disaster response, protests, and independent media.

Cryptographic mechanisms remain out of this main section. It does not claim
production readiness, completed security audits, guaranteed availability,
completed remote-device sync, or confidentiality for the public Newswire.

### Visual: "One update, different paths"

The page includes one accessible, site-native HTML/CSS illustration:

```text
You post or update something
              |
Riot publishes a Willow update
              |
       +------+------------------+
       |                         |
 nearby/local exchange      internet seed
 working prototype         architecture in progress
       |                         |
       +-----------+-------------+
                   |
       Riot devices reconcile the update

Separate public window:
exported public content -> web gateway -> ordinary browser
```

Supporting copy:

> Willow does not tie an update to one network path. In the current prototype,
> nearby Riot devices can exchange updates over a local network, and exported
> public content can be rendered on the web. Remote Riot-device synchronization
> through an internet seed is the next transport path being built.

The visual must:

- remain understandable in document order without CSS;
- expose meaningful labels to assistive technology;
- keep current, tested, and planned labels visible without relying on color;
- never depict the current web gateway as a live Willow sync server; and
- never imply verified physical-phone Bluetooth or a production global network.

### For builders: "Willow separates shared data from the network carrying it"

The non-technical explanation is:

> Riot publishes your update into the community's Willow space. Willow is
> designed so the update is not permanently tied to one transport. Riot's
> current prototype carries updates locally; internet seed synchronization is
> the next path under development.

The key insight is:

> Offline and online do not need separate community databases. They can be
> different ways of carrying the same shared state.

A native `details` and `summary` disclosure, collapsed by default and usable
without JavaScript, provides **Under the hood** detail.

Willow provides:

- independent namespaces;
- subspaces, paths, timestamps, and arbitrary payloads;
- deterministic joins for partial replicas;
- a data model independent of any one network transport; and
- optional Meadowcap capability and Willow synchronization protocols.

Riot adds:

- community and profile semantics;
- community relationships and per-community data identities;
- public Newswire records and editorial actions;
- community-approved, content-addressed mini-apps;
- app-scoped bridge access and hardened native execution;
- preview, validation, and acceptance policy;
- durable multi-community management;
- product transport integrations and public export rendering; and
- native interfaces for people who should not need to understand Willow.

The guide states that Willow itself does not define Riot profiles, communities,
Newswires, moderation, mini-apps, gateways, or native sandbox behavior.

### Privacy: "Privacy through control, not secrecy"

The public Newswire is intentionally semi-public and plaintext. Alerts, mutual
aid requests, public reporting, and community publications are often meant to
circulate. The page must not imply that widely shared information is secret.

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

#### Willow possibility versus current Riot behavior

The page visually separates upstream protocol possibilities from deployed Riot
behavior:

| Willow can express or support | Current Riot public communities |
| --- | --- |
| End-to-end encryptable data | Plaintext by design |
| Meadowcap scoped read/write capabilities | Write authority is checked; no confidential public read boundary |
| Confidential interest-overlap sync | Not the current public sync path |
| Logical destructive editing | No recall or secure erasure of copies already received |

Meadowcap wording must say:

- write authority travels with an entry and can be checked;
- read authority is policy enforced by conforming replication peers;
- read capabilities do not encrypt stored data by themselves;
- a malicious or authorized recipient can retain or re-share copied plaintext;
  and
- Riot's public Newswire does not use Meadowcap as a confidentiality boundary.

#### What Riot does not hide

- public Newswire content;
- IP addresses or ordinary metadata visible to an internet service used;
- nearby radio presence, device labels, proximity, and timing;
- copies already carried away by another device, export, backup, or log; or
- identity correlations caused by reused names, writing style, behavior,
  timing, proximity, or network metadata.

Separate per-community keys provide pseudonym separation at the data layer, not
an anonymity guarantee.

Community mini-apps receive app-scoped bridge access and run behind strong
native network restrictions. The guide must not describe their data as secret
or claim absolute zero egress; the current Apple host documents a residual
WebRTC hardening boundary.

#### Public web-reader trust boundary

Riot clients can independently validate canonical community data. The current
public gateway renders a proof-free export for an ordinary browser. A browser
reader therefore trusts the selected gateway's presentation and may receive a
view that is censored, stale, incomplete, fabricated, or incorrectly rendered.

A gateway may also log connection metadata. "Replaceable" and "not the
protocol authority" must not be translated into "cannot observe readers" or
"cannot mislead a browser visitor."

### "Working in the prototype / Direction being built"

The closing status block uses explicit text headings, not badge color alone.

**Working in the prototype:**

- durable core support for creating, following, switching, archiving,
  restoring, and reopening multiple communities;
- native/iOS community chooser and link/QR flows;
- local Newswire creation and durable display;
- human confirmation and preview-before-accept import in nearby flows;
- tested local-network/Bonjour peer exchange;
- public gateway rendering from exported community data;
- community-approved mini-app packages;
- app-scoped bridge access and hardened app execution; and
- fresh per-community author identities when joining another community.

**Direction being built or still unverified:**

- encrypted private groups;
- confidentiality for public communities;
- full deletion from devices that already copied public content;
- integrated remote Riot-device synchronization through an internet
  seed/anchor data path;
- production scale, audited security, or guaranteed availability;
- full interoperability with every Willow draft and transport; and
- physical two-iPhone Bluetooth exchange.

Each material claim earlier in the page carries the same scope label; this
closing block is a summary, not a correction.

### Audience next steps

- **Communities:** open Using Riot, view the current prototype evidence, or
  follow development.
- **Partners:** read the status/evidence section and project contact path.
- **Builders:** open the Protocol field guide or source repository.

No call to action requires connectivity for understanding the guide itself.

## Guide 2: Using Riot

### Purpose

Using Riot is a task-oriented field manual for the current app. It does not
teach future architecture or repeat the full product argument.

Every instruction is tested against the visible UI of each platform. If a flow
is not exposed on a platform, the guide says so and points to the available
alternative. Shared core support is not presented as user-facing parity.

### Information architecture

1. **Start here**
   - What Riot keeps on this device
   - Prototype status
   - What works without internet
   - Public-content warning
2. **Create or join a community**
   - Create a community
   - Join with a link
   - Scan a QR code where the platform supports it
   - Join from a nearby device
   - Review the community identifier before joining
3. **Manage your communities**
   - Open the chooser
   - Switch communities
   - Archive and restore where surfaced
   - Understand organizer, member, follower, and public-reader labels
4. **Post and read updates**
   - Create and review a post
   - Understand local success and pending exchange
   - Recognize display names as self-claimed names plus key-derived tags
5. **Exchange nearby**
   - Turn on required Bluetooth/local-network permissions
   - Find a device
   - Accept or decline a connection
   - Confirm joining a different community
   - Preview, add, or reject offered updates
   - Stop discovery
6. **Share a community**
   - Share the canonical link
   - Display or scan a QR code where supported
   - Explain that anyone receiving a public reference can pass it onward
7. **Use community tools**
   - Understand carried, approved, incomplete, and unavailable tools
   - Review what a tool can access
   - Organizer approval
   - Tool data and network boundaries
8. **Privacy and safety**
   - Public content is plaintext
   - Pseudonymity is not anonymity
   - Nearby and gateway metadata
   - No guaranteed recall or universal deletion
   - Private encrypted groups are not available
9. **Troubleshooting**
   - No community yet
   - Community details have not arrived
   - Nearby permission denied
   - No nearby device appears
   - Peer is in a different community
   - Updates are offered but not yet accepted
   - Share link unavailable before first sync
   - Tool incomplete, unapproved, or revoked
   - Local profile recovery/quarantine guidance exposed by the app
10. **Platform notes**
    - iOS
    - macOS
    - Android
11. **What is not available yet**
    - remote seed/server sync
    - encrypted private groups
    - production guarantees
    - any other gap named by the current build

### Instruction format

The opening contains a linked table of contents. Every task and troubleshooting
section ends with **Back to contents**, so a person working under pressure never
has to scroll through the full manual to choose another task.

Each task uses:

- a plain-language goal;
- numbered actions matching visible labels exactly;
- a **Works offline** or **Needs a connection/permission** note;
- the expected result;
- one concise recovery path; and
- a platform label when behavior differs.

Screenshots are optional supplements. No instruction depends on recognizing an
image, and screenshots must never show a feature state unavailable in the
corresponding shipped build.

### Offline behavior

The complete guide, styles, navigation, diagrams, and status notes live in the
application bundle. It does not fetch help articles, fonts, analytics,
screenshots, or configuration.

External citations and project links are visibly marked **Opens in browser**.
Only an explicit main-frame link activation with a user gesture may hand a
manifest-declared HTTP(S) destination to the system browser. Redirects, meta
refreshes, subframes, downloads, new windows, and programmatic navigation never
launch the browser. If no connection exists, the bundled guide remains in place
and complete; failing to open the browser never replaces or blanks the guide.

The rendered guide shows the tested app version and checked date. The public
web copy tells people using an older installed build to prefer the bundled
Using Riot guide that shipped with their app, because its labels match that
version.

## Canonical guide bundle

### Source of truth

The canonical, dependency-free bundle lives under:

```text
guides/
  manifest.json
  why-riot/
    index.html
  guide/
    index.html
  assets/
    guide.css
```

The documents use semantic HTML and shared local CSS. They use system font
stacks rather than duplicating the marketing homepage's large inline font
payload. The visual language still reuses Riot's paper, ink, pink, blue, hard
rules, and stamped labels.

`manifest.json` contains:

- schema version;
- stable guide IDs;
- titles;
- entry-point paths;
- the tested app version or version range;
- checked dates;
- an exact per-file allowlist containing a normalized relative path, SHA-256
  digest, and MIME type for both HTML entry points and every local asset;
- an exact per-document allowlist of external HTTP(S) destinations; and
- minimum reader schema version.

`manifest.json` is not included in its own content map, avoiding a circular
digest. Every target receives the manifest itself byte-for-byte. Paths use
forward slashes, contain no empty, dot, dot-dot, query, fragment, encoded
separator, absolute, or backslash component, and are compared after one
specified canonicalization pass.

### Deterministic distribution

A checked-in Node script copies the canonical bundle without transformation to:

```text
marketing/why-riot/
marketing/guide/
marketing/assets/guide.css
marketing/public/why-riot/
marketing/public/guide/
marketing/public/assets/guide.css
apps/ios/Riot/Resources/Guides/
apps/android/app/src/main/assets/guides/
```

The web copies adjust no content bytes. The canonical pages therefore use
relative paths that work in all destinations. The deployment layout preserves
the relative guide and asset paths.

Apple uses the single copied bundle under
`apps/ios/Riot/Resources/Guides/`. Both `apps/ios/Riot.xcodeproj` and
`apps/macos/Riot.xcodeproj` register that directory in their independent
resource build phases. macOS resource presence is tested separately; iOS
success is not treated as macOS evidence.

The sync command is idempotent. A `--check` mode exits nonzero on missing,
changed, additional, or stale target files. Hand-edited distribution copies are
rejected. Generation stages and validates a complete target bundle before an
atomic directory replacement, so interruption cannot leave packaging inputs
partially updated.

## Native app integration

### Help & Guides entry points

Both guide cards are reachable:

- from the first-run/no-community screen;
- from the community chooser;
- from the active-community shell through **Help & Guides**;
- from community settings as a recovery route; and
- on macOS, from the standard Help menu as well as the shared app surface.

The entry points use the same labels on all platforms:

- **Why Riot**
- **Using Riot**

Opening a guide preserves the person's current community and composer draft.
Back or Close returns to the exact prior surface.

The implementation plan defines the native presentation pattern per platform
(push, sheet, activity, or window) and the exact Back/Close and keyboard
behavior. The state contract is:

| State | Required behavior |
| --- | --- |
| Active community | Unchanged while guides open and after return |
| Composer draft | Preserved byte-for-byte |
| Guide and section | Preserved across system-browser handoff and foregrounding |
| Scroll position | Preserved per guide during cross-guide navigation |
| App process restoration | Reopens safely to the prior Riot surface or the same bundled guide; never loses a draft |

An invalid or unsupported local guide path shows: **This guide page isn't
available in this copy of Riot.** It offers **Back to Help & Guides** and
**Close guide**; it never exposes a raw URL, filesystem path, or WebView error.

### Apple reader

iOS and macOS share a dedicated documentation reader implemented in the shared
Swift source set. It is not `AppRuntimeView` and receives:

- no Riot JavaScript bridge;
- no community data session;
- no persistent website data store;
- no permission to load arbitrary local files; and
- no automatic network access.

The reader:

- disables JavaScript;
- registers a dedicated manifest-backed local guide scheme before loading;
- resolves only an exact manifest-declared path whose bytes, SHA-256 digest, and
  MIME type match;
- applies a block-all network content rule before the document loads;
- allows cross-guide navigation only through explicit relative `index.html`
  paths declared in the manifest;
- sends an external destination to the system browser only for a main-frame
  `.linkActivated` action with a user gesture and an exact allowlisted HTTP(S)
  URL; and
- refuses redirects, meta refreshes, subframes, downloads, new windows,
  programmatic navigation, undeclared local files, and every other scheme.

The resolver does not grant directory-wide file access. It rejects absolute
paths, encoded dot segments or separators, backslashes, queries used as path
confusion, traversal, symlinks escaping the guide root, digest mismatch, MIME
mismatch, and initialization without the network block in place.

The iOS and macOS project resource phases are updated separately and verified
by inspecting the built `.app` bundles.

### Android reader

Android uses a dedicated documentation reader, separate from
`AppWebViewHost` and without `RiotJsBridge`.

It:

- reads through a dedicated manifest-backed `WebViewAssetLoader.PathHandler` on
  the constrained `appassets.androidplatform.net` origin;
- disables JavaScript, DOM storage, mixed content, file-URL universal access,
  arbitrary content/file access, downloads, service workers, geolocation,
  media capture, multiple windows, and network-dependent Safe Browsing calls;
- resolves only exact manifest-declared local paths whose digest and MIME type
  match, treating that one constrained HTTPS origin as local;
- permits cross-guide navigation only through explicit relative `index.html`
  paths;
- sends an allowlisted external HTTP(S) destination to the system browser only
  when `isForMainFrame` and `hasGesture` prove explicit link activation;
- rejects redirects, meta refreshes, subframes, downloads, intents, custom
  schemes, alternate origins, undeclared paths, encoded traversal, digest/MIME
  mismatch, and automatic remote subresources; and
- restores the prior Riot screen on Back.

The packaged APK is inspected to prove both guide documents, the manifest, and
the stylesheet are present.

## Public web integration

The marketing homepage gains:

- a primary **Why Riot** navigation link;
- a prominent **Why Riot is different** callout;
- a **Using Riot** link; and
- footer links to both guides.

The existing `/protocols/` route remains the deep protocol comparison. Why Riot
links to it from the builder section rather than reproducing its matrix.

Responsive navigation must keep both **Why Riot** and **Protocols** reachable
on small screens. The current rule that keeps only `.protocol-nav` visible must
be intentionally revised and covered by tests.

Both long pages provide a visible skip link to the main article. Guide jump
links, native `details` disclosures, and heading order work without JavaScript.

## Web and embedded-document security

Both canonical documents require:

- no user-derived HTML;
- no inline or external JavaScript;
- no remote fonts, images, styles, analytics, or other subresources;
- the exact starting CSP `default-src 'none'; script-src 'none'; style-src
  'self'; img-src 'none'; connect-src 'none'; object-src 'none'; frame-src
  'none'; base-uri 'none'; form-action 'none'`;
- a `no-referrer` policy;
- explicit external-link labels;
- `noopener noreferrer` when a web link opens a new context; and
- no dependency on service workers or cached network content.

The deployed web response headers are verified in addition to the document
meta policy and add `frame-ancestors 'none'`. Before a person deliberately
follows an external link, the exact allowed request set is the selected
top-level guide document plus manifest-declared same-origin guide assets such as
`assets/guide.css`. Cross-origin, redirected, remote, analytic, scripted, and
undeclared requests are forbidden.

## Willow source alignment

The explanation is grounded in Willow's primary materials:

- the Willow homepage describes independent digital spaces stored on users'
  hardware, explicit receipt, offline operation, and multiple transport paths;
- the Data Model describes payloads addressed by paths, timestamps, subspaces,
  and namespaces, with deterministic store joins;
- Meadowcap describes fine-grained, delegable read/write authority;
- Confidential Sync describes private interest overlap and partial
  synchronization, but Riot's public Newswire must not inherit its
  confidentiality claims; and
- Drop Format demonstrates asynchronous movement through improvised channels,
  but Riot must not claim interoperable Drop Format support until its
  conformance bar is met.

Primary sources:

- <https://willowprotocol.org/>
- <https://willowprotocol.org/specs/data-model/>
- <https://willowprotocol.org/specs/meadowcap/>
- <https://willowprotocol.org/specs/confidential-sync/>
- <https://willowprotocol.org/specs/drop-format/>

The guide includes a visible checked date. These URLs are optional references
in the offline bundle, not required reading.

## TDD and verification

Implementation follows red-green-refactor. Before reader or navigation code is
written, failing tests establish:

1. canonical-to-target byte and manifest-hash equality;
2. stale, missing, extra, and modified distribution-copy rejection;
3. both Apple projects register and package the guide resources;
4. the Android asset set and packaged APK contain the exact guide revision;
5. every app state required by this design exposes Help & Guides;
6. opening and closing a guide preserves the prior app state;
7. local guide and cross-guide navigation works without connectivity;
8. JavaScript and Riot bridges are absent from documentation readers;
9. only manifest-declared local paths with matching SHA-256 and MIME type load;
10. undeclared files, modified bytes, wrong MIME types, absolute paths, encoded
    traversal, escaping symlinks, and alternate local origins fail closed;
11. the document plus declared local stylesheet load while automatic remote,
    redirected, WebSocket, and undeclared subresource requests are blocked;
12. only allowlisted main-frame links with a user gesture leave through the
    system browser;
13. redirects, meta refresh, iframes, `target=_blank`, downloads, `data:`,
    `javascript:`, `file:`, `intent:`, and custom schemes never launch an
    external application;
14. external-link failure and app backgrounding preserve the exact guide and
    scroll position;
15. malformed or unknown guide paths fail closed with the approved local
    recovery view;
16. semantic headings, skip links, accessible names, dynamic text, keyboard
    focus, and reduced motion pass focused checks; and
17. current/planned and privacy boundary copy remains present in every target.

The implementation plan must name the exact first failing test for each work
unit before production code.

### Automated verification contract

Implementation is complete only when:

1. the guide sync script passes in `--check` mode;
2. source, marketing, deployment, Apple, and Android guide bytes match the
   manifest;
3. existing marketing protocol-page contracts remain green;
4. new guide structural/security contracts pass;
5. `/why-riot/` contains the three approved audience depths;
6. `/guide/` contains every approved task and platform/status boundary;
7. the two-path visual distinguishes nearby exchange, internet seed sync, and
   public web rendering;
8. public Newswire plaintext, gateway browser trust, pseudonym correlation,
   cooperative read control, and non-recall boundaries are explicit;
9. current and planned capabilities are labeled where first mentioned;
10. the web request set before deliberate external navigation is exactly the
    top-level document plus manifest-declared same-origin assets;
11. deployed CSP and referrer headers match the contract;
12. phone and desktop screenshots show no clipping, overlap, or page-level
    horizontal overflow at 320 CSS pixels and target viewports;
13. iOS tests and an iOS build pass;
14. macOS tests and a macOS build pass;
15. Android JVM tests, relevant instrumented tests, lint, and an APK build pass;
16. built `.app` and APK artifacts contain the exact guide bundle; and
17. repository formatting, linting, tests, and coverage floors remain green.

### No-network rehearsal

Before distribution, run the installed iOS, macOS, and Android builds with
network connectivity disabled:

- open both guides before joining a community;
- open both guides from the chooser and an active community;
- navigate between every local section;
- verify essential text, the visual, and troubleshooting content;
- return without losing current state; and
- confirm no blank, spinner, failed-resource, or network-dependent surface.

### Audience comprehension gate

Before public deployment, conduct a lightweight moderated or questionnaire
review with at least six people: two community/organizer readers, two
partner/journalist readers, and two builder/protocol readers.

After reading the relevant depth, at least five of six must correctly explain:

- that a Willow update is not permanently tied to one transport;
- which nearby, web-rendering, and server-sync behaviors are current or planned;
- that the public Newswire is plaintext and pseudonymity is not anonymity;
- that public copies cannot be guaranteed to disappear; and
- that Riot is a prototype without verified physical two-iPhone Bluetooth.

Both participants in every audience group must reject the high-risk false
claims below; the overall threshold cannot conceal a failed audience section.
No participant may leave with the high-risk belief that current Riot public
communities are end-to-end encrypted, anonymous, remotely recallable, fully
production-ready, or already synchronized globally through the web gateway. A
failure requires copy revision and a repeated review before deployment.

### Practical guide usability gate

Before release, test current installed builds with at least six representative
participants, covering iOS, macOS, and Android with at least two people on each
platform. Connectivity is disabled throughout the app tasks.

The test records completion, time on task, assistance, copy/UI mismatch, and
recovery outcome while participants:

1. find **Help & Guides** from the first-run/no-community state without a hint;
2. identify whether a named task works offline;
3. complete the platform's exposed create-or-join flow;
4. share or exchange a community through a route supported on that platform;
5. explain and exercise preview-before-accept using the prepared peer fixture;
6. recover from one documented permission or connection failure; and
7. return to Riot without losing the active community, draft, guide section, or
   scroll position.

Release thresholds are:

- 100% find Help & Guides within 60 seconds without assistance;
- at least 85% unassisted task completion overall;
- no platform has more than one failed supported task across its two
  participants;
- zero stale, nonexistent, or incorrectly platform-labeled UI instructions;
- zero critical privacy misunderstandings; and
- zero state-loss or network-dependency failures.

Any missed threshold requires guide or product-copy revision and a complete
retest of the affected platform. Results are retained with the packaged-artifact
inspection, no-network rehearsal, and audience-comprehension evidence.

## Declared implementation scope

Expected scope includes:

- `guides/**`
- `marketing/index.html`
- `marketing/README.md`
- marketing deployment/header configuration, including a generated `_headers`
  file if that is the supported Workers Assets mechanism
- generated `marketing/why-riot/**`, `marketing/guide/**`, and public mirrors
- `scripts/guides/**`
- focused marketing contract scripts
- shared Apple guide reader, navigation, tests, and both Xcode project files
- Apple bundled guide resources
- Android guide reader, navigation, assets configuration, tests, and manifest
  changes if required
- build/rehearsal documentation

The implementation plan must refine this to an exact file list per work unit.
No work unit may modify files outside its declared scope without returning
through plan review.

## Out of scope

- Implementing remote seed/server data synchronization.
- Implementing encrypted private groups.
- Changing Willow protocol behavior.
- Adding analytics, a CMS, a service worker, or remote runtime assets.
- Replacing the existing protocol comparison.
- Claiming audited security, production readiness, guaranteed availability,
  anonymity, universal deletion, or completed physical-phone Bluetooth.
- Reusing the community mini-app runtime or its JavaScript bridge for docs.
- Deploying before design, implementation, security, packaging, comprehension,
  and no-network gates pass.
