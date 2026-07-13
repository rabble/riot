# Community-first navigation design

Date: 2026-07-13  
Status: Approved by PM, architecture, design, security, and CTO review gate

## Product decision

Riot is organized around a **community**. A Willow space is the community's
technical container and the unit used to isolate, select, and sync data. It is
not the product people are asked to understand.

After opening a community, a person should be able to answer two questions:

1. What is happening here?
2. What can we do together?

Readable updates answer the first question. Community tools answer the second.
Identity, signatures, packages, trust markers, namespaces, and transports remain
security boundaries, but appear only where a person makes a decision that needs
them.

This design deliberately replaces the current five debug-shaped surfaces with a
community shell. It does not weaken organizer authority or pretend that physical
radio behavior has been proved.

## Supersession and delivery scope

This document is normative when it conflicts with these earlier conference
design decisions:

- The mandatory personal-space onboarding gate in
  `2026-07-12-personal-spaces-and-pages-design.md` is superseded. First use is
  community-first. A personal space is an optional community container after
  multi-community persistence exists; it is never required before joining or
  using a community.
- The local, per-device app-approval rule in
  `2026-07-12-multi-space-sqlite-store-design.md` is superseded. Until full
  Meadowcap management ships, the only execution authority is an accepted
  signed trust marker from the fixed recognized organizer for that namespace.
  SQLite may project that result, but cannot create authority.
- The visible rename from Checklist to Tasks in
  `2026-07-12-community-miniapp-suite-design.md` is superseded for Slices 0–2.
  The shipped content-addressed app remains **Checklist** so the visible name,
  manifest, fixture, accessibility identifiers, and app ID agree. A future
  rename is a new app version and migration, not a presentation alias.

Slices 0–2 support exactly one selected **public communal** community. They do
not advertise a community list or switcher. Slice 3 adds durable multi-community
selection. Personal, connections-only, managed, and private communities cannot
use legacy Nearby; they remain out of selectable scope until receiver-authenticated,
capability-bound protected sync exists.

## Measured failure in the current app

The 2026-07-13 macOS walkthrough exposed a product failure, not rough copy:

- Spaces pins profile editing, namespace data, demo controls, and app-review
  state above useful community content.
- Board renders headlines, timestamps, full entry IDs, and signer IDs, but omits
  descriptions and resolved names. It reads like a ledger inspector.
- Compose & sign names a cryptographic mechanism rather than the human goal and
  starts with model assistance selected.
- The Riverside demo profile is a member, so it can inspect Checklist but cannot
  approve or use it.
- Rust embeds eight starter pairs. Swift names four, and both Apple products
  bundle only Checklist. Missing resources are silently dropped.

Renaming tabs cannot repair those mismatches between goals, shipped resources,
authority, and navigation.

## User goals

- **Find or establish a community.** Create one, join one nearby, or return to
  the one already carried without first editing a public profile.
- **Know what is happening.** Read the headline, body, author, age, source, and
  expiry of current community updates without parsing identifiers.
- **Do useful work.** Open an available tool and complete its primary action in
  no more than two actions from Home.
- **Contribute.** Post an update using outcome language, while preserving an
  exact review and cryptographic commitment.
- **Exchange changes offline.** Understand which public community and content
  will exchange; radio and frame details belong in diagnostics.
- **Organize when authorized.** Review exact permissions and make a tool
  available through real organizer authority. Members never see dead organizer
  controls.
- **Manage identity in context.** Edit **Your profile** without pinning it above
  everyday community content.

## Truthful product transition

### First product trial: Slices 0–2

The first trial is not Slice 0 alone. It is the sequential result of Slices
0–2: real tools, readable updates/posting, and the single-community shell.

- With no retained community, launch shows **Create a community** and **Find one
  nearby**. Display name is offered inline and may be skipped.
- With one retained community, launch opens its Home directly. The community
  name and **Your profile** menu are visible; there is no fake list or switcher.
- A community that cannot open shows recovery in place: Retry, Find nearby, or
  Remove from this device after confirmation. It never becomes a blank space.

