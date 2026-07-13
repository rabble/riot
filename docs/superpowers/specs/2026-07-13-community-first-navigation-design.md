# Community-first navigation design

Date: 2026-07-13  
Status: Draft after Rabble approved the visual flow; pending design review gate

## Product decision

Riot is organized around a **community**. A Willow space is the community's
technical container and the unit used to switch, sync, and isolate data. It is
not the product people are asked to understand.

After choosing a community, a person should be able to answer two questions
immediately:

1. What is happening here?
2. What can we do together?

Updates answer the first question. Community tools answer the second. Identity,
signatures, app packages, trust markers, namespaces, and transport state remain
important, but they appear where a person makes a decision that needs them. They
do not dominate the everyday interface.

## Evidence from the current app

The 2026-07-13 macOS walkthrough exposed a product failure, not merely rough
copy:

- **Spaces** pins profile editing, namespace data, demo controls, and app-review
  state above the community's useful content.
- **Board** renders alert headlines, timestamps, full entry IDs, and signer IDs,
  but omits alert descriptions and resolved author names. It reads like a ledger
  inspector and offers no next action.
- **Compose & sign** starts with a model-assistance checkbox and a cryptographic
  ceremony. The human goal—post an update to the community—is never stated.
- **Tools** offers `Review` for the only visible app. On the imported Riverside
  demo space the current profile is a member, not the organizer, so approval is
  structurally impossible. The person can inspect Checklist but cannot use it.
- The repository contains eight finished community miniapps, while
  `RiotAppModel.starterAppNames` names only four and both Apple Xcode projects
  bundle only Checklist's manifest and bundle. Missing resources are silently
  dropped by `compactMap`, so a successful build produces an app with almost no
  apps.
- The approved demo script expects Checklist to be on in Riverside and expects
  the presenter to approve Shift Signup. Those states cannot both be reached by
  a profile that imported someone else's space under the organizer gate.

These are mismatches between user goals, information architecture, shipped
resources, and authority semantics. Renaming the existing five tabs alone will
not resolve them.

## User goals

### G1 — Find or establish a community

A new person wants to create a community, join one from someone nearby, or open
one they already carry. They should not need to create or edit a public profile
first. Riot creates the local cryptographic identity it needs silently; choosing
a human-readable name is offered during setup and remains editable later.

### G2 — Know what is happening

A member wants the current, actionable updates in a community: what happened,
what is needed, who posted it, how fresh it is, and when it expires. Protocol IDs
are available for verification, but not in the primary reading hierarchy.

### G3 — Do useful work

A member wants to open Chat, Checklist, Needs & Offers, Events, Decisions, Wiki,
Photo Wall, or Dispatches and complete an obvious action. If a tool is available
to the community, opening it is one tap. Install and approval mechanics do not
stand between members and already-approved tools.

### G4 — Contribute an update

A member wants to post an alert or announcement. The flow is called **Post an
update**. Human review and signing remain mandatory, but signing is explained at
the final commitment point as authorship and tamper evidence—not used as the
feature name.

### G5 — Exchange changes offline

A member wants to find someone nearby, understand which community they share or
are being invited into, and exchange changes. Transport names and radio details
appear in diagnostics, not as prerequisites for the ordinary flow.

### G6 — Manage the community when responsible for it

An organizer wants to review a newly carried tool, read its complete permissions,
make it available to the community, manage membership or capabilities, and see
sync health. Members do not see controls they cannot use.

### G7 — Manage identity without making identity the product

A person wants to set their name, understand the key-derived disambiguation tag,
and eventually maintain community-specific personas. This lives in the account
or community profile surface, not permanently above community content.

## Information architecture

Riot has two navigation levels.

### Level 1 — Community containers

The launch surface is **Your communities**. Each row shows:

- community name;
- relationship: organized here, member, or public reader;
- recent activity summary;
- sync freshness or pending nearby changes in plain language;
- availability problems that require action.

The surface also offers **Create or join a community**. The person's name/avatar
opens account and profile settings. It is not a full-width card above the list.

The underlying model may continue to call these records spaces. User-facing copy
uses *community* unless the distinction between public newswire, personal page,
and private group needs to be explained.

### Level 2 — Inside one community

The selected community has four destinations:

1. **Home** — current updates plus a compact row of frequently used tools.
2. **Tools** — every tool available in this community, followed by discoverable
   tools that an organizer may add.
3. **People** — members or known contributors, their rendered names, roles, and
   connection state.
