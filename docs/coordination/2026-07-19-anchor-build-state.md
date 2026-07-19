# Anchor network build — execution state & resume steps (2026-07-19)

**Resume doc for the public-community-anchor-network build.** Any agent can pick up from here.
Branch: **`overnight/2026-07-18-anchor-protocol`** (checked out in the MAIN repo dir
`/Users/rabble/code/explorations/riot`). Plan:
`docs/superpowers/plans/2026-07-18-public-community-anchor-network-implementation.md` (now committed;
read its **Coordinator Addendum** for the M1–M5 milestone phasing + pilot deferral). Design spec:
`docs/superpowers/specs/2026-07-18-public-community-anchor-network-design.md`.

## Shared-checkout rules for THIS branch (read first)
- The branch lives in the main checkout. It carries KNOWN-IGNORABLE dirty files (`package.json`,
  `package-lock.json`, `*.xcuserstate`) — the plan's Delivery Rules say leave them alone. **Commit
  only your work via explicit pathspec**, never `git add -A`.
- One coder subagent per work unit, scoped to `crates/riot-anchor-protocol/` (until M2 opens other
  crates). Subagents do NOT commit — the coordinator verifies (`cargo test`) + commits via pathspec.
- Do NOT run two cargo builds in the same target dir at once (contention). Serialize units that
  share files (`lib.rs`/`records.rs`/`schema` are shared by most protocol WUs).

## Work-unit status (M1 = WU-001–007, protocol + transport)
| WU | State | Commit |
| --- | --- | --- |
| 001 dependency boundary | ✅ done | `9c71274` |
| 002 canonical codec + digests | ✅ done | `0f4c051` |
| 003A core listing authority boundary | ✅ done | `145283c` |
| 003B tickets/listings/authority | ✅ done + security-reviewed | `584b5a6` |
| 004 descriptors/receipts/82-limits/control | ✅ done | `d1304e9` |
| 005 routed paginated `sync/2` | ✅ done (133 crate tests) | `80a8a2a` |
| WU-003B security hardening | ✅ done (lifetime-cap + indefinite tests; phantom-guard doc) | `00a6a5f` |
| 006A/006B conformance vectors (Rust/TS + native) | ⛔ **NEXT — blocked on the pre-006 confirmation checklist below** | — |
| 007 multi-ALPN iroh router | ✅ done (`AlpnRouter`, bounded lifecycle, sync/1 wrapper preserved; 14 tests) | `2da6200` |
| riot-ffi latent workspace break | ✅ fixed (WU-003A added a `WillowError` variant; riot-ffi match was non-exhaustive) | `0b19d06` |

**M1 is 6/7 COMPLETE + hardened.** Only **WU-006** remains — gated on the pre-006 checklist (the four `EnabledRole` tokens).

