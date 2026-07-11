# Riot iOS visual identity + custom navigation chrome

Status: approved, ready for implementation plan.

## Goal

The native iOS shell (`apps/ios/Riot/ConferenceShellView.swift`, `AppModel.swift`)
currently uses stock SwiftUI chrome — default `List`/`Form` styling, a native
floating-capsule `TabView`, system large titles, SF Symbols at default weight.
It works but looks like every other iOS app.

`marketing/index.html` (the `riot.protest.net` site, built this session) has
a strong, deliberate visual identity: bold uppercase poster type, a flat
hard-bordered "protest zine" aesthetic, sparing blue/pink accent, monospace
uppercase labels for anything structural. rabble asked for the iOS app to
carry that same identity. This spec ports it into a small reusable SwiftUI
design-system layer and re-skins all five shell screens with it, including
replacing the native tab bar with fully custom chrome (rabble's explicit
call over the lighter "native shape, ported identity" option).

## Non-goals / boundaries

- No changes to `RiotDestination`'s `title`/`tabTitle` string values or to
  `RiotAppModel`'s public API — `ShellNavigationTests` and
  `BindingSemanticsTests` must keep passing unmodified.
- No changes to `apps/ios/Riot/Core/` (identity/Keychain layer, just landed
  in `5bb25fa`) — this is a UI-only pass.
- No changes to `crates/`, FFI bindings, or Android.
- No new backend/network behavior. The app's actual functionality
  (create space, compose/sign, import preview, connection status) is
  unchanged — only its presentation.
- Android gets a matching pass later, as a separate spec — out of scope here.

## Visual language (source of truth: `marketing/index.html`)

**Color tokens** (ported verbatim from the site's CSS custom properties,
light and dark):

| Token | Light | Dark | Use |
|---|---|---|---|
| `paper` | `#eae6da` | `#131209` | Screen background |
| `paper2` | `#e1dccb` | `#1c1a10` | Card fill |
| `ink` | `#17160f` | `#efe9d8` | Primary text, borders, primary button fill |
| `inkSoft` | `#4a473b` | `#beb69e` | Secondary text |
| `blue` | `#22399f` | `#6d84ff` | Accent (headline shadow, selected-tab underline) |
| `pink` | `#d1216e` | `#ff5f9e` | Accent (headline shadow, selected state, hover/press) |
| `line` | `rgba(23,22,15,.18)` | `rgba(239,233,216,.16)` | Hairline dividers |
| `lineStrong` | `rgba(23,22,15,.4)` | `rgba(239,233,216,.36)` | Card borders |

Implemented as a `RiotColorScheme` enum-backed set of `Color` values that
resolve against `@Environment(\.colorScheme)`, not static asset-catalog
colors — this keeps the exact hex math (including the `rgba` alpha tokens)
in one Swift file next to its CSS source of truth, easy to diff against
`marketing/index.html` if that palette ever changes.

**Typography**, same three families and roles as the site:

- **Anton** — poster/display: screen headers, empty-state headlines, big
  numerals.
- **Work Sans** — body copy, form field text.
- **Space Mono** — uppercase structural labels: tab bar labels, eyebrows,
  badges/stamps, button labels.

**Shape language**: flat, square-cornered, 2pt solid borders, no drop
shadows, no translucency/blur. Selected/active states are shown with a
pink or blue accent (fill, border, or offset "stamp" rotation), not with
iOS's default blue tint.

## Design tokens & components

New directory: `apps/ios/Riot/Design/`

- `RiotTheme.swift` — color tokens (as above) + font accessors
  (`Font.riotPoster(size:)`, `Font.riotBody(size:)`, `Font.riotMono(size:)`),
  each built on `.custom(_:size:relativeTo:)` so Dynamic Type still scales
  them.
- `RiotTabBar.swift` — replaces the native `TabView` chrome. A custom
  `View` docked to the bottom, `paper` background, 2pt `ink` top border,
  no blur. Five items (SF Symbol, `.bold` weight, + Space Mono uppercase
  label), driven by `RiotDestination.phoneTabs` — same five cases, same
  order, no new destinations. Selected item gets a pink bordered "stamp"
  box (slight rotation, matching `.hero-stamp`) instead of a tint pill.
  `ConferenceShellView` keeps using `NavigationStack` per destination for
  push navigation and back-swipe; only the tab-selection chrome at the
  bottom is replaced — `TabView`'s own bar is hidden via
  `.toolbar(.hidden, for: .tabBar)`-equivalent (SwiftUI's
  `.tabViewStyle`/custom `ZStack` composition, chosen at implementation
  time), with `RiotTabBar` overlaid instead.
