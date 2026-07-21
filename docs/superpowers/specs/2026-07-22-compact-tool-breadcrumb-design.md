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

Selecting the community crumb opens the existing community chooser. Dismissing the chooser keeps the current tool and page. Selecting another community uses the existing guarded community-transition path, which tears down the old tool runtime and switches community scope.

### UC2: Return to an app's root

**WHO:** A member reading a nested app page  
**WANTS:** to select the app crumb  
**SO THAT:** they can return to that app's index  
**WHEN:** a page crumb is present

Selecting the app crumb sends the fixed, payload-free `riot:navigate-root` DOM event into the existing sandboxed WebView. An app that exposes a nested page adopts that event as its root-navigation contract. This does not give native code app-specific data access. At the app root the app crumb is the current level and is not presented as an enabled action.

Wiki handles the event by explicitly selecting its page index and setting an `explicitIndex` state. That state suppresses Wiki's current wide-layout first-page auto-selection until the person deliberately opens a page. Initial wide-layout launch may still select the first page, but an explicit app-crumb action always settles at the index and does not rebound into that first page.

### UC3: Leave a tool through ordinary shell navigation

**WHO:** A member with a tool open  
**WANTS:** Home, Tools, People, and Nearby to work normally  
**SO THAT:** the open tool never traps or masks them  
**WHEN:** they select a sidebar destination

Every shell-route selection closes the running tool before showing the selected route, on macOS sidebar/keyboard navigation and iPhone tab navigation alike. Escape retains the documented behavior of returning from the tool to Tools. The existing `ToolFocusRestoration` bookkeeping is cleared through the same close path; applying the returned identifier to actual UI focus is not implemented today and is not claimed by this change.

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

The compact row preserves the same hit targets by padding controls inside the 36-point row. Emoji do not replace accessible text.

### Existing alternatives

- The oversized Close button is removed from the macOS host.
- Escape continues to close the tool to Tools.
- iPhone keeps its existing community header and NavigationStack back behavior; duplicating the full breadcrumb there would add, rather than remove, chrome.

## Host/app location contract

The host observes the WebView's existing document title. No new privileged message-handler API is added.

- Root title: `<app name>` (for example, `Wiki`).
- Nested title: `<page name> — <app name>` (for example, `Meeting guide — Wiki`).
- The host accepts a nested page only when the normalized title ends with the exact app-name suffix.
- The page portion is trimmed, must contain no Unicode control or newline characters, and must be no more than 120 extended grapheme clusters, matching Wiki's existing page-title limit.
- Empty titles, root-equal titles, malformed suffixes, controls, or over-bound titles do not create a page crumb.
- Unrecognized titles fall back to app-only navigation rather than displaying untrusted or malformed chrome.

Wiki updates `document.title` whenever `selectedKey` changes. Other apps remain app-only until they adopt the same title convention; the host behavior is backwards compatible.

The app-root action evaluates one constant script that dispatches `riot:navigate-root` with no payload. It neither changes the current URL, relaxes `AppRuntimeCoordinator`'s navigation lock, accepts page-provided script, nor permits external navigation. The host clears the native page crumb immediately when it dispatches the event, and accepts later title callbacks only from the still-mounted coordinator.

## State and component boundaries

### `CommunityShellView`

- Passes `community.name` and `model.openCommunityChooser` into the macOS runtime host.
- Treats selecting any sidebar destination as a tool-close transition before applying the route.
- Keeps the existing community-transition gate, focus bookkeeping, and Escape behavior authoritative.

### `AppRuntimeView` / `AppHostView`

- Accepts the community name and chooser action on macOS.
- Owns bounded current-page presentation and a root-navigation generation token.
- Replaces the old title/Close HStack with `AppBreadcrumbView`.

### `AppWebView` / `AppRuntimeCoordinator`

- Owns an `NSKeyValueObservation` on `WKWebView.title` and reports document-title changes to the host on the main actor.
- Stores the last processed root-navigation generation. `updateNSView`/`updateUIView` forward a newer generation to the coordinator, which dispatches only the constant root event and clears the native page state before doing so.
- Ignores title callbacks after teardown and removes title observation during the existing idempotent teardown.

### Wiki fixture

