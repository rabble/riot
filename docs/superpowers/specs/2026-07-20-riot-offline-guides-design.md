# Riot Marketing and Offline Guides Design

Date: 2026-07-20
Status: Design review round 3 pending

## Product decision

Ship two first-class Riot guides:

1. **Why Riot** — the product, political, privacy, and Willow explanation; and
2. **Using Riot** — practical, current-product instructions and recovery help.

Both guides are published on the existing marketing site at
`riot.divine.video` and bundled inside the iOS, macOS, and Android
applications. Every essential explanation and instruction must remain readable
with no internet connection.

The web routes are:

- `/` — the new paired-story marketing homepage;
- `/why-riot/`
- `/guide/`
- `/protocols/` — the existing deep protocol field guide.

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
8. **Pair concepts with evidence.** Official Willow illustrations explain
   protocol concepts; real Riot screenshots show current product behavior.
   Captions must state that boundary wherever the pairing could otherwise imply
   that Riot ships every possibility shown by Willow.

## Guide 1: Why Riot

### One story at three depths

The guide uses one story told progressively for:

1. community members and organizers;
2. partners, funders, and journalists; and
3. builders and protocol readers.

A reader can stop after the depth they need or continue into the technical
explanation. A persistent audience label or strong visual transition identifies
each depth, including when a reader arrives through a jump link.

### Paired visual story

The guide uses the user-approved **Paired story** direction. Each audience depth
pairs current Riot interface evidence with one official Willow illustration
that explains the underlying idea:

| Audience depth | Riot evidence | Willow concept |
| --- | --- | --- |
| Communities | current iOS **Home** | Drop Format's ad-hoc transport chain |
| Partners | current iOS **Tools** with **Checklist** | independent community namespaces |
| Builders | current iOS **Post an update** | paths, timestamps, and subspaces |

The Willow art is explanatory evidence about the protocol, not a screenshot of
Riot behavior. The community caption says that Willow data can be transported
through improvised channels, while Riot's current tested path is
Bonjour/local-network exchange; it does not claim that Riot currently imports
Drop Format files from USB, email, messaging apps, or every channel shown. The
caption visibly labels Drop Format **Proposal**.

The partner pairing explains that separate Riot communities map to independent
Willow namespaces. Riot mini-apps do not each receive an independent namespace:
their current isolation comes from app-specific paths such as
`apps/<app_id>/...` inside the selected community plus app-scoped native bridge
policy. The pairing does not imply per-app namespaces, confidentiality,
cryptographic isolation, or automatic end-to-end encryption. The builder
pairing explains how multiple authors contribute to a shared data model without
requiring readers to learn signatures in the public copy.

All pairings:

- use the full, unaltered Willow illustration rather than cropping,
  recoloring, tracing, or AI-modifying it;
- show a real Riot screen produced from a deterministic synthetic profile and
  captured from the exact recorded iOS prototype build;
- visibly label the screenshot platform, app version, and prototype status;
- include a concise visible caption and the original upstream alternative text,
  preserved in both the rendered image and manifest;
- repeat every material fact in prose so the image is never the only source;
- present boundary prose and caption before the image pair in document order;
- render each image as its own `figure` and `figcaption`;
- place transparent Willow art on an opaque, high-contrast paper panel in every
  theme without changing the upstream image bytes;
- stack the two figures at narrow widths rather than shrinking them into an
  unreadable row;
- remain legible on a 320 CSS-pixel viewport and at desktop width; and
- remain available byte-for-byte in every offline application bundle.

Rabble explicitly confirmed on 2026-07-11 and again on 2026-07-20 that the
official Willow illustrations are available under the same
`MIT OR Apache-2.0` terms as Willow's code. That confirmation is the project
decision to use the artwork. Before asset bytes enter a distributable guide,
the canonical `docs/assets/willow/LICENSE-EVIDENCE.md` must make the basis
auditable: copyright holder or authorized licensor, their authority over the
exact artwork, grant date, durable evidence, exact asset IDs and SHA-256
digests, license expression, required copyright and attribution text, NOTICE
obligations or an explicit statement that none apply, acquisition reviewer,
and license reviewer. If those fields cannot be recorded, distribution stops
before vendoring and the release returns through design review. This design has
no silent or pre-approved substitute artwork path: the paired story, captions,
manifests, tests, and acceptance criteria all require these exact three Willow
assets.

The exact initial Willow asset set is:

