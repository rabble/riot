# Overnight Log — 2026-07-19 — composite Rust/FFI lane

> **Why a separate log file:** `OVERNIGHT_LOG.md` at the repo root is actively owned by
> another live overnight session (the iOS-UX lane — their morning summary is on `main`).
> Appending to the same file from my branch would collide on merge. This session logs
> here instead; intent (a repo-root overnight log) preserved. — session on branch
> `overnight/2026-07-19-composite-rust`.

## Setup / bearings
- Branch `overnight/2026-07-19-composite-rust` off `origin/main` (`1f6ecb2`), isolated
  worktree `/Users/rabble/code/explorations/riot-on19`. Shared checkout, many concurrent
  sessions. Never main, pathspec commits, no force-push.
- **Contention map (2026-07-19 night):** iOS-UX lane = active (owns `OVERNIGHT_LOG.md`,
  merged #59 iOS regression fixes). PR #68 = composite Unit 6 native UI (another session).
  New specs just landed on main by others: `2026-07-19-article-authoring-flow-design.md`,
  `2026-07-19-cross-space-activity-log-design.md` → those tracks are CONTENDED.
- **Lane chosen:** Rust core/FFI only — cargo-verifiable, no native shells (guardrail: no
  unsupervised native), and NOT the contended article-authoring / activity-log / native-UI
  tracks. Just landed `delegate_editor_section` FFI (#72) + its TAI/J2000 micros time-unit
  fix (#76) earlier this session.
- Skills: no repo `skills/`; plugin skills (TDD, plan/design gates) are the SOP.

## Task 1 — Rust workspace green-baseline (IN PROGRESS)
Running fmt + clippy --all-targets + `cargo test --workspace --all-features` + validate-contracts
against `main` (`1f6ecb2`). The iOS-UX session verified the iOS half tonight; nobody verified
the Rust half. A RED main blocks every session, so this is the highest-value safe opener.

- fmt: clean.
- clippy `--all-targets`: 3 lints in **xtask TEST code** ("owned instance just for comparison",
  "stripping a prefix manually" ×2). **Not a CI regression** — CI's clippy is
  `cargo clippy --workspace --all-features -- -D warnings` (NO `--all-targets`), so it never
  lints test code; these are pre-existing latent lints CI doesn't gate. Out of my lane (xtask) —
  NOT fixing, flagged for the owner. (A separate cleanup PR could `cargo clippy --all-targets`
  the whole tree and fix ~3 xtask-test lints if desired.)
- workspace test / contracts: result appended below.

## Task 2 — `/articles/` section-delegation authority boundary suite (chosen)
**Why:** the composite security primitives are well-covered EXCEPT an asymmetry — `delegate_listing`
has a full boundary suite (`crates/riot-core/tests/listing_authority_boundary.rs`: receiver-mismatch,
time-escape, cross-region) while the older, security-critical `delegate_section` (the `/articles/`
editor delegation) has only inline succeed + escape-refused tests. Missing at the core level:
**receiver-mismatch** and **time-escape** — and time-escape is the exact bug class that hit the
`delegate_editor_section` FFI in #76 (cap TimeRange built in the wrong time unit → every real entry
escapes). New file `crates/riot-core/tests/section_authority_boundary.rs` (NEW file → no contention
with the article-authoring session that owns `masthead.rs`), all via the real willow25
`into_authorised_entry` path so nothing is tautological. Non-contended, cargo-verifiable, raises the
coverage ratchet on a primitive whose FFI just regressed. 8 tests (predicate, owner-write, section
scope + cross-region negatives, sibling-section, receiver-mismatch, time-escape, belt-escape sweep,
listing-cap-can't-write-article). RED/GREEN + gate result appended below.
