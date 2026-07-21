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

## Prioritized use cases

### UC1: Do useful work without orientation overhead

**WHO:** a new or returning community member
**WANTS:** to open a built-in tool, recognize its first useful action, move
between its real views, and return to Tools
**SO THAT:** they can coordinate without learning eight navigation systems
**WHEN:** the device is online or completely offline

The host supplies location and exit; the bottom toolbar supplies the tool's
destinations and actions. The first actionable content begins within 96 CSS
pixels of the WebView's top edge at the 390×844 reference viewport, excluding a
required error/read-only notice. A useful row or empty-state action is visible
without scrolling.

### UC2: Create, edit, cancel, and recover

**WHO:** a member contributing an item, event, decision, message, dispatch,
wiki edit, or photo
**WANTS:** contextual Cancel and Save/Post/Share actions with draft-safe failure
behavior
**SO THAT:** a slow write, rejected write, or accidental route does not make
their work disappear
**WHEN:** identity is ready, delayed, unavailable, or invalidated mid-use

Pending writes disable duplicate submission. Recoverable failure preserves the
exact draft. Trust invalidation still closes the runtime and does not claim to
preserve an in-memory draft after the sandbox is destroyed.

### UC3: Choose a personal offline appearance

**WHO:** a person using Riot on a shared or personal device profile
**WANTS:** to preview and select one tested tool theme
**SO THAT:** the microapp suite feels comfortable and expressive to them without
changing anyone else's view
**WHEN:** no tool is mounted and regardless of internet availability

The picker explains that the selection applies the next time a tool opens. The
choice is isolated by an opaque local profile scope and is not community policy.

### UC4: Keep using an existing v1 tool

**WHO:** a person whose community already has v1 tool data
**WANTS:** to distinguish the redesigned v2 tool from the launchable legacy
version and return to either intentionally
**SO THAT:** an empty v2 namespace is never mistaken for deleted v1 data
**WHEN:** Riot is upgraded to a build containing the v2 suite

Existing profiles are never auto-upgraded. Current and legacy versions are
visibly separated, and the v2 confirmation names the separate-data boundary
before installation or trust.

### UC5: Navigate with assistive technology

**WHO:** a keyboard, switch-control, screen-reader, large-text, high-contrast,
or reduced-motion user
**WANTS:** early access to tool navigation, explicit current state, readable
labels, and stable focus after transitions
**SO THAT:** a long chat, gallery, or wiki index does not trap them before the
bottom controls
**WHEN:** using any supported platform, viewport, color scheme, or software
keyboard state

The toolbar is visually bottom-aligned but precedes the unbounded content list
in sequential focus order. Landmark navigation and a focus-visible “Skip to
content” link make both regions directly reachable.

### UC6: Understand and recover from local state

**WHO:** any member opening a tool whose data is empty, filtered empty, loading,
read-only, malformed, or failing to write
**WANTS:** consistent language and an action that matches the actual recovery
path
**SO THAT:** local-first behavior feels dependable rather than mysterious
**WHEN:** startup, identity resolution, synchronization, filtering, or a write
changes what can safely happen

Shared state components distinguish “nothing has been added” from “nothing
matches this filter,” never imply that offline is an error, and never expose raw
bridge or protocol failures.

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
  `+` controls are not the shipping accessibility contract. The shared icon
  catalog uses fixed 18×18 SVG path data, a two-pixel `currentColor` stroke,
  square caps/joins, no external references, and no user-controlled attributes.
  The helper constructs SVG nodes with `createElementNS`; it never uses
  `innerHTML`. The closed catalog contains list, check, add, calendar, person,
  ballot, pencil, latest, book, camera, save, cancel, send, and back glyphs.
  Icons are decorative beside their visible labels.
- Targets are at least 44 by 44 CSS pixels and never depend on hover.
- Destination groups use a `<nav aria-label="Tool views">`; contextual actions
  use a separate `<div role="toolbar" aria-label="Tool actions">`. The current
  destination has `aria-current="page"`, a visible ink notch, and text weight in
  addition to color.
- The primary create/write/add action uses the theme's action role.
- Two to four items are allowed. Tools do not receive meaningless tabs merely
  to fill the bar.
- The visually bottom-aligned navigation/toolbar precedes the potentially
  unbounded content list in DOM focus order. A focus-visible “Skip to content”
  link follows it, and `<main>` is a named landmark. View changes focus the new
  view heading; cancellation returns to the originating action.
- The toolbar stays in the same location on phone and desktop. Wide tools may
  retain split content layouts above it; they do not turn the toolbar into an
  unrelated sidebar navigation system.
- At 200% browser zoom or maximum supported platform text size, labels remain
  visible in one toolbar row; its height may grow from 48 to 64 CSS pixels. It
  never horizontally scrolls.

### Contextual views

Root navigation yields to contextual actions when appropriate:

- a detail view exposes its root/list destination and relevant action;
- a create/edit view exposes Cancel and Save/Publish;
- Save/Publish is disabled until valid and while a write is pending;
- Cancel never destroys a draft while a write is pending; and
- a failed write keeps exact form input and returns focus to a useful field.