| Local ID | Dimensions | Protocol status | Official content-addressed source |
| --- | ---: | --- | --- |
| `drop-adhoc-transport-chain` | 898 × 1353 | Drop Format: Proposal | `https://willowprotocol.org/assets/dropformat/02718468ec241a3adc2175ddb3ff04d93e1d1f59deb0b2c840da5fd01fa80246.png` |
| `data-model-namespaces` | 1513 × 1134 | Data Model: Final | `https://willowprotocol.org/assets/data_model/7a2e8b02247a06101594b16f3994cf851f5a54be08548430a1c7e1eb125c23e9.png` |
| `data-model-subspaces` | 1499 × 1363 | Data Model: Final | `https://willowprotocol.org/assets/data_model/1aa5504899909482194d395cdcc0bfdb1cb51f9b09c7d834ca2f7fc538b4d751.png` |

For each asset, provenance records the official specification page, its
protocol maturity, deployed content-addressed URL, byte digest, dimensions,
verbatim alt text, the Willow website repository commit and source path where
available, and any generated-output lineage. The deployed content-addressed
bytes are the release artifact; the design does not falsely claim that a raw
repository source image must be byte-identical after Willow's site build.

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

### Status visual: "One update, different paths"

In addition to the paired Willow and Riot visuals, the page includes one
accessible, site-native HTML/CSS status illustration:

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
- **Partners:** read the status/evidence section or follow the public build.
- **Builders:** open the Protocol field guide or source repository.

The initial release links **source repository** to
`https://github.com/rabble/riot` and **build status** to
`https://github.com/rabble/riot/actions`. It does not promise a partner contact
path because no reviewed destination has been supplied. Adding one requires an
explicit destination and copy review.

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

Screenshots are required supplements for four orientation surfaces per
platform. The exact initial coverage matrix is:

| Platform | Required current surfaces |
| --- | --- |
| iOS | **Home**, **Tools** with **Checklist**, **Post an update**, **Nearby** |
| macOS | **Home**, **Tools** with **Checklist**, **Post an update**, **Nearby** |
| Android | **Spaces**, **App directory** with **Checklist**, **Compose & sign**, **Connection** |

The guide groups images inside the matching platform section. It never places
an iOS or macOS screenshot beside Android-only labels, or the reverse. When a
surface is unavailable in a shipped platform build, that platform's guide
section states the absence and never substitutes another platform, but the
initial twelve-capture release contract remains unmet and the change must
return through design review.

No instruction depends on recognizing an image, and screenshots must never show
a feature state unavailable in the corresponding shipped build. Each canonical
capture record contains:

- the full 40-character source commit, which must exist and be an ancestor of
  the release commit;
- app semantic version and build number;
- platform, OS version, physical or simulator/emulator model, pixel dimensions,
  and scale;
- deterministic capture command and fixture ID;
- UTC capture time, capture reviewer, and privacy reviewer;
- the exact relevant UI source-path set;
- original-capture SHA-256;
- bundled-derivative SHA-256 and deterministic derivation tool/version; and
- a content-safety decision.

Release validation fails if any relevant UI source path changed after the
capture commit, the capture commit is not an ancestor, metadata is incomplete,
or the guide's named labels differ from the current build. Failure requires a
fresh capture and review.

Every capture uses an isolated deterministic demo profile. It contains no real
participant or community data, device or host label, location, address,
notification, ticket, secret, reusable QR or join reference, account, or
network identifier. A human privacy review checks the visible pixels.
Riot-owned screenshots are deterministically re-encoded to remove EXIF, GPS,
text/comment, timestamp, and other non-pixel metadata before bundling. The four
existing generic marketing screenshots predate the current Apple navigation
and are reference material only; they are not eligible for the new guides.

Original captures, the typed capture manifest, capture logs, and privacy-review
decisions are retained under `docs/evidence/guides/screenshots/`. Only the
deterministic display derivatives enter `guides/assets/riot/` and the
applications.

### Offline behavior

The complete guide, styles, navigation, diagrams, screenshots, license notices,
and status notes live in the application bundle. It does not fetch help
articles, fonts, analytics, images, or configuration.

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
  notices/
    index.html
  assets/
    guide.css
    riot/
      ios/
        home.png
        tools-checklist.png
        post-update.png
        nearby.png
      macos/
        home.png
        tools-checklist.png
        post-update.png
        nearby.png
      android/
        spaces.png
        app-directory-checklist.png
        compose-sign.png
        connection.png
    willow/
      drop-adhoc-transport-chain.png
      data-model-namespaces.png
      data-model-subspaces.png
      LICENSE-EVIDENCE.md
      LICENSE-MIT
      LICENSE-APACHE
    licenses/
      Bricolage-Grotesque-OFL-1.1.txt
      Inter-OFL-1.1.txt
