# Compact Tool Breadcrumb Design

**Status:** Approved direction (2026-07-22)

**User request:** Replace the macOS tool host's oversized, ineffective Close navigation with a compact guide that shows `community › app › page`, lets a person move back through those levels, and collapses each level to emoji when horizontal space is scarce. The user confirmed that selecting the community level must open the community chooser.

## Problem

The macOS tool detail currently reserves a large row for an uppercase app name and an outlined `Close` button. It consumes disproportionate vertical and horizontal space while failing to explain where the person is. The shell also has a concrete routing defect: sidebar selections update `RiotNavigationModel.destination`, but `runningTool` remains non-nil, and the detail pane always prefers `runningTool`. A person can therefore click Home, People, or Nearby while the open tool continues to cover the selected destination.

The hosted app also has no host-visible page location. For example, Wiki changes between its page index and a particular page entirely inside JavaScript. Native SwiftUI chrome can name the app but cannot currently name “Meeting guide” or reset Wiki to its root.

## Product intent

Riot should keep people oriented inside a community without making a local-first tool feel like a modal or a browser tab. The hierarchy is deliberately human-facing:

```text
Riverside Community › Wiki › Meeting guide
```

It communicates, in order:

1. which community owns the shared data;
2. which app is operating on that data; and
3. which page inside the app is current.

The hierarchy must stay understandable without exposing app IDs, namespace IDs, URLs, or other protocol material.

## Use cases

### UC1: Switch communities from inside a tool

**WHO:** A member using a page inside a community tool
**WANTS:** to select the community crumb
**SO THAT:** they can see or switch their joined communities without first finding a Close control
**WHEN:** the tool detail is open on macOS

Selecting the community crumb opens the existing community chooser. Dismissing the chooser keeps the same mounted WebView and current page. Selecting another community while a tool is mounted first presents a conservative Stay-or-Switch confirmation: “Switch communities? Any unsaved changes in <app> will be lost.” “Stay” leaves the chooser and runtime untouched; “Switch” uses the existing community-transition path, which tears down the old runtime and switches community scope. Because the host cannot inspect an app's in-memory edit state, this guard appears whenever a different community is chosen from a mounted tool, not only when the host guesses that a draft exists.

### UC2: Return to an app's root

**WHO:** A member reading a nested app page
**WANTS:** to select the app crumb
**SO THAT:** they can return to that app's index
**WHEN:** a page crumb is present

Selecting the app crumb evaluates the fixed expression `(() => { window.dispatchEvent(new Event("riot:navigate-root")); return document.title; })()` in the existing sandboxed WebView. A nested app registers its root listener on `window`; the event never carries a payload. This does not give native code app-specific data access. At the app root the app crumb is the current level and is not presented as an enabled action. Apps without nested-page adoption continue working unchanged, so the event is an additive, backwards-compatible contract.

Wiki handles the event by explicitly selecting its page index and setting an `explicitIndex` state. That state suppresses Wiki's current wide-layout first-page auto-selection until the person deliberately opens a page. Initial wide-layout launch may still select the first page, but an explicit app-crumb action always settles at the index and does not rebound into that first page.

### UC3: Leave a tool through ordinary shell navigation

**WHO:** A member with a tool open
**WANTS:** Home, Tools, People, and Nearby to work normally
**SO THAT:** the open tool never traps or masks them
**WHEN:** they select a sidebar destination

Every macOS sidebar or Command-1 through Command-4 route selection closes the running tool before showing the selected route. Escape retains the documented behavior of returning from the tool to Tools. The existing `ToolFocusRestoration` bookkeeping is cleared through the same close path; applying the returned identifier to actual UI focus is not implemented today and is not claimed by this change. iPhone tab selection remains unchanged and continues preserving the Tools NavigationStack while another tab is temporarily selected.

The chooser's existing “Find one nearby” action is also a macOS route change. When the chooser was opened over a running tool, that action closes the tool through the same macOS close-then-route boundary before selecting Nearby, rather than calling `model.findNearby()` directly and leaving the tool masking the destination.

### UC4: Stay oriented in narrow windows

**WHO:** A member using a narrow macOS window or long community/page names
**WANTS:** the hierarchy to remain visible without clipping or wrapping
**SO THAT:** navigation uses one compact row
**WHEN:** all full labels do not fit

The breadcrumb progressively falls back as a single horizontal unit:

```text
Riverside Community › Wiki › Meeting guide
🏘 › 🧰 › 📄
```

The emoji are visual fallbacks only. Accessibility labels remain the full names and actions at every width. The row stays one line and does not horizontally scroll.

