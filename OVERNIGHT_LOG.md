# Overnight Work Log — 2026-07-18 (getting + collaborating with spaces)

## ☀️ MORNING SUMMARY

**Branch `overnight/2026-07-18-collab`** (off origin/main @ ae9ec47), 6 commits, clean tree, NOT
pushed, NOT merged. Separate from the other session's `overnight/2026-07-18` (nav planning).

**Biggest result: the scope was mostly already built — I de-duplicated instead of rebuilding.**
The in-session mapper had read a STALE local checkout and wrongly said comments were missing.
Against real origin/main:
- **Unit 2 (replies/comments): already DONE end-to-end** (core + FFI + Android + iOS all merged).
  Nothing to build. This was the night's biggest save — I nearly rebuilt an existing feature.
- **Unit 1 (invite/share): already BUILT but unmerged & stale** on `feat/ios-share-community`.
  Reviewed it, documented the exact API drift + rebase steps for morning (see Task 3).

**What's DONE & TESTED tonight (committed a96f787):**
- **Reactions record family — Rust core + FFI**, the only genuinely-absent feature. `NewsReactionV1`
  (Support/Solidarity/Important/Grief, toggle), mirrored across all 5 newswire registration sites;
  per-post distinct-author tally on the projection; `toggle_newswire_reaction` FFI + tally on the
  projected post. **Verified independently:** workspace builds; riot-core 159 tests (+14) green;
  riot-ffi reaction contract tests green; clippy + fmt clean; Cargo.lock unchanged.

**What's OPEN / needs a build machine (I could not verify overnight → did NOT commit blind):**
1. **Reactions iOS reaction-bar UI** — planned in full
   (`docs/superpowers/plans/2026-07-18-newswire-reactions-implementation.md`), mirrors the (already
   built) comment UI. Needs Xcode.
2. **UniFFI regen for reactions** — `scripts/conference/build-native-core.sh` MUST regenerate
   bindings + the 5 staticlibs together before any native app runs (checksum coupling), THEN the
   iOS UI. Not done (no build machine overnight).
3. **Unit 1 share merge** — rebase the stale branch onto origin/main, fix the one API-drift line
   (`community.newswireDescriptorEntryID` → `AppModel.newswireDescriptorEntryID`), register the file
   in both Xcode targets, run `scripts/green.sh`, PR. Details in Task 3.
4. **Coverage gate** — run `scripts/web/coverage.sh` before cutting a reactions PR (blocking gate).

**Assumptions to review:** (a) I treated the owner-locked scope (Invite+Replies+Reactions) as the
mandate over the other session's nav track — separate branch to avoid collision. (b) reactions =
new record family (matches comment pattern + the design you approved). (c) closed 4-kind set, no
free-form emoji (canonical CBOR). (d) tombstone drops a post's reactions, hide keeps them.

**Suggested next steps (morning, in order):** regen native core → build the iOS reaction bar (Task
3.1–3.3 of the plan) → green.sh → rebase+PR the share screen → run coverage → PR reactions. QR
scanner-to-join (design had it; share branch is generate-only) is a separate small follow-up.

---

Append-only. Newest entries at the bottom. Morning summary goes at the TOP when done.

Branch: `overnight/2026-07-18-collab` (off `origin/main` @ ae9ec47). Worktree:
`/Users/rabble/code/explorations/riot-wt-collab`. Never main, never force-push.

## Why a separate branch from `overnight/2026-07-18`
Another overnight session already owns `overnight/2026-07-18` (worktree `riot-overnight`) and is
PLANNING spaces-first **navigation** (Rungs 2–5). This session's owner-locked mandate is a
**different** sub-project: make the app WORK for **getting** spaces (invite/share) and
**collaborating** in them (replies/comments + reactions). To avoid clobbering that session on a
shared branch, this work lives on `overnight/2026-07-18-collab`. ~13 sessions share this checkout;
pathspec commits only, no cross-session file edits.

## Owner-locked scope (this session, before the overnight brief)
Sub-project "Get people in, and talk back":
- **Unit 1 — Invite & Share** (getting): generate `riot://newswire/join/...` link + QR + share sheet.
- **Unit 2 — Replies/Comments** (collaborating): threaded replies on posts.
- **Unit 3 — Reactions** (collaborating): toggle reactions on posts.
Owner picked full scope (Invite + Replies + Reactions).

## Bearings — DE-DUPLICATED against origin/main (the local checkout was STALE)
The earlier in-session mapper read a stale local checkout (HEAD 30563cb, behind origin/main
ae9ec47) and wrongly reported "comments don't exist." Against **origin/main** the real state is:

- **Unit 1 (Invite/Share): ALREADY BUILT, unmerged.** Branch `feat/ios-share-community`
  (worktree `riot-wt-share`, 00f0dc1) adds `ShareCommunityView.swift` (233 lines) +
  `ShareCommunityTests.swift` (126) + `CommunityChooser.swift` wiring + both pbxproj targets.
  A forgotten in-flight branch. → **Do NOT rebuild.** Validate + surface for merge.
- **Unit 2 (Comments/replies): core + Android DONE & merged; iOS UI is the gap.**
  Core: `NewsCommentV1`, `create_signed_news_comment`, communal admission (no roster, like a
  post), tombstone/hide, flat grouping under parent (`dangling_and_reply_to_comment_are_dropped`
  = one level only). FFI: `create_newswire_comment`. iOS: `NewswireCommenting` protocol,
  `createNewswireComment`, `NewswireProjectedComment`, `NewswireCommentRow` view-model all exist —
  but there is **no wired thread UI + composer** in the newswire surface (Android has it: #60).
  → **Real build = iOS comment thread UI**, IF the build is verifiable overnight.
- **Unit 3 (Reactions): does NOT exist anywhere.** → full-stack; pure-Rust core is the most
  verifiable overnight deliverable.

## Guardrails adopted (matching the other overnight session's sound discipline)
- Verify before commit. Pure-Rust-core work is `cargo test`-verifiable → safe to build tonight.
- Native (iOS/Swift) code: commit ONLY if `xcodebuild`/`green.sh` is confirmed working on this
  machine (probing early). If not verifiable, PLAN it and leave execution for morning.
- No merges to main, no force-push, no history rewrite, no new deps without logging here.

---

## Log

### Task 1 — bearings + de-dup (done)
See "Bearings" above. Toolchain confirmed: `cargo test -p riot-core --all-features` builds green
(note: bare `cargo test -p riot-core` fails — an integration test needs the `conformance` feature;
always pass `--all-features`, matching CLAUDE.md).

### Task 2 — Reactions Rust core (Unit 3), dispatched
Dispatched a coder subagent to build the `NewsReactionV1` newswire record family in
`crates/riot-core/src/newswire/` with strict TDD, mirroring `NewsCommentV1` across all 5
registration sites (model/path/store-scan/projection/mod) + closed `ReactionKind`
(Support/Solidarity/Important/Grief), toggle via `active` bool, latest-wins dedup per
(author,parent,kind), tally of distinct active authors per kind. Pure Rust, `cargo test`-verifiable.
I verify + commit; subagent does not commit. (Running at time of writing.)

### Task 3 — Unit 1 (Invite & Share) review: READY IN SUBSTANCE, needs rebase + green (NOT committed)
Reviewed `feat/ios-share-community` (`riot-wt-share`, 00f0dc1). The screen is well-built and matches
the design: `ShareCommunityView.swift` (protocol seam `NewswireShareReferencing`; honest
missing-descriptor + mint-failure states; CoreImage `CIQRCodeGenerator` QR — no dep, no camera;
`ShareLink` + Copy cross-platform UIKit/AppKit; themed paper/ink/pink + `riotHeader`/`RiotCard`;
a11y IDs) + `ShareCommunityTests.swift` (126 lines) + chooser wiring + both pbxproj targets.
**Omission vs design:** generate-only — NO QR *scanner* to join (separable follow-up).

**Why NOT landed tonight (safety):** the branch is STALE — merge-base is PR #36 (`f2a33de`), far
behind `origin/main` (`ae9ec47`); `git merge-tree` shows conflicts in composite-site files merged
since. Re-applying Swift + re-registering pbxproj targets blind — with no way to run `green.sh`
overnight — is exactly how the two prior app-target breaks slipped past Linux-only CI (#33, #39).
Not repeating that in the dark.

**API drift the morning rebase MUST fix (confirmed against origin/main):**
- Seam intact: `RiotProfileRepository.newswireShareReference(spaceDescriptorEntryID:)`
  (ProfileRepository.swift:1494); `riotHeader`, `RiotCard` present. The new view file drops in clean.
- The chooser wiring reads `community.newswireDescriptorEntryID` — that property does NOT exist on
  origin/main. The descriptor id now lives on **`AppModel.newswireDescriptorEntryID: String?`**
  (AppModel.swift:175, derived from `repository.listCommunities()…descriptorEntryId`). Present the
  share sheet from AppModel context: `spaceDescriptorEntryID: model.newswireDescriptorEntryID ?? ""`,
  `communityName:` from the active community, `referencing:` = the repository.
- Verify the current `CommunityChooserView` model exposes the repository + active community before
  reusing the branch's `model.community` / `model.profileRepository` wiring; adapt to the current shape.

**Morning steps:** cherry-pick the two NEW files onto a fresh branch off origin/main (new files can't
conflict), redo the chooser hook against the AppModel API above, register the file in both Xcode
targets, run `scripts/green.sh`, then PR. Est. small once green.sh is available.

### Task 4 — Unit 2 (Replies/Comments) is ALREADY DONE end-to-end (correction)
Confirmed against origin/main: comments/replies are fully built at EVERY layer, no work needed.
- Core: `NewsCommentV1`, communal admission, tombstone/hide, flat grouping (merged).
- FFI: `create_newswire_comment` (merged).
- Android: comment row + grouping + threaded into surface (#60, merged).
- **iOS (the layer I feared was missing — it is NOT):** `NewswireEditorial.swift` has
  `commentsByParent: [String:[NewswireCommentRow]]` (L567), `canComment` gate (L625),
  `comments(under:)` (L634), reply signing (L638), `groupComments` (L669), load wires
  `projection.comments` (L759), `@State replyTarget` + `.sheet` → `NewswireCommentComposeSheet`
  (L874/923), a per-post **Reply** button (L1074 `Button("Reply"){ replyTarget = post }`), and
  comment rows drawn indented under each post (L1096–1107). `NewswireCommentRow` struct at L345.
- **The in-session mapper's "comments don't exist" was 100% a stale-checkout artifact** (local HEAD
  was behind origin/main). Nothing to build for Unit 2. This is the night's biggest de-dup.

### Refined remaining scope (only two things are real)
1. **Unit 3 — Reactions:** genuinely absent. Core + FFI are pure Rust → BOTH `cargo test`-verifiable
   and safe to build+commit tonight on this branch. Only the iOS reaction-bar UI needs Xcode → plan
   it (mirror the now-complete comment UI template: a `reactionsByPost` map + a tally bar under
   `postRow`, exactly like `commentsByParent`/`commentRow`). NOTE the UniFFI coupling
   (`riot-uniffi-record-change-coupling`): adding FFI fns changes the checksum, so the binding regen
   + native staticlib rebuild MUST happen together on a build machine before the app runs — that is
   morning work, not tonight's.
2. **Unit 1 — Share:** rebase the stale branch (see Task 3). Morning (needs green.sh).

### Task 2b — Reactions core landed + verified (commit a96f787)
Coder subagent built the core; I verified INDEPENDENTLY (did not self-certify) and fixed the one
out-of-scope break it flagged.
- Fixed `crates/xtask/src/export_newswire.rs` `record_kind()` (E0004 non-exhaustive) — added the
  `NewsReaction => "news_reaction"` arm + a `signed_record_json` block emitting kind/active/parent.
- `cargo build --workspace --all-features` green (2m); `cargo test -p riot-core --all-features` green
  (159, +14 reaction); `cargo test -p riot-ffi --all-features` green.
- Subagent decisions I accepted (see its report): reactions on a **tombstoned** post are dropped but
  a **hidden** post keeps its tally (pinned by a test); `validate_reaction` is an intentional no-op
  guard kept for encode/decode symmetry; `ReactionKind` derives `Ord` for deterministic tally order.

### Task 5 — Reactions FFI landed + verified (same commit a96f787)
Wrote the FFI layer myself (pure Rust, verifiable):
- `NewswireReactionTally { kind: String, count: u32 }` (uniffi::Record) + `reactions:
  Vec<NewswireReactionTally>` on `NewswireProjectedPost`, mapped in `projected_post_view` via
  `reaction_kind_name` (stable lowercase: support/solidarity/important/grief).
- `toggle_newswire_reaction(space_descriptor_entry_id, parent_entry_id, kind, active)` mirroring
  `create_newswire_comment`; `reaction_kind_from_name` rejects unknown kinds as `InvalidInput`
  (a UNIT variant here — an earlier struct-form `{reason}` was my bug, fixed).
- 3 new FFI contract tests in `newswire_contract.rs` (tally on react; toggle-off retracts; unknown
  kind refused) — green. clippy + fmt clean across core/ffi/xtask. `Cargo.lock` unchanged.

**UniFFI coupling reminder (`riot-uniffi-record-change-coupling`):** the added FFI fn + record
changed the interface. Bindings + the 5 native staticlibs MUST be regenerated together
(`scripts/conference/build-native-core.sh`) before ANY native app runs, or it aborts at runtime with
a checksum mismatch. Not done tonight (that step + the iOS UI need a build machine / Xcode).

### Coverage gate note
`.coverage-thresholds.json` is a BLOCKING pre-PR gate (tarpaulin lines / llvm branches). New code is
densely tested (14 core + 3 FFI), so the ratchet should hold, but I did NOT run the full tarpaulin
composite overnight (long, and no PR is being cut on my branch tonight). **Morning:** run
`scripts/web/coverage.sh` (or the CI coverage job) before opening a reactions PR.