```

The documents use semantic HTML and shared local CSS. They use system font
stacks rather than duplicating the marketing homepage's large inline font
payload. The visual language still reuses Riot's paper, ink, pink, blue, hard
rules, and stamped labels.

`docs/assets/willow/` is the sole provenance and license authority for official
Willow art, following the approved Willow Visual Documentation System design.
The guide bundle imports a validated three-asset subset and the canonical
license files from that catalog without transformation. It does not create a
second acquisition manifest. If the shared catalog has not yet been
implemented, it is an explicit prerequisite work unit for this feature.

The guide manifest references each Willow asset by the canonical catalog ID and
catalog digest, then records only its local release path and bundle digest.
Validation requires equality with the shared catalog for source URL, SHA-256,
dimensions, verbatim alt text, attribution, license evidence, protocol ID,
protocol maturity, and bytes. A field or byte mismatch fails the guide build.

`manifest.json` contains:

- schema version;
- stable guide IDs and the notices support-page ID;
- titles;
- entry-point paths;
- the tested app version or version range;
- checked dates;
- an exact per-file allowlist containing a normalized relative path, SHA-256
  digest, and MIME type for both HTML entry points and every local asset;
- the complete typed capture record for each Riot screenshot;
- the shared Willow catalog digest and, for each imported Willow image, its
  canonical catalog ID, local path, and bundle digest;
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
marketing/guides-manifest.json
marketing/why-riot/
marketing/guide/
marketing/assets/guide.css
marketing/assets/riot/
marketing/assets/willow/
marketing/assets/licenses/
marketing/notices/
marketing/public/guides-manifest.json
marketing/public/why-riot/
marketing/public/guide/
marketing/public/assets/guide.css
marketing/public/assets/riot/
marketing/public/assets/willow/
marketing/public/assets/licenses/
marketing/public/notices/
apps/ios/Riot/Resources/Guides/
apps/android/app/src/main/assets/guides/
```

The web copies adjust no content bytes. The canonical pages therefore use
relative paths that work in all destinations. The deployment layout preserves
the relative guide and asset paths. Both web manifest destinations receive the
canonical `guides/manifest.json` byte-for-byte and are included in sync-check,
deployed-artifact, response-header, and mirror-drift verification.

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

### Image performance budget

Every bundled image is at most 1 MiB compressed and at most 1600 pixels on
either axis. An exact upstream Willow image may contain at most 2.1 megapixels;
a generated Riot display derivative may contain at most 1.2 megapixels. The
three Willow images already fit those bounds and remain unmodified.
Higher-resolution Riot captures are retained as review evidence outside the
guide bundle, while deterministic display derivatives fit the bound and retain
both original and derivative hashes in their capture record.

Each HTML document declares intrinsic image width and height. Images below the
first visible figure use native `loading="lazy"` and `decoding="async"`. The
eager image set is at most 3 megapixels; the total declared images referenced
by either guide are at most 15 megapixels and 6 MiB compressed. Bundle
validation rejects a per-file, eager-set, per-document, or complete-bundle
budget violation. Release rehearsal records cold offline open time on the
oldest supported device class; each guide must show its title and first
paragraph within one second and image decoding must add no more than 64 MiB to
the documentation reader's measured resident memory.

The benchmark matrix is fixed to an iPhone SE (2nd generation) on iOS 17, a
base 8 GiB Apple M1 Mac on macOS 14, and a 2 GiB API 26 Android emulator using
the repository's deterministic device profile. Each uses the release build,
airplane/no-network mode, a terminated app process, and five cold opens per
guide. Instrumentation records monotonic time from user activation until the
title and first paragraph are painted, plus reader-process resident memory
immediately before activation and peak resident memory during the next five
seconds. Every run, not merely the median, must meet both thresholds. The
implementation plan assigns a named owner to each benchmark and accessibility
rehearsal.

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

