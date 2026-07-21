# Riot Microapp Family Design

Date: 2026-07-22
Status: User-approved direction; design review pending

## Product decision

Redesign Riot's eight built-in community microapps as one recognizable family
without flattening them into identical templates.

The suite uses:

- a compact native host that explains how to leave or move up;
- a focused tool screen with no competing Riot tab bar on phones;
- a shared bottom toolbar for in-tool views and actions;
- one offline visual system built from Riot's paper, ink, display, reading, and
  mono typography;
- restrained app-specific layouts and motifs;
- six accessibility-tested personal color themes; and
- **Night Garden** as the default theme.

This design incorporates the approved compact-tool breadcrumb contract from
`docs/superpowers/specs/2026-07-22-compact-tool-breadcrumb-design.md`, currently
being integrated from the `overnight-2026-07-22` worktree. That sibling spec
must land before or with this work. The breadcrumb owns macOS host location;
the bottom toolbar owns navigation inside the mounted microapp. Neither repeats
the other.

## Why this change

The current suite has two different visual histories:

- the original frozen Checklist is a plain white page with browser-default
  controls; and
- the other seven tools use copies of a soft, rounded token system with large
  system headings, rounded cards, shadows, and unrelated accent colors.

Those seven tools are broadly related to one another but not to Riot's native
paper-and-poster identity. Each also invents its own header and action placement.
Inside the native host this produces up to three stacked orientation layers:
native app chrome, an activity strip, and a repeated microapp eyebrow/title/
description/action block. The result wastes the most valuable part of a phone
screen while still leaving some detail and editing views hard to escape.

Riot should feel like a box of community tools made by the same people: familiar
enough to use under pressure, distinct enough that Checklist does not feel like
Photo Wall, and complete without an internet connection.

## Product values

The visual and interaction system should evoke the ordinary collective capacity
described in Rebecca Solnit's *A Paradise Built in Hell*: people finding one
another, improvising useful structures, sharing resources, and making a livable
commons when centralized systems are absent or fail.

The target is not glossy eco-technology or emergency-management bureaucracy.
It is a practical, hand-made field kit: garden growth, repair materials, shared
food, fluorescent notices, human warmth, and serious information that remains
readable.

The default palette also reflects current 2026 color signals without becoming a
seasonal costume. Pinterest's 2026 palette identifies Plum Noir, Jade, Wasabi,
and Persimmon from search-and-save behavior, while WGSN's Spring/Summer 2026
forecast pairs ecological and energetic colors such as Transformative Teal,
Electric Fuchsia, Jelly Mint, and Amber Haze. Riot keeps its stable paper, ink,
and pink identity and uses those signals only for theme accents.

Sources:

- <https://business.pinterest.com/en-ca/pinterest-palette/>
- <https://www.wgsn.com/cs/node/1955>
- <https://www.kirkusreviews.com/book-reviews/rebecca-solnit/a-paradise-built-in-hell/>

## Goals

1. Make every built-in microapp immediately recognizable as Riot.
2. Give every meaningful view an obvious route home, a visible current state,
   and a predictable primary action.
3. Remove duplicated and oversized header chrome.
4. Keep every font, style, theme, and interaction available offline.
5. Let each person choose a theme without imposing it on their community or
   leaking the preference into shared Willow data.
6. Preserve the content-addressed identity and data of already-installed app
   versions.
7. Maintain the current WebView sandbox: no network, arbitrary navigation,
   local storage, or new privileged app bridge.
8. Meet keyboard, screen-reader, contrast, reduced-motion, and touch-target
   requirements across phone and desktop layouts.

## Non-goals

- Redesigning Riot's global Home, People, Nearby, or community surfaces.
- Giving a community owner control over another person's theme.
- Synchronizing theme preferences between devices or through Willow.
- Supporting arbitrary user-entered colors in the first release.
- Migrating data automatically between content-addressed app versions.
- Adding remote fonts, analytics, trackers, service workers, or network access.
- Making every tool use identical content layout or identical toolbar items.
- Changing app permissions, trust, signing, or Willow reconciliation behavior.