### Multi-community transition: Slice 3

Slice 3 introduces **Your communities** as the Level 1 container. Returning
opens the last available community directly. The community name/back control is
one action from the chooser. If the last community is unavailable, Riot opens
the chooser and preserves its record with recovery actions.

Each community row shows name, organizer/member/public-reader relationship,
recent activity, and sync freshness in plain language. Create and Find nearby
are actions on the chooser. Your profile remains global; community settings are
scoped to the selected community.

## Navigation and platform behavior

Inside a selected community are four destinations:

1. **Home** — updates and four deterministic tool shortcuts.
2. **Tools** — all tools available in this community and organizer-only tools
   available to review.
3. **People** — known contributors, not a membership directory.
4. **Nearby** — public-communal discovery, joining, and exchange.

**Post an update** is a primary Home action, not a fifth destination.

On iPhone, the community name/profile control forms the header and Home, Tools,
People, and Nearby form the bottom tab bar. Opening a tool pushes a full-screen
tool route whose header retains the community name and Back. A dirty tool or
post draft requires confirmation before a community change.

The header keeps two separate labeled paths. The profile avatar opens **Your
profile**. A gear next to the community name opens **Community settings**.
Members can see About, sync health, and Technical details there; organizer-only
tool and governance controls are omitted unless core authority says the acting
profile can use them. On macOS the same labels are distinct sidebar footer and
selected-community actions, not an ambiguous combined menu.

On macOS, `NavigationSplitView` shows communities (when Slice 3 exists) and the
four selected-community destinations in the sidebar. The content/detail pane
shows Home or the selected tool; a tool does not open as a modal sheet. The
sidebar and community name remain visible while the tool runs. Command-1…4
select destinations; beginning with Slice 3, Command-K focuses community
selection. Escape returns from a tool when it is safe, and focus returns to the
invoking tool card.

At compact desktop widths, the sidebar may collapse behind the standard
disclosure control, but navigation never becomes the phone's oversized bottom
bar. Selection uses label, icon, shape, and accessibility state—not color alone.

Home shortcuts are deterministic: walk canonical catalog order and show the
first four approved tools. When the first four are approved these are Checklist,
Needs & Offers, Events, and Decisions; if one is not approved, continue through
the catalog rather than leaving a mysterious hole. Organizer pinning and local
recency are deferred because an unexplained ranking is worse than a stable one.

## Content and interaction contracts

### Updates

Slice 1 introduces versioned `CurrentEntryV2` across Rust FFI and Swift:

| Field | Contract |
| --- | --- |
| `entry_id`, `signer_id` | complete bytes/hex; technical disclosure only |
| `headline`, `description` | decoded alert text; malformed payload rejected, never blank-substituted |
| `source_claims` | the signed protocol's required 1–16 source strings, preserved in order |
| `urgency`, `severity`, `certainty` | closed CAP-derived values 0–4; any other value rejects the payload |
| `created_at_unix_seconds`, `expires_at_unix_seconds` | the signed protocol's unsigned Unix seconds; Swift derives localized freshness |
| `author` | rendered display name plus mandatory key-derived tag and full profile ID |
| `ai_assisted` | provenance shown in details, never an author substitute |

`CurrentEntry` remains available for one binding transition; the Apple client
moves atomically to V2 after regenerated bindings pass contract tests. The
protocol's explicit enum value `unknown` renders neutral copy; out-of-range
enums, invalid required bytes, or malformed payloads return one fixed
`invalidUpdate` error without raw Rust/CBOR text.

Correction and verification state are **not** in Slice 1 because no signed model
exists. They remain absent until a separate protocol design defines them.

An ordinary update's primary action is **Read update**. It may be valuable and
complete without a response. Riot does not infer actions from headline text. A
future operational update may expose **Open [tool]** only after a signed,
versioned related-app/action field is designed. This prevents prettier but fake
action cards.

Home ordering is stable: non-expired updates by signed creation time descending,
then expired updates in a collapsed Earlier section. Newly synced content is
inserted by that order and announced without moving keyboard/VoiceOver focus.