Both guide footers contain **Third-party notices**. The Help & Guides surface
also exposes **Third-party notices** as a subordinate legal-and-credits row, not
as a third first-class guide card. The notices page displays the local
attribution and license summary, identifies the exact Willow and marketing-font
asset IDs and their distribution scope, and renders the complete text of the
Willow MIT and Apache licenses plus both font-specific OFL files. Its license
sections are generated from the canonical license files into semantic `pre`
blocks; tests decode the HTML text and require byte equality with those files
so the visible notice cannot drift. The app copy labels the font entries
**Marketing website only** rather than implying the offline reader loads them.

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
| Notices return | Returns to the originating guide and exact section/scroll position, or to Help & Guides when opened there |
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
- allows guide and notices navigation only through explicit relative
  `index.html` paths declared in the manifest;
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
- permits guide and notices navigation only through explicit relative
  `index.html` paths declared in the manifest;
- sends an allowlisted external HTTP(S) destination to the system browser only
  when `isForMainFrame` and `hasGesture` prove explicit link activation;
- rejects redirects, meta refreshes, subframes, downloads, intents, custom
  schemes, alternate origins, undeclared paths, encoded traversal, digest/MIME
  mismatch, and automatic remote subresources; and
- restores the prior Riot screen on Back.

The packaged APK is inspected to prove both guide documents, the notices page,
the manifest, the stylesheet, every declared image, and both Willow license
texts are present.

## New marketing site

The existing dependency-free Workers Assets site is rebuilt around the same
paired story rather than merely gaining two navigation links. The new `/`
homepage is the concise public entry point; `/why-riot/` carries the complete
argument, `/guide/` carries the practical manual, and `/protocols/` remains the
deep comparison.

The homepage order is:

1. **Community infrastructure that travels with people** — outcome-led hero,
   visible **Prototype** label, and calls to **Why Riot** and **Using Riot**.
2. **For communities** — current iOS Home evidence paired with Willow's
   proposal-status ad-hoc transport chain and the accurate Riot boundary.
3. **For partners** — current iOS Tools/Checklist evidence paired with community
   namespaces, including the app-path isolation clarification.
4. **For builders** — current iOS Post an update evidence paired with the Data
   Model subspaces figure and a link to `/protocols/`.
5. **One update, different paths** — the site-native current-versus-planned
   status diagram.
6. **Privacy through control, not secrecy** — public plaintext, metadata,
   gateway trust, non-recall, and optional/future encryption boundaries.
7. **Working now / direction being built** — the same prototype status contract
   as Why Riot.
8. **Learn and use Riot** — links to both complete guides, Third-party notices,
   source, and build status.

The homepage uses the same generated Riot derivatives and the same validated
Willow catalog subset as the guides; it does not create another image copy or
provenance record. A download or install call to action appears only when a
real, reviewed release URL and platform requirement are present. Until then,
the page says **Follow the build** and does not imply that a public app release
exists.

The public site's visual system follows the Divine brand constraints:

- Bricolage Grotesque for display copy and Inter for reading copy, served only
  from checked-in, same-origin font files;
- candid, plain language with collective optimism and a small punk edge, without
  startup, platform, or "ecosystem" jargon;
- no icons in the initial site; any future icon must come from Phosphor and
  retain a visible text label;
- Divine off-white, dark green, and green as the foundation, with Riot pink and
  blue used only as secondary accents, plus paper, hard rules, and stamped
  labels; no gradients or colors outside the approved palette; and
- paired evidence that alternates its visual weight without changing the
  audience order or separating a claim from its boundary.

The initial marketing asset catalog is exact:

| Local file | Upstream version and immutable source | Bytes | SHA-256 | Use |
| --- | --- | ---: | --- | --- |
| `assets/fonts/bricolage-grotesque-latin.woff2` | Google Fonts Bricolage Grotesque v9, `https://fonts.gstatic.com/s/bricolagegrotesque/v9/3y9H6as8bTXq_nANBjzKo3IeZx8z6up5BeSl5jBNz_19PpbpMXuECpwUxJBOm_OJWiawA1XphjhQYg.woff2` | 41,236 | `4fd48b2c1ab27220e71f15f990550261b35245c3bdfd8d8025b4bdac0459ee2d` | normal 700 and 800 headings |
| `assets/fonts/inter-latin.woff2` | Google Fonts Inter v20, `https://fonts.gstatic.com/s/inter/v20/UcC73FwrK3iLTeHuS_nVMrMxCp50SjIa1ZL7W0Q5nw.woff2` | 48,432 | `c940764593d0fe5d596be327ca7558855e018039fb78509aa21921fd3644c3e4` | normal 400 and 600 copy/UI |

