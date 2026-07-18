# Interaction Frame — Design

**Sub-project 0 of the Riot interaction-model work.** The through-line that makes the rest of the
app legible: what a *community*, an *app/tool*, and *your identity* are, and — the hero — **how
offline works**, communicated to users without leading with tech. Frames sub-projects 1 (apps),
2 (engage), 3 (manage), each of which gets its own spec.

## Goal
A new activist opens Riot and, without reading a manual, comes away with one clear idea —
**"this works when they cut the internet"** — and picks up the supporting model (no servers,
per-community identity, signed content) exactly when each becomes real. No jargon, no lecture.

## Two decisions that anchor this design
1. **Hero = resilience.** Offline/nearby is the *pitch*, not a caveat. Everything leads with
   "Riot keeps going when the net is cut; posts spread phone-to-phone."
2. **Teaching = short hero upfront + in-context after.** A tight first-run moment lands the hero
   + one line each on the supporting ideas; everything else is taught once, on first encounter,
   in place.

## Design

### A. First-run hero (replaces the bare "WELCOME" onboarding screen)
One hero screen, then three one-liners, then the existing setup (name / create / join / demo —
unchanged, from the onboarding flow already shipped in #29).
- **Hero:** *"When they cut the internet, Riot keeps going."* with a simple phone→phone visual.
- **Three one-liners** (plain, ~5 words, one beat each):
  - *"No servers. Nothing to seize."*
  - *"A separate you in each community."*
  - *"Every post signed — can't be faked."*
- Skippable; shown only at first run (reuses the `Onboarding.isFirstRun` gate from #29).

### B. In-context teaching (taught once, on first encounter, in place, dismissible)
A tiny reusable "first-encounter note" primitive (a dismissible inline cue + a per-concept
"seen" flag persisted locally). Four moments:
- **Offline / nearby — the load-bearing everyday piece.** Replace the deleted
  "offline · local device only" with a *positive resilience* cue driven by the existing
  `connectionStatus`:
  - peers present → **"Reaching N phones nearby"**
  - posting with no net → **"Spreading nearby — no internet needed"**
  - after a nearby sync → **"Synced with N nearby"**
  - nothing happening → quiet/empty (never "you're offline / degraded").
  This is the hero made real; it is the single most important element of the frame.
- **Second community → identity.** First time a person joins/creates a 2nd community:
  *"You're a separate, unlinkable you here — on purpose. What you do in one community can't be
  tied to another."* (Surfaces the per-community-unlinkable model as a safety feature, not a bug.)
- **Verify → signed.** First "Open in Riot" / first signed post viewed: the honest
  *"Verified — checked in Riot, can't be faked"* (the verify sheet from #28) + a one-time why.
- **First post → what happens.** *"Signed and spread to your community. If they publish it, it
  reaches the web too."*

### C. "How Riot works" reference (pull, secondary)
A plain-language, illustrated page linked from Settings + onboarding — a backstop for any concept
not met in context: resilience, no servers, per-community identity, signed content, offline.

## Principle
Lead with resilience; teach the rest exactly when it becomes real; never lead with jargon. The
offline resilience cue (B-offline) is the everyday embodiment of the hero and the highest-priority
element.

## What it touches (surfaces, not new machinery)
- `ConferenceShellView` / `OnboardingView` (#29) — the hero screen + three one-liners.
- `AppModel.connectionStatus` / the connection-disclosure surface — reframed as the positive
  resilience cue (this is where the deleted offline branding lived; #40/#43).
- A small `FirstEncounterNote` primitive (dismissible inline cue + persisted per-concept "seen"
  set) reused by the four in-context moments.
- The identity moment hooks the join/create-2nd-community path; verify reuses the #28 sheet.
- A static "How Riot works" view + a Settings link.
All of it is presentation over existing model state (connection status, community count, verify
outcome) — no protocol or core changes.

## Units (each a small, independent, testable piece)
1. **`FirstEncounterNote`** — the dismissible-once inline cue + persisted seen-set. Pure, unit-
   testable ("shown when unseen; not shown after dismiss; survives relaunch").
2. **Resilience cue** — map `connectionStatus` → the positive nearby strings + placement.
   Unit-test the state→copy mapping (peers/posting/synced/quiet).
3. **First-run hero** — hero screen + three one-liners, gated on `isFirstRun`, skippable.
4. **Identity moment** — the 2nd-community note (trigger + copy).
5. **Verify + first-post notes** — reuse #28 sheet; first-post note on first compose.
6. **"How Riot works" page** + Settings link.

## Testing
RiotKit unit tests for `FirstEncounterNote` (seen-set logic) and the resilience-cue mapping;
a UI check that the hero shows at first run and not after; that each in-context note shows once.
No core/FFI changes, so no cross-language coupling. Runs under `scripts/green.sh` (iOS + macOS
app targets) before merge.

## Out of scope (their own sub-projects)
Apps/tools make-show-see-run (1), engagement/reactions (2), community management (3). This spec is
only the frame that makes those legible.