### Posting

The flow is **Post an update**:

1. Enter headline and what people need to know. The required **Where this came
   from** field contributes one or more signed source claims, and required expiry
   uses a plain-language default that remains visible and editable before review.
2. Optional local assistance is invoked explicitly. It is off by default.
3. Review the exact fields, destination community, and acting rendered identity
   including key-derived tag.
4. Tap **Post update**. In Slice 1, finalization binds the reviewed bytes to an
   immutable namespace/signer/repository-generation snapshot and revalidates all
   three immediately before commit; a change invalidates review. Unit 3 replaces
   that compatibility snapshot with the real immutable `SpaceSession`.
5. Home shows the committed local update and Pending nearby exchange status.

Slice 1 handles validation, repository-write, signing, and malformed-payload
failures that exist today. It preserves drafts across destination changes, tool
opening, and relaunch. `KeyUnavailable` is deferred to the SQLite signer work;
the UI must not claim a typed locked-key recovery before the API provides it.

### Known contributors

People is intentionally **Known contributors** in Slices 1–3. Its job is to
resolve an update/tool author and inspect a public profile already carried in
the selected community. It makes no complete-membership, role, presence, or
online claims.

`ContributorRowV1` contains full profile ID, rendered name, key-derived tag,
profile-card availability, latest accepted contribution time, and organizer
status only when derived from the recognized organizer coordinate. Empty copy is
**No known contributors yet** with actions Post an update or Find nearby.
Meadowcap capability state must precede any future Members or Roles surface.

### Tools and canonical catalog

`crates/riot-core/src/apps/starter.rs::STARTER_CATALOG` is the sole authority for
ordered built-in bytes and content-derived IDs. Swift does not maintain a second
handwritten list. Build tooling generates or verifies an Apple resource manifest
from the eight Rust-owned pairs and fails on a missing, extra, invalid, reordered,
or ID-mismatched pair.

Current canonical order and IDs are frozen for Slices 0–2:

| Slug | Visible name | App ID |
| --- | --- | --- |
| `checklist` | Checklist | `3fe5f89af18d9244756c8925750280f0c51479030cf3cd7b4d26940b51eaa4b7` |
| `supply-board` | Needs & Offers | `05200e07ca8c11da106366dbe2f7386dc11826aa723479352a916158ac649ac8` |
| `roll-call` | Events | `266b7978d2bcd143d7b93b6246884c85343ca4b6e4bb4aa406dbf8d87e39d382` |
| `quick-poll` | Decisions | `36a4c50030b5dbac3e84d40c503b6413e2b39b276f6010215e87c29c96453d1a` |
| `chat` | Chat | `6a5cadd381460f15b871cf898b59a4db97d5ddb80130cef335136c619bacdfac` |
| `dispatches` | Dispatches | `848a8e1551f34a1443eb1c1dc6601b730db413eee500a49695c8956cac5f2459` |
| `wiki` | Wiki | `c2a54df288701afe8ed95e91af8fafec34a56d9132cde914b9ec76ce826ac714` |
| `photo-wall` | Photo Wall | `ae1ac55cfe563dab67c4139ff2fc84fa59647e75848ffaa0132ef1110ff8066b` |

The primary actions proved by browser and host tests are respectively: add and
toggle an item; post and resolve a need/offer; add an event and RSVP; create and
vote on a decision; send a message; publish/open a dispatch; edit/save a page;
and add a captioned photo.

An approved tool opens in one action. A locally missing package says **Add to
this device**; acquisition does not grant authority. Organizer review shows the
exact app ID/version, author tag, permissions, package availability, and selected
community. The final action is **Add to this community** and shows the organizer
identity and role.

All eight built-ins are already local. Riverside imports the complete Shift
Signup manifest and bundle. With a valid organizer marker it also opens directly;
it does not show Get, Review, or Add to this device. Add to this device appears
only for an approved tool whose verified package bytes are actually absent.
Directory/repository projection automatically admits a complete, verified
package when an accepted organizer marker for its exact app ID exists. That
admission is persisted or deterministically re-derived from verified package and
marker entries after replay/relaunch. An incomplete, invalid, revoked, or
unapproved package is never auto-admitted.