Both files are distributed under SIL Open Font License 1.1. Bricolage
Grotesque carries `Copyright 2022 The Bricolage Grotesque Project Authors
(https://github.com/ateliertriay/bricolage)`; Inter carries `Copyright 2020 The
Inter Project Authors (https://github.com/rsms/inter)`. Their complete,
unmodified license files are pinned respectively to Google Fonts commits
`6ce172f74aa355ea43eb964fa4a91570a4d3064d` and
`0b58fb370093f9a9f4ff785d94405710b79de67c`, with license-file SHA-256 values
`4b5a7d8f37f5602621c8a8d7358a6a2e71317e6c231c661e15aef0275d3e07ba`
and `5b9321a4298cfeb6b34354164a1c3afc3db114569984c502b9b35d988fd58c57`.

`marketing/assets/third-party-manifest.json` is the canonical typed catalog for
those two font files and two license files. It records local path, official
source, upstream version or commit, byte count, SHA-256, MIME type, family,
style, weight declarations, license expression, copyright, acquisition time,
acquisition reviewer, and a distinct license reviewer. Its
`marketing/public/assets/third-party-manifest.json` mirror and every cataloged
asset are byte-identical and hash-verified locally and after deployment.

The initial design uses no Phosphor glyph, font, SVG, or other icon asset.
Navigation and calls to action use visible text and typographic separators.
Tests reject any icon asset or icon-only control; adding one requires an updated
catalog, license/notice record, accessible name, and design review.

The homepage and its deployment mirror are byte-identical checked-in artifacts
at `marketing/index.html` and `marketing/public/index.html`. They reference the
generated guide assets in place rather than the four stale files under
`marketing/assets/screenshots/`. Those legacy screenshots may remain for
history until a separately reviewed cleanup, but no new public route may load
or link to them. Homepage presentation lives in the checked-in same-origin
`marketing/assets/site.css` and its byte-identical public mirror; font files are
separate same-origin assets rather than data URLs.

The same `site.css` and branded font files restyle `/protocols/` so the public
marketing site is visually coherent. Its source-backed editorial comparison,
citations, route, headings, and existing contract assertions remain intact.
This is a presentation migration, not replacement of the protocol comparison.
The canonical offline Why Riot, Using Riot, and notices documents deliberately
retain their system-font stack and stricter no-font CSP so they remain small and
self-contained in the apps.

Responsive navigation must keep both **Why Riot** and **Protocols** reachable
on small screens. The current rule that keeps only `.protocol-nav` visible must
be intentionally revised and covered by tests.

The homepage and both long pages provide a visible-on-focus skip link to the
main content. Guide jump links, native `details` disclosures, and heading order
work without JavaScript. The entire new marketing site is static: it removes
the current homepage reveal script and does not add JavaScript, analytics,
remote fonts, remote images, or remote styles.

The homepage response uses a site-specific CSP with
`default-src 'none'; script-src 'none'; style-src 'self'; font-src 'self';
img-src 'self'; connect-src 'none'; object-src 'none'; frame-src 'none';
base-uri 'none'; form-action 'none'; frame-ancestors 'none'`. It also uses
`Referrer-Policy: no-referrer` and `X-Content-Type-Options: nosniff`. The guide
documents retain their stricter no-font policy below.

The checked-in deployment remains `marketing/wrangler.toml` with
`marketing/public` as the Workers Assets directory and declares the exact
custom-domain route:

```toml
routes = [
  { pattern = "riot.divine.video", custom_domain = true }
]
```

Publication uses the repository's Wrangler flow. The sole accepted production
origin is `https://riot.divine.video`; a `workers.dev` URL or deployment
identifier is preview evidence only. After deployment, verification fetches
`/`, `/why-riot/`, `/guide/`, `/notices/`, `/protocols/`,
`/guides-manifest.json`, and every declared same-origin image and font. It
validates expected headings, CSP/referrer/`nosniff` headers, MIME types, byte
hashes, canonical redirect behavior, and the absence of remote runtime
requests. Wrangler success alone is not publication evidence.

