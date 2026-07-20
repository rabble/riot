# Paired App + Website Explainer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship one accurate five-beat “How Riot works” story in the first-run app, marketing homepage, and contextual newswire About page, with a direct join action and tests that prevent trust-boundary drift.

**Architecture:** Put the testable native story and welcome-to-setup intent in the shared RiotKit layer (`CommunityShell.swift`), while `ConferenceShellView.swift` remains presentation-only. Keep the marketing site static and dependency-free; extend its existing contract script into one build-time cross-surface check that reads both Swift and HTML. Correct the gateway’s contextual copy without changing its rendering or data flow.

**Tech Stack:** Swift 6, SwiftUI, XCTest/XCUITest, static HTML/CSS, Node.js contract assertions, Python `unittest`, Xcode 26.2.

**Approved design:** `docs/superpowers/specs/2026-07-20-paired-app-website-explainer-design.md`

---

## File Map

- `apps/ios/Riot/CommunityShell.swift` — shared, testable explainer beats and setup intent.
- `apps/ios/Riot/ConferenceShellView.swift` — first-run welcome, direct join routing, and native explainer sheet.
- `apps/ios/RiotTests/ShellNavigationTests.swift` — ordered story, safe-copy, rejected-copy, and setup-intent contracts.
- `apps/ios/RiotUITests/RiotTabNavigationUITests.swift` — end-to-end explainer dismissal and direct join behavior.
- `scripts/marketing/protocol-page-contracts.mjs` — single build-time contract across Swift and the marketing homepage.
- `marketing/index.html` — canonical marketing homepage source.
- `marketing/public/index.html` — byte-identical deployment mirror.
- `apps/gateway/newswire.py` — corrected contextual About copy.
- `apps/gateway/tests/test_newswire.py` — hostile-mirror and provenance regression assertions.

No new source file or Xcode project-file membership is required.

## Work Unit 0: Isolate the paired-story PR

The current `feat/onboarding-find-and-explainer` checkout is shared and already
contains unrelated committed and untracked work. Do not remove, reset, or
rewrite any of it. Execute the implementation in a clean linked worktree.

**Files:**

- Add: `docs/superpowers/specs/2026-07-20-paired-app-website-explainer-design.md`
- Add: `docs/superpowers/plans/2026-07-20-paired-app-website-explainer.md`

- [ ] **Step 1: Commit the reviewed plan by exact path in the shared checkout**

From `/Users/rabble/code/explorations/riot`:

```sh
git add docs/superpowers/plans/2026-07-20-paired-app-website-explainer.md
git commit -m "docs: plan paired app and website explainer"
```

Stage no other file. Preserve Xcode user state, Playwright state, overnight
logs, caches, screenshots, and the unrelated expanded-space design.

- [ ] **Step 2: Create a clean implementation worktree from origin/main**

```sh
git fetch origin main
git worktree add \
  /Users/rabble/code/explorations/riot-wt-paired \
  -b feat/paired-app-website-explainer \
  97da050
```

Expected: a clean worktree at
`/Users/rabble/code/explorations/riot-wt-paired`. If either path or branch
already exists, inspect and reuse it only when it points at this paired work;
never delete or reset an unknown worktree.

`97da050` is the recorded clean merge of the existing iOS explainer
(`b947623`), the initial paired-story spec (`5f6b82d`), and current main at the
time of recovery. It excludes the later unrelated expanded-space commit. After
entering the worktree, incorporate any newer main commit without rewriting
history:

```sh
git merge --no-edit origin/main
```

- [ ] **Step 3: Bring only paired-story documents into the clean branch**

From `/Users/rabble/code/explorations/riot-wt-paired`:

```sh
PLAN_COMMIT="$(
  git -C /Users/rabble/code/explorations/riot \
    log -1 --format=%H -- \
    docs/superpowers/plans/2026-07-20-paired-app-website-explainer.md
)"
git cherry-pick 065fdf2 3573fc2 "$PLAN_COMMIT"
git diff --name-only origin/main...HEAD
```

Expected before implementation: only the existing
`apps/ios/Riot/ConferenceShellView.swift` explainer plus the approved
paired-story spec and plan are listed. Do not cherry-pick `29aa9ac`; Work Units
1–3 finish and correct the pre-existing native slice test-first while unrelated
work remains in its original checkout.