## Interaction design

### Full presentation

- Height target: 36 points, including a one-pixel bottom divider.
- Community crumb: plain text button using `community.name`; opens “Your communities”.
- App crumb: plain text button using `appName` only when a nested page is current; requests the app root through the fixed DOM event.
- Page crumb: plain current-location text; no misleading button behavior.
- Separators: subdued `›` glyphs, hidden from accessibility.
- The current level uses the normal ink color and semibold weight. Ancestor actions use the pink link color with a plain button style.
- Long labels remain single-line and are handled by the compact fallback rather than truncating identity-bearing names.

At an app root, the hierarchy is `community › app`; there is no duplicate page crumb such as `Wiki › Wiki`.

### Compact presentation

SwiftUI `ViewThatFits(in: .horizontal)` chooses between the complete names and the emoji representation based on actual available width rather than a hard-coded window breakpoint. The full candidate uses uncompressed intrinsic horizontal sizing (`fixedSize(horizontal: true, vertical: false)`), so long labels make that candidate fail instead of silently truncating or compressing it.

- Community: `🏘`, accessibility label “Choose community, current community: <name>”.
- App: `🧰`, accessibility label “Return to <app> home” when actionable, otherwise “<app>”.
- Page: `📄`, accessibility label “Current page: <page>”.

Each compact crumb also exposes macOS hover help with its complete community, app, or page name, so sighted pointer users can recover the names without resizing.

The compact row preserves the same hit targets by padding controls inside the 36-point row. Emoji do not replace accessible text.

### Existing alternatives

- The oversized Close button is removed from the macOS host.
- Escape continues to close the tool to Tools.
- iPhone keeps its existing community header and NavigationStack back behavior; duplicating the full breadcrumb there would add, rather than remove, chrome.

## Host/app location contract

The host observes the WebView's existing document title. No new privileged message-handler API is added.

- Root title: `<app name>` (for example, `Wiki`).
- Nested title: `<page name> — <app name>` (for example, `Meeting guide — Wiki`).
- A pure `AppPageLocation` parser first rejects a raw title over 512 UTF-16 code units, then scans the untrimmed raw title and rejects Unicode general categories `Cc` (control), `Cf` (format, including bidi controls), `Zl` (line separator), and `Zp` (paragraph separator). Only after those rejections does it canonicalize both title and app name to Unicode NFC, trim leading/trailing Unicode whitespace from the whole title, and match the exact ` — <NFC app name>` suffix. Forbidden scalars are therefore rejected at the beginning, middle, and end rather than disappearing during whitespace trimming.
- The host accepts a nested page only when that normalized title ends with the exact normalized app-name suffix.
- The page portion is trimmed again, must be non-empty, and must be no more than 120 Swift `Character` values (extended grapheme clusters). This is an independent native-chrome bound; Wiki's existing JavaScript content validation continues using its own UTF-16-code-unit bound.
- Empty titles, root-equal titles, malformed suffixes, controls, or over-bound titles do not create a page crumb.
- Unrecognized titles fall back to app-only navigation rather than displaying untrusted or malformed chrome.

Wiki updates `document.title` whenever `selectedKey` changes. Other apps remain app-only until they adopt the same title convention; the host behavior is backwards compatible.

The app-root action uses exactly `(() => { window.dispatchEvent(new Event("riot:navigate-root")); return document.title; })()`. Wiki registers `window.addEventListener("riot:navigate-root", openIndex)`. The host expression neither changes the current URL, relaxes `AppRuntimeCoordinator`'s navigation lock, accepts page-provided script, nor permits external navigation. Native state is updated only by parsing the returned title or a later KVO title callback. If evaluation fails, or an app does not handle the event and returns the same nested title, the existing page crumb remains accurate and actionable so the person can retry. Root/malformed titles clear a previously valid page; duplicate identical parsed locations are coalesced.

## State and component boundaries

### `CommunityShellView`

- Passes `community.name` and `model.openCommunityChooser` into the macOS runtime host.
- Treats selecting any macOS sidebar or keyboard destination as a tool-close transition before applying the route; iPhone's tab-selection path is unchanged.
- Presents `CommunityChooserView` with immutable `currentCommunityID`, `mountedAppName`, `onSelectCommunity`, and `onFindNearby` inputs. Decisions compare the selected technical ID to the immutable current ID, never display names; the repository remains authoritative for whether a target is held and openable. Choosing the already-current community dismisses the chooser without reconstructing the runtime. Choosing a different community while a tool is mounted presents the Stay-or-Switch confirmation from the chooser sheet's active hierarchy; “Switch” uses a destructive role before calling `model.switchCommunity`. On macOS, `onFindNearby` closes the running tool before routing; the default/iPhone action keeps today's behavior.
- Keeps the existing community-transition gate, focus bookkeeping, and Escape behavior authoritative.