The hostname currently resolves through Divine's Fastly wildcard and serves the
Riot profile surface, including a public NIP-05 response at
`/.well-known/nostr.json`. Before cutover, release tooling records the exact DNS
record, TTL, TLS certificate, response headers, homepage hash, and NIP-05 bytes.
The static deployment preserves this exact identity mapping:
`riot` →
`4691d54f806fbf625ab9e9fc73294759c1f056b62b49a97b3c68ae814e2e4535`,
with `application/json`, `Access-Control-Allow-Origin: *`, and no redirect. A
pre-cutover mismatch blocks publication and requires explicit review; the
deployment never overwrites a changed identity mapping from stale checked-in
data.

Cutover may replace the existing Fastly-targeted DNS record only after the
preview artifact passes. If DNS, TLS, identity, route, header, hash, or
no-remote-request verification fails, release tooling restores the recorded
Fastly DNS state and verifies rollback before reporting failure. Publication is
complete only when DNS no longer selects the generic profile route,
`https://riot.divine.video` serves the reviewed site and complete route set over
valid TLS, and the preserved NIP-05 mapping passes.

The release record captures the Wrangler deployment identifier, exact production
origin, UTC publication time, deployed commit, pre/post-cutover DNS and TLS
evidence, NIP-05 evidence, rollback record or not-needed result, and response
evidence. It also proves that `/` contains all eight ordered sections, the three
audience boundaries, and the exact current/planned and privacy qualifications;
that no legacy screenshot URL is requested; and that an install call to action
is absent unless its release metadata passed review.

## Web and embedded-document security

All three canonical HTML documents require:

- no user-derived HTML;
- no inline or external JavaScript;
- no remote fonts, images, styles, analytics, or other subresources;
- only static, manifest-declared PNG images from the constrained local guide
  origin; SVG, APNG, HTML polyglots, and images with undeclared or mismatched
  bytes or MIME types fail closed;
- every PNG must be a regular, non-symlink file with the exact PNG signature,
  one valid `IHDR` as the first chunk, valid declared dimensions/bit depth/color
  type, bounded row bytes and decoded size, valid chunk lengths and CRCs, at
  most one `PLTE` and `tRNS`, one or more `IDAT` chunks, exactly one terminal
  `IEND`, and zero trailing bytes;
- the only accepted chunk types are `IHDR`, `PLTE`, `tRNS`, `IDAT`, and `IEND`;
  unknown critical chunks, text/comment/time/location metadata, and the APNG
  `acTL`, `fcTL`, or `fdAT` chunks are rejected;
- every image must satisfy the compressed, pixel, eager-set, document, bundle,
  and measured-memory budgets above;
- PNG responses use `image/png` with `X-Content-Type-Options: nosniff`, may load
  only as manifest-declared image subresources, and are rejected as local
  top-level documents;
- the exact starting CSP `default-src 'none'; script-src 'none'; style-src
  'self'; img-src 'self'; connect-src 'none'; object-src 'none'; frame-src
  'none'; base-uri 'none'; form-action 'none'`;
- a `no-referrer` policy;
- explicit external-link labels;
- `noopener noreferrer` when a web link opens a new context; and
- no dependency on service workers or cached network content.

The deployed web response headers are verified in addition to the document
meta policy and add `frame-ancestors 'none'` and
`X-Content-Type-Options: nosniff`. Before a person deliberately follows an
external link, the exact allowed request set is the selected top-level guide
document plus manifest-declared same-origin guide assets such as
`assets/guide.css` and the declared PNGs. Cross-origin, redirected, remote,
analytic, scripted, and undeclared requests are forbidden.

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

The three Willow illustrations are acquired from the official, content-addressed
Data Model and Drop Format asset URLs. They retain their exact upstream bytes,
are visibly credited with the exact copyright and attribution text required by
the accepted license evidence, and ship with both license texts only after the
canonical license gate passes. The official source repository, specification
page, generated-output lineage, and Cargo package license metadata are retained
as separate provenance facts; none is treated alone as artwork permission or
publisher authentication. A file hash proves byte identity, not publisher
identity or copyright permission.

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
11. the document plus declared local stylesheet and PNGs load while automatic
    remote, redirected, WebSocket, and undeclared subresource requests are
    blocked;
12. only allowlisted main-frame links with a user gesture leave through the
    system browser;
13. redirects, meta refresh, iframes, `target=_blank`, downloads, `data:`,
    `javascript:`, `file:`, `intent:`, and custom schemes never launch an
    external application;
14. external-link failure and app backgrounding preserve the exact guide and
    scroll position;
15. malformed or unknown guide paths fail closed with the approved local
    recovery view;