- Sets the root or nested document title from already-validated page data.
- Handles `riot:navigate-root` with an explicit index state that prevents wide-window auto-selection from undoing the action; opening any page clears that state.
- Keeps its existing in-app “All pages” action for local context and accessibility.
- Is repacked through `scripts/apps/repack-starter.sh`; committed source and CBOR artifacts must remain in sync.

## Error and edge behavior

- If the title observation is late, missing, malformed, or fails, the breadcrumb safely shows only community and app.
- If a selected Wiki page disappears during sync, Wiki's existing reconciliation returns to the index and resets the title to `Wiki`.
- If trust is revoked, the existing invalidation notification closes the runtime; breadcrumb code does not bypass or delay it.
- If the community chooser is dismissed, the current runtime remains mounted.
- If another community is selected, the existing transition preparation clears `runningTool` before rebuilding community-scoped state.
- Repeated root selections are idempotent from the user's perspective. The app crumb is disabled/absent as an action at root, avoiding unnecessary events.
- Very long names switch the complete breadcrumb to emoji; identifiers are never truncated into ambiguous strings.
- Opening and dismissing the chooser preserves the mounted WebView instance and current page; switching communities uses the existing teardown path.
- Wiki's ancestor app action has the same editing semantics as its existing “All pages” action: leaving the page exits edit mode. Adding a new unsaved-edit prompt is outside this navigation repair.

## Accessibility and keyboard behavior

- Each interactive ancestor is a real SwiftUI `Button` with a full action-oriented accessibility label.
- Separators are accessibility-hidden.
- The current page has header/current-location semantics but no button trait.
- Full and compact variants expose the same accessibility meaning; only one variant exists in the rendered accessibility tree.
- Keyboard focus order is community, app when actionable, then WebView content.
- Escape closes to Tools as before. Command-1 through Command-4 also work while a tool is open because the route transition now clears the tool first.

## Security and privacy

- The breadcrumb displays only community/app/page display strings, never Nostr/Willow identifiers or signing material.
- Page titles come from already trusted local app content but are still parsed and bounded before entering native chrome.
- Root navigation reuses the verified resolver and existing `riot-app` navigation policy; no network capability, new scheme, or additional bridge permission is introduced.
- Opening the chooser uses existing authorization and unsaved-draft transition handling.

## TDD and verification

Implementation follows explicit red-green cycles:

1. Add pure tests for document-title parsing: root, valid nested page, wrong app suffix, empty, controls, and length bound.
2. Add shell-navigation tests proving a route selection while a tool is open yields a close-then-route action and clears the existing invoking-tool bookkeeping.
3. Add WebKit host tests proving a nested document-title change reaches the coordinator callback, late callbacks after teardown are ignored, and a root request dispatches only the fixed payload-free DOM event.
4. Add Wiki JavaScript assertions for root/detail title changes and for no first-page rebound at both narrow and wide viewport widths, then repack and run the committed starter-artifact drift test.
5. Add a macOS UI test or inspectable presentation tests for full and emoji breadcrumb variants, action labels, and removal of `app-close`.
6. Run focused Swift tests, both Apple builds, strict repository formatting/lint checks, the full Rust workspace test suite, and the starter artifact audit.
7. Capture a macOS screenshot at normal and constrained widths and confirm one compact row, no oversized Close button, correct hierarchy, and working sidebar/chooser/app-root actions.

## Acceptance criteria

- The macOS tool host shows `community › app` at root and adds `› page` for a conforming nested app page.
- Selecting the community crumb opens the existing community chooser.
- Selecting the app crumb from a nested page returns to the app index and remains there at both narrow and wide viewport widths.
- Home, Tools, People, and Nearby all replace a running tool with the selected route.
- The hierarchy automatically collapses to `🏘 › 🧰 › 📄` when full labels do not fit and keeps full accessibility labels.
- The breadcrumb occupies at most 36 points vertically and never wraps.
- Escape, trust invalidation, community switching, WebView teardown, and iPhone navigation continue to behave as documented.
- Wiki source and packed starter artifacts remain deterministic and pass drift checks.
- No external navigation or new app permission is introduced; the only host-to-app addition is the constant payload-free `riot:navigate-root` event.

## Out of scope

- Giving every built-in app bespoke nested navigation in this change.
- Redesigning the community chooser, sidebar, Wiki content layout, or iPhone shell.
- Adding browser history, arbitrary URL display, or network navigation.
- Changing app trust, data storage, sync, or protocol behavior.
