# Writable Public-Newswire Browser Client — Master Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the smallest honest browser version of Riot — a Chromium-only static PWA that creates or opens one public community, prepares and locally signs newswire updates, preserves accepted state across offline reloads, and imports/exports native-compatible `.riot-evidence` files.

**Architecture:** Four layers, top to bottom: `apps/web` (framework-free static PWA: DOM, IndexedDB, localStorage, Web Locks, service worker) → `riot-web` (thin wasm-bindgen adapter, no independent state) → `riot-client` (browser-neutral workflow and state machine, pure Rust library) → `riot-core` (Willow, Meadowcap, signing, verification, admission, projection — unchanged authority). The IndexedDB accepted-bundle log is durable truth; Rust state is reconstructed by replaying every bundle through the ordinary verification and admission path on startup. A vendored `willow25` patch makes its filesystem store optional so the Wasm dependency graph excludes `fjall`/`lsm-tree`/`async-fs`.

**Tech Stack:** Rust 1.95.0 (pinned `rust-toolchain.toml`), wasm-bindgen-cli 0.2.126, willow25 `=0.6.0-alpha.3` (vendored + patched), Node 26.4.0 / npm 11.17.0 (`node --test`, c8 11.0.0), Playwright 1.61.1 (real Chromium), no JS bundler, no runtime framework, no third-party runtime JS.

**Source design:** `docs/superpowers/specs/2026-07-24-writable-public-newswire-browser-client-design.md` at approved commit `8521a324` ("docs: record browser client design approval").

---

## Scope and decomposition rule

This master plan fixes scope, stable interfaces, file ownership, and work-unit ordering. It does **not** embed TDD task detail. Each work unit gets its own detailed plan written **immediately before its execution**, named `2026-07-24-writable-public-newswire-browser-client-wuNNN-<slug>.md`, containing bite-sized RED→GREEN→commit steps with complete code. Each unit plan passes the three-reviewer plan-review-gate before execution begins. No unit starts before its hard dependencies complete.

In-scope subsystems (from spec "In scope", all required): one active public community; create/open/import flows; preview-then-accept updates; compose/review/sign/commit; IndexedDB + localStorage persistence; offline reload; `.riot-evidence` import/export; file-only exchange; Chromium desktop + Android Chromium.

Out of scope (spec "Out of scope", binding): Safari/Firefox; multiple communities; private groups; any live transport; accounts/key recovery/rotation/revocation; sites/articles/governance/miniapps; push/analytics/deployment/hosting; hardened-key-storage claims.

## Verified baseline (research findings that shape this plan)

Codebase verified 2026-07-27 against branch `feat/identity-handles-logging-share` (tip `dd3f4ac2`, contains spec commit `8521a324`) — the execution base declared in §Execution isolation. All file:line citations below are from that branch. These findings override assumptions; unit plans must respect them.