- `RiotHeader.swift` — replaces native large titles. Space Mono uppercase
  eyebrow line above an Anton poster title with the blue/pink offset
  text-shadow from the hero headline. Used via `.riotHeader("Spaces")`
  instead of `.navigationTitle`.
- `RiotCard.swift` — flat container: `paper2` fill, 2pt `ink` (or `line`)
  border, square corners, standard internal padding. Replaces `Form`
  `Section` and `List` row backgrounds.
- `RiotButtonStyle.swift` — a `ButtonStyle` matching `.btn`/`.btn.solid`:
  2pt border, Space Mono uppercase label, `.primary` variant solid-`ink`
  fill, `.secondary` variant outline; pressed state shifts toward `pink`.
- `RiotBadge.swift` — small bordered Space Mono uppercase chip (for
  "AI-assisted draft") and a rotated stamp variant (for the connection
  status line, echoing `.hero-stamp`).
- `RiotEmptyState.swift` — replaces `ContentUnavailableView`: Anton
  headline + Work Sans description, no system icon bubble.

Fonts (Anton, Work Sans, Space Mono — all SIL OFL/Apache, Google Fonts)
ship as `.ttf` resources under `apps/ios/Riot/Resources/Fonts/`, registered
via `UIAppFonts` in `Info.plist`. Sourced fresh from Google Fonts rather
than extracted from the marketing site's inlined base64 (that's WOFF2,
wrong format for `UIAppFonts`/`CTFontManager` registration).

## Screen-by-screen mapping

- **Spaces** — `RiotHeader("Spaces")`, `RiotCard` wrapping the
  title/namespace display or the create-space form, `RiotButtonStyle`
  `.primary` on "Create public space".
- **Incident board** — `RiotHeader`, entries rendered as `RiotCard` rows
  (headline in Work Sans semibold, `RiotBadge` for "AI-assisted draft",
  mono timestamp/identifier rows), `RiotEmptyState` when empty.
- **Compose & sign** — `RiotHeader`, form fields inside a `RiotCard`,
  `RiotButtonStyle` `.primary` on "Review complete — sign locally"
  (disabled state keeps existing logic, just restyled).
- **Import preview** — `RiotHeader`, `RiotCard` for the "nothing is
  accepted automatically" notice, entries as `RiotCard` rows,
  `RiotEmptyState` when empty.
- **Connection** — `RiotHeader`, connection status as a `RiotBadge` stamp
  (rotated, bordered) instead of a plain `Label`, `RiotCard` for the
  "on this device" stats.

`ConferenceShellView`'s `destinationView(_:)` switch and
`NavigationStack`-per-tab structure stay as-is; only the leaf views'
internals and the tab-selection chrome change.

## Accessibility

Custom chrome means losing some native-for-free accessibility, so this is
explicit scope, not an afterthought:

- `RiotTabBar` items get `.accessibilityLabel` (plain tab title, not the
  mono-uppercased display string) and `.accessibilityAddTraits(.isButton)`
  plus `.isSelected` on the active tab.
- All custom fonts scale with Dynamic Type via `relativeTo:` text styles;
  verify at the largest accessibility size that the poster headline and
  tab labels don't clip (truncation/line-limit fallback if they do).
- Color contrast: `ink` on `paper` and `paper` on `ink` both exceed WCAG AA
  in both light and dark; `pink`/`blue` accents are decorative (never the
  sole carrier of state — selected tab also gets the stamp border, not
  color alone).

## Testing

- `ShellNavigationTests` and `BindingSemanticsTests` run unmodified and
  must stay green — they assert on `RiotDestination`/`RiotAppModel`, not
  view internals.
- New `RiotTabBarTests` (or extend `ShellNavigationTests`): tapping each
  custom tab item updates `model.destination`, matching
  `testEveryPhoneTabCanBecomeTheVisibleDestination`'s existing coverage
  but through the new view instead of the native `TabView` binding.
- Manual verification per the `verify` skill: launch the simulator, screen
  through all five tabs, both light and dark appearance, and one
  accessibility-large text size, before calling this done.

## Risks / open questions

- Fully hiding the native `TabView` bar while keeping `NavigationStack`
  back-swipe/push behavior per tab needs a concrete SwiftUI composition
  decision (custom `ZStack` over a bar-hidden `TabView`, vs. dropping
  `TabView` entirely for a manually-switched `Group`) — left for the
  implementation plan to pick and prove out, since both are viable and
  the choice mostly affects animation/state-preservation details, not the
  design.
- Custom fonts add real app-bundle size (three families). Acceptable for
  a debug/demo build; worth a follow-up check before any real release.