4. **Nearby** — find peers, invite or join, preview incoming changes, and sync.

The community name and a back/switch affordance stay visible. Community settings
live behind a contextual menu. The profile editor and namespace are not part of
the community's scroll content.

**Post an update** is a primary action on Home. Its exact placement may adapt to
screen size, but it is not a permanent fifth information destination.

## Core flows

### First run

1. Riot opens **Your communities** with two primary choices: create one or find
   one nearby.
2. Riot offers a display name inline or as a skippable setup step. Skipping uses
   a neutral rendered identity; it does not block community work.
3. Creating enters the new community as organizer. Joining enters it as member
   with the authority the invitation granted.
4. The community opens to Home with clear empty states: post an update, add a
   tool, or find someone nearby.

### Returning member

1. Riot returns to the last community when it is available.
2. Home shows readable updates and frequently used tools.
3. The community switcher is one step away and never discards the current
   community's data.

### Read an update

An update card shows headline, meaningful body preview, resolved author,
freshness, expiry, and applicable status such as correction or verification.
Opening it shows the full content and provenance. Full namespace, entry, and
signer identifiers live in a **Technical details** disclosure with copy/share
actions.

### Post an update

1. Tap **Post an update**.
2. Enter a headline and what people need to know. Operational types also collect
   expiry and source information in plain language.
3. Optional local assistance can help rewrite or translate, but it is invoked as
   a tool rather than represented by a default-on provenance checkbox.
4. Review the exact update and destination community.
5. Tap **Post update**. Supporting copy says the update will carry this person's
   identity and can be verified after it is shared.
6. The new update appears on Home with a local/pending-sync receipt.

The cryptographic operation is still signing. The action label is the outcome
the person intends.

### Use a tool

1. Tap an approved tool from Home or Tools.
2. The tool opens immediately with the selected community visibly in context.
3. Complete the tool's primary action.
4. Close or navigate back to the same community.

No approved tool requires a second visit to an app directory. A tool that loses
approval while open closes with a plain explanation.

### Add a tool as organizer

1. A newly carried or built-in tool appears under **Available to add** in Tools.
2. An organizer taps **Review and add**.
3. Riot shows name, author/provenance, exact permissions, independent
   recommendations, and any unavailable package bytes.
4. The organizer taps **Add to this community**.
5. The tool moves to **Available to everyone** and becomes directly openable.

Members see only tools already available to their community. Unapproved packages
appear in **Available to add** only for organizers. If a member reaches one from
a notification or deep link, its detail page says **An organizer needs to add
this tool** and offers no dead action. Recommendations remain advisory and
separate from approval.

### Sync nearby

1. Open Nearby and start looking.
2. A discovered device is identified honestly as a device until Riot has a
   verified person identity.
3. Before joining or exchanging, show the community name and the concrete
   outcome: join, get changes, already current, or different community.
4. Preview decisions that need consent; otherwise show progress in terms of
   updates and tools, not frames or bundles.
5. Return to Home and show newly arrived content in place.

## Demo and fixture contract

The demo must exercise rules the product actually enforces.

- An imported Riverside profile is a member. It must not gain an approval bypass.
- Every tool used in the Riverside walkthrough arrives with a valid organizer
  trust marker in the signed fixture. The imported member can open it directly.
- The Riverside member walkthrough does not demonstrate organizer approval. A
  separate test or walkthrough that starts from a genuinely created community
  covers that flow; there is no demo-only authority bypass.
- The Apple targets must bundle every starter manifest and bundle named by the
  runtime. Missing resources are a build/test failure, never a silent catalog
  reduction.
- The demo setup asserts that the expected tools are openable before the radio
  rehearsal begins.

## Current-to-target mapping

| Current surface | Target location |
| --- | --- |
| Spaces tab | Your communities switcher |
| Pinned You card | Account/profile menu |
| Namespace text | Community settings → Technical details |
| Public Incident Space card | Community header/settings |
| Tools card | Home shortcuts + Tools destination |
| Apps tab | Tools destination; organizer discovery section |
| Board tab | Home updates |
| Compose tab | Post an update flow |
| Connect tab | Nearby destination |
| App Review sheet | Organizer-only Review and add flow |

## State and error behavior

- **No communities:** explain create vs. join; do not show empty Board/Apps tabs.
- **No updates:** invite the person to post or sync nearby.
- **No tools approved:** members see an honest empty state; organizers see tools
  available to add.