Root navigation uses `position: fixed` with safe-area padding. Chat's composer
and create/edit action bars use `position: sticky` at the end of the form so a
software keyboard cannot cover the active field or submit controls. The real
iOS and Android keyboard-open matrix is a blocking manual/device check; a
platform that does not resize the WebView must scroll the focused field and
sticky actions into the visible viewport rather than overlay them.

The macOS app breadcrumb remains an additional ancestor route for nested pages.
It does not replace local contextual actions that phone, Android, keyboard, and
screen-reader users need.

## App-by-app information architecture

The toolbar changes must be backed by real, useful behavior rather than visual
decoration.

| Tool | Exact state model | App character |
| --- | --- | --- |
| Checklist | Root defaults to **All**; **Open** and **Done** are exclusive filters; **Add** opens create. Create shows **Cancel** and **Add item**. Pending changes Add item to **Adding…**. Failure preserves text and exposes **Try again**. Completion that removes a row from the current filter focuses the next row or the filter-empty heading. | Pink tick stamps, dense task rows, immediate completion state |
| Needs & Offers | Root defaults to **Needs**; **Offers** is the other exclusive lane; **Add** opens a form whose first required choice is Need or Offer. Create shows **Cancel** and **Post item**, then **Posting…** while pending. Phone and desktop use the same one-lane destination model; desktop widens the selected lane rather than displaying an ambiguous two-lane current state. | Exchange-board rhythm; pink needs and quiet-green offers |
| Events | Root defaults to **Upcoming**; **Going** filters events containing the current person's RSVP; **Add** opens create. Create shows **Cancel** and **Save event**, then **Saving…**. RSVP writes remain row actions and do not change the toolbar. An RSVP removed in Going uses the same next-row/filter-empty focus rule. | Calendar/date blocks and ticket-like left rules |
| Decisions | **Vote** is the sole root/current destination even when no question exists; **Ask** is an action, not a second destination. Ask shows **Cancel** and **Post question**, then **Posting…**. Casting/changing a vote remains in Vote and announces the updated selection/tally. | One dominant question and honest tally bars |
| Chat | The root has no fake destination tabs. Its shared bottom dock contains **Latest**, a labeled message field, and **Send**; Latest scrolls/focuses the newest message. The sticky dock moves above the software keyboard. Pending changes Send to **Sending…**; failure preserves the exact message and exposes **Try again**. | Dense conversational strips; no invented People screen |
| Dispatches | Root defaults to **All**; **Mine** is an exclusive author filter; **Write** opens compose. Detail shows **All dispatches** as ancestor, **Read** as current, and **Write** as action. Compose shows **Cancel** and **Publish**, then **Publishing…**. A successful publish opens Read on the new dispatch. | Editorial rules and broad reading cards |
| Wiki | Root defaults to **Pages**; **Recent** changes the index order to most-recently updated. On phone a selected page shows **Pages** as ancestor, **Read** as current, and **Edit** as action. On desktop the index remains visible beside detail; Pages/Recent controls only index ordering and the selected row, while **Edit** remains contextual. Edit shows **Cancel** and **Save**, then **Saving…**. The app breadcrumb and Pages action both enter explicit index state without wide-layout rebound. | Index-card edges and desktop list/detail split |
| Photo Wall | Root defaults to **Photos**; **Mine** filters by current author; **Add** opens the picker/caption form. Create shows **Cancel** and **Share photo**, with **Preparing…** during local image work and **Sharing…** during the write. Failure preserves the prepared image and caption while the runtime remains mounted. | Slightly irregular framed grid with captions always available |

`Open`, `Done`, `Going`, `Mine`, and `Recent` are local projections of already
available app rows. They do not write new shared metadata. Every root filter
keeps its own scroll position for the current mount. When a write or sync moves
the selected row out of that filter, focus moves to the next row, previous row,
or filter-empty heading in that order. `Latest` scrolls and focuses the newest
message; it is an action, not a fake second message store.

Content widths prevent both unreadably long lines and today's empty desktop
expanse: forms cap at 640 CSS pixels, prose/detail at 720, lists/boards at 880,
Wiki's split workspace at 1100, and Photo Wall at 1200. Narrow layouts use the
full available width.

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
- Its storage key includes a new opaque `appearanceProfileID`: a lowercase
  canonical UUID generated from platform secure randomness when a local profile
  is created or first migrated. It is at most 36 ASCII characters and is never
  derived from a community, Willow author/subspace/signing identifier, display
  name, filesystem path, or device advertising identifier.
- Profile reset/deletion creates a new `appearanceProfileID` and removes the old
  preference entry on a best-effort basis. Importing or joining a community does
  not change it. Existing profiles receive one ID atomically before the picker
  can write a preference.
- Riot itself never writes the preference to Willow, an app-data namespace, a
  bundle, diagnostics, a URL, or peer sync. A themed WebView necessarily learns
  the active theme well enough to render it, so injection is restricted to the
  exact eight reviewed v2 IDs and static/runtime audits prove that their code
  never persists the key. The theme is low-sensitivity personalization, not a
  secret from the mounted reviewed tool.
- DOM storage remains disabled.
- An absent, malformed, or retired value resolves to `night-garden`.
- Frozen v1 and arbitrary third-party apps receive no theme injection and keep
  their original presentation.