## Navigation ownership

Navigation is split into two explicit layers:

```text
native host: community / tool / current page / leave or move upward
microapp:    tool views / contextual actions / create and edit flow
```

The native host never duplicates a microapp's view tabs. The microapp never
duplicates the community name, app name, native Back/Close control, or macOS
breadcrumb.

### macOS host

The compact breadcrumb design is authoritative:

```text
Riverside Community › Wiki › Meeting guide
🏘 › 🧰 › 📄
```

- The row is at most 36 points high, including its divider.
- The community ancestor opens the community chooser.
- The app ancestor requests the app root with the fixed, payload-free
  `riot:navigate-root` event when a nested page is present.
- The current page is text, not a misleading button.
- `ViewThatFits` selects complete labels or the accessible emoji fallback.
- Community switching retains the conservative Stay-or-Switch confirmation.
- Home, Tools, People, Nearby, and their keyboard equivalents close the mounted
  tool before applying the selected route, fixing the existing route-masking
  defect.
- Escape returns from the tool to Tools.

The existing multi-line activity strip is folded into the same compact host
row. At sufficient width it displays a small activity mark/count at the trailing
edge with names in help/accessibility text. Under pressure it yields before any
breadcrumb ancestor or current location is clipped. An empty activity digest
renders nothing.

### iPhone host

- The mounted tool remains a push inside the Tools `NavigationStack`.
- One inline native row supplies `‹ Tools`, the app name, and the compact
  activity indicator when non-empty.
- Riot's global tab bar hides only while the tool is mounted. This deliberately
  amends the earlier breadcrumb design's statement that the global tabs remain
  visible, while preserving the Tools stack and ordinary return behavior.
- Leaving the tool restores the same Tools destination and global tab bar.
- Trust invalidation continues to close the tool through the existing path.

### Android host

- Replace the standalone `Close <app>` action with the same compact conceptual
  row: Back/Tools, app name, and optional activity indicator.
- The tool owns the remaining content height and bottom edge.
- Android's system Back follows the same close-to-Tools route.
- Host state, not page script, remains authoritative for closing the runtime.

### WebView root

At an app root, the WebView begins with useful content. It does not render:

- a second app name;
- a “Riot tool” eyebrow;
- a generic tagline that displaces the first task; or
- a duplicate Close or Back-to-Tools button.

An app may use one short, expressive content heading such as “Tick it off” or
“Shared memory” when it helps distinguish the work. It must not restate the app
name, stack with another intro paragraph, or push the first useful row below the
fold.

## Bottom toolbar

The user-selected navigation pattern is a bottom toolbar inside every focused
tool.

### Shared behavior

- The toolbar is fixed to the tool's safe bottom edge.
- Scrollable content receives padding equal to the toolbar plus safe area.
- It uses a two-pixel ink divider and the selected theme's surface.
- Every destination/action has a visible icon and short text label. Icon-only
  `+` controls are not the shipping accessibility contract.
- Targets are at least 44 by 44 CSS pixels and never depend on hover.
- The current view has `aria-current` or an equivalent state and is conveyed by
  more than color alone.
- The primary create/write/add action uses the theme's action role.
- Two to four items are allowed. Tools do not receive meaningless tabs merely
  to fill the bar.
- Keyboard focus moves predictably after a view change and returns to the
  originating control after cancellation.
- The toolbar stays in the same location on phone and desktop. Wide tools may
  retain split content layouts above it; they do not turn the toolbar into an
  unrelated sidebar navigation system.

### Contextual views

Root navigation yields to contextual actions when appropriate:

- a detail view exposes its root/list destination and relevant action;
- a create/edit view exposes Cancel and Save/Publish;
- Save/Publish is disabled until valid and while a write is pending;
- Cancel never destroys a draft while a write is pending; and
- a failed write keeps exact form input and returns focus to a useful field.

The macOS app breadcrumb remains an additional ancestor route for nested pages.
It does not replace local contextual actions that phone, Android, keyboard, and
screen-reader users need.

## App-by-app information architecture

The toolbar changes must be backed by real, useful behavior rather than visual
decoration.

