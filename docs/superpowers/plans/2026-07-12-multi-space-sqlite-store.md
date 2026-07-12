# Multi-Space SQLite Production Cutover

## Goal

Make the Rust SQLite store the single source of truth for Riot's iOS conference
build.

After this work, these all persist in SQLite:

- accepted Willow entries;
- live Willow state;
- spaces;
- app approvals;
- app documents.

Swift becomes a thin client over UniFFI. The in-memory store remains only as a
test oracle.

The detailed schema, security model, recovery protocol, SQLite configuration,
performance budgets, and signer envelope are defined in
`docs/superpowers/specs/2026-07-12-multi-space-sqlite-store-design.md`. This plan
defines execution order and observable completion criteria.

## Principles

- Every change begins with a failing test.
- Implement the smallest production code needed to pass.
- Every task ends with adversarial review before commit.
- One transaction updates accepted state, live state, receipts, and projections.
- Namespace isolation is mandatory.
- Rust owns persistence. Swift never reconstructs authoritative state.
- Canonical signed Willow entries are authoritative; JSON documents are
  rebuildable, app-facing projections.

## Task 1 — SQLite foundation

**Goal:** Prove bundled SQLite works on every target in the conference package.

**RED**

- SQLite platform and JSON-support tests fail.

**GREEN**

- Pin `rusqlite`.
- Verify JSON support.
- Verify iOS device and simulator builds.
- Verify the conference native package, including deferred platform artifacts.

Review and commit.

## Task 2 — Database lifecycle

**Goal:** Open a durable SQLite database safely.

**RED**

Tests cover first open, reopen, migrations, corruption, concurrent access, data
protection, and storage failures.

**GREEN**

- Implement schema migration and recoverable open.
- Serialize database mutation off the UI thread.
- Return typed recovery errors without silently replacing unreadable data.
- Protect the database and its sidecars before application writes begin.

Review and commit.

## Task 3 — Space registry

**Goal:** Persist spaces independently of their Willow entries.

**RED**

Tests cover create, join, archive, selection, switching, and restart with at
least two namespaces.

**GREEN**

- Persist space and namespace metadata.
- Persist local selection and lifecycle state.
- Persist signer references without secret key material.

Review and commit.

## Task 4 — SQLite evidence store

**Goal:** Replace the memory store in the iOS production path.

**RED**

Characterization and differential tests prove the SQLite backend preserves the
existing inspect/plan/commit and Willow live-set behavior.

**GREEN**

Implement `SqliteEvidenceStore` behind the existing storage boundary and verify:

- accepted entries and capabilities;
- live state and payload access;
- receipts;
- pruning, forgetting, and restoration;
- namespace isolation;
- restart;
- atomic rollback.

The CLI also uses SQLite. The memory implementation remains only as the
differential oracle or temporary compatibility path until the native cutover.

Review and commit.

## Task 5 — Signer persistence

**Goal:** Persist namespace signers securely.

**RED**

Tests cover create, import, restart, missing or incorrect secure-storage keys,
interrupted updates, and rollback.

**GREEN**

- Store only encrypted signer material and secure-storage references.
- Bind every signer to its namespace and role.
- Keep wrapping and signing keys transient during cryptographic operations.

Review and commit.

## Task 6 — Applications

**Goal:** Persist app packages and namespace-local approvals.

**RED**

Tests cover package installation, approval, revocation, restart, namespace
isolation, and stale sessions after approval changes.

**GREEN**

- Store immutable, content-addressed app packages.
- Store approvals independently per namespace.
- Invalidate app sessions when approval state changes.

Review and commit.

## Task 7 — Documents

**Goal:** Make signed Willow entries the only document authority while giving
apps a natural document API.

**RED**

Tests cover local writes, imported writes, restart, conflicts, multiple authors,
pruning, forgetting, restoration, namespace isolation, paging, and watches.

**GREEN**

- Project JSON documents atomically from accepted Willow entries.
- Scope every document by namespace, app, collection, document ID, and author.
- Expose bounded `put`, `get`, `list`, and `watch` behavior.
- Remove any standalone JSON authority.

Review and commit.

## Task 8 — Native API

**Goal:** Expose the complete database API through UniFFI.

**RED**

End-to-end contract tests create two spaces, approve apps, write independent
documents, close all handles, reopen, and prove isolation.

**GREEN**

Introduce:

- `DatabaseSession`;
- `SpaceSession`;
- `AppSession`;
- typed pages, watches, and recovery errors.

All handles are bound to immutable namespace and generation state. Deprecated
bindings needed by deferred platforms remain compatible but are not used by the
iOS release path.

Review and commit.

## Task 9 — iOS cutover

**Goal:** Remove Swift JSON replay and make SQLite authoritative on iOS.

**RED**

Restart tests prove two spaces, app approvals, and independent documents survive
termination. Tests also prove stale sessions cannot write across a switch or
revocation and corrupt storage does not become an empty profile.

**GREEN**

- Swift opens the database once.
- All reads and writes go through namespace-bound Rust sessions.
- Remove app-data, trust, carried-app, and demo-bundle replay as authority.
- Bind WebView app sessions to one space and approval generation.

Review and commit.

## Task 10 — Nearby sync

**Goal:** Make existing nearby transport write directly into SQLite.

**RED**

Tests cover imports for multiple namespaces, mixed-namespace input, rollback,
reopen, and zero cross-namespace leakage.

**GREEN**

- Commit verified bundles through `SqliteEvidenceStore`.
- Derive namespace only from verified Willow entries.
- Never redirect an import into the currently visible space.

Review and commit.

## Task 11 — Release gate

Verify:

- full Rust workspace tests, formatting, and strict linting;
- the repository's 100% coverage gate;
- differential Willow behavior;
- the two-space conference fixture in release configuration;
- iOS unit and integration tests with coverage;
- device and simulator native builds;
- conference native-package checks;
- the performance budgets defined by the design.

The conference fixture must create two namespaces, approve the representative
app independently, commit different documents, import transport data, close and
reopen, and observe exact state with zero leakage.

Run final adversarial review, record evidence, and commit.

## Stop line

This plan ends with the iOS SQLite store slice. Legacy-container migration,
complete Meadowcap management UI, all four starter-app UIs, Android/macOS runtime
adoption, physical-phone rehearsal, and TestFlight delivery remain separately
gated follow-up work.