## Authority, runtime, and migration

For Slices 0–3 the authoritative approval record is the latest accepted
organizer-signed `TrustMarker::Trust` or `TrustMarker::Revoke` at
`app_index_trust_path(app_id, recognized_organizer_subspace)`. The recognized
organizer is the subspace whose public bytes equal the communal namespace. Only
that organizer may write the marker. Members evaluate it but cannot create or
override it.

Legacy `trustedAppIDs` and future SQLite `space_app_state` are projection caches.
On migration they are recomputed from verified markers for the exact namespace;
a local true bit without a valid marker becomes unavailable, not grandfathered.
There is no local-approval fallback. Full Meadowcap `AppApproval` replaces this
fixed-organizer scheme in a separately reviewed migration.

Rust owns a UniFFI `AppExecutionSession`; Swift never treats a raw app ID as
execution authority. `open_app_execution` verifies the exact package and current
organizer marker, then binds an opaque session to namespace, app ID, manifest
digest, acting subspace/profile, WebView nonce/origin, repository namespace
generation, and approval generation. Approval generation is the digest of the
accepted current organizer trust-marker entry ID and Trust/Revoke value for that
namespace/app coordinate; any accepted replacement therefore changes it.

Every Rust-owned operation—resource delivery, `whoami`, profile lookup,
get/list/put, watch creation/delivery/cancellation, and notification callback—
revalidates that session against current repository namespace and approval
generations. `close_app_execution` invalidates the WebView nonce and cancels its
watches. Accepted revocation, community switch, profile/repository replacement,
WebView destruction, navigation outside the app origin, or generation mismatch
fails the Rust call before access or commit, discards buffered work, and tells
Swift to close the tool immediately with fixed copy. The UI bridge does not call
legacy raw app-ID data operations. No stale write may commit.

One-tap execution also depends on the independent iOS network backstop from the
approved runtime security design. CSP and navigation policy are not sufficient.
The hostile-page suite covers DNS prefetch, WebRTC/STUN, forms, windows,
subresources, custom schemes, downloads, and powerful APIs before all approved
tools can be called directly usable.

## Riverside fixture contract

Riverside is an imported member demonstration, never an approval bypass.

- Replace `namespace_secret_seed` with a deterministic
  `organizer_subspace_secret_seed` in fixture source. Derive its public subspace
  and use those exact bytes as the communal namespace, creating the recognized
  organizer coordinate by construction.
- Deterministic private material exists only in committed conformance source and
  fixture generation. Shipped bundles contain signed public entries, not the
  organizer seed.
- Because the organizer seed is public in conformance source, Riverside is
  permanently marked demo-only and cannot be presented, invited into, or
  exported as production community authority.
- The exact approved set is the eight app IDs in the catalog table plus the
  content-derived Shift Signup ID produced from the fixture's committed manifest
  and bundle. Generation writes that ninth ID into the fixture drift snapshot;
  any byte or ID drift fails before packaging.
- The organizer writes a Trust marker for all nine exact IDs at a timestamp
  before the walkthrough alerts. No fixture Revoke follows them.
- Demo loading generates a fresh local communal author inside Riverside. That
  independently generated local profile remains a member; Ana remains only a
  signed known contributor/profile card in the imported bundle. A valid marker
  signed by the fresh member is included in a negative test and ignored.
- Fresh-state UI proof fails if any of the nine tools renders Review. It opens
  each and completes the named primary action. Shift Signup proves Take this
  shift / Give it back.

The demo walkthrough uses Checklist and Shift Signup; approving a new tool is a
separate organizer-created-community walkthrough.

## Nearby security and lifecycle

Legacy Nearby is available only for public communal communities. It does not
cryptographically authenticate the remote receiver. Prior to bilateral human
confirmation it may advertise an opaque service instance, never a community
title, namespace, profile ID, inventory, or content. Discovery never auto-connects
or auto-accepts. After explicit device selection and confirmation on both
devices, the flow may disclose the public community name and exact outcome:
Join, Get changes, Already current, or Different community. Cryptographic
receiver authentication is reserved for the protected-sync work below.