- **Package incomplete:** say the tool is still arriving and identify the next
  useful action.
- **Member cannot approve:** never render an approval button. Explain the
  organizer relationship only if the person opens tool details.
- **Legacy profile:** surface recovery/reset guidance in account settings before
  an organizer action depends on it.
- **Key unavailable:** preserve the draft, explain that the device must be
  unlocked, and perform no partial post.
- **Sync interrupted:** keep already committed content, show that exchange
  stopped, and offer retry without losing community context.
- **Space unavailable/corrupt:** do not replace it with a blank community.

## Security and authority boundaries

This design changes presentation, not authority:

- only a genuine organizer may approve a tool for a community;
- every app still runs inside the existing isolated runtime and app-scoped data
  paths;
- recommendation never grants execution authority;
- signing remains required for published updates;
- IDs are never truncated in storage, fixtures, logs, or technical details;
- community switching must bind app and sync sessions immutably to one namespace;
- profile relocation must not hide meaningful identity/provenance at the moment
  a person posts, joins, approves, or verifies content.

## Delivery path

### Slice 0 — Make the existing build tell the truth

Fix the product blockers without changing navigation architecture:

- bundle all eight verified starter app pairs in both Apple app targets;
- make the Swift starter list match the Rust catalog;
- fail tests/build verification when any named starter pair is absent;
- make the Riverside fixture's usable tools carry valid organizer approval and
  replace its impossible member-approval beat with direct tool use;
- add an end-to-end assertion that a fresh demo member can open and interact with
  the expected tools.

This is the smallest slice that converts the present “Review only” experience
into usable software.

### Slice 1 — Useful content and language

- enrich the update DTO so Home can show body, resolved author, expiry, source,
  severity, and correction/verification state;
- replace the identifier-first board cards with readable update cards and a
  technical-details disclosure;
- rename Compose & sign to Post an update and rewrite the commitment step around
  authorship and destination;
- make model assistance an explicit optional action rather than a default-on
  checkbox.

### Slice 2 — Community shell

- introduce the community header and Home / Tools / People / Nearby navigation;
- move profile editing to account settings;
- move app approval and namespace details to community settings;
- preserve existing app runtime and nearby controllers behind the new routes;
- keep the community name visible while a tool runs.

This slice may initially expose the one current space through the new container
UI. It must not fake multi-space retention.

### Slice 3 — Multiple communities

Resume the reviewed SQLite/multi-space design:

- durable community registry and selection;
- create, join, list, switch, archive, and restore without replaying or deleting
  another namespace;
- per-community update, approval, tool-data, identity, and sync sessions;
- launch and switch performance budgets from the multi-space design.

This is what makes the Level 1 container navigation fully real.

### Slice 4 — Personal space and richer community setup

Resume the approved personal-spaces/pages work after multi-space persistence:

- offer a person's own space during onboarding without treating it as their only
  identity;
- support personal page creation and connections-only read gates;
- add community-specific profile presentation without a global profile clobbering
  local overrides.

## Verification contract

Each slice uses TDD and adds user-flow coverage before implementation.

Required proof includes:

- both Apple bundles contain every named starter pair;
- the catalog count and identities agree across Rust, Swift, and packaged
  resources;
- an organizer can add a tool and a member cannot self-approve it;
- a member can directly open every already-approved tool and complete its primary
  action;
- the Riverside demo member can open seeded tools without an authority bypass;
- update cards render meaningful content and resolved authors while technical
  identifiers remain available on demand;
- posting creates one signed update in the selected community and preserves the
  draft on failure;
- switching communities never leaks updates, approvals, or app data;
- nearby sync returns the person to a visibly updated community;
- macOS and iPhone builds include every new or moved Swift source and resource;
- visual review covers phone and desktop widths, long names, empty/error states,
  keyboard navigation, VoiceOver labels, reduced motion, and large text.

Physical-radio proof remains separate: loopback or Bonjour-on-one-Mac tests do
not establish BLE behavior between two iPhones.

## Success criteria

The community-first redesign is successful when a new evaluator can, without
explanation:

1. create or join a community;
2. understand what is happening there;
3. open a useful tool and change shared state;
4. post an update in language that matches their intent;
5. find another nearby device and understand what will exchange;
6. switch communities without losing state;
7. find profile and organizer controls when they need them, without those
   controls displacing everyday community work.

The redesign fails if the primary experience still asks people to understand
signatures, namespaces, app packages, or trust state before they can accomplish
those goals.