### `AppRuntimeView` / `AppHostView`

- Accepts the community name and chooser action on macOS.
- Owns bounded current-page presentation and a root-navigation generation token.
- Replaces the old title/Close HStack with `AppBreadcrumbView`.

### `AppWebView` / `AppRuntimeCoordinator`

- Owns an `NSKeyValueObservation` on `WKWebView.title` through a weak coordinator capture and reports document-title changes to the host on the main actor.
- Stores the initial and last processed root-navigation generations. `updateNSView`/`updateUIView` ignore the initial and duplicate generations; a newer generation dispatches only the constant root event, parses the returned document title, and leaves the prior location intact on evaluation failure.
- Ignores root requests and title callbacks after teardown; invalidates and nils title observation during the existing idempotent teardown and again defensively in `deinit`.

### Wiki fixture

- Sets the root or nested document title from already-validated page data.
- Handles `riot:navigate-root` with an explicit index state that prevents wide-window auto-selection from undoing the action; opening any page clears that state.
- If the selected page disappears while not editing, Wiki enters the explicit index state and resets the title to `Wiki`. If it disappears during editing, Wiki preserves `selectedKey`, the exact edit draft, conflict UI, and the existing page title; it does not call `openIndex` until the person chooses to leave.
- Keeps its existing in-app “All pages” action for local context and accessibility.
- Is repacked through `scripts/apps/repack-starter.sh`; committed source and CBOR artifacts must remain in sync.

## Error and edge behavior

- If the title observation is late, missing, malformed, or fails, the breadcrumb safely shows only community and app.
- If a selected Wiki page disappears during sync outside editing, Wiki returns to the index and resets the title to `Wiki`; during an editing conflict it preserves the draft and page title until the person explicitly leaves.
- If trust is revoked, the existing invalidation notification closes the runtime; breadcrumb code does not bypass or delay it.
- If the community chooser is dismissed or “Stay” is selected, the current runtime and page remain mounted.
- If another community is selected while a tool is mounted, the explicit confirmation precedes the existing transition preparation that clears `runningTool` and rebuilds community-scoped state.
- Repeated root selections are idempotent from the user's perspective. The app crumb is disabled/absent as an action at root, avoiding unnecessary events.
- Very long names switch the complete breadcrumb to emoji; identifiers are never truncated into ambiguous strings.
- Opening and dismissing the chooser preserves the mounted WebView instance and current page; confirmed switching uses the existing teardown path.
- Wiki's ancestor app action has the same editing semantics as its existing “All pages” action: leaving the page exits edit mode. Adding a new unsaved-edit prompt is outside this navigation repair.

## Accessibility and keyboard behavior

- Each interactive ancestor is a real SwiftUI `Button` with a full action-oriented accessibility label.
- Separators are accessibility-hidden.
- The current page has header/current-location semantics but no button trait.
- Full and compact variants expose the same accessibility meaning; only one variant exists in the rendered accessibility tree.
- Keyboard focus order is community, app when actionable, then WebView content.
- Escape closes to Tools as before. On macOS, Command-1 through Command-4 also work while a tool is open because the route transition now clears the tool first.

## Security and privacy

- The breadcrumb displays only community/app/page display strings, never Nostr/Willow identifiers or signing material.
- Page titles are app-controlled display metadata, not authenticated page identity. They never authorize storage, community selection, or navigation policy and are normalized, parsed, and bounded before entering native chrome.
- Page titles may contain private community language and are never written to diagnostic logs or error messages.
- Root navigation dispatches one constant payload-free DOM event inside the already trust-gated runtime; no network capability, new scheme, page-supplied script, or additional bridge permission is introduced.
- Opening the chooser uses existing authorization; switching from a mounted runtime adds an explicit possible-edit-loss confirmation before existing transition teardown.

## TDD and verification

Implementation follows four explicit red-green-refactor slices:

1. **Location and presentation.** RED: create `apps/ios/RiotTests/AppBreadcrumbTests.swift` with pure parser-state assertions for root, valid nested page, exact/wrong suffix, 512/513 raw UTF-16 bounds, NFC equivalence, whitespace order, and `Cc`/`Cf`/`Zl`/`Zp` rejection at beginning/middle/end (including U+2028/U+2029 and bidi controls), plus 120/121-Character page boundaries and valid-page-then-root/malformed clearing. Define and test a pure `AppBreadcrumbPresentation` descriptor for full/emoji labels, hover help, action availability, and accessibility; `AppBreadcrumbView` renders that descriptor without a ViewInspector dependency. GREEN: add `AppPageLocation`, the presentation descriptor, and `AppBreadcrumbView`, replacing `app-close`. REFACTOR: centralize normalization and coalesce equal location updates.
2. **WebKit location/root lifecycle.** RED: in the same test file, reuse the `SpyDataBridge`, `NavProbe`, `makeWebView`, and `loadEntryPoint` patterns already proven in `AppRuntimeHostTests`; cover title KVO delivery plus initial-generation no-op, newer-generation dispatch, duplicate-generation no-op, returned-root-title clearing, returned-nested-title preservation, evaluation-failure preservation, and post-teardown no dispatch/callback. GREEN: add the weak KVO observation, generation handling, fixed event script, result parsing, and teardown. REFACTOR: keep one main-actor location delivery method for KVO and evaluation results.
3. **Shell and chooser safety.** RED: add pure mac-route transition and chooser-switch-decision tests proving macOS close-then-route, iPhone preservation, current-community dismissal/runtime preservation, chooser cancellation, Stay versus destructive confirmed Switch when a tool is mounted, and chooser “Find one nearby” close-then-route on macOS. GREEN: isolate `changeMacRoute`, inject the chooser selection/destination closures, and add the conservative confirmation in the sheet hierarchy. REFACTOR: route Escape/sidebar/keyboard/chooser destination changes through the existing single `closeTool` bookkeeping path.
4. **Wiki adoption.** RED: extend the real Wiki WebView test fixture for root/detail titles, exact window-targeted payload-free root-event handling, no first-page rebound below and above 640 points, non-editing deleted-page return, and editing-conflict draft/title preservation. GREEN: add `explicitIndex`, title updates, and the window event listener. REFACTOR: funnel explicit “All pages” and the host event through `openIndex` while retaining the distinct editing-conflict branch; repack with `scripts/apps/repack-starter.sh`.

`AppBreadcrumbTests.swift` is registered in both `apps/ios/Riot.xcodeproj/project.pbxproj` (`RiotTests`) and `apps/macos/Riot.xcodeproj/project.pbxproj` (`RiotKitTests-macOS`). This makes the parser, coordinator lifecycle, presentation contract, and mac route transition execute under macOS rather than relying on an absent macOS UI-test target. Pixel fitting and live actions are additionally verified by launching the macOS app and capturing normal/constrained screenshots.

After focused tests, run both Apple builds/tests, starter drift/audit, formatting, strict Clippy, the full Rust workspace tests, and the authoritative `scripts/web/coverage.sh` threshold-reading coverage gate. Update `apps/macos/README.md` with the new shared test file. Manual macOS review includes normal/constrained screenshots plus VoiceOver parity between full and emoji variants and useful WebView focus after returning to Wiki's index. Any failure in normal-width, constrained-width, keyboard, chooser, app-root, accessibility, artifact drift, build, test, lint, or coverage journeys blocks completion/PR.

## Acceptance criteria

- The macOS tool host shows `community › app` at root and adds `› page` for a conforming nested app page.
- Selecting the community crumb opens the existing community chooser.
- Choosing a different community from a mounted tool requires Stay-or-Switch confirmation; cancel/Stay preserves the same runtime and page.
- Selecting the app crumb from a nested page returns to the app index and remains there at both narrow and wide viewport widths.
- On macOS, Home, Tools, People, and Nearby all replace a running tool with the selected route; iPhone tab state remains unchanged.
- The hierarchy automatically collapses to `🏘 › 🧰 › 📄` when full labels do not fit and keeps full accessibility labels.
- The breadcrumb occupies at most 36 points vertically and never wraps.
- Escape, trust invalidation, community switching, WebView teardown, and iPhone navigation continue to behave as documented.
- Wiki source and packed starter artifacts remain deterministic and pass drift checks.
- No external navigation or new app permission is introduced; the only host-to-app addition is the constant payload-free `riot:navigate-root` event.
- All three ancestor tasks—open chooser, return to app index, leave through macOS shell navigation—complete without the removed Close control or visible protocol identifiers.

## Out of scope

- Giving every built-in app bespoke nested navigation in this change.
- Redesigning the community chooser, sidebar, Wiki content layout, or iPhone shell.
- Adding browser history, arbitrary URL display, or network navigation.
- Changing app trust, data storage, sync, or protocol behavior.
