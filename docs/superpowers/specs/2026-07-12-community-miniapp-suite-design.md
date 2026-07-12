# Community Miniapp Suite Design

## Goal

Ship eight distinct, useful, seeded-but-editable social miniapps for Riot’s Local-First Conf demo:

1. Chat
2. Dispatches
3. Wiki
4. Photo Wall
5. Tasks
6. Needs & Offers
7. Events
8. Decisions

Activity Feed is a ninth app and a separate follow-up unit. It will eventually aggregate explicit activity records from these apps, but no part of the first eight may depend on it.

## Product Story

The suite belongs to one fictional neighborhood community. Opening an app should immediately show a lived-in group rather than an empty database, while every app still supports a small live interaction that can be demonstrated on stage. Together, the apps show that Riot is a platform for group-made software rather than a single fixed workflow.

The apps remain genuinely separate: each has its own manifest, app identity, signed bundle, trust decision, and app-scoped Willow data. Shared styling and conventions make them feel like a suite without merging their storage or permissions.

## Runtime Contract

Every app uses the existing `window.riot` API:

- `get`, `put`, and `list` for app-scoped JSON data;
- `watch` for live rerendering after local writes and completed sync;
- `whoami` when creating attributed records;
- `profile` when rendering a stored contributor ID.

No app gets network access, cross-app reads, native secrets, or a private storage bypass. Records use stable slash-separated key families. Creation keys include a timestamp plus a random suffix so separate devices do not overwrite one another. Mutable objects store state in a stable coordinate where Willow last-write-wins behavior is intentional.

Seed content is bundled in each app’s JavaScript and copied into app storage only when its root collection is empty. Once initialized, the stored dataset is authoritative: reopening never resets edits or resurrects deleted/completed demo state.

## Shared Experience

Each app is framework-free HTML, CSS, and JavaScript with no build step. The suite shares a compact visual language: readable system typography, high-contrast cards, one strong accent color per app, large touch targets, clear empty states, and plain-language errors. Layouts work on iPhone and macOS-sized WebViews.

Each app must:

- render meaningful seed content immediately;
- expose one obvious primary action without a tutorial;
- attribute human contributions using stored profile IDs;
- rerender through `riot.watch`;
- report save failures inline without discarding the current form;
- remain usable with JavaScript data already persisted from an earlier version.

## Visual and Interaction Design

The suite must feel intentionally designed, not like eight CRUD examples. All apps share structural tokens—spacing, typography, surfaces, radii, shadows, focus rings, and motion timing—while each app has its own recognizable accent and visual motif. Assets remain local and fonts use the platform stack, so appearance never depends on a network request.

The shared visual direction is warm, civic, and human: paper-like neutral backgrounds, ink-first contrast, restrained texture, expressive color, and content written in the voice of real neighbors. It avoids generic admin-dashboard styling, dense grids of identical cards, decorative gradients without hierarchy, and technical platform language.

Each app gets a distinct accent without changing the interaction grammar:

- Chat: electric blue with conversational bubbles;
- Dispatches: coral with editorial typography;
- Wiki: ochre with document and index motifs;
- Photo Wall: magenta with image-forward edge-to-edge tiles;
- Tasks: leaf green with tactile completion states;
- Needs & Offers: teal and orange as a clear two-sided system;
- Events: violet with date blocks and chronological rhythm;
- Decisions: indigo with legible choice and result bars.

Phone layouts prioritize a single column and one obvious primary action. Wider layouts may add a list/detail split where it materially improves navigation, but they must not merely stretch phone cards across the window. Forms open close to the action that invoked them, retain drafts after validation or save errors, focus the first meaningful field, and return focus sensibly when dismissed.

Interaction requirements:

- minimum 44-point touch targets and visible keyboard focus;
- semantic labels, form labels, status text, and contrast meeting WCAG AA;
- no color-only meaning;
- immediate pressed, saving, success, empty, and error feedback;
- destructive actions omitted from this release rather than made ambiguous;
- subtle transitions that honor reduced-motion preferences;
- long names and content wrap without clipping at 320-point width;
- primary actions remain reachable with the software keyboard visible;
- seeded content demonstrates hierarchy, truncation, attribution, and realistic edge lengths.

Visual quality is a blocking release gate. Every app is rendered and reviewed at 390×844 and 1280×800 in seeded, composer/editor, post-action, empty, and error states. Screenshot review checks hierarchy, spacing, typography, contrast, overflow, focus, and whether the app’s identity is recognizable without reading its title. Failed visual review requires iteration and recapture before the app is accepted.

## App Definitions

### Chat

A chronological group conversation. Seed messages establish the neighborhood scenario. A person can send a text message and see their rendered identity. Messages are append-only for this release; channels, attachments, reactions, editing, and deletion are deferred.

Key family: `messages/<created-at>-<suffix>`.

