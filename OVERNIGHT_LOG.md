# Overnight Work Log — 2026-07-18 (getting + collaborating with spaces)

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
