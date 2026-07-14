# Service Inventory

> Updated by the orchestrator after each work unit commit.
> Coder agents MUST read this before implementing to avoid duplicating existing services.

## Rust Core (`crates/riot-core/`)

| Service | File | Responsibility | Key Methods / Functions |
|---------|------|---------------|------------------------|
| `RiotSession` | `src/session.rs` | Session arbiter, store lifecycle, preview-first atomic import | `open()`, `open_sqlite(db)`, `create_store()`, `inspect()`, `plan()`, `commit()` |
| `EvidenceStore` | `src/session.rs` | Verified entry retention, byte/count/path budgets, retained-store ceiling | `entries()`, `receipts()`, `prefix_snapshot()` |
| `RiotDatabase` | `src/store/database.rs` | Durable SQLite persistence (WAL, backup/restore, migrations) | `open(path, config)`, `backup_to()`, `restore_from()`, `read_snapshot()`, `write_transaction()` |
| `EvidenceRepository` | `src/store/evidence.rs` | Store backend enum (memory vs SQLite); bridges session ↔ database | `memory()`, `sqlite(db)` |
| Newswire projection | `src/newswire/store.rs` | Rebuilds the collective front page / open wire / editorial history from a descriptor-pinned record set | `project_space()`, `load_space_descriptor()`, `load_space_records()` |
| Newswire records | `src/newswire/entry.rs` | Creates and verifies signed newswire descriptors, posts, editorial actions | `create_signed_space_descriptor()`, `create_signed_news_post()`, `create_signed_editorial_action()` |
| Newswire codec | `src/newswire/model.rs` | Canonical CBOR encode/decode for all newswire record families | `encode_space_descriptor()`, `decode_news_post()`, ... |
| `ByteSyncSession` | `src/sync/ffi_bridge.rs` | Byte-level sync wire adapter; owns no transport, holds at most one outbound frame | `begin()`, `receive_bytes()`, `take_outbound()` |
| `ReconcileSession` | `src/sync/state.rs` | Transport-independent reconciliation state machine (Idle → Hello → Summary → Request → Entries → Complete) | `receive()`, `import_accepted()`, `import_rejected()` |
| Alert codec | `src/model/mod.rs` | Deterministic CAP-style alert CBOR codec, CDDL-validated | `decode_alert()`, `encode_alert()` |
| Import join | `src/import/join.rs` | Copy-on-write join plan computation; byte-budget enforcement | `plan_join_with_payloads()` |
| Bundle codec | `src/import/bundle.rs` | Development Drop Format codec for site bundles (non-interoperable alpha) | `decode_bundle()` |
| Miniapp subsystem | `src/apps/` | Signed miniapp manifests, bundles, trust, endorsement, directory, index | `manifest.rs`, `bundle.rs`, `trust.rs`, `endorse.rs`, `directory.rs`, `index.rs` |
| Profile | `src/profile/` | Display name resolution, profile cards, profile entry paths | `resolver.rs`, `card.rs`, `path.rs` |

## FFI Boundary (`crates/riot-ffi/`)

| Service | File | Responsibility | Key Methods |
|---------|------|---------------|-------------|
| `MobileProfile` | `src/mobile_state.rs` + `src/mobile_api.rs` | The primary FFI handle; all profile operations | `open_local_profile()`, `open_profile_from_sealed_identity()`, `create_public_space()`, `sign_draft()`, `inspect_bytes()`, `open_sync_session()`, `app_runtime()` |
| `MobileImportPreview` / `MobileImportPlan` | `src/mobile_api.rs` | Preview → plan → commit import handles | `eligible_entries()`, `create_plan()`, `accept()` |
| `MobileSyncSession` | `src/mobile_api.rs` + `src/mobile_state.rs` | Live sync wire session handle | `begin()`, `receive_frame()`, `take_outbound_frame()`, `accept_import()`, `reject_import()`, `cancel()` |
| `AppRuntimeSession` | `src/apps_ffi.rs` | Miniapp install, trust, data put/get, directory, share, endorse | `install_app()`, `app_data_put()`, `directory_listings()`, `share_app()` |
| `ProfileSession` | `src/profile_ffi.rs` | Display name and whoami management | `set_display_name()`, `whoami()`, `profile_for()` |

## Native Apps

| App | Path | Stack | Role |
|-----|------|-------|------|
| iOS | `apps/ios/` | Swift 6 / SwiftUI / WebKit | Native shell: BLE + LAN transport, miniapp WebView host, directory, profile, demo mode |
| macOS | `apps/macos/` | Swift (compiles iOS by reference) | Multi-instance testing, desktop demo |
| Android | `apps/android/` | Kotlin 2.2 / Jetpack Compose | Native shell: GATT BLE + socket transport, WebView host, directory, Keystore profile |

## Gateway

| Service | File | Responsibility | Key Methods |
|---------|------|---------------|-------------|
| `PublicGateway` | `apps/gateway/riot_gateway.py` | Stateless renderer: serves signed newswire space bundles as browsable HTML with signature verification badges | `_validate_document()`, `_render_page()`, `_render_entry()` |
| Server | `apps/gateway/server.py` | HTTP server entry point (wraps `PublicGateway`) | `--export`, `--port` |

## Shared Modules

| Module | File | Exports | Used By |
|--------|------|---------|---------|
| xtask | `crates/xtask/src/main.rs` | `validate-contracts`, `generate-bindings`, `sign-conference-fixture`, `verify-conference-export` | CI, developer workflows |
| Conformance vectors | `crates/riot-conformance/` | WILLIAM3 golden test vectors | `cargo test` |

## Established Patterns

- **Copy-on-write atomic import.** The join plan is computed against a cloned store and installed with one pointer swap; a fault before the swap leaves all observable state unchanged. (`session.rs`)
- **Handle + arbiter FFI.** All FFI handles carry only an ID plus `Arc<Mutex<SessionState>>`; every method re-acquires the arbiter before any mutation. (`mobile_state.rs`)
- **Sealed-identity restore.** The signing identity is sealed (ChaCha20-Poly1305) under a wrapping key before returning to the native host; restore requires the exact key. (`profile_ffi.rs`, `EvidenceAuthor`)
- **Panic-catch FFI boundary.** The release profile uses `panic = "unwind"` so the FFI boundary can catch panics via `catch_unwind` and quarantine the session. (`mobile_state.rs`)
- **Dependency-pin contract.** xtask `validate-contracts` structurally verifies that `willow25` / `bab_rs` alpha pins are unchanged; stable releases compute incorrect digests. (`xtask/src/main.rs`)
- **In-memory store (Phase 0A).** The FFI currently calls `RiotSession::open()` (memory-only). Durable SQLite via `open_sqlite` exists but is not yet surfaced through FFI. (`mobile_state.rs:150-183`)