1. **Workspace:** members at root `Cargo.toml:2-12` — `riot-client` and `riot-web` do not exist; `apps/` has no `web`. `riot-core` features (`crates/riot-core/Cargo.toml:7-24`): `default = ["sqlite"]`, `sqlite = ["dep:rusqlite"]`; wasm32 `getrandom` with `js` feature already wired (`crates/riot-core/Cargo.toml:49-50`). `riot-anchor-protocol` and `riot-transport` already consume `riot-core` with `default-features = false` — the Wasm graph precedent.
2. **willow25 is NOT vendored.** It comes from crates.io (`Cargo.lock:8049` on the base branch), with non-optional `async-fs 2.2.0` and `fjall 3.0.3` deps; `storage/mod.rs:16-24` gates `persistent_store` only on `std`. The spec's vendor patch (new `persistent-storage` feature gating `persistent_store` + `PersistentStore` re-export behind `all(feature = "std", feature = "persistent-storage")`) matches source reality and is WU-000. The base branch has **no** `[patch.crates-io]` section, `vendor/` directory, or workspace `exclude` yet — a tor-dirmgr vendor-patch precedent exists on unmerged branch `fix/riverside-member-tool-uitest` (`vendor/tor-dirmgr-0.44.0/RIOT_PATCH.md` convention: verbatim upstream source, provenance + license + integrity notes, one-justification patch). WU-000 establishes the first vendor patch on the base branch following that convention.
3. **Prepare→review→post-later needs a new public riot-core API.** All `create_signed_*` (`crates/riot-core/src/newswire/entry.rs:290-341`) take `system_snapshot()` at call time; shared `build_signed` (entry.rs:142) is private; injectable clocks exist only under the `conformance` feature, which is forbidden in the release graph. WU-001 adds a public prepare/sign-split API.
4. **Whole-file import rejection is riot-client's job.** `decode_bundle` (`crates/riot-core/src/import/bundle.rs:276`) isolates per-item (`ItemStatus::Invalid`) and `inspect` declines to carry ineligible items into the preview (`session.rs:834`, "Ineligible items are simply not carried into the preview"). Spec §Import-2 demands atomic whole-file rejection before selection; riot-client gates on `DecodedBundle.items` itself.
5. **Sealed identity requires a caller wrapping key** (`willow/identity.rs:86,114`); no plaintext/profile serialization exists. `EvidenceAuthor` fields are private; there is no raw-parts constructor. riot-client owns a versioned profile envelope (Stable interface I-1) without new core crypto surface. `generate_communal_author_for_namespace` (identity.rs:266) is the fresh-member-identity factory for clean-browser join; `generate_space_organizer_author` (identity.rs:243) for create.
6. **`MemoryEvidenceStore` is `pub(crate)`** (`store/memory.rs:7`); riot-client reaches memory behavior only via `RiotSession::open()` (`session.rs:375`). The capability/signature drop after admission lives in the private `Stored` struct (`import/join.rs:55`, documented at `join.rs:47-64`) — this is why riot-client owns a separate exact-proof ledger for export.
7. **`commit_at` is `pub(crate)`** (`session.rs:913`). riot-client posts via the public sequence: `create_signed_*` → `encode_bundle` → `inspect` → `preview.plan(...)` → `plan.commit()`.
8. **`VerifiedNewswireRecord` is `#[non_exhaustive]` with private fields** (`newswire/entry.rs:64-72`) — DTO extraction goes through its accessors only; riot-client never constructs it.
9. **Ceilings already exist in core** matching spec: `MAX_BUNDLE_BYTES = 8 MiB`, `MAX_BUNDLE_ENTRIES = 64` (`import/bundle.rs:27-42`), `MAX_PROJECTED_RECORDS = 1024` (`newswire/projection.rs:12`), `MAX_NEWSWIRE_PAYLOAD_BYTES = 128 KiB` (`newswire/model.rs:15`). The 128 KiB payload bound (not the 1 MiB bundle-item bound) keys compose limits. The descriptor consumes one of the 64 export entries; expiry never prunes (`projection.rs:543-556`).
10. **Projection clock is wall-clock only** (`ProjectionClockV1::system()`, `projection.rs:46`) outside conformance. Replay projection is therefore not time-invariant (expired/open-wire split moves with the clock). Accepted as spec-consistent: projection is a view, not evidence; the bundle log is truth.
11. **Bundle component bytes** are available as `SignedWillowEntry { entry_bytes, capability_bytes, signature, payload_bytes }` (`willow/entry.rs:32-38`) and per-item `BundleItemFrame` accessors (`import/bundle.rs:103-135`) — the raw material for the exact-proof ledger. `encode_bundle` (`bundle.rs:181`) is the validating re-encoder for export.
12. **JS/infra precedent:** no service worker, PWA manifest, or `apps/web` anywhere; hardened static dev server `scripts/web/serve.mjs` already emits the spec's exact CSP/Permissions-Policy/Referrer-Policy/nosniff headers and knows `.wasm`/`.webmanifest` MIME types; Playwright precedent `scripts/apps/playwright.config.mjs`; Node tests via `node --test` with c8 at 100% floors; `scripts/web/bootstrap.sh` already pins the wasm target + wasm-bindgen-cli 0.2.126; CI has rust/coverage/gateway/web/android jobs but no wasm/browser-test job. `.coverage-thresholds.json` is the ratchet source of truth and is never lowered.
13. **Workspace release profile is `panic = "unwind"`** (root `Cargo.toml:67-68`) — no per-target panic=abort config exists; Wasm panics surface via wasm-bindgen as JS exceptions, which `apps/web` treats as terminal client errors per spec.