| Tool | Root toolbar | Contextual toolbar | App character |
| --- | --- | --- | --- |
| Checklist | All, Open, Done, Add | Cancel, Add | Pink tick stamps, dense task rows, immediate completion state |
| Needs & Offers | Needs, Offers, Add | Cancel, Post | Split exchange-board rhythm; pink needs and quiet-green offers |
| Events | Upcoming, Going, Add | Cancel, Save | Calendar/date blocks and ticket-like left rules |
| Decisions | Vote, Ask | Cancel, Post | One dominant question and honest tally bars |
| Chat | Messages, Latest, Write | Composer occupies the toolbar frame | Dense conversational strips; no invented People screen |
| Dispatches | All, Mine, Write | All, Read, or Cancel/Publish | Editorial rules and broad reading cards |
| Wiki | Pages, Recent | Pages/Read/Edit or Cancel/Save | Index-card edges and desktop list/detail split |
| Photo Wall | Photos, Mine, Add | Cancel, Share | Slightly irregular framed grid with captions always available |

`Open`, `Done`, `Going`, `Mine`, and `Recent` are local projections of already
available app rows. They do not write new shared metadata. `Latest` scrolls and
focuses the newest message; it is an action, not a fake second message store.

## Visual family

### Stable foundation

Every tool uses the same semantic foundation:

- warm paper rather than browser white;
- near-black ink and hard two-pixel rules;
- Anton for short display emphasis;
- Work Sans for reading and form controls;
- Space Mono for metadata, status, dates, and toolbar labels;
- square or lightly eased corners rather than soft pill/card UI;
- deliberate offset shadows used sparingly;
- one spacing scale and one responsive type scale;
- one field, button, empty-state, error, loading, and read-only language; and
- visible focus, reduced motion, 44-pixel targets, and zoom-safe layout.

### App character

Character comes from information layout and small motifs, not private design
systems. An app may choose how it uses the shared structure, quiet, signal, and
action roles, but may not invent new untested colors or change their semantic
meaning.

The result should read as “Riot family, app character”:

- Checklist is compact and kinetic.
- Needs & Offers feels like a physical exchange board.
- Events makes time scannable.
- Decisions makes the current question unavoidable.
- Chat prioritizes conversation density and a reachable composer.
- Dispatches feels editorial and durable.
- Wiki feels indexed and navigable.
- Photo Wall lets images lead without losing captions or authorship.

## Personal themes

### Scope and storage

Theme choice is personal, profile-specific, device-local, and available without
internet.

- It applies to all built-in microapps and the compact tool-host accents.
- It does not recolor Riot's global community navigation or trust/review UI.
- It is stored through a small native preference abstraction backed by
  `UserDefaults` on Apple platforms and `SharedPreferences` on Android.
- The storage key includes the local Riot profile identifier, never a community
  identifier.
- It never enters Willow, an app-data namespace, a bundle, diagnostics, a URL,
  or peer sync.
- DOM storage remains disabled.
- An absent, malformed, or retired value resolves to `night-garden`.

The picker lives in Tools as **Tool appearance**, outside a mounted runtime.
Choosing a preset therefore cannot reload a WebView or discard an in-memory
draft. The next tool mount uses the new theme. A future live picker is outside
scope unless it preserves exact drafts without broadening the bridge.

### Presets

All presets preserve the same neutral paper/ink system and semantic roles.
These are light-mode anchor colors; each ships with a contrast-tested dark-mode
tone for the same named role.

| Theme | Structure | Action | Quiet/growth | Signal/energy | Intent |
| --- | --- | --- | --- | --- | --- |
| Night Garden (default) | Deep amaranth `#642B58` | Riot pink `#D1216E` | Jade `#AEB8A0` | Wasabi `#E9F056` | Serious, alive, collective |
| Repair Picnic | Plum noir `#351E28` | Persimmon `#FF5C34` | Jade `#AEB8A0` | Faded gold `#D3B86A` | Material, warm, practical |
| Living Network | Transformative teal `#006B62` | Riot pink `#D1216E` | Jelly mint `#B8DFBD` | Amber `#D5A83F` | Ecological, connected, technical |
| Deep Amaranth | Deep amaranth `#642B58` | Riot pink `#D1216E` | Amaranth tint | Pink tint | Focused, editorial, minimal |
| Signal Chartreuse | Chartreuse `#C8E63C` | Riot pink `#D1216E` | Chartreuse tint | Pink tint | Fluorescent poster energy |
| Burnt Tomato | Burnt tomato `#E94B35` | Riot pink `#D1216E` | Tomato tint | Warm gold tint | Sun-baked and confident |