## Work Unit 1: Native story and setup-intent contracts

**Files:**

- Modify: `apps/ios/RiotTests/ShellNavigationTests.swift`
- Modify: `apps/ios/Riot/CommunityShell.swift`

- [ ] **Step 1: Build the current native prerequisites**

Run:

```sh
sh scripts/conference/build-native-core.sh
```

Expected: bindings and simulator/device-appropriate native libraries complete with exit code `0`.

- [ ] **Step 2: Add failing story and intent tests**

Add under the first-run onboarding section of `ShellNavigationTests`:

```swift
func testExplainerStoryPinsOrderedTrustBoundaries() {
    XCTAssertEqual(
        OnboardingExplainerStory.points.map(\.title),
        [
            "No central account or publishing server",
            "Publishing moves peer to peer",
            "Many mirrors, not one site",
            "Signed records, checked in the app",
            "Web for reach; the app for provenance",
        ]
    )

    let copy = OnboardingExplainerStory.points.map(\.body).joined(separator: " ")
    XCTAssertTrue(copy.contains("not anonymous"))
    XCTAssertTrue(copy.contains("display altered text"))
    XCTAssertTrue(copy.contains("false attribution"))
    XCTAssertTrue(copy.contains("accepts as the claimed author"))
    XCTAssertTrue(copy.contains("independently synced record"))
    XCTAssertTrue(copy.contains("not whether its claims are true"))
    XCTAssertFalse(copy.contains("safe to read from"))
    XCTAssertFalse(copy.contains("cannot alter it"))
    XCTAssertFalse(copy.contains("app is proof"))
}

func testWelcomeSetupIntentsAreDistinct() {
    XCTAssertNotEqual(OnboardingSetupIntent.general, .join)
}
```

- [ ] **Step 3: Run the focused test to prove RED**

Run:

```sh
SIMULATOR_ID="$(sh scripts/ios-check.sh simulator-id)"
xcodebuild test \
  -project apps/ios/Riot.xcodeproj \
  -scheme RiotKit \
  -destination "platform=iOS Simulator,id=$SIMULATOR_ID" \
  -derivedDataPath build/ios-derived \
  -only-testing:RiotTests/ShellNavigationTests
```

Expected: compile failure because `OnboardingExplainerStory` and `OnboardingSetupIntent` do not exist.

- [ ] **Step 4: Add the minimal shared story and intent**

Add after `OnboardingStep` in `CommunityShell.swift`:

```swift
public struct OnboardingExplainerPoint: Equatable, Sendable {
    public let title: String
    public let body: String

    init(title: String, body: String) {
        self.title = title
        self.body = body
    }
}

public enum OnboardingExplainerStory {
    public static let points: [OnboardingExplainerPoint] = [
        OnboardingExplainerPoint(
            title: "No central account or publishing server",
            body: "Your identity is a cryptographic key, not a service login. Volunteer seeds, anchors, and mirrors can run on servers, but none owns your identity or is the single place Riot must publish."
        ),
        OnboardingExplainerPoint(
            title: "Publishing moves peer to peer",
            body: "Signed posts move between phones and volunteer seeds. Peer-to-peer does not mean anonymous: devices and infrastructure may observe connections."
        ),
        OnboardingExplainerPoint(
            title: "Many mirrors, not one site",
            body: "Websites are replaceable views, not the authority. A mirror can display altered text or false attribution, but it cannot produce an independently synced signed record Riot accepts as the claimed author."
        ),
        OnboardingExplainerPoint(
            title: "Signed records, checked in the app",
            body: "Riot checks the signature and authorization of the independently synced record. That establishes who signed an unchanged admitted record—not whether its claims are true, current, complete, safe, or endorsed."
        ),
        OnboardingExplainerPoint(
            title: "Web for reach; the app for provenance",
            body: "Use the web to reach readers. When provenance matters, read the independently synced record in Riot instead of trusting what a mirror displayed."
        ),
    ]
}

public enum OnboardingSetupIntent: Equatable, Sendable {
    case general
    case join
}
```

Also update the stale `OnboardingStep.welcome` documentation so it describes the welcome actions rather than claiming **Get started** is the only action.