The picker lives in Tools as **Tool appearance**, outside a mounted runtime.
Choosing a preset therefore cannot reload a WebView or discard an in-memory
draft. The next tool mount uses the new theme. A future live picker is outside
scope unless it preserves exact drafts without broadening the bridge.

The picker is one radio group in reading order. Each option shows its name, four
to six swatches, and the same representative toolbar/card/status preview so
comparisons are honest. Arrow keys change the preview selection; **Use this
theme** persists it, announces “<name> will be used the next time you open a
tool,” and returns to Tools. **Cancel** discards the preview. **Reset to Night
Garden** selects the default but does not persist until Use this theme. The
current stored choice is checked and labeled “Current”; Night Garden is labeled
“Default.” The screen remains available when no tool is installed.

### Presets

All presets preserve the same neutral paper/ink system and semantic roles. The
complete shipping values are fixed here rather than deferred to implementation.

| Neutral role | Light | Dark |
| --- | --- | --- |
| Paper | `#ECE7DB` | `#17160F` |
| Surface | `#F8F4E9` | `#242219` |
| Ink | `#17160F` | `#F5F0E4` |
| Soft ink | `#5F594F` | `#C8C1B4` |
| Line | `#17160F` | `#8D8679` |

Light preset values:

| Theme | Structure / on | Action / on | Quiet / on | Signal / on | Focus |
| --- | --- | --- | --- | --- | --- |
| Night Garden (default) | `#642B58` / `#FFFFFF` | `#D1216E` / `#FFFFFF` | `#AEB8A0` / `#17160F` | `#E9F056` / `#17160F` | `#642B58` |
| Repair Picnic | `#351E28` / `#FFFFFF` | `#FF5C34` / `#17160F` | `#AEB8A0` / `#17160F` | `#D3B86A` / `#17160F` | `#351E28` |
| Living Network | `#006B62` / `#FFFFFF` | `#D1216E` / `#FFFFFF` | `#B8DFBD` / `#17160F` | `#D5A83F` / `#17160F` | `#006B62` |
| Deep Amaranth | `#642B58` / `#FFFFFF` | `#D1216E` / `#FFFFFF` | `#C9AEC0` / `#17160F` | `#E8B4CE` / `#17160F` | `#642B58` |
| Signal Chartreuse | `#C8E63C` / `#17160F` | `#D1216E` / `#FFFFFF` | `#DDE8A3` / `#17160F` | `#F0A7C7` / `#17160F` | `#D1216E` |
| Burnt Tomato | `#E94B35` / `#17160F` | `#D1216E` / `#FFFFFF` | `#F1B2A5` / `#17160F` | `#E5B54B` / `#17160F` | `#E94B35` |

Dark preset values:

| Theme | Structure / on | Action / on | Quiet / on | Signal / on | Focus |
| --- | --- | --- | --- | --- | --- |
| Night Garden (default) | `#B982AD` / `#17160F` | `#E45A96` / `#17160F` | `#7F8975` / `#17160F` | `#E9F056` / `#17160F` | `#E9F056` |
| Repair Picnic | `#B07A91` / `#17160F` | `#FF7A57` / `#17160F` | `#909A84` / `#17160F` | `#D7BF72` / `#17160F` | `#FF7A57` |
| Living Network | `#4FB5A9` / `#17160F` | `#E45A96` / `#17160F` | `#8FC99B` / `#17160F` | `#E0B94C` / `#17160F` | `#8FC99B` |
| Deep Amaranth | `#B982AD` / `#17160F` | `#E45A96` / `#17160F` | `#96778E` / `#17160F` | `#D993B2` / `#17160F` | `#B982AD` |
| Signal Chartreuse | `#D8ED5A` / `#17160F` | `#E45A96` / `#17160F` | `#9DAA67` / `#17160F` | `#E79BBD` / `#17160F` | `#D8ED5A` |
| Burnt Tomato | `#F06A50` / `#17160F` | `#E45A96` / `#17160F` | `#B9786B` / `#17160F` | `#E0B94C` / `#17160F` | `#E0B94C` |

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
--riot-on-quiet
--riot-signal
--riot-on-signal
--riot-focus
```

Text/background pairs meet WCAG AA. Focus indicators and non-text control
boundaries meet at least 3:1 against adjacent colors. Status, selection,
completion, and error are never conveyed by color alone.

### Host-to-WebView theme contract

The host maps stored values to a closed native enum and injects only the enum's
canonical key for an exact generated allowlist of the eight v2 IDs. The Riot
bridge and theme scripts are both registered before navigation; the theme
script runs first, sets the attribute before app-authored script, and the
bundled stylesheet contains a static Night Garden fallback so first paint never
flashes another person's theme. The document root receives a value such as:

```html
<html data-riot-theme="night-garden">
```

The page can observe that value and its computed colors, but cannot supply
script to the host, choose arbitrary CSS, read the native preference store, or
learn any profile identifier. An unrecognized value is never injected. Origin
mismatch, unsupported document-start injection, teardown, or profile switching
fails closed to the page's static Night Garden CSS. The operation adds no
message handler, scheme, storage API, or network capability.

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

Every render-critical byte travels with either the content-addressed app bundle
or Riot's immutable, versioned host asset pack. Nothing is fetched from the
network. A bundle always includes system-font fallbacks and remains usable on
an older host that does not recognize the optional asset-pack path.

### Canonical shared source

`fixtures/apps/_shared/` remains the authored source of truth for:

- semantic tokens and all theme presets;
- base typography and versioned host-font declarations with system fallbacks;
- bottom-toolbar and common control styles; and
- the small unprivileged helper used for toolbar state, theme fallback, and the
  root-navigation event.

The pack/check workflow copies or generates deterministic app-local assets
before encoding each content-addressed bundle. Contract tests compare every
app-local copy to the canonical source so the suite cannot drift through manual
copy/paste.

App-specific CSS and JavaScript remain inside the app directory. Shared code
does not gain app-data access beyond calls each app already makes explicitly.

### Immutable host font pack and CSP

Fonts are not duplicated into eight content-addressed bundles. Riot exposes one
immutable host-owned `RiotToolFonts.v1` pack at reserved same-origin paths such
as `/.riot/fonts/Anton-Regular.ttf`. App packers reject the reserved `/.riot/`
prefix, so bundle content can never shadow host assets. The resolver serves a
font only when:

1. the mounted app ID is in the generated allowlist of the exact eight reviewed
   v2 IDs;
2. the normalized request path exactly matches one of four catalog entries;
3. the packaged byte count and SHA-256 match the catalog; and
4. the response MIME is exactly `font/ttf`.

The v1 pack reuses the exact TTF bytes already shipped by Riot:

| Host path | Bytes | SHA-256 |
| --- | ---: | --- |
| `/.riot/fonts/Anton-Regular.ttf` | 170,812 | `a4ba3a92350ebb031da0cb47630ac49eb265082ca1bc0450442f4a83ab947cab` |
| `/.riot/fonts/WorkSans-Variable.ttf` | 361,072 | `f50f61f2ba738e239442d40bf1069adb195c224b6a5a73a581fc2f3ed62a9f63` |
| `/.riot/fonts/SpaceMono-Regular.ttf` | 99,356 | `95837e182baeeada83368f7748db28357f0a1b75c6b84ff7065b5edf933c8e18` |
| `/.riot/fonts/SpaceMono-Bold.ttf` | 98,232 | `405e73d41afb7e5906efce206a326af5c956f38e255f35421c260e861e599c59` |

Total host-font bytes are exactly 729,472. Their Google Fonts source commit,
SIL OFL 1.1 license hashes, copyright lines, and acquisition/license review
contract are inherited from the independently reviewed font table in
`2026-07-20-riot-offline-guides-design.md`; those complete license files and
third-party notices must land before the WebView font path ships.

Only exact allowlisted v2 IDs receive a CSP with `font-src 'self'`. Frozen v1
and arbitrary third-party apps keep the existing CSP where `default-src 'none'`
therefore implies `font-src 'none'`. Apple, Android, and preview resource
responses add `X-Content-Type-Options: nosniff`. The existing blocked network
loads, exact app-origin checks, navigation refusal, disabled file/content
access, and JavaScript bridge boundary remain unchanged.

The host pack is versioned and immutable: changing any font byte, catalog
entry, MIME, or license mapping requires `RiotToolFonts.v2` and a new reviewed
path. CSS falls back to `Impact`/`Arial Narrow`, `system-ui`, and `ui-monospace`
if the v1 path is unavailable; typography may soften, but navigation and data
remain usable offline.

## Content identity and upgrades

Changing any HTML, CSS, JavaScript, font, manifest, or bundle byte produces a
new app ID. App data is stored under `apps/<app_id>/...`; a new version therefore
does not automatically see the old version's rows.

The suite ships as version 2 rather than silently pretending the redesign is
the same app. Two catalogs have different responsibilities:

- `CURRENT_STARTER_CATALOG` contains only the eight v2 pairs. It is advertised
  in the directory and auto-installed only for a newly created generation-2
  local profile.
- `LEGACY_BUILTIN_CATALOG` contains the exact eight v1 pairs shipped before this
  redesign. It is never advertised as a starter or assigned a synthetic
  directory timestamp. It exists only to resolve an already-held v1 ID for a
  generation-1/existing profile.

The generated catalog report is authoritative. For every visible app name it
records v1 and v2 source/artifact slug, semantic version, app ID, manifest and
bundle SHA-256, encoded size, resource count, current/legacy membership, theme/
font capability, and data namespace prefix. Runtime selection and authorization
always use the complete app ID, never name or semantic version.

The checked-in v1 side of that report begins with these current derived IDs.
V2 IDs are deliberately generated only after their source and deterministic
bundle exist; a plan or developer may not invent or pre-pin them.

| Visible name | V1 source/artifact | V1 app ID | V2 source → artifact | Versions |
| --- | --- | --- | --- | --- |
| Checklist | frozen `checklist` | `3fe5f89af18d9244756c8925750280f0c51479030cf3cd7b4d26940b51eaa4b7` | `v2/checklist` → `checklist-v2` | 1.0.0 → 2.0.0 |
| Needs & Offers | `supply-board` | `05200e07ca8c11da106366dbe2f7386dc11826aa723479352a916158ac649ac8` | `v2/supply-board` → `supply-board-v2` | 1.0.0 → 2.0.0 |
| Events | `roll-call` | `266b7978d2bcd143d7b93b6246884c85343ca4b6e4bb4aa406dbf8d87e39d382` | `v2/roll-call` → `roll-call-v2` | 1.0.0 → 2.0.0 |
| Decisions | `quick-poll` | `36a4c50030b5dbac3e84d40c503b6413e2b39b276f6010215e87c29c96453d1a` | `v2/quick-poll` → `quick-poll-v2` | 1.0.0 → 2.0.0 |
| Chat | `chat` | `6a5cadd381460f15b871cf898b59a4db97d5ddb80130cef335136c619bacdfac` | `v2/chat` → `chat-v2` | 1.0.0 → 2.0.0 |
| Dispatches | `dispatches` | `848a8e1551f34a1443eb1c1dc6601b730db413eee500a49695c8956cac5f2459` | `v2/dispatches` → `dispatches-v2` | 1.0.0 → 2.0.0 |
| Wiki | `wiki` | `c2a54df288701afe8ed95e91af8fafec34a56d9132cde914b9ec76ce826ac714` | `v2/wiki` → `wiki-v2` | 1.0.0 → 2.0.0 |
| Photo Wall | `photo-wall` | `ae1ac55cfe563dab67c4139ff2fc84fa59647e75848ffaa0132ef1110ff8066b` | `v2/photo-wall` → `photo-wall-v2` | 1.0.0 → 2.0.0 |

The seven non-Checklist v1 pairs are copied byte-for-byte into
`fixtures/apps/legacy/` before their authored v2 sources are introduced.
Checklist keeps its existing frozen paths. V2 source lives under
`fixtures/apps/v2/<slug>/`, preventing any redesign task from editing a legacy
source directory by accident.

Existing persisted profiles without `starterCatalogGeneration` migrate to
generation 1. They reinstall their legacy starter pairs from the non-advertised
resolver on bootstrap and are never auto-installed or auto-trusted into v2.
Fresh profiles record generation 2 and receive only v2. Android profiles that
already carry exact v1 bytes keep those bytes; the legacy resolver is a verified
fallback, not a replacement for persisted user-held content.

The current v2 directory listing is still visible to an existing organizer as
an optional redesign. Installing it starts a distinct app-data namespace. No
host-side copying, aliasing, name-based lookup, or cross-ID read permission is
introduced. A future explicit, reviewed migration design may address cross-
version data.

The FFI installed-app cap increases from 16 to 32, matching Android's existing
persisted-profile cap. This bounded change permits eight v1 starters, eight v2
tools, and up to sixteen carried/custom apps during transition. Installation at
32 fails before mutation with the existing plain-language capacity error; Riot
does not quarantine valid existing apps to make room. Boundary tests cover
31→32 success, 32→33 refusal, restart, and mixed legacy/current/custom ordering.
Current starters install first only for generation-2 profiles, required legacy
pairs install first only for generation-1 profiles, then carried apps restore in
their persisted order.

### Existing-user presentation

Tools never presents two unlabeled cards with the same name.

- Current v2 cards are labeled **Redesigned · Version 2** and sort in the normal
  Tools group.
- A v1 app appears under a collapsed but clearly counted **Legacy tools with
  existing data** section when that profile holds its ID, trust, or app rows.
  Its card and runtime title use `<name> · Legacy 1`.
- The v1 card remains launchable and offers no theme claim.
- A v2 action is **Install redesigned version**, not Update.
- Before install/trust, the confirmation says: “This is a separate version.
  Your <name> Legacy 1 information stays there and will not appear here. You can
  return to Legacy tools at any time.”
- Cancel leaves v1 mounted/trusted state untouched. Confirm installs v2 through
  the ordinary verification path and then uses the existing organizer trust
  review; it does not revoke v1.
- An empty v2 state repeats once, without alarm styling: “This redesigned tool
  starts separately. Your Legacy 1 information is still available in Tools.”
- If capacity is full, the confirmation makes no change and explains that Riot
  can keep at most 32 installed tools on this profile. Removal/archive UX is a
  separate design; this work never silently discards a tool.

Checklist is a special compatibility boundary. Its existing source, manifest,
bundle, and app ID remain byte-for-byte frozen. The unified Checklist is a new
source/artifact slug and version. Existing demo/community data pinned to the old
Checklist ID remains valid; fresh demo/catalog data must pin the v2 ID only when
the fixture intentionally uses Checklist v2.

The other seven v1 pairs remain packaged only in `LEGACY_BUILTIN_CATALOG`, not
the current starter catalog. A previous-release profile must open every trusted
v1 app with its original data after app upgrade and restart before release.

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

### Shared state components and language

State occupies the content region immediately below the optional expressive
heading; it never creates another page header. `role="status"` is polite for
loading, identity readiness, successful writes, and filter changes.
`role="alert"` is assertive only for a failed requested action. Repeated paint
cycles do not re-announce identical text.

| State | Placement and exact core language | Actions and focus |
| --- | --- | --- |
| Initial load | Inline status: **Loading shared information…** Existing cached rows render as soon as available; no skeleton implies remote network progress. | Root toolbar is present; mutation actions disabled. No focus steal. |
| Delayed identity | Compact persistent status above content: **Getting your local identity ready… You can read while Riot gets contribution tools ready.** | Read/filter actions work; mutations disabled. Polite announcement once. |
| Identity failure/read-only | Hard-bordered notice: **Read-only: Riot couldn't verify your identity on this device. Your shared information is still here.** | **Try again** reruns the app's identity initialization once per activation; **Back to Tools** remains native. Focus moves to the notice only after a user-requested retry fails. |
| Globally empty | App-specific noun in the shared pattern: **Nothing here yet.** Supporting line explains the first useful contribution. | The create/add action is repeated in content and bottom actions; one is removed from sequential focus with `aria-hidden`/noninteractive duplication rules so it is announced once. |
| Filter empty | **No <filter> items here.** It never says the underlying tool is empty. | **Show all** returns to the default root filter and focuses its heading. |
| Write pending | Inline form status: **Saving…**, **Posting…**, **Sending…**, **Publishing…**, **Preparing…**, or **Sharing…** as specified by the app state table. | Submit and Cancel are disabled only for the irreversible pending interval; other duplicate mutation paths are locked. Focus stays on the submit control. |
| Write success | Brief polite status using the visible noun, for example **Event saved.** | Focus moves to the new/updated row or its detail heading. |
| Write failure | Inline alert: **Couldn't <save/post/send/share> that. Your draft is still here. Try again.** Raw errors are never shown. | **Try again** is the same guarded submit path; focus moves to the alert, then the next Tab reaches the first invalid/retry-relevant field. Exact fields and prepared photo remain. |
| Malformed synchronized row | No per-row warning and no raw payload. Valid rows continue. | If all rows are rejected, the normal global/filter empty state appears; diagnostics count only a bounded category code, never content. |
| Selected row deleted by sync | Return to the valid root/filter unless an active edit conflict already requires draft preservation. | Focus moves to the root heading and a polite **That item is no longer available.** status. Wiki's existing editing-conflict exception remains authoritative. |

Offline operation is the normal state and receives no warning banner. These
tools have no remote fetch whose absence needs to be “reconnected.”

## Accessibility

- Body text supports platform text scaling and browser zoom without horizontal
  page scrolling at 320 CSS pixels.
- Display type is limited to short phrases and is never required to understand
  an action.
- Every toolbar item has visible text, an accessible name, and a 44-pixel target.
- Current view, selected vote, completion, RSVP, and errors have text/icon/state
  semantics in addition to color.
- Focus order follows host ancestors, tool navigation/actions, the skip link,
  then page content unless a completed transition intentionally moves focus to
  its heading, alert, or first invalid field.
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
- Theme preference stays in device-local native preferences. Only the exact
  reviewed v2 tools receive its low-sensitivity enum key; static/runtime audits
  prohibit those tools from writing it to app data. Riot makes no impossible
  claim that a page can render a theme without observing its colors/key.
- The theme contract accepts one allowlisted enum value for one allowlisted v2
  app ID and no arbitrary CSS or script. The opaque appearance-profile UUID is
  never injected.
- The root-navigation contract sends one constant, payload-free DOM event.
- Document titles remain bounded, untrusted display strings and are not logged.
  They confer no authority; a reviewed or hostile app can perform writes within
  its already-granted namespace when any page event fires, so the native crumb
  is not represented as a transaction-free security boundary.
- User-generated text is built with DOM text nodes/`textContent`, never
  `innerHTML`, dynamic SVG, CSS, URL, or event-handler strings. Static and
  runtime hostile-row tests cover each sink class.
- URL, file, content, service-worker, pop-up, and external-navigation defenses
  remain in place.
- `font-src 'self'` is emitted only for allowlisted v2 IDs and exposes only the
  four immutable host-owned catalog hashes under the reserved path. V1 and
  third-party CSPs do not change. All resource responses use `nosniff`.
- A v2 app never gains implicit access to a v1 namespace.
- Theme keys, appearance-profile IDs, titles, drafts, and community content are
  excluded from console forwarding, crash annotations, screenshot fixtures,
  diagnostic text, and test artifact names. Visual evidence uses synthetic
  profiles and is inspected before commit.

## TDD and verification strategy

Implementation is divided into red-green-refactor slices. Each slice begins
with a failing test and stays within the file scope declared by the later
implementation plan.

### 0. Breadcrumb prerequisite

Reconcile the compact breadcrumb baseline from design commit `f9d3d58` in the
`overnight-2026-07-22` worktree before changing the tool host. The family spec
is authoritative only for its deliberate iPhone tab-bar amendment and merged
activity indicator; the breadcrumb parser, title bounds, chooser safety,
teardown, root-event, emoji fallback, and close-before-route tests remain intact.

### 1. Legacy continuity and inventory

RED tests open a previous-release generation-1 profile after restart and prove
all eight v1 tools remain launchable with original IDs/data while no v2 app is
auto-installed or trusted. Boundary fixtures combine eight legacy apps, eight
current apps, and 15/16 custom apps to exercise 31→32 and 32→33. GREEN adds the
non-advertised legacy resolver, starter-generation migration, 32-app FFI cap,
current/legacy presentation descriptors, and generated inventory report.
REFACTOR keeps catalog lookup, authorization, and storage keyed only by app ID.

### 2. Shared visual, theme, and host-font contracts

RED contract tests define the complete semantic token tables, Night Garden
fallback, theme picker reducer/store, `appearanceProfileID` lifecycle, reserved
font path, exact font hashes, per-ID CSP selection, `nosniff`, shared asset
drift, focus/target/reduced-motion requirements, and duplicated-header ban.
GREEN adds canonical assets, preference stores, theme picker, allowlist report,
host-font resolver, and document-start injection. REFACTOR keeps one typed theme
descriptor per native platform and one canonical CSS source.

### 3. Events vertical pilot

Events is the non-frozen pilot because it exercises a root filter, current-user
projection, row mutation, create form, pending/error recovery, date layout, and
native host. Before another app changes, Events v2 must prove end to end:

- deterministic pack and generated ID/report;
- current/legacy coexistence and separate namespace;
- all six light/dark themes and static fallback;
- exact-v2-only theme/font capability;
- Upcoming/Going/Add and Cancel/Save state machines;
- toolbar focus/zoom/safe-area/software-keyboard behavior;
- compact host, hidden/restored phone tabs, breadcrumb integration, title/root
  event, and trust invalidation; and
- rollback by returning the current catalog to the prior artifact without
  affecting the legacy resolver or data.

### 4. One app per reviewed work unit

After the pilot passes, migrate Supply Board, Quick Poll, Chat, Dispatches,
Wiki, Photo Wall, and finally Checklist v2 in separate red-green-refactor work
units. Each unit adds its exact state-machine tests before source changes,
repacks only its v2 artifacts, updates the inventory/allowlist, runs the complete
pilot regression set, and receives adversarial review. Checklist v1 source and
artifacts are never an edit target.

### 5. Deterministic preview fixtures

The preview URL accepts only validated test enums:

```text
?state=<state>&theme=<theme>&scheme=<light|dark>&view=<view>
```

Unknown values fall back to seeded/Night Garden/system-root and are never copied
into HTML. Fixtures include:

- `seeded`, `initialized-empty`, `filter-empty`, `delayed-identity`,
  `identity-error`, `interrupted-seeding`, `malformed`, `slow-write`, and
  `write-error`;
- `seeded-self`, containing current-user rows for Going/Mine and both Open/Done;
- root, every filter, detail, compose, edit, pending, deleted-selection, and
  Wiki edit-conflict views;
- all six themes in light and dark;
- duplicate taps, profile switch, malformed theme, origin mismatch, repeated
  root event, and trust revocation during a pending write; and
- generation-1 legacy-only, generation-2 current-only, coexistence, and
  capacity-full profiles.

Seed data uses full synthetic IDs and deterministic timestamps. No fixture
contains a real community title, person, draft, photo metadata, or profile key.

### 6. Visual and assistive review

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

Real-device checks add software keyboard open, 200%/maximum text, long labels,
screen reader, keyboard-only traversal, narrow macOS breadcrumb, and safe-area
variations. Captures and traces use synthetic data, are retained on failure for
seven days in CI, and are inspected before any artifact is committed.

### Blocking commands and CI ownership

`package.json` gains:

```json
{
  "test:apps:contracts": "node scripts/apps/miniapp-contracts.mjs",
  "test:apps:browser": "playwright test --config scripts/apps/playwright.config.mjs",
  "test:apps": "npm run test:apps:unit && npm run test:apps:contracts && npm run test:apps:browser"
}
```

`.github/workflows/ci.yml` adds a blocking `miniapps` job on Ubuntu with Node
26.4.0, `npm ci`, the pinned Playwright 1.61.1 Chromium install, a 15-minute job
timeout, `npm run test:apps`, and screenshot/trace upload on failure with seven-
day retention. Unit/contracts run before browser tests so structural failures
fail cheaply. Apple real-device/simulator visual work remains a recorded
protected local release gate because native CI is not present in this checkout;
it cannot be skipped for release.

The complete local gate is:

```text
npm run test:apps
scripts/apps/repack-starter.sh
sh scripts/ios-check.sh fast
sh scripts/ios-check.sh test
sh scripts/ios-check.sh sim
sh scripts/ios-check.sh ios
(cd apps/android && ./gradlew :app:testDebugUnitTest)
(cd apps/android && ./gradlew :app:connectedDebugAndroidTest)
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace --all-features
scripts/web/coverage.sh
```

`.coverage-thresholds.json` remains the coverage source of truth and floors may
not be lowered without committed justification. Browser timeout reports name
the app/state/theme/view tuple; no test silently retries past a deterministic
failure.

## Size and performance budgets

All are blocking release budgets measured from clean local state:

| Budget | Limit |
| --- | ---: |
| One encoded v2 app bundle | ≤ 256 KiB and ≤ 24 resources (below canonical 1 MiB/32 limits) |
| Complete eight-app v2 catalog | ≤ 2 MiB encoded |
| Immutable host-font pack | exactly 729,472 bytes; no additional font bytes |
| Current + legacy catalogs + host-font app-size growth | ≤ 3 MiB per native package |
| One peer-shared v2 manifest/bundle pair | ≤ 256 KiB |
| Installed-profile app bytes | ≤ 32 MiB under the 32-app/1 MiB-per-app hard bounds |
| Local cold mount to first useful paint | p95 ≤ 750 ms |
| Theme/toolbar script before app script | no unthemed first paint in 100 repeated cold mounts |
| Additional peak private memory for one mounted tool | ≤ 32 MiB above pre-mount baseline |

Timing and memory are recorded on the oldest supported iPhone/simulator class,
oldest supported macOS hardware class available to release testing, and a low-
end supported Android device. A regression over budget blocks release or
requires a new reviewed design; it is not waived by a fast development machine.

## Moderated usability gate

Before release, at least eight representative participants complete the core
journeys with the network disabled: at least four phone and four desktop
sessions, including at least two screen-reader/keyboard sessions and two at
maximum text size (participants may satisfy more than one category).

Pass criteria:

- at least 90% return to Tools and identify the current toolbar view within five
  seconds without prompting;
- at least 90% locate the first useful action within five seconds;
- at least 85% complete and cancel a representative create/edit flow without
  prompting;
- 100% of recoverable write-failure attempts retain the exact supplied draft;
- at least 90% change the theme while offline and correctly state when it takes
  effect and who can see it;
- 100% of existing-user participants distinguish Legacy 1 from Version 2 after
  reading the install warning and correctly locate their original data; and
- zero participants believe selecting a theme changes peers, that v2 erased v1
  data, or that an offline tool is waiting for a server.

Unsupported attempts remain in the denominator. Timeouts, abandonment,
prompts, wrong-version launches, clipped controls, inaccessible focus, or draft
loss are recorded as failures. The same synthetic profile containing real v1
rows and empty v2 rows is used for the coexistence journey.

## Acceptance criteria

- All eight redesigned v2 starter tools use one Riot visual family and retain a
  clear app-specific character; each encoded bundle is at most 256 KiB and 24
  resources, and the full v2 catalog is at most 2 MiB.
- No mounted microapp repeats its app name or renders an oversized three-line
  intro header.
- At the 390×844 reference viewport, first actionable content starts within 96
  CSS pixels of the WebView top and a useful row or empty-state action is visible
  without scrolling.
- Phone tools hide Riot's global tab bar while mounted and restore it on exit.
- macOS shows the approved compact community/app/page breadcrumb and no
  oversized Close row.
- Host location and app navigation fit into one compact top row plus one bottom
  toolbar; no stacked bottom navigation bars appear.
- Every meaningful root, filter, detail, and create/edit state has an obvious
  route, the exact toolbar behavior in this spec, and an accessible current-
  state indication. Tool controls precede unbounded content in focus order.
- Night Garden is the default, and all six themes can be chosen under Tool
  appearance while offline using the specified preview/commit/cancel/reset
  interaction. Arbitrary custom colors are not accepted.
- Theme choices are scoped by an opaque local appearance profile, do not sync,
  and do not affect what peers see. Only exact reviewed v2 IDs receive the
  low-sensitivity theme key; frozen v1 and third-party apps receive no theme.
- Riot does not persist or propagate the theme through Willow or app data, and
  static/runtime audits prove the eight reviewed v2 tools do not call the bridge
  with the theme key. The mounted tool is correctly treated as able to observe
  its own active theme.
- Every preset passes light/dark contrast, focus, target-size, keyboard, text-
  scaling, and screen-reader checks.
- All typography and theme assets render with the network disabled. Only exact
  reviewed v2 IDs can resolve the four immutable host fonts; hashes, total
  729,472-byte size, MIME, reserved paths, CSP, and `nosniff` match this spec.
- Existing installed app versions and their data remain addressable under their
  original IDs after upgrade/restart; no silent cross-ID migration occurs and
  no existing profile auto-installs or auto-trusts v2.
- Fresh profiles receive only the v2 starter catalog. Existing profiles visibly
  separate launchable `<name> · Legacy 1` tools from `Redesigned · Version 2`,
  and the install warning states that their data namespaces remain separate.
- The shared cap is 32 installed apps; 31→32 succeeds atomically, 32→33 refuses
  before mutation, and restart preserves deterministic legacy/current/custom
  ordering.
- Checklist v1 source, manifest, bundle, and pinned app ID remain unchanged.
- v2 source and committed artifacts repack deterministically and pass the
  generated current/legacy inventory and allowlist audits.
- Existing sandbox, trust, invalidation, malformed-row, draft-preservation, and
  no-network tests remain green.
- The app-size, mount-time, first-paint, memory, peer-share, and installed-profile
  budgets in this spec pass on the named release device classes.
- The moderated offline usability study meets every threshold in this spec,
  including 100% draft retention and 100% correct Legacy 1/v2 distinction.
- `npm run test:apps` is a blocking CI job with deterministic contract/browser
  fixtures and failure evidence; the named Apple and Android checks are blocking
  release gates.
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