16. semantic headings, skip links, accessible names, image alternatives,
    captions, dynamic text, keyboard focus, and reduced motion pass focused
    checks;
17. the Drop Format transport asset is exactly the `02718468...` 898-by-1353
    ad-hoc chain, not the `159b061a...` emblem, and all three Willow PNGs match
    the shared catalog's URL, bytes, SHA-256, dimensions, verbatim alt text,
    protocol maturity, attribution, and catalog digest;
18. incomplete artwork authority, scope, grant, copyright, attribution, NOTICE,
    reviewer, or license evidence stops before any Willow bytes enter a
    distributable bundle;
19. PNG fixtures with bad signatures, lengths, CRCs, duplicate or misplaced
    structural chunks, APNG chunks, unknown critical chunks, text/time/location
    metadata, decompression excess, wrong MIME, wrong hashes, or trailing bytes
    fail closed;
20. every Riot screenshot has the complete typed capture record, passes
    platform/surface coverage, ancestor and relevant-source-path freshness
    checks, contains only synthetic fixture data, has no prohibited metadata,
    and records an approved pixel-level privacy review;
21. per-image, eager-set, per-document, complete-bundle, offline-open-time, and
    measured-memory budgets pass;
22. Willow figures remain comprehensible on opaque paper panels in light, dark,
    forced-colors, and increased-contrast modes at 200% zoom;
23. **Third-party notices** is reachable from both guide footers and Help &
    Guides, contains the four complete license files with accurate asset scope,
    and returns with origin, section, scroll, keyboard focus, and
    assistive-technology focus preserved;
24. manual VoiceOver, TalkBack, macOS keyboard/screen-reader, dark-mode,
    forced-colors, increased-contrast, and 200%-zoom passes cover every paired
    figure and notices path; and
25. current/planned and privacy boundary copy remains present in every target;
26. the new homepage contains all eight sections in order, keeps every paired
    claim with its boundary, uses only the generated current screenshot set,
    and never references the four legacy screenshot URLs;
27. the source and deployment homepage are byte-identical, the complete public
    route set is present, and mobile navigation keeps Why Riot and Protocols
    reachable without script;
28. public marketing-page tests reject JavaScript, remote runtime assets,
    non-cataloged fonts or icons, and gradients on layout surfaces, while
    canonical guide tests require their documented system-font/no-font-resource
    exception;
29. `/protocols/` uses the branded site stylesheet and font catalog without
    changing its editorial comparison, citations, headings, route, or existing
    contract assertions;
30. every font and license byte matches the exact third-party catalog and
    source/public/deployed mirrors, all required copyright and OFL notices are
    visible, and undeclared or unused font/icon files fail;
31. the release preflight and acceptance checks require the exact
    `https://riot.divine.video` origin, valid DNS/TLS cutover, complete route
    set, preserved full NIP-05 mapping, and verified rollback on failure; and
32. a download or install call to action fails unless its exact reviewed release
    URL, platform requirements, and release metadata are present.

The implementation plan must name the exact first failing test for each work
unit before production code.

### Automated verification contract

Implementation is complete only when:

1. the guide sync script passes in `--check` mode;
2. source, marketing, deployment, Apple, and Android guide bytes match the
   manifest, including the byte-identical
   `marketing/guides-manifest.json` and public mirror;
3. existing marketing protocol-page contracts remain green;
4. the source and public homepage are byte-identical, and `/` contains the eight
   approved sections in order using Divine typography, icon, surface, voice,
   and no-script constraints;
5. no new public route references the four legacy generic screenshots;
6. `/protocols/` uses the shared branded marketing stylesheet and fonts while
   preserving its editorial comparison, citations, headings, route, and
   existing contract assertions;
7. the exact two-font/two-license third-party catalog, source/public mirrors,
   hashes, MIME types, copyrights, reviewer records, and notices pass, with no
   Phosphor or other icon asset present;
8. new guide structural/security contracts pass, including the intentional
   system-font and no-font-resource contract;
9. `/why-riot/` contains the three approved audience depths;
10. `/guide/` contains every approved task and platform/status boundary;
11. the two-path visual distinguishes nearby exchange, internet seed sync, and
   public web rendering;
12. all three audience pairings contain the approved Riot and Willow visuals,
   platform/build labels, opaque Willow panels, accurate boundary captions, and
   equivalent prose;
13. the three Willow assets and license files import from the canonical shared
   catalog with no field or byte drift, and the complete license-evidence gate
   passes;
