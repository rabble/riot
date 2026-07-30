# Branch consolidation: get the stranded work into main

**Date:** 2026-07-30
**Scope:** 15 local branches, 6 worktrees, 1 open PR. No feature work.

## Why this is a mess

Riot is developed by many concurrent agent sessions sharing one checkout plus
worktrees, and `main` takes every PR as a **squash merge**. Those two facts
interact badly:

- A squash rewrites the commit, so a branch's original commits never become
  ancestors of `main`. `git log origin/main..branch` therefore keeps listing
  work that fully landed months ago.
- Sessions branch from each other rather than from `main`, so the same fix ends
  up committed on two or three branches independently.

Result today: 56 commits on `feat/identity-handles-logging-share`, of which
roughly half is already in production.

## How to tell what has actually landed

Ranked by reliability. **Do not use `git merge-base --is-ancestor`** — under
squash merging it reports "not landed" for content that shipped weeks ago.

1. **Sample the added lines.** Take the branch's three-dot diff, strip `vendor/`,
   lockfiles, `*.pbxproj`, and `*.xcuserstate`, keep added lines over 40 chars,
   sample ~15 spread evenly, and `git grep -F` each against `origin/main`. The
   hit ratio is the landed ratio. This is what produced the table below.
2. **Added-file existence.** `git cat-file -e origin/main:<path>` for each file
   the branch adds. Coarser but instant.
3. **Two-dot `git diff origin/main..branch`.** Only trustworthy for the "is it
   *completely* landed" question — an empty diff is proof. Otherwise it is
   swamped by main's own drift and tells you nothing.

Three-dot diffs (`origin/main...branch`) show the branch's own work regardless of
whether it landed, so they size the work but never answer the landed question.

Note: `while read` loops that assign counters behave differently under this
project's zsh default. Run audit loops with an explicit `bash -c`.

## Measured state (2026-07-30, against origin/main 93401401)

| Branch | Added lines | Sampled on main | Verdict |
|---|---|---|---|
| `feat/tor-arti-dial-transport` | 796 | 15/15 | landed |
| `worktree-meadowcap-slice1` | 766 | 15/15 | landed |
| `ci/release-workflow` | 91 | 15/15 | landed |
| `codex/riot-public-store-release-kit` | — | two-dot empty | landed (#155) |
| `fix/riverside-member-tool-uitest` | — | ahead=0 | landed |
| `feat/browser-client-wu001-prepared-update-api` | 3514 | 11/15 | mostly landed; **PR #157 conflicts** |
| `design/composite-site-manifest` | 2382 | 11/15 | mostly landed |
| `feat/identity-handles-logging-share` | 3826 | 8/15 | half landed |
| `feat/onboarding-find-and-explainer` | 698 | 5/15 | mostly stranded |
| `feat/ios-share-community` | 203 | 5/15 | mostly stranded |
| `feat/composite-owner-articles-write` | 842 | 3/15 | stranded |
| `chore/release-0.1.1-artifacts` | 74 | 1/15 | stranded |
| `feat/microapp-wu002c-native-transaction-wiring` | 3325 | 1/15 | stranded, largest |
| `feat/pre-sync-community-preview` | 237 | 0/15 | stranded (docs only) |

## Practices this plan adopts

1. **Never rebase an ancient branch.** Several are 200–400 commits behind. Take
   the three-dot diff as a patch, apply it to a branch cut from current `main`,
   and resolve once. Rebasing replays every intermediate conflict instead.
2. **Land the duplicate once, drop the copies.** `fix(ffi): single OsLogger
   layer` and `fix(ios): drop duplicate ReactionKind.glyph` exist independently
   on both `chore/release-0.1.1-artifacts` and
   `feat/identity-handles-logging-share`. Whichever lands first wins; the other
   copy is dropped, not merged.
3. **Smallest and most urgent first.** Each landed PR shrinks every later diff.
   Release fixes before features before vendor imports.
4. **One concern per PR.** The vendor import ships alone: 87 verbatim upstream
   files should never share a PR with hand-written logic.
5. **Never delete a branch a worktree holds.** Other sessions own those
   worktrees. Report them; let their owner remove them.
6. **Never `git stash` here.** The stash stack is shared across every session in
   this checkout.
7. **Drop editor and coverage droppings when porting.** `*.xcuserstate` is
   gitignored and untracked on `main`; branches that re-add it must have it
   stripped.
8. **CI green before stacking the next PR**, because each one rebases onto the
   last.

## Execution order

### Phase 0 — hygiene (no code movement)

- Update local `main` to `origin/main`.
- `git rm --cached coverage/tmp/*.json` and gitignore `coverage/tmp/` — 17 files
  are tracked on `main` while the working tree churns them constantly, which is
  why every session starts dirty.
- Delete the five landed branches **that no worktree holds**. Report the rest to
  their owners rather than deleting.

### Phase 1 — release unblocking (74 lines)

`chore/release-0.1.1-artifacts` → PR. Carries the TestFlight launch SIGABRT fix
(`logging.rs` single `OsLogger` layer), the duplicate `ReactionKind.glyph`
removal, ASC API-key auth, App Store profile pinning, and a bash 3.2 expansion
fix. Smallest stranded branch and the one blocking a real release.

### Phase 2 — small stranded features

- `feat/pre-sync-community-preview` (237 lines, docs only — trivial).
- `feat/ios-share-community` (203 lines).
- `feat/onboarding-find-and-explainer` (698 lines; 5/15 already landed, so port
  the remainder only).

### Phase 3 — medium stranded features

- `feat/composite-owner-articles-write` (842 lines).
- `feat/microapp-wu002c-native-transaction-wiring` (3325 lines, 17 commits,
  iOS + Android). Largest genuinely-unlanded body of work in the repo. Its
  commits already follow the work-unit pattern with review commits, so it can
  ship as a stack rather than one blob.

### Phase 4 — the identity tangle

`feat/identity-handles-logging-share` splits into:

1. `chore(vendor)`: willow25 0.6.0-alpha.3 + bab_rs 0.8.1 verbatim, feature
   gating, `Cargo.lock` + `fixtures/manifest.json` contract refresh.
2. `feat(core,ffi)`: identity handles, FFI logging init, newswire share
   plumbing.
3. `docs`: designs and plans, including the Android app-to-app distribution
   design.

Drop from it: the arti commits (landed), the two release fixes (Phase 1), and
the re-added `*.xcuserstate`.

### Phase 5 — resolve PR #157

`feat/browser-client-wu001-prepared-update-api` is `CONFLICTING`/`DIRTY` against
main **and has no CI checks at all**. It is also already merged into the identity
branch, so Phase 4 would land its content under a different SHA and leave #157
permanently dirty. Decide explicitly: either port it fresh onto main as the
canonical home and close #157, or exclude it from Phase 4 and rebase #157.
Do not let both land.

## What "done" looks like

`main` carries every stranded change above; the branch list is the five active
worktrees plus `main`; `git status` is clean on a fresh checkout; and
`.coverage-thresholds.json` floors still pass.