Theme CSS uses role names, never color names:

```css
--riot-paper
--riot-surface
--riot-ink
--riot-ink-soft
--riot-line
--riot-structure
--riot-on-structure
--riot-action
--riot-on-action
--riot-quiet
--riot-signal
--riot-focus
```

Text/background pairs meet WCAG AA. Focus indicators and non-text control
boundaries meet at least 3:1 against adjacent colors. Status, selection,
completion, and error are never conveyed by color alone.

### Host-to-WebView theme contract

The host maps stored values to a closed native enum and injects only the enum's
canonical key at document start. The document root receives a value such as:

```html
<html data-riot-theme="night-garden">
```

The page cannot supply script to the host, choose arbitrary CSS, read native
preferences, or learn anything about other profiles. An unrecognized value is
never injected. The operation adds no message handler, scheme, storage API, or
network capability.

## Nested page location

The compact breadcrumb title contract applies to every redesigned app that has
a real nested view:

- root: `<app name>`;
- nested: `<page name> — <app name>`; and
- app-root request: the fixed `riot:navigate-root` event.

Wiki keeps the complete parser, title-observation, explicit-index, and editing-
conflict behavior from the compact breadcrumb design. Dispatches detail and
compose, Wiki detail/edit, and other genuine full-page create flows adopt the
same title convention. Filter changes such as All to Mine do not become native
page crumbs.

Titles are display metadata only. They are bounded, normalized, control-
character filtered, never logged, and never used to authorize storage,
community switching, trust, or navigation policy.

## Offline asset architecture

Every render-critical byte travels with the app bundle.

### Canonical shared source

`fixtures/apps/_shared/` remains the authored source of truth for:

- semantic tokens and all theme presets;
- base typography and locally bundled font declarations;
- bottom-toolbar and common control styles; and
- the small unprivileged helper used for toolbar state, theme fallback, and the
  root-navigation event.

The pack/check workflow copies or generates deterministic app-local assets
before encoding each content-addressed bundle. Contract tests compare every
app-local copy to the canonical source so the suite cannot drift through manual
copy/paste.

App-specific CSS and JavaScript remain inside the app directory. Shared code
does not gain app-data access beyond calls each app already makes explicitly.

### Fonts and CSP

The existing Riot Anton, Work Sans, and Space Mono font sources are converted
to deterministic WOFF2 assets and packaged locally. License/NOTICE evidence for
redistribution with peer-shareable app bundles must remain present in the
repository and release notices.

The host and preview CSP add only:

```text
font-src 'self'
```

Resolvers add explicit WOFF2 content types. `default-src 'none'`, blocked
network loads, exact app-origin checks, navigation refusal, disabled file/
content access, and the existing JavaScript bridge boundary remain unchanged.

## Content identity and upgrades

Changing any HTML, CSS, JavaScript, font, manifest, or bundle byte produces a
new app ID. App data is stored under `apps/<app_id>/...`; a new version therefore
does not automatically see the old version's rows.

The suite ships as version 2 rather than silently pretending the redesign is
the same app:

- fresh starter catalogs offer the eight v2 bundles;
- already-installed v1 apps remain mounted against their original IDs and data;
- v1 and v2 may coexist while a community decides what to trust;
- installing v2 starts a distinct app-data namespace;
- no host-side copying, aliasing, or cross-ID read permission is introduced;
  and
- a future explicit, reviewed migration design may address cross-version data.