14. all twelve platform-qualified Riot screenshot derivatives satisfy the
    capture, synthetic-data, privacy, freshness, metadata, and platform-label
    contracts;
15. public Newswire plaintext, gateway browser trust, pseudonym correlation,
   cooperative read control, and non-recall boundaries are explicit;
16. current and planned capabilities are labeled where first mentioned;
17. the web request set before deliberate external navigation is exactly the
    top-level document plus manifest-declared same-origin assets;
18. deployed CSP, `nosniff`, MIME, and referrer headers match the contract;
19. phone and desktop screenshots show no clipping, overlap, or page-level
    horizontal overflow at 320 CSS pixels and target viewports;
20. image size, pixel, lazy/eager, document, bundle, offline-open-time, and
    measured-memory budgets pass;
21. notices discovery, all four full local license files, accurate asset scope,
    and state/focus-preserving
    return behavior pass on web, iOS, macOS, and Android;
22. `https://riot.divine.video` serves the complete route set with valid DNS and
    TLS, expected headings, hashes, canonical redirect behavior, headers,
    preserved full NIP-05 mapping, and zero remote runtime requests;
23. failed cutover verification restores and proves the exact recorded Fastly
    DNS state before failure is reported;
24. the release record contains the deployment identifier, exact production
    origin, UTC publication time, deployed commit, pre/post DNS and TLS state,
    NIP-05 evidence, rollback result, and response evidence;
25. phone and desktop marketing-site checks prove the paired story, navigation,
    typography, contrast, focus, zoom, reduced-motion, and no-gradient surface
    contracts;
26. a public install call to action is absent unless reviewed release metadata
    exists;
27. automated and required manual visual/accessibility checks pass;
28. iOS tests and an iOS build pass;
29. macOS tests and a macOS build pass;
30. Android JVM tests, relevant instrumented tests, lint, and an APK build pass;
31. built `.app` and APK artifacts contain the exact guide bundle; and
32. repository formatting, linting, tests, and coverage floors remain green.

### No-network rehearsal

Before distribution, run the installed iOS, macOS, and Android builds with
network connectivity disabled:

- open both guides before joining a community;
- open both guides from the chooser and an active community;
- navigate between every local section;
- open Third-party notices from each origin and verify all four full license
  files and their accurate distribution scope;
- verify essential text, every visual, opaque Willow panels, platform labels,
  licenses, and troubleshooting content;
- verify lazy images appear without a network request, blank state, or scroll
  jump;
- return without losing current state; and
- confirm no blank, spinner, failed-resource, or network-dependent surface.

### Audience comprehension gate

Before public deployment, conduct a lightweight moderated or questionnaire
review with at least six people: two community/organizer readers, two
partner/journalist readers, and two builder/protocol readers.

The concise homepage is the first stimulus. Before opening Why Riot,
participants identify which of its three paths is for them and distinguish the
Willow possibility from the current Riot evidence in that pairing. They then
open the matching deeper section for the remaining questions. The release
record keeps homepage and deeper-guide results separately.

After reading the relevant depth, at least five of six must correctly explain:

- that a Willow update is not permanently tied to one transport;
- which nearby, web-rendering, and server-sync behaviors are current or planned;
- that the public Newswire is plaintext and pseudonymity is not anonymity;
- that public copies cannot be guaranteed to disappear; and
- that Riot is a prototype without verified physical two-iPhone Bluetooth.

For every pairing, participants answer two separate questions: what the Willow
illustration demonstrates about the protocol, and what the adjacent Riot
screenshot proves about the recorded current build. All six must keep those
answers separate. Any transfer of a Willow possibility into a claim about
current Riot requires caption or layout revision and a repeated review.

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
- the canonical shared `docs/assets/willow/**` catalog prerequisite defined by
  the approved Willow Visual Documentation System
- `docs/evidence/guides/screenshots/**`
- `marketing/index.html`
- `marketing/protocols/index.html`
- `marketing/README.md`
- marketing deployment/header configuration, including a generated `_headers`
  file if that is the supported Workers Assets mechanism
- the exact `marketing/.well-known/nostr.json` identity mirror and its public
  deployment copy
- generated `marketing/why-riot/**`, `marketing/guide/**`, and public mirrors
- generated marketing image, notices, and license mirrors
- the exact two checked-in same-origin font files, two OFL license files,
  third-party manifest, shared site stylesheet, and their public mirrors
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