The selected community owns one community-scoped Nearby coordinator. It survives
Home/Tools/People/Nearby routing and foreground transitions. Switching or
removing a community first cancels pairing, transfer, and callbacks for the old
immutable `SpaceSession`; an active transfer requires confirmation. A race
between switch, app write, and import may commit to at most one namespace and
cannot repaint the destination community.

Personal, managed, connections-only, and private communities are ineligible for
this path. They require protected sync that authenticates the receiver and binds
capabilities before disclosing metadata or accepting content.

## Recovery and accessibility contract

Every state has a useful primary action and omits unavailable role actions:

| State | Primary action | Secondary behavior |
| --- | --- | --- |
| profile/store loading | accessible progress, no fake empty state | bounded wait then Retry |
| no community | Create a community | Find one nearby |
| community unavailable/corrupt | Retry or Recover | remove only after confirmation |
| no updates | Post the first update | Find nearby |
| no tools | organizer: Add a tool; member: Find nearby | explain role without dead button |
| catalog/package failed | Retry package | Technical details with fixed error code |
| Bluetooth/local-network denied | Open Settings | explain what remains usable offline |
| sync interrupted | Retry | keep accepted content and community context |
| post validation/write/sign failure | Edit and retry | preserve draft; no partial post |
| unauthorized/revoked/stale session | Return to Tools | never expose raw internal errors |

Structural accessibility requirements:

- screen titles are headings; sidebar, tab bar, updates, and tools have semantic
  landmarks and stable labels;
- interactive targets are at least 44×44 points and remain operable at largest
  Dynamic Type without horizontal text clipping;
- selected destination, organizer role, sync state, and errors use text/state in
  addition to color;
- VoiceOver announces newly accepted updates without stealing focus and reads
  rendered author plus key-derived tag, never bare untrusted display name;
- macOS has logical keyboard traversal, visible focus, the shortcuts above, and
  focus restoration after closing a tool, posting, switching, or review;
- drafts survive route changes and relaunch; switching communities with a
  non-empty draft requires choose Stay or Discard draft.

Full IDs appear only after deliberate Technical details disclosure and never in
ordinary cards, nearby advertisements, analytics, or accessibility labels.

## Delivery work units and TDD gates

No implementation begins while either Apple project file is owned/dirty in a
hot session. Before every unit: owner releases the path, claim exact files in
`COLLABORATION.md`, `git pull --rebase --autostash`, inspect current diffs, write
the RED test, and reread every edited file. Before commit: pull again, stage only
explicit paths, inspect `git diff --cached`, and run `sh scripts/green.sh`.

### Prerequisite C — Coverage baseline

The configured gate is 100%, while the last measured Rust line baseline is
83.37%. This is a pre-existing blocker, not a pass. Before implementation, claim
and commit `.coverage-thresholds.json` as the source of truth and make its
executable enforcement list match the commands below. Before any product slice
is declared complete, either add tests until its line/branch/function/statement
(LLVM region) thresholds pass or obtain and commit an authorized threshold
change.

```sh
cargo test --workspace --all-features
cargo tarpaulin --fail-under 100
cargo llvm-cov --workspace --all-features --branch \
  --fail-under-lines 100 --fail-under-functions 100 \
  --fail-under-regions 100 --fail-under-branches 100
xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
  -destination 'platform=iOS Simulator,name=iPhone 17 Pro,OS=26.2,arch=arm64' \
  -derivedDataPath build/ios-derived -enableCodeCoverage YES
xcrun xccov view --report build/ios-derived/Logs/Test/*.xcresult
xcodebuild test -project apps/macos/Riot.xcodeproj -scheme RiotKit-macOS \
  -destination 'platform=macOS' -derivedDataPath build/macos-coverage \
  -enableCodeCoverage YES
xcrun xccov view --report build/macos-coverage/Logs/Test/*.xcresult
```