## Execution isolation

Each work unit executes in an isolated worktree created via the `superpowers:using-git-worktrees` skill at execution time, on a branch named `feat/browser-client-wuNNN-<slug>`. **Base branch is `feat/identity-handles-logging-share` (contains spec commit `8521a324`), not `main`:** the approved spec and the identity-handles core work exist only on that branch as of this plan; they are not ancestors of `main` or of `fix/riverside-member-tool-uitest`. Commits use explicit `git add <paths>` (never `git add -A`/`git add .`), pathspec-limited to the unit's file ownership. Conventional-commit messages. Never `--no-verify`. TDD is mandatory: every behavior lands test-first, observed failing.

## Stable repository interfaces

Unit plans may refine names but must not change these contracts without amending this master plan and re-running the plan-review-gate.

### I-1 — Signing profile envelope (riot-client → opaque bytes to host)

A versioned canonical-CBOR record, schema id `org.riot.signing-profile/1`, containing: profile format version; namespace id (32 bytes); subspace secret (32 bytes, plaintext — the spec's explicit prototype concession); created unix-seconds; identity kind (organizer | member). Byte ceiling 256 KiB (actual size ≈ 200 bytes). JavaScript stores it base64 in one versioned `localStorage` record and never parses it. Reconstruction uses existing tested `riot-core` identity APIs; no new core crypto surface. The envelope, docs, and UI all state clearing site data destroys posting authority.

### I-2 — riot-client public API surface

One `PublicNewswireClient` per process, owning the state machine. Operations (spec §Interface shape, Rust names final at WU-002):

```text
create_community(title) -> PendingCreate           // profile bytes + signed descriptor bundle + digest
confirm_profile_and_bundle_saved(profile_id, bundle_digest) -> CommunityView
resume_pending_profile(pending_operation) -> PendingState
restore(profile_bytes, community_record, ordered_bundles) -> CommunityView
prepare_update(draft) -> UpdateReview              // immutable, single-use, state-bound
post_review(review_id) -> PendingBundle            // signs exactly the prepared bytes
preview_import(bundle_bytes) -> ImportReview       // whole-file atomic rejection; no mutation
join_reviewed_community(review_id, selected_entry_ids) -> PendingJoin
accept_import(review_id, selected_entry_ids) -> PendingBundle
acknowledge_bundle_saved(bundle_digest) -> CommunityView
list_updates() -> ProjectedUpdates
export_community() -> Vec<u8>                      // one canonical .riot-evidence artifact
close()
```

Rules: mutation producing a new canonical bundle enters pending-persistence; further mutation blocks until that exact bundle digest is acknowledged. Review ids are single-use and invalidated by state change. Every post/import is prospectively applied to cloned state and must prove the resulting live proof set still encodes as one valid canonical export within all ceilings before commit is offered (`BROWSER_CAPACITY_EXCEEDED` otherwise). The exact-proof ledger retains verified entry/capability/signature/payload component bytes per accepted entry plus live/pruned selection; export passes exact live bytes through `riot-core`'s validating `encode_bundle`.

### I-3 — riot-web Wasm boundary

wasm-bindgen adapter; no independent client state; no UniFFI/riot-ffi dependency. Inputs: bounded JS strings/byte arrays. Outputs: versioned DTOs (schema-tagged) and stable SCREAMING_SNAKE error codes (including `BROWSER_CAPACITY_EXCEEDED`, `INVALID_IMPORT`, `NAMESPACE_MISMATCH`, `DESCRIPTOR_MISMATCH`, `STALE_REVIEW`, `PENDING_PERSISTENCE`, `STORAGE_CORRUPT`, `TERMINAL`). Wasm panics surface as JS exceptions and are terminal. Wasm runs on the page main thread for this slice.

### I-4 — IndexedDB schema v1 (`apps/web`)

Database `riot-web`, version 1, fixed stores and keys only:

- `meta` — active-community record (canonical-CBOR Blob ≤ 64 KiB); schema + application-release versions.
- `bundles` — ordered append-only log; each record: sequence, exact bundle bytes (Blob), actual byte length, SHA-256 digest, accepted entry IDs.
- `manifest` — single record: ordered record digests + aggregate counts (canonical-CBOR Blob ≤ 64 KiB).
- `drafts` — durable composer drafts.
- `operations` — pending create/join two-phase records (`prepared` | `committed`), including pending profile bytes for crash recovery.

Startup walks `bundles` with a cursor capped at 257 records, checks `Blob.size` before every read, stops before 32 MiB aggregate, recomputes every digest, compares sequence/digest order against `manifest`, and never sizes reads from manifest counts. Transactions request `durability: "strict"`.

### I-5 — localStorage record v1

Single key `riot.signing-profile.v1`: JSON `{ version: 1, state: "pending" | "active", profileId, namespaceId, descriptorEntryId, profileB64 }`. Compare-and-set on every field during the two-phase create/join transaction (spec §Cross-store create and join transaction, steps 1–6, is binding verbatim).

### I-6 — Release manifest + service worker contract

Build emits `release-manifest.json`: `{ releaseId, assets: [{ url, bytes, sha256 }] }` covering every authored/generated asset. Build-time static-root contract fails on any file absent from the manifest. Worker install: fetch every listed response with `cache: "reload"`, verify actual bytes against manifest, populate release-named cache; any mismatch aborts install. No `skipWaiting`, no `clients.claim`; activation only after all old-release clients are gone; UI reports **Update ready — close all Riot tabs and reopen**.

### I-7 — Browser ceilings (binding, from spec)

```text
profile record                         256 KiB
one canonical bundle                    8 MiB / 64 entries
accepted-bundle log                    256 records / 32 MiB aggregate
accepted entries across replay       1,024
live entries eligible for export        64
one consolidated export                 8 MiB
metadata/manifest records               64 KiB each
newswire payload per post              128 KiB (MAX_NEWSWIRE_PAYLOAD_BYTES)
```

### I-8 — Security headers

`scripts/web/serve.mjs` already emits the spec's exact baseline header set; WU-006 adds a contract test asserting byte-exact equality with the spec block, plus COOP/COEP-class headers (`Cross-Origin-Opener-Policy: same-origin`, `Cross-Origin-Resource-Policy: same-origin`) which serve.mjs must gain if missing.

## File ownership map

| Work unit | Owns (exclusive) |
|---|---|
| WU-000 | `vendor/willow25-0.6.0-alpha.3/**`, root `Cargo.toml` `[patch.crates-io]` + `exclude`, `Cargo.lock`, `scripts/web/check-wasm-graph.mjs` (created here, extended in WU-003) |
| WU-001 | `crates/riot-core/src/newswire/entry.rs` (implementation + colocated `#[cfg(test)]` module — tests live inline in this crate), `crates/riot-core/src/newswire/mod.rs` (exports) |
| WU-002 | `crates/riot-client/**`, root `Cargo.toml` members list (add), `Cargo.lock` |
| WU-003 | `crates/riot-web/**`, `scripts/web/build-wasm.sh` (new), `scripts/web/check-wasm-graph.mjs` (extend to full forbidden-list), root `Cargo.toml` members list (add), `Cargo.lock` |
| WU-004 | `apps/web/src/storage/**`, `apps/web/src/lock.js`, `apps/web/test/storage/**`, `package.json` (test scripts) |
| WU-005 | `apps/web/src/ui/**`, `apps/web/src/main.js`, `apps/web/index.html`, `apps/web/src/styles.css`, `apps/web/test/ui**` |
| WU-006 | `apps/web/sw.js`, `apps/web/manifest.webmanifest`, `scripts/web/build-release.mjs` (new), `scripts/web/serve.mjs` (header gap only), `apps/web/test/sw/**` |
| WU-007 | `apps/web/e2e/**`, `scripts/web/playwright.config.mjs` (new), `.github/workflows/ci.yml` (browser-test job), `.coverage-thresholds.json` (add apps-web scope; floors never lowered) |

Shared files (root `Cargo.toml`, `Cargo.lock`, `package.json`, CI) are touched only in the owning unit's step, with explicit `git add`.

## Work-unit arc

| WU | Title | Hard dependencies | Completion artifact |
|---|---|---|---|
| WU-000 | willow25 vendor patch | none | Wasm graph free of fjall/lsm-tree/async-fs; native graph unchanged; provenance docs |
| WU-001 | riot-core prepared-update API | WU-000 | Public prepare/sign-split post API; prepared bytes == signed bytes proven by test |
| WU-002 | riot-client crate | WU-001 | `PublicNewswireClient` full state machine, native-tested, at coverage floors |
| WU-003 | riot-web Wasm adapter | WU-002 | Locked wasm32 build + DTO/error contract + graph contract script |
| WU-004 | apps/web storage host | WU-003 | IndexedDB log + localStorage profile + two-phase txn + startup walk + Web Lock, Node-tested helpers |
| WU-005 | apps/web UI flows | WU-004 | All spec user flows + recovery panels + a11y contracts |
| WU-006 | Service worker + release pipeline | WU-005 | Content-hashed release manifest, verified precache, offline reload, header contract test |
| WU-007 | Chromium end-to-end + CI | WU-006 | Every spec "Browser contracts" bullet automated in real Chromium; CI job; thresholds updated |

Ordering notes: WU-001 depends on WU-000 only because both touch `Cargo.lock`; logically independent otherwise — keep order to avoid lock conflicts. WU-004/WU-005 could overlap but stay sequential: the two-phase transaction and startup walk are the highest-risk browser code and must settle before UI lands on them.

## WU-000 — willow25 vendor patch

**Files:**
- Create: `vendor/willow25-0.6.0-alpha.3/**` (verbatim crates.io source), `vendor/willow25-0.6.0-alpha.3/RIOT_PATCH.md`
- Modify: `vendor/willow25-0.6.0-alpha.3/Cargo.toml` (fjall/async-fs optional behind `persistent-storage`), `vendor/willow25-0.6.0-alpha.3/src/storage/mod.rs` (gate module + re-export)
- Modify: root `Cargo.toml` (`[patch.crates-io] willow25`, workspace `exclude`), `Cargo.lock`
- Create: `scripts/web/check-wasm-graph.mjs` (graph contract assertion; extended in WU-003)

- [ ] Vendor upstream source; verify checksum against `Cargo.lock` (`willow25 0.6.0-alpha.3`, `:8049` on the base branch); record provenance, license retention, and per-file integrity manifest in `RIOT_PATCH.md`
- [ ] RED: graph contract test asserting `cargo tree` for a `default-features = false` riot-core consumer contains no `fjall`, `lsm-tree`, `async-fs` — watch fail
- [ ] Patch: `persistent-storage` feature; gate `persistent_store` module + `PersistentStore` re-export on `all(feature = "std", feature = "persistent-storage")`; riot-core's native/default consumers enable it, Wasm graph disables it
- [ ] GREEN: graph contract passes; `cargo test --workspace --all-features` green (native behavior unchanged)
- [ ] `xtask validate-contracts` updated if it pins willow25 metadata; `fixtures/manifest.json` `cargo_lock_sha256` refreshed
- [ ] Commit

## WU-001 — riot-core prepared-update API

**Files:**
- Modify: `crates/riot-core/src/newswire/entry.rs` (public prepare/sign-split; refactor private `build_signed` at entry.rs:142 to accept a pre-encoded payload + `ClockSnapshot`)
- Modify: `crates/riot-core/src/newswire/mod.rs` (exports)

- [ ] RED: test that `prepare_news_post(author, descriptor, post) -> PreparedNewsPost` captures canonical payload bytes + `ClockSnapshot`, and that `sign_prepared_news_post(author, &prepared)` signs exactly those bytes (byte-identical payload, identical timestamp) — watch fail
- [ ] RED: test that two `sign_prepared_news_post` calls on the same prepared value are deterministic, and that prepared values are author-bound
- [ ] Implement minimal public API; keep existing `create_signed_*` behavior untouched (they delegate to the same core)
- [ ] GREEN; clippy/fmt; coverage floor holds
- [ ] Commit

## WU-002 — riot-client crate

**Files:**
- Create: `crates/riot-client/Cargo.toml`, `crates/riot-client/src/lib.rs`, `crates/riot-client/src/profile.rs` (I-1 envelope), `crates/riot-client/src/client.rs` (state machine), `crates/riot-client/src/review.rs` (immutable reviews), `crates/riot-client/src/ledger.rs` (exact-proof ledger), `crates/riot-client/src/capacity.rs` (I-7 prospective checks), `crates/riot-client/src/export.rs`, `crates/riot-client/src/dto.rs` (versioned DTOs + error codes per I-3), `crates/riot-client/tests/**`
- Modify: root `Cargo.toml` members, `Cargo.lock`

Covers spec sections: `riot-client` architecture, exact-proof ledger, prospective capacity proof, pending-persistence acknowledgement, whole-file import rejection (baseline finding 4), descriptor pinning (namespace + exact descriptor entry ID), cross-community/mixed-record/multiple-descriptor atomic rejection, replay reconstruction via ordinary admission path, export via validating `encode_bundle`.

- [ ] RED→GREEN per unit plan for: profile envelope round-trip; create→pending→ack gating; restore/replay equivalence (same projection, same signer relationship); prepare/post immutability and single-use reviews; post blocked until ack; import preview atomic rejection; clean join mints fresh member identity, never impersonates imported authors; accept-import append + ack; capacity refusal (`BROWSER_CAPACITY_EXCEEDED`) at each I-7 ceiling; ledger byte-exactness across replay; export re-enters native admission path on a clean session
- [ ] Coverage gate at `.coverage-thresholds.json` floors
- [ ] Commit

## WU-003 — riot-web Wasm adapter

**Files:**
- Create: `crates/riot-web/Cargo.toml` (`crate-type = ["cdylib"]`, wasm-bindgen 0.2.126), `crates/riot-web/src/lib.rs` (I-3 boundary), `scripts/web/build-wasm.sh` (locked `cargo build --locked --release -p riot-web --target wasm32-unknown-unknown` + `wasm-bindgen`), `scripts/web/check-wasm-graph.mjs` (full forbidden-list per spec §Testing: fjall, lsm-tree, async-fs, SQLite/rusqlite, UniFFI, iroh, Tokio transport, BLE, Bonjour, Arti)
- Modify: root `Cargo.toml` members, `Cargo.lock`, `package.json` (wasm build/graph scripts)

- [ ] RED: graph contract fails before crate exists / with forbidden dep
- [ ] RED→GREEN per unit plan for each I-2 operation's adapter binding: bounded inputs, versioned DTOs, stable error codes, panic→JS exception
- [ ] Locked wasm build reproducible; graph contract green
- [ ] Commit

## WU-004 — apps/web storage host

**Files:**
- Create: `apps/web/src/storage/db.js` (I-4 schema open/migrate), `apps/web/src/storage/profile-store.js` (I-5), `apps/web/src/storage/bundle-log.js` (append-only log + manifest), `apps/web/src/storage/operations.js` (two-phase records), `apps/web/src/storage/startup.js` (bounded walk + digest verification), `apps/web/src/storage/transaction.js` (cross-store create/join protocol, spec steps 1–6), `apps/web/src/lock.js` (Chromium Web Lock single-writer), `apps/web/test/storage/*.test.mjs`
- Modify: `package.json` (test scripts)

- [ ] RED→GREEN per unit plan: pure helpers (digest, compare-and-set validators, resume-decision table from spec §Startup resumes) under `node --test`; storage behavior verified in real Chromium (deferred e2e smoke here, full matrix in WU-007)
- [ ] Coverage scoping: the 100% `jsTooling` c8 floor applies to pure Node-testable helpers only; IndexedDB/localStorage/Web Lock code cannot execute under Node and is verified by the WU-007 Chromium matrix. `.coverage-thresholds.json` gains an explicit apps-web scope in WU-007 **before** that unit's implementation begins; existing floors are never lowered
- [ ] Idempotent resume from interruption before/after every two-phase step; every mismatch class opens read-only recovery, never mints identity
- [ ] Commit

## WU-005 — apps/web UI flows

**Files:**
- Create: `apps/web/index.html`, `apps/web/src/styles.css`, `apps/web/src/main.js`, `apps/web/src/ui/**` (first-run, home newswire, composer, immutable review, import preview, community menu, recovery panels), `apps/web/test/ui/**`
- Modify: `package.json`

Covers spec §User flows, §UI, §Recovery interactions verbatim, including: exact status vocabulary (**Saved on this browser**, **Export prepared**, never **Exchanged**); capacity remaining display, projected post/import use, warning at five remaining entries or 10% remaining byte capacity, and the full state with the exact full-community message; **Your author on this browser** labeling; expiry/AI-assisted markers; full untruncated identifiers behind technical details; key-storage warning in onboarding + community settings; focus management, `fieldset`/`legend`, polite live regions, validation-error summary focus; user text rendered as text, never `innerHTML`.

- [ ] RED→GREEN per unit plan, flow by flow
- [ ] Commit

## WU-006 — Service worker + release pipeline

**Files:**
- Create: `apps/web/sw.js`, `apps/web/manifest.webmanifest`, `scripts/web/build-release.mjs` (content-hashed assets + `release-manifest.json` + static-root contract), `apps/web/test/sw/**`
- Modify: `scripts/web/serve.mjs` (only if header gap vs I-8), `package.json`

- [ ] RED→GREEN per unit plan: manifest byte-verification, abort-install on missing/wrong-size/wrong-digest asset, no `skipWaiting`/`clients.claim`, waiting-release UI message, coherent-release navigation, offline reload after one successful load
- [ ] Header contract test: byte-exact spec header block
- [ ] Commit

## WU-007 — Chromium end-to-end + CI

**Files:**
- Create: `apps/web/e2e/*.spec.mjs`, `scripts/web/playwright.config.mjs`
- Modify: `.github/workflows/ci.yml` (browser-test job mirroring existing web job), `.coverage-thresholds.json` (add apps-web JS scope; floors never lowered), `package.json`

The spec's in-scope browser family is **Chromium desktop and Android Chromium** (spec §In scope); both are verified:

- Playwright config ships two CI projects: `desktop-chromium` and `android-chromium` (Playwright Pixel device descriptor: Chrome for Android UA, mobile viewport, touch). The full contract matrix below runs on **both** projects; Android-shaped failures are CI-blocking, not advisory.
- An optional real-device lane (Playwright Android over ADB against a local emulator or device) is documented in the WU-007 unit plan for pre-release soak; it is not CI-gated because CI has no emulator runner today. The device-emulation project is the accepted Android signal for this slice — flag at WU-007 planning if a real-device CI gate is wanted instead.

- [ ] Automate every bullet of spec §Browser contracts in real Chromium (create→post→reload; offline reload; two-phase interruption matrix; mismatch→read-only recovery; three-browser export/import/authorship relay; storage-failure no-phantom-post; draft survival; second-tab read-only; profile-cleared authority loss; hostile imports; ceiling boundary values incl. record 257 / 32 MiB+1 / entry 1,025 / profile 256 KiB+1 / bundle 8 MiB+1 / metadata 64 KiB+1 / digest-reuse-with-different-bytes; capacity UX; recovery action sets; precache failure; single coherent release; keyboard/focus/labels/announcements; header contract) on both the desktop-chromium and android-chromium projects
- [ ] CI job green on both projects; coverage gates green
- [ ] Commit

## Final verification matrix

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace --all-features
cargo run -p xtask -- validate-contracts
cargo tarpaulin --workspace --all-features --fail-under "$(jq -r .thresholds.tarpaulin.lines .coverage-thresholds.json)"
npm ci --ignore-scripts
npm run test:web:unit
npm run test:web:coverage
bash scripts/web/build-wasm.sh            # locked wasm32 build
node scripts/web/check-wasm-graph.mjs     # forbidden-dep graph contract
node scripts/web/build-release.mjs        # release manifest + static-root contract
npx --yes playwright@1.61.1 test --config scripts/web/playwright.config.mjs   # desktop-chromium + android-chromium projects
```

Plus the spec §Definition of done walkthrough on desktop Chromium and the android-chromium project: install; create or import one community; post without network; full offline reload with same identity and projection; export/import native-compatible artifacts. Floors from `.coverage-thresholds.json` are never lowered.

## Final review and handoff

After the final unit merges: `superpowers:requesting-code-review` on the full branch set; secrets check (`git log -p | grep` for key material — none may exist outside test fixtures); confirm spec "Out of scope" items are genuinely absent (no transport, no multi-community, no Safari/Firefox shims).

## Self-review: spec requirement → work unit map

| Spec section | Work unit |
|---|---|
| Product boundary (in/out of scope) | master (scope rule); enforced WU-007 absence checks |
| riot-core / willow25 vendor patch | WU-000 |
| Prepare/sign split (Testing: "Prepared review bytes are exactly the bytes later signed") | WU-001 |
| riot-client responsibilities, exact-proof ledger, capacity proof | WU-002 |
| riot-web adapter, main-thread wasm | WU-003 |
| Storage: profile, bundle log, manifest, ceilings, startup walk | WU-002 (replay/capacity) + WU-004 (host) |
| Cross-store create/join two-phase transaction | WU-004 |
| Single writer (Web Lock) | WU-004 |
| User flows: first run, create, open, compose/post, import, export | WU-002 (logic) + WU-005 (UI) |
| Interface shape | I-2 / WU-002 + WU-003 |
| UI + accessibility contracts | WU-005 |
| Recovery interactions | WU-004 (decision table) + WU-005 (panels) + WU-007 (matrix) |
| Failure and security model, headers, release manifest, worker | WU-006 (+ I-8) |
| Testing: Rust/graph contracts | WU-000…WU-003 |
| Testing: Browser contracts in real Chromium (desktop + Android Chromium) | WU-007 |
| Definition of done | Final verification matrix |

Placeholder scan: none — per-unit TDD detail is deliberately deferred to unit plans per the decomposition rule (house pattern). Type consistency: interface names in I-2 match spec §Interface shape; error codes in I-3 match spec's `BROWSER_CAPACITY_EXCEEDED` vocabulary.