- [ ] **Step 5: Run the focused test to prove GREEN**

Repeat the Step 3 command.

Expected: `ShellNavigationTests` passes with zero failures.

- [ ] **Step 6: Commit the native contract**

```sh
git add apps/ios/Riot/CommunityShell.swift apps/ios/RiotTests/ShellNavigationTests.swift
git commit -m "test(ios): pin the paired onboarding story"
```

## Work Unit 2: Native presentation and direct join flow

**Files:**

- Modify: `apps/ios/RiotUITests/RiotTabNavigationUITests.swift`
- Modify: `apps/ios/Riot/ConferenceShellView.swift`

- [ ] **Step 1: Extend the isolated UI flow before changing presentation**

At the start of `testCompactCoreFlowFromFirstRunThroughReadingAReport`, after finding `onboarding-get-started`, add:

```swift
let howItWorks = app.buttons["onboarding-how-it-works"]
XCTAssertTrue(howItWorks.waitForExistence(timeout: 5))
howItWorks.tap()

XCTAssertTrue(app.staticTexts["No central account or publishing server"].waitForExistence(timeout: 5))
XCTAssertTrue(app.staticTexts["Web for reach; the app for provenance"].exists)
app.buttons["explainer-done"].tap()
XCTAssertTrue(getStarted.waitForExistence(timeout: 5))

let directJoin = app.buttons["onboarding-join-by-reference"]
XCTAssertTrue(directJoin.waitForExistence(timeout: 5))
directJoin.tap()
XCTAssertTrue(
    app.textFields["join-reference-field"].waitForExistence(timeout: 5),
    "the join welcome action must open the real link/QR join sheet"
)
app.buttons["join-reference-done"].tap()
XCTAssertFalse(app.buttons["find-nearby"].exists)
app.buttons["onboarding-back"].tap()
XCTAssertTrue(getStarted.waitForExistence(timeout: 5))
getStarted.tap()
```

Remove the original later `getStarted.tap()` so setup is entered exactly once after the direct-join round trip. Keep the existing unique `RIOT_UI_TEST_RUN_ID` environment and the nearby-absence assertion.

- [ ] **Step 2: Run the focused UI test to prove RED**

Run:

```sh
SIMULATOR_ID="$(sh scripts/ios-check.sh simulator-id)"
xcodebuild test \
  -project apps/ios/Riot.xcodeproj \
  -scheme Riot \
  -destination "platform=iOS Simulator,id=$SIMULATOR_ID" \
  -derivedDataPath build/ios-ui-derived \
  -only-testing:RiotUITests/RiotTabNavigationUITests/testCompactCoreFlowFromFirstRunThroughReadingAReport
```

Expected: failure because the current sheet uses the old headings and the join action does not directly present `join-reference-field`.

- [ ] **Step 3: Route setup with an explicit intent**

Change `OnboardingView` to retain an intent and provide distinct callbacks:

```swift
private struct OnboardingView: View {
    @ObservedObject var model: RiotAppModel
    @State private var step: OnboardingStep = .first
    @State private var setupIntent: OnboardingSetupIntent = .general

    var body: some View {
        switch step {
        case .welcome:
            OnboardingWelcomeView(
                onContinue: {
                    setupIntent = .general
                    step = .setup
                },
                onJoin: {
                    setupIntent = .join
                    step = .setup
                }
            )
        case .setup:
            OnboardingSetupView(
                model: model,
                intent: setupIntent,
                onBack: { step = .welcome }
            )
        }
    }
}
```

Give `OnboardingWelcomeView` an `onJoin` closure. Keep **Get started** and replace the duplicate old action with:

```swift
Button("Join with a link or QR", action: onJoin)
    .buttonStyle(.riotSecondary)
    .accessibilityIdentifier("onboarding-join-by-reference")
```

Replace both existing welcome paragraphs so no adjacent presentation copy keeps
the old “proof stays in the app” or literal “no servers” claims:

```swift
Text("Reach people on the web. Check record provenance in the app—without a central account or single publishing server.")
    .font(.riot(.body, size: 17, relativeTo: .body))
    .foregroundStyle(RiotTheme.ink(for: colorScheme))
Text("Riot helps a community report what is happening and carry signed records between devices. A browser mirror may display altered text or false attribution; when provenance matters, read the independently synced record in Riot.")
    .font(.riot(.body, size: 15, relativeTo: .callout))
    .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
```