Checklist is a special compatibility boundary. Its existing source, manifest,
bundle, and app ID remain byte-for-byte frozen. The unified Checklist is a new
source/artifact slug and version. Existing demo/community data pinned to the old
Checklist ID remains valid; fresh demo/catalog data must pin the v2 ID only when
the fixture intentionally uses Checklist v2.

The other seven prior artifacts need not remain built into the current starter
catalog, but synced or installed copies continue to verify and run by content
identity.

## State, failure, and conflict behavior

The redesign must preserve current local-first guarantees:

- seeded data is never overwritten merely to make a screenshot attractive;
- malformed or hostile rows are ignored without breaking valid content;
- delayed identity keeps the UI legible and actions disabled until safe;
- identity failure leaves useful content read-only;
- interrupted seeding resumes without duplicating visible records;
- failed writes preserve exact drafts and prior committed state;
- repeated taps cannot create duplicate writes while an operation is pending;
- sync-driven deletion of a selected item returns safely to a valid view unless
  an edit conflict requires preserving the draft;
- trust invalidation tears down the runtime; and
- all empty, loading, read-only, error, and retry states use shared language and
  visual structure.

Theme selection cannot affect app data, seeding, sorting, or reconciliation.
Filters such as Mine or Going derive from already-held rows and current local
identity.

## Accessibility

- Body text supports platform text scaling and browser zoom without horizontal
  page scrolling at 320 CSS pixels.
- Display type is limited to short phrases and is never required to understand
  an action.
- Every toolbar item has visible text, an accessible name, and a 44-pixel target.
- Current view, selected vote, completion, RSVP, and errors have text/icon/state
  semantics in addition to color.
- Focus order follows host ancestors, page content, then bottom toolbar unless a
  contextual view intentionally moves focus to its heading or first invalid
  field.
- `:focus-visible` uses a three-pixel ring with at least 3:1 contrast.
- Animations honor `prefers-reduced-motion`; no essential state depends on
  animation.
- Photo captions and author labels remain available; decorative app motifs and
  toolbar glyphs are accessibility-hidden when the adjacent text names them.
- Full and emoji breadcrumb presentations expose identical meaning.
- Dark and light variants of all six themes pass automated contrast checks and
  manual screen-reader/keyboard review.

## Security and privacy

- Microapps retain no network capability and load no remote assets.
- Theme preference stays in device-local native preferences and never becomes
  community or app data.
- The theme contract accepts one allowlisted enum value and no arbitrary CSS or
  script.
- The root-navigation contract sends one constant, payload-free DOM event.
- Document titles remain bounded display strings and are not logged.
- User-generated text is built with DOM text nodes/`textContent`, never
  `innerHTML`.
- URL, file, content, service-worker, pop-up, and external-navigation defenses
  remain in place.
- New font MIME support and `font-src 'self'` expose only verified bytes inside
  the already-selected app bundle.
- A v2 app never gains implicit access to a v1 namespace.

## TDD and verification strategy

Implementation is divided into red-green-refactor slices. Each slice begins
with a failing test and stays within the file scope declared by the later
implementation plan.

### 1. Shared visual and theme contracts

Add contract tests before tokens or components change. They prove:

- every v2 app loads canonical shared assets before app styles;
- all six preset keys and semantic roles exist;
- Night Garden is the fallback/default;
- light and dark text/control/focus pairs meet their contrast floors;
- controls meet target size, focus, and reduced-motion requirements;
- root HTML does not repeat the app name/eyebrow/intro stack; and
- fonts are local, resolvable, correctly typed, and permitted only by
  `font-src 'self'`.

### 2. Native preference and host injection

Pure store tests cover profile-key isolation, round-trip of each allowlisted
theme, malformed-value fallback, and absence of community/Willow writes. Host
tests prove document-start injection uses only the canonical enum key and occurs
before app CSS resolves. Existing host egress/navigation tests remain green.

### 3. Focused host navigation

Apple and Android tests prove compact host presentation, merged/absent activity,
phone tab-bar hiding/restoration, system/native Back behavior, trust
invalidation, and close-before-route behavior. The compact breadcrumb parser,
chooser, title observation, emoji fallback, and fixed root-event tests remain
authoritative on macOS.

