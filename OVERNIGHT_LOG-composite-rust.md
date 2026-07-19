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

**RESULT: DONE — PR #82.** 8/8 green (existing behavior correctly enforced — these pin it,
not RED-first, since the primitive is already correct; the positive/negative pairing keeps them
non-vacuous). riot-core clippy `--tests` + fmt clean. Baseline: full workspace `--all-features`
GREEN on main `1f6ecb2` (all binaries pass, 1 known-ignored). Committed on branch, pushed, PR #82
opened. Awaiting CI, then self-merge (tests-only, additive, low-risk — within overnight guardrail).

## Task 3 — direct unit tests for `tai_j2000_micros_from_unix_seconds` (DONE, batched into PR #82)
**Why:** the production converter added with the #76 fix was exercised only INDIRECTLY through the
`delegate_editor_section` FFI contract. New file `crates/riot-core/tests/clock_conversion.rs` pins it
directly: (1) agrees exactly with the live `system_snapshot` tai_j2000_micros for the same unix
seconds; (2) one Unix second == exactly 1_000_000 micros (pins the UNIT — the #76 seconds/micros
mixup); (3) output is in the micros domain (>1e12), not seconds; (4) strictly increasing (justifies
the FFI keeping its expiry guard in seconds); (5) pre-J2000 (unix 0 = 1970) fails closed
`ClockUnavailable`; (6) u64::MAX out-of-range fails closed. 6/6 green (the pre-J2000 + overflow
fail-closed edges confirmed, not just asserted). New file, non-contended. Batched onto the same
branch/PR #82 (both are tests-only core hardening) to save a CI cycle; PR retitled accordingly.

## Task 4 — masthead at-rest sealing-envelope boundary (DONE, batched into PR #82)
**Why:** `OwnedMasthead::seal`/`open_sealed` protect the owner's ROOT SECRET at rest
(XChaCha20-Poly1305, `[MAGIC ‖ nonce ‖ ciphertext+tag]`). Inline tests covered only the happy
roundtrip + wrong-key — no tamper / malformed-envelope coverage on a crown-jewel secret. New file
`crates/riot-core/tests/masthead_sealing_boundary.rs`: control (clean blob opens), tag tamper,
ciphertext tamper, nonce tamper (index into the real 24-byte nonce at 8..32, verified against
`MASTHEAD_MAGIC = "RIOTMH\x01\0"`, 8 bytes — an earlier draft flipped a magic byte and mislabeled it),
corrupted magic, truncated + empty, over-long — every rejection is the typed `SealedMastheadInvalid`,
fail-closed, no panic, no partial masthead. 7/7 green. New file, non-contended. Batched into PR #82.

**PR #82 is now 3 tests-only suites** (section authority boundary, clock conversion, sealing boundary)
— all new files under `crates/riot-core/tests/`, no production change, no contention with the active
article-authoring / native-UI sessions.