Tarpaulin and LLVM coverage establish the configured four-dimensional Rust gate.
Xcode reports Swift coverage separately; neither Rust coverage nor
`scripts/green.sh` is represented as Swift or packaging coverage.

### Unit 0A — Canonical catalog and Apple artifacts

- **RED:** extend `apps_starter.rs` to emit/verify the exact ordered catalog
  contract; add `StarterResourceTests` that inspects both built `.app` resource
  directories and fails today because seven pairs are absent. Delete the Swift
  `compactMap`/four-name tolerance test path.
- **GREEN:** derive/verify Apple resources from Rust's eight pairs, add all pairs
  to both Xcode targets, and make missing/extra/invalid pairs fatal.
- **Focused proof:** `cargo test -p riot-core --test apps_starter`; iOS and macOS
  `StarterResourceTests`; artifact `find`/hash comparison on both `.app` builds.
- **Scope:** starter catalog/build script, `AppModel` loader, both Xcode projects,
  existing starter resource tests. No navigation or transport files.

### Unit 0B — Deterministic Riverside authority

- **RED:** `demo_fixture_drift` expects recognized organizer coordinate and nine
  Trust markers; `apps_contract` proves member-signed trust is ignored; fresh
  repository/directory tests prove a complete trusted carried package skips Get
  across replay/relaunch while incomplete or untrusted packages do not;
  `RiversideMemberToolUITests` fails on Get/Review and primary write for each tool.
- **GREEN:** generate organizer-shaped namespace/markers and import their signed
  entries, then deterministically admit complete organizer-trusted packages
  without an authority bypass.
- **Focused proof:** `cargo test -p riot-core --test demo_fixture_drift`;
  `cargo test -p riot-ffi --test apps_contract`; clean-state macOS/iOS UI test.
- **Scope:** fixture source/generator/drift tests, repository/directory admission
  projection, and existing app contract/repository/directory/UI tests. No shell
  redesign.

### Unit 0C — Runtime containment and invalidation

- **RED:** Rust `apps_contract` tests call session resource/whoami/profile/get/
  list/put/watch operations directly and prove revoke, namespace replacement,
  explicit destruction, and stale approval/repository generation all fail before
  read or commit. Swift `AppRuntimeHostTests`/`AppSyncReplicationTests` prove the
  bridge cancels watches and closes UI. Hostile-page tests fail every network/
  exfiltration vector listed above, including with CSP stripped.
- **GREEN:** Rust-owned `AppExecutionSession`, generation derivation/revalidation,
  FFI binding regeneration, Swift bridge invalidation/close handling, and the
  independently enforced iOS network backstop.
- **Focused proof:** `cargo test -p riot-ffi --test apps_contract`; FFI binding
  dirty check; focused Xcode host/sync/security suites and
  `miniapp-browser.spec.mjs`; then both Apple builds/tests.
- **Scope:** Rust app session/data FFI and tests, generated bindings, Swift app
  runtime/bridge/policy and existing runtime tests.

### Unit 1A — Versioned readable updates

- **RED:** Rust FFI contract tests assert every `CurrentEntryV2` field, the
  signed 1–16 source-claim order, closed-enum rejection, second-based timestamps,
  and malformed-payload mapping; Swift repository/view tests fail on body,
  rendered author, sources, expiry, and Technical details disclosure.
- **GREEN:** add V2, regenerate bindings, map it atomically, and render stable
  update ordering. Do not implement correction/verification.
- **Focused proof:** `cargo test -p riot-ffi`; binding generation/dirty check;
  focused `ProfileRepositoryTests` and Home update tests.
- **Scope:** alert projection/FFI/bindings/repository and update views/tests.

### Unit 1B — Post update

- **RED:** flow tests require outcome labels, assistance off, exact review
  identity/community, one signed write, draft restoration, and fixed failures.
- **GREEN:** implement Post update against the existing signer error set.
- **Focused proof:** Rust signing contract plus Swift post-flow model/view tests;
  one deterministic UI happy path and write-failure path.