### 4. Bottom toolbar and app flows

Browser tests exercise every toolbar item against the real preview bridge at
phone and desktop sizes. They cover root, filter, detail, create/edit, cancel,
successful write, failed write, duplicate-tap protection, and focus restoration.
The tests assert visible labels, current-view semantics, safe-area/content
clearance, and correct document titles/root events.

### 5. App state matrix

Every app runs under seeded, initialized-empty, delayed-identity,
identity-error, interrupted-seeding, malformed, slow-write where applicable,
and deterministic write-error states. Existing data validation and hostile-row
tests remain blocking.

### 6. Versioning and deterministic artifacts

Rust and script tests prove:

- frozen Checklist v1 bytes and ID do not change;
- each v2 manifest/bundle verifies and has the intended visible name/version;
- the starter catalog contains exactly the intended current v2 entries;
- old and new app IDs never share storage paths;
- repacking is deterministic; and
- committed source and CBOR artifacts do not drift.

### 7. Visual review

Playwright captures all eight apps at 390×844 and 1280×800 for:

- Night Garden seeded state;
- every other theme on representative high-risk screens;
- empty and error states;
- long labels and maximum text size; and
- Wiki/Dispatches root, detail, and edit states.

Review checks toolbar stability, first-useful-content position, no clipped text,
theme contrast, app distinction, phone safe areas, desktop whitespace, and
breadcrumb integration. Native macOS/iPhone/Android captures verify the real
host rather than only the preview page.

After focused tests, run the repository's full formatting, strict lint,
platform build/test, starter audit/drift, Rust workspace, and authoritative
coverage commands. `.coverage-thresholds.json` remains the coverage source of
truth and floors may not be lowered without committed justification.

## Acceptance criteria

- All eight current starter tools use one Riot visual family and retain a clear
  app-specific character.
- No mounted microapp repeats its app name or renders an oversized three-line
  intro header.
- Phone tools hide Riot's global tab bar while mounted and restore it on exit.
- macOS shows the approved compact community/app/page breadcrumb and no
  oversized Close row.
- Host location and app navigation fit into one compact top row plus one bottom
  toolbar; no stacked bottom navigation bars appear.
- Every meaningful root, filter, detail, and create/edit state has an obvious
  route and accessible current-state indication.
- Night Garden is the default, and all six themes can be chosen under Tool
  appearance while offline.
- Theme choices are per-profile/per-device, do not sync, and do not affect what
  peers see.
- Every preset passes light/dark contrast, focus, target-size, keyboard, text-
  scaling, and screen-reader checks.
- All typography and theme assets render with the network disabled.
- Existing installed app versions and their data remain addressable under their
  original IDs; no silent cross-ID migration occurs.
- Checklist v1 source, manifest, bundle, and pinned app ID remain unchanged.
- v2 source and committed artifacts repack deterministically and pass the
  starter catalog audit.
- Existing sandbox, trust, invalidation, malformed-row, draft-preservation, and
  no-network tests remain green.
- The complete repository quality and coverage gates pass without lowering the
  checked-in threshold floor.

## Alternatives considered

### One identical poster template

Maximum consistency, but it makes the tools visually interchangeable and
ignores the different scanning needs of chat, photos, events, and reference
material.

### Quiet rounded field kit

Calm and familiar, but too close to generic productivity software and not
recognizably Riot.

### Top segmented navigation

Compact and explicit, but the user preferred a reachable bottom toolbar. It
also leaves primary actions competing with the host header.

### Keep both global and tool bottom bars

Maintains one-tap access to global destinations but consumes substantial phone
height and makes ownership ambiguous. The focused tool screen preserves a clear
Back-to-Tools route instead.

### Arbitrary theme editor

Offers maximum expression but creates contrast, error-state, focus-ring, and
support failures. Curated offline presets provide meaningful choice while
keeping every state tested.

### Silent in-place app upgrades

Impossible under content-addressed identity without misrepresenting changed
bytes or adding cross-ID storage authority. Explicit v2 apps preserve the
current security and data model.