- [ ] **Step 4: Present the existing join sheet from the explicit intent**

Give `OnboardingSetupView`:

```swift
let intent: OnboardingSetupIntent
@State private var hasAppliedInitialIntent = false
```

Replace its existing `.onAppear` with:

```swift
.onAppear {
    displayName = model.claimedName ?? ""
    guard !hasAppliedInitialIntent else { return }
    hasAppliedInitialIntent = true
    if intent == .join {
        isJoinPresented = true
    }
}
```

The one-shot guard prevents dismissing the join sheet from immediately reopening
it. Do not add nearby as an exit. Cancellation returns to the existing setup
screen, where the existing Back toolbar action remains available.

- [ ] **Step 5: Render the shared story with accessible headings**

Remove the view-local `points` tuple and render `OnboardingExplainerStory.points`. Each title must remain a separate heading:

```swift
ForEach(OnboardingExplainerStory.points, id: \.title) { point in
    VStack(alignment: .leading, spacing: 4) {
        Text(point.title)
            .font(.riot(.body, size: 17, relativeTo: .body))
            .foregroundStyle(RiotTheme.ink(for: colorScheme))
            .accessibilityAddTraits(.isHeader)
        Text(point.body)
            .font(.riot(.body, size: 15, relativeTo: .callout))
            .foregroundStyle(RiotTheme.inkSoft(for: colorScheme))
    }
}
```

Wrap the explainer content in `NavigationStack`, matching the existing
`JoinByReferenceSheet` toolbar-hosting pattern, and move **Done** into that
navigation toolbar so it stays reachable at accessibility Dynamic Type sizes:

```swift
var body: some View {
    NavigationStack {
        ScrollView {
            // Existing RiotCard content rendering OnboardingExplainerStory.points.
        }
        .riotHeader(eyebrow: "Riot", "How it works")
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Done", action: onClose)
                    .accessibilityIdentifier("explainer-done")
            }
        }
    }
}
```

- [ ] **Step 6: Run native GREEN checks**

Repeat the focused UI command from Step 2, then run:

```sh
sh scripts/ios-check.sh test
sh scripts/ios-check.sh fast
```

Expected: focused UI flow, macOS RiotKit tests, and shared SwiftUI compile all pass.

- [ ] **Step 7: Commit the native flow**

```sh
git add apps/ios/Riot/ConferenceShellView.swift apps/ios/RiotUITests/RiotTabNavigationUITests.swift
git commit -m "feat(ios): pair onboarding story with direct join"
```

## Work Unit 3: Cross-surface website contract

**Files:**

- Modify: `scripts/marketing/protocol-page-contracts.mjs`
- Modify: `apps/gateway/tests/test_newswire.py`
- Modify: `marketing/index.html`
- Modify: `marketing/public/index.html`
- Modify: `apps/gateway/newswire.py`

- [ ] **Step 1: Add the failing cross-surface contract**

Add both native sources to the existing path map and `Promise.all` reads:

```js
swiftStory: resolve(root, "apps/ios/Riot/CommunityShell.swift"),
swiftPresentation: resolve(root, "apps/ios/Riot/ConferenceShellView.swift"),
```

Extend the read destructuring in the same order:

```js
const [
  home,
  publicHome,
  protocols,
  publicProtocols,
  swiftStory,
  swiftPresentation,
] = await Promise.all([
  read(paths.home),
  read(paths.publicHome),
  read(paths.protocols),
  read(paths.publicProtocols),
  read(paths.swiftStory),
  read(paths.swiftPresentation),
]);
```

Define the expected paired story:

```js
const pairedStory = [
  {
    title: "No central account or publishing server",
    required: ["cryptographic key", "single place Riot must publish"],
  },
  {
    title: "Publishing moves peer to peer",
    required: ["not mean anonymous", "observe connections"],
  },
  {
    title: "Many mirrors, not one site",
    required: [
      "display altered text",
      "false attribution",
      "accepts as the claimed author",
    ],
  },
  {
    title: "Signed records, checked in the app",
    required: ["independently synced record", "not whether its claims are true"],
  },
  {
    title: "Web for reach; the app for provenance",
    required: ["instead of trusting what a mirror displayed"],
  },
];

const howSection = home.match(
  /<section id="how">([\s\S]*?)<\/section>/
)?.[1] ?? "";
assert.match(howSection, /<h2>One story, wherever you meet Riot<\/h2>/);

const primer = howSection.match(
  /<ol class="story-beats"[^>]*>([\s\S]*?)<\/ol>/
)?.[1] ?? "";
const htmlBeats = [...primer.matchAll(
  /<li class="story-beat">([\s\S]*?)<\/li>/g
)].map((match) => match[1]);
assert.equal(htmlBeats.length, 5, "primer must contain five semantic list items");

const swiftBeats = [...swiftStory.matchAll(
  /OnboardingExplainerPoint\(\s*title: "([^"]+)",\s*body: "([^"]+)"/g
)].map(([, title, body]) => ({ title, body }));
assert.equal(swiftBeats.length, 5, "Swift story must contain five explicit points");

for (const [index, { title, required }] of pairedStory.entries()) {
  const htmlBeat = htmlBeats[index];
  const swiftBeat = swiftBeats[index];
  assert.equal(swiftBeat.title, title, `Swift beat ${index + 1} title drift`);
  assert.match(
    htmlBeat,
    new RegExp(`<h3>${title.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}</h3>`)
  );
  assert.match(htmlBeat, /<p>[\s\S]+<\/p>/, `homepage beat ${index + 1} needs body copy`);
  for (const phrase of required) {
    assert.ok(swiftBeat.body.includes(phrase), `Swift beat ${index + 1} missing: ${phrase}`);
    assert.ok(htmlBeat.includes(phrase), `homepage beat ${index + 1} missing: ${phrase}`);
  }
}

for (const rejected of [
  "safe to read from",
  "cannot alter it",
  "app is proof",
  "proof stays in the app",
  "No servers, no accounts",
]) {
  assert.ok(!swiftStory.includes(rejected), `unsafe Swift claim: ${rejected}`);
  assert.ok(!swiftPresentation.includes(rejected), `unsafe Swift presentation claim: ${rejected}`);
  assert.ok(!home.includes(rejected), `unsafe homepage claim: ${rejected}`);
}

assert.match(
  home,
  /<div class="workflow-head"[\s\S]*<h3>What you do<\/h3>[\s\S]*What actually happens, screen by screen/
);
assert.doesNotMatch(
  home,
  /<ol class="story-beats[^"]*\breveal\b/,
  "paired primer must remain visible without JavaScript"
);
```

Scope positive claims to their ordered `.story-beat` list items so copy cannot
move under the wrong heading. Scan the whole native presentation and homepage
for rejected unsafe phrases so contradictory adjacent copy cannot hide outside
the primer.

- [ ] **Step 2: Add failing gateway safety assertions**

Replace the old “Signed, not trusted” assertion in
`test_about_page_covers_the_collective_and_censorship_model` and extend the test
with the corrected positive and negative boundaries:

```python
self.assertIn("Signed records, checked in the app", page)
self.assertIn("independently synced record", page)
self.assertIn("display altered text", page)
self.assertIn("false attribution", page)
self.assertIn("provenance", page.lower())
self.assertIn("not anonymous", page.lower())
self.assertNotIn("safe to read from", page)
self.assertNotIn("cannot alter it", page)
self.assertNotIn("app is proof", page.lower())
self.assertNotIn("peer-to-peer and hidden", page.lower())
```

- [ ] **Step 3: Run both contracts to prove RED**

Run:

```sh
node scripts/marketing/protocol-page-contracts.mjs
python3 -m unittest \
  apps.gateway.tests.test_newswire.NewswireRenderTest.test_about_page_covers_the_collective_and_censorship_model
```

Expected: Node fails because `.story-beats` and Swift safety phrases do not yet match; Python fails on the old gateway claims.

- [ ] **Step 4: Add the paired primer to the marketing homepage**

Under the existing **How it works** section introduction, add:

```html
<ol class="story-beats" aria-label="How Riot works in five parts">
  <li class="story-beat">
    <h3>No central account or publishing server</h3>
    <p>Your identity is a cryptographic key, not a service login. Volunteer seeds, anchors, and mirrors can run on servers, but none owns your identity or is the single place Riot must publish.</p>
  </li>
  <li class="story-beat">
    <h3>Publishing moves peer to peer</h3>
    <p>Signed posts move between phones and volunteer seeds. Peer-to-peer does not mean anonymous: devices and infrastructure may observe connections.</p>
  </li>
  <li class="story-beat">
    <h3>Many mirrors, not one site</h3>
    <p>Websites are replaceable views, not the authority. A mirror can display altered text or false attribution, but it cannot produce an independently synced signed record Riot accepts as the claimed author.</p>
  </li>
  <li class="story-beat">
    <h3>Signed records, checked in the app</h3>
    <p>Riot checks the signature and authorization of the independently synced record. That establishes who signed an unchanged admitted record—not whether its claims are true, current, complete, safe, or endorsed.</p>
  </li>
  <li class="story-beat">
    <h3>Web for reach; the app for provenance</h3>
    <p>Use the web to reach readers. When provenance matters, read the independently synced record in Riot instead of trusting what a mirror displayed.</p>
  </li>
</ol>
<div class="workflow-head reveal">
  <h3>What you do</h3>
  <p>What actually happens, screen by screen</p>
</div>
```

Change the section heading above the primer to **One story, wherever you meet Riot** and explain that the first five beats are the mental model while the workflow follows below.
The primer deliberately does not use `.reveal`: it must remain visible when
JavaScript is unavailable. The existing workflow may retain its progressive
enhancement.

- [ ] **Step 5: Add minimal responsive styling**

Add to the existing `/* ---------- how it works ---------- */` block:

```css
.story-beats {
  list-style: none;
  counter-reset: story;
  margin: 44px 0 0;
  padding: 0;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  border: 2px solid var(--ink);
}
.story-beat {
  counter-increment: story;
  min-width: 0;
  padding: clamp(22px, 4vw, 34px);
  border-right: 2px solid var(--ink);
  border-bottom: 2px solid var(--ink);
}
.story-beat::before {
  content: "0" counter(story);
  display: block;
  margin-bottom: 14px;
  color: var(--pink);
  font: 700 12px/1 'Space Mono', monospace;
  letter-spacing: 0.1em;
}
.story-beat h3 {
  margin: 0 0 10px;
  font-family: 'Anton', sans-serif;
  font-size: clamp(23px, 3vw, 32px);
  line-height: 1;
  text-transform: uppercase;
}
.story-beat p { margin: 0; color: var(--ink-soft); }
.workflow-head { margin-top: 60px; }
.workflow-head h3 { margin: 0; font-size: clamp(28px, 4vw, 44px); }
.workflow-head p { margin: 8px 0 0; color: var(--ink-soft); }
@media (max-width: 700px) {
  .story-beats { grid-template-columns: minmax(0, 1fr); }
  .story-beat { border-right: 0; }
}
```

Use `minmax(0, 1fr)` and `min-width: 0` so 320-pixel layouts cannot overflow.

- [ ] **Step 6: Correct the contextual gateway copy**

Replace the unsafe About paragraphs with copy that preserves the gateway context:

```html
<p class="point"><b>Signed records, checked in the app</b>A mirror can display altered text or false attribution in a browser. It cannot produce an independently synced signed record that Riot accepts as the claimed author. Riot checks record signatures and authorization; that establishes provenance, not whether a claim is true, current, complete, safe, or endorsed.</p>
<p class="point"><b>Publishing moves peer to peer</b>Publishers use Riot; signed posts travel between phones and volunteer seeds. Peer-to-peer does not mean anonymous: devices and infrastructure may observe connections. There is no single publishing server that owns the newswire.</p>
<p class="point"><b>Verify when it matters</b>When provenance matters, open the story in Riot and read the independently synced record instead of trusting what a mirror displayed. The web provides reach; the app checks the record.</p>
```

Keep escaping, CSP, routes, deep links, and projected data unchanged.

- [ ] **Step 7: Refresh the byte-identical deployment mirror**

Run:

```sh
cp marketing/index.html marketing/public/index.html
cmp marketing/index.html marketing/public/index.html
```

Expected: `cmp` exits `0` with no output.