- **Scope:** post model/view and existing signing repository; no SQLite key-lock
  promise.

### Unit 1C — Known contributors

- **RED:** DTO/view tests reject membership/presence labels, resolve rendered
  names with tags, derive organizer only from the recognized coordinate, and
  exercise the actionable empty state.
- **GREEN:** add `ContributorRowV1` projection and People surface.
- **Focused proof:** Rust projection tests and Swift People tests.
- **Scope:** profile projection/FFI/bindings and People view/tests.

### Unit 2A — Adaptive single-community shell

- **RED:** `ShellNavigationTests` prove the four routes, exact iPhone/macOS
  presentation, deterministic shortcuts, profile/settings relocation, focus
  restoration, draft survival, and one retained community launch. They also
  prove the current tab-lifecycle performance contract remains intact.
- **GREEN:** implement typed `CommunityContext`/`CommunityRoute` over a
  community-list/selection protocol shaped for future `RiotDatabase` and
  immutable `SpaceSession`; do not bind new views directly to a singleton space.
- **Focused proof:** focused shell tests, XCUITest keyboard/VoiceOver identifiers,
  macOS and iPhone visual review at compact/regular/large-text widths.
- **Performance proof:** cached community switch is under 300 ms once Slice 3
  exists and a starter tool opens in under 500 ms on the agreed demo devices.
- **Scope:** shell/navigation/design-system views and tests; no project files.

### Unit 2B — Nearby ownership and recovery states

- **RED:** lifecycle tests prove routing does not deallocate the coordinator,
  discovery cannot auto-connect/accept, both devices must confirm before public
  metadata disclosure, switching cancels old callbacks, pre-confirmation
  metadata is opaque, denied permissions offer Settings, and a switch/write/import
  race fails closed.
- **GREEN:** move ownership to selected-community coordinator and enforce the
  public-communal visibility gate.
- **Focused proof:** `SpaceAdoptionTests`, `LocalNetworkNearbyTests`,
  `AppSyncReplicationTests`, and shell recovery tests. Report loopback/Bonjour
  separately from any two-iPhone BLE run.
- **Scope:** community coordinator, Nearby presentation/controller, existing
  transport tests. No claim of protected private sync.

### Unit 3 — Multiple communities

- **RED/GREEN:** resume the reviewed SQLite registry/session work only after
  rewriting its approval projection to this document's organizer-marker rule.
  Add chooser, last-available launch, switch cancellation, isolation, archive,
  restore, and migration-quarantine tests before implementation.
- **Proof:** exact commands in the revised multi-space implementation plan plus
  all shell/runtime/sync isolation suites.

## Whole-product verification

Every unit runs its focused RED/GREEN command, then strict formatting/Clippy,
`cargo test --workspace --all-features`, binding checks when relevant, JavaScript
miniapp tests, iOS and macOS tests/builds, `sh scripts/green.sh`, and the configured
coverage gate. Android remains part of `green.sh` when shared Rust/FFI changes.

Physical-radio proof is separate: loopback and Bonjour on one Mac do not prove
BLE between two iPhones. The report must state proven paths and assumptions
separately.

## Measurable trial success

Run the trial separately on iPhone and macOS with five first-time evaluators per
platform. Every evaluator starts from the same clean retained-Riverside fixture;
the timer starts when the task card is handed over and stops on the observable
outcome. The same four or more evaluators must complete every required task on
that platform without coaching:

1. state the selected community and explain one current update within 20 seconds;
2. open a named tool from Home and change shared state within 30 seconds and no
   more than two actions;
3. post an update within 60 seconds and describe the action as posting, not
   signing;
4. find Your profile and community settings without mistaking either for Home;
5. identify the selected public community and whether a deterministic nearby
   peer means Join, Get changes, Already current, or Different community;
6. on macOS, use the sidebar/keyboard to open a tool and return with focus intact.

The trial fails if any approved Riverside tool says Review to the member, if
technical IDs dominate ordinary reading, if the Mac uses the phone bottom bar or
modal tool sheet, or if the report implies untested physical BLE works.