### Dispatches

A lightweight group publication for longer updates. The home screen lists title, summary, author, and publication time. A person can write and publish a new dispatch, then open full post text. Comments, drafts, rich text, and editing are deferred.

Key family: `posts/<created-at>-<suffix>`.

### Wiki

A shared set of titled plain-text pages. A person can open a page, edit its body, and save it. Page slugs are normalized locally and identify stable Willow coordinates, so concurrent replacements resolve through the existing last-write-wins model. History, backlinks, rich text, and page deletion are deferred.

Key family: `pages/<slug>`.

### Photo Wall

A captioned community photo grid. A person can choose an image, add a caption, and publish it. Browser-side canvas processing fixes orientation where available, constrains the longest edge, encodes JPEG, and rejects a result above the app’s conservative value budget before calling `put`. Seed photos are small bundled data assets. Albums, video, original-resolution storage, and editing are deferred.

Key family: `photos/<created-at>-<suffix>`.

### Tasks

A shared task list derived from the existing checklist interaction. A person can add a task, assign it to themselves or leave it unassigned, and toggle completion. Due dates, subtasks, recurring tasks, deletion, and arbitrary reassignment are deferred.

Key family: `tasks/<created-at>-<suffix>`.

### Needs & Offers

A mutual-aid matching board. A person can post either a need or an offer, view open items in separate sections, and mark one resolved. Direct messaging, location precision, expiry, matching recommendations, and deletion are deferred.

Key family: `items/<created-at>-<suffix>`.

### Events

A chronological list of upcoming gatherings. A person can add an event with title, local date/time, and place, then mark themselves as attending. Each RSVP is its own contributor-scoped coordinate so one person’s response does not overwrite another’s. Reminders, recurrence, maps, timezone conversion, and cancellation are deferred.

Key families: `events/<created-at>-<suffix>` and `rsvps/<event-id>/<profile-id>`.

### Decisions

A lightweight proposal and voting tool. A person can add a proposal with two to four choices and cast one current vote. Each contributor’s vote occupies its own stable coordinate and may be replaced by that contributor. Secret ballots, ranked choice, quorum enforcement, deadlines, and proposal editing are deferred.

Key families: `proposals/<created-at>-<suffix>` and `votes/<proposal-id>/<profile-id>`.

## Packaging and Catalog

Each app directory contains `riot-app.json`, `index.html`, `style.css`, and `app.js`, plus small local image assets where needed. The existing Riot app packer produces canonical manifest and bundle artifacts. Packed artifacts are checked in and verified against their sources so source/bundle drift fails tests.

All eight apps join the built-in starter catalog. They go through the same validation and organizer trust flow as the existing Checklist; built-in provenance is not automatic execution authority. Existing Checklist compatibility may be retained internally, but the visible Tasks app replaces it in the demo suite rather than creating two nearly identical tools.

## Activity Feed Follow-up

The Activity Feed will consume explicit, privacy-reviewed activity summaries rather than reading other apps’ private storage. The follow-up design must define:

- a bounded `riot.activity.publish` operation;
- which app actions emit summaries;
- a host-owned activity namespace and retention limits;
- the Feed app’s privileged read contract;
- sync, attribution, redaction, and deletion behavior.

Until that contract ships, the first eight apps do not emit pretend activity records and the catalog does not expose a nonfunctional Feed app.

## Error Handling and Limits

All forms trim required text and disable invalid submission. Failed bridge writes leave the draft visible and display “Couldn’t save that — try again.” Malformed stored rows are skipped individually so one bad record does not blank an app. Every rendered user string uses DOM text nodes or `textContent`, never HTML interpolation.

Photo Wall performs its own preflight because image data can approach runtime limits. Other apps bound text lengths and collection sizes conservatively in the UI, while Rust remains the authoritative byte/count enforcement layer.

## Verification

Implementation is test-driven and divided into independently reviewable slices:

- shared fixture helpers and canonical packaging checks;
- one focused behavior test per app covering initialization plus its primary live action;
- manifest tests for names, descriptions, permissions, entry points, and packed-byte drift;
- starter-catalog tests proving all eight verified apps are present without bypassing trust;
- host tests proving their pages load under Riot’s CSP and use only the supported bridge;
- a smoke flow that opens every trusted app and exercises the primary action;
- Playwright screenshots at phone and desktop widths for each app’s critical states, with documented visual findings and recapture after fixes;
- accessibility checks for labels, keyboard focus, reduced motion, touch-target sizing, contrast, and narrow-width overflow;
- full Rust, binding-generation, iOS RiotKit, macOS RiotKit, and relevant Android checks after integration.

The demo is ready when all eight apps appear in the directory, can be approved and opened, display their seed content, accept their primary interaction, survive reopen, and introduce no regression in nearby sync or app isolation.