- [ ] **Step 8: Run website and gateway GREEN checks**

Run:

```sh
node scripts/marketing/protocol-page-contracts.mjs
python3 -m unittest \
  apps.gateway.tests.test_newswire \
  apps.gateway.tests.test_build_newswire
```

Expected: marketing contract prints `PASS`; both Python suites pass with zero failures.

- [ ] **Step 9: Commit the paired website story**

```sh
git add \
  scripts/marketing/protocol-page-contracts.mjs \
  marketing/index.html \
  marketing/public/index.html \
  apps/gateway/newswire.py \
  apps/gateway/tests/test_newswire.py
git commit -m "feat(web): publish the paired Riot explainer"
```

## Work Unit 4: Visual, coverage, and release verification

**Files:**

- No production file changes expected.
- If a verification failure requires a fix, return to the relevant RED/GREEN work unit and commit only that scoped fix.

- [ ] **Step 1: Verify the marketing site visually**

Serve the deployment mirror:

```sh
python3 -m http.server 4173 --directory marketing/public
```

Capture and inspect `/` at:

- desktop: `1440 × 1000`;
- mobile: `390 × 844`;
- narrow boundary: `320 × 800`.

Check ordered numbering, heading hierarchy, wrapping, contrast, no horizontal overflow, reduced-motion behavior, and separation between the five-beat story and **What you do**.
Disable JavaScript for one 320-pixel capture and confirm the entire
`.story-beats` primer remains visible.

- [ ] **Step 2: Verify native accessibility**

On the iPhone 17 Pro simulator:

- open **How Riot works** from a fresh first run;
- use an accessibility Dynamic Type size;
- verify all five headings remain reachable in VoiceOver order;
- verify the toolbar **Done** action stays reachable;
- dismiss, open **Join with a link or QR**, close it, and confirm nearby is not offered as an onboarding exit.

Then terminate Riot, disconnect the Mac from Wi-Fi/Ethernet without enabling
any alternate network, relaunch the unique first-run UI-test profile, and open
and dismiss **How Riot works**. Restore connectivity immediately after the
check. The explainer must render and dismiss identically with no server
available; do not run this network-disruption check while another release or
deployment task is active.

- [ ] **Step 3: Run the exact full native gates**

Run:

```sh
sh scripts/ios-check.sh test
sh scripts/ios-check.sh fast
sh scripts/ios-check.sh ios
```

Expected: all exit `0`; the final command confirms iOS device target membership.

- [ ] **Step 4: Run marketing, gateway, and mirror gates**

Run:

```sh
node scripts/marketing/protocol-page-contracts.mjs
python3 -m unittest \
  apps.gateway.tests.test_newswire \
  apps.gateway.tests.test_build_newswire
cmp marketing/index.html marketing/public/index.html
```

Expected: all exit `0`.

- [ ] **Step 5: Run the mandatory coverage source of truth**

Run:

```sh
sh scripts/web/coverage.sh
```

Expected: tarpaulin meets 97% lines, llvm-cov meets the configured line/function/region/branch floors, and JS tooling coverage meets 100%, all read from `.coverage-thresholds.json`.

- [ ] **Step 6: Inspect branch scope**

Run:

```sh
git status --short
git diff --check origin/main...HEAD
git diff --name-only origin/main...HEAD
```

Expected: only the approved spec/plan and declared implementation files are changed. Preserve unrelated existing worktree files such as Xcode user state, overnight logs, caches, screenshots, and Playwright state.

- [ ] **Step 7: Prepare the PR and record the manual release gate**

Push without force and open a PR whose body includes:

- the paired five-beat story;
- RED/GREEN evidence for native and web contracts;
- exact verification results;
- screenshots at desktop, mobile, and 320 pixels;
- an unchecked pre-release item for the five-person, privacy-preserving
  comprehension check from the approved design: at least two community
  organizers and two prospective members, with at least two participants using
  the app explainer and two using the website (roles and surfaces may overlap),
  and four-of-five correct on the three trust boundaries.

Commands:

```sh
git push -u origin feat/paired-app-website-explainer
gh pr create --base main --head feat/paired-app-website-explainer
```

Do not claim the manual comprehension gate passed until humans have actually completed it.