### ⚠️ VERIFICATION LESSON (do this from now on)
The workspace build was RED from WU-003A (`145283c`) to WU-007 because every WU only ran
`cargo test -p riot-anchor-protocol` (its scope) — nobody built the workspace, so a cross-crate break
(riot-core added `WillowError::DelegationAreaEscapesDirectory`; riot-ffi's match went non-exhaustive)
sat undetected for ~7 commits. **When a WU touches a `riot-core` enum (or any type other crates match
on), the coordinator MUST run `cargo build --workspace --all-features` before committing** — a scoped
`cargo test -p <crate>` cannot see downstream breakage. This is the same failure class as
`[[riot-uniffi-record-change-coupling]]` but at the Rust-match layer.

## NEXT STEPS (in order)
1. **When WU-005 lands:** run `cargo test -p riot-anchor-protocol --all-features` (independent verify),
   `cargo clippy … -D warnings` (clean on new files; ~23 pre-existing riot-core no-default-features
   warnings are unrelated — ignore), `cargo fmt --check`. If it added a dep, refresh
   `fixtures/manifest.json` `cargo_lock_sha256` (run `cargo run -q -p xtask -- validate-contracts`, it
   prints the actual sha) and include `Cargo.lock` + `fixtures/manifest.json` in the commit. Commit via
   pathspec: `feat(anchor): add routed paginated sync v2 (WU-005)`.
2. **Apply the queued WU-003B security hardening** (see `docs/research/2026-07-19-wu003b-security-findings.md`):
   close 3 test gaps in `tests/authority_records.rs` (90-day cap; real indefinite-length rejection;
   drive `admit`'s oversize/structure branch) + add `resolve_listing` self-check (re-decode embedded
   `ticket_core_bytes`, assert listing↔ticket coordinate equality). Verify + commit
   `fix(anchor): harden ticket/listing authority tests + listing self-check`. (Edits `authority.rs` +
   `authority_records.rs` only — safe once WU-005 is done.)
3. **Resolve the pre-WU-006 confirmation checklist (below) with the owner BEFORE building 006** — 006
   freezes these into Swift/Kotlin/TS conformance vectors; wrong = cross-language rework.
4. Then WU-006A/006B, then WU-007 → M1 complete → consider a PR for the whole M1 protocol slice.

## Pre-WU-006 confirmation checklist (invented / under-specified wire decisions — OWNER CONFIRM)
The design gives field lists but not always byte layout; these were minted following the WU-002
canonical conventions and MUST be confirmed before 006 freezes vectors:
- **WU-003B (`584b5a6`):** `SITE_MANIFEST_DIGEST_LABEL`, `TERMINAL_CAPABILITY_DIGEST_LABEL`, and the
  manifest-digest preimage (design binds a manifest digest but never specifies its preimage). All
  domain-separated + fail-closed (safe), but frozen at 006.
- **WU-004 (`d1304e9`):** `EnabledRole` tokens — design says "roles have at most the four defined
  values" (spec line 1353) but never enumerates them; builder used `host/mirror/directory/gossip`
  (last two inferred). **CONFIRM the exact four.** `HostingStatus` vocab = only `committed` (matches
  the Commit terminal outcome `["committed", hosting_receipt]`, low risk). `*BodyV1` lead-with-version
  layouts (WU-002 convention).
- **WU-005 (`80a8a2a`):** frame positional layouts (leading snake_case frame-name discriminant,
  implicit v2, no per-frame version int); opaque `ticket_core_bytes`/`session_id`(≤32)/`entry_id`(≤128)
  bounds; `EntriesChunk.bundle_bytes` = canonical array of per-item byte strings (≤64 items/≤2 MiB
  each/≤8 MiB total); `MAX_SYNC2_FRAME_BYTES` = 8 MiB + 64 KiB framing slack. No new digest labels
  (reused `riot/sync-snapshot/v2` + `riot/sync-ids-page/v2`).

## Security posture (from the WU-003B trust-root review)
`admit_public_site_ticket` is strong (no forge/downgrade/replay found). `resolve_listing` is a pure
state machine that trusts upstream verification and **cannot self-defend** — the delegate-grant
signature isn't even in the envelope. **This is a WU-015 acceptance criterion:** the willow25-owning
caller MUST verify entry + grant + Meadowcap-capability signatures (and that the listing entry was
signed by `root_id`) before constructing an `AdmittedListingEnvelopeV1`, and the precondition should
live in the TYPE SYSTEM (a verified proof token), not a doc comment. Full detail:
`docs/research/2026-07-19-wu003b-security-findings.md`.

## Milestone reminder (from the plan addendum)
M1 protocol+transport (001–007, current) → M2 hosting MVP (008–016) → M3 directory+web+handoff
(017–021) → M4 native UX (022–023) → **M5 pilot DEFERRED** (024–025, own track, needs human
coordinators + signed public-pilot fixtures). Native "Final Verification" gates are LOCAL-only (CI is
Linux); WU-028's CI additions must be the dependency-graph assertions, not device tests.
