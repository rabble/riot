# Phase 0A — WU1 Report: Alert Codec and Communal Willow Authority

- **Status:** PASS (G1)
- **Owning work unit:** WU1
- **Date:** 2026-07-10
- **Elapsed agent-hours:** ~2.0 of 2.5 budgeted (includes the WU0R dependency revision executed mid-unit when the Willow implementation audit landed)

## G1 PASS evidence

All 23 `public_` tests pass under the corrected pins (`willow25 =0.6.0-alpha.3`, `bab_rs =0.8.1`). Command: `cargo test -p riot-core public_`.

**Deterministic alert codec** (`tests/public_alert.rs`, 10 tests):

- canonical CBOR with integer keys ascending, definite lengths; identical bytes on repeat encode; decode inverts encode exactly;
- golden vector frozen at `fixtures/objects/alert-golden-1.cbor` (bless-once, compare-forever);
- operational constraints enforced pre-sign and post-decode: expiry strictly after created, ≥1 non-empty source claim, closed CAP-style enums, per-field byte bounds;
- hostile decoder bounds: oversized input (`payload_bytes`+1), truncated bytes, smuggled unknown key (distinct `UnknownKey` code), indefinite-length map rejected, re-encode canonicality proof rejects every non-canonical encoding.

**Communal Willow authority** (`tests/public_willow.rs`, 6 tests):

- WILLIAM3 golden vectors (empty/short/partial-block/multi-block) frozen at `fixtures/willow/william3-vectors.txt` — dependency-drift tripwire for the corrected `bab_rs 0.8.1` digests;
- communal namespace generated with even-LSB check (`is_communal`), namespace secret discarded at generation; zero-delegation communal write capability authorises the author's own subspace; checked `PossiblyAuthorisedEntry` verification only;
- cross-subspace denial both directions: an intruder secret cannot mint a token for another subspace (`DoesNotAuthorise`), and a valid token does not verify an entry in a different subspace (area check fails before signature);
- fixed evidence path `objects/alert/<object_id:16>/<revision_id:16>` (4 binary components, prefix-unrelated revisions); entry binds exact payload length and corrected WILLIAM3 digest; Willow timestamp (TAI µs) distinct from alert UTC fields;
- canonical `Entry` and `WriteCapability` byte roundtrips via upstream codecs (`new_vec_storing_encoding` / `decode_canonic`), trailing bytes rejected.

**Evidence bundle codec** (`tests/public_bundle.rs`, 7 tests):

- `RIOTE1` visible magic + deterministic CBOR framing of `{entry_bytes, capability_bytes, signature_bytes[64], payload_bytes}` per item; roundtrip byte-identical on re-encode;
- ceilings enforced on both sides: 64 entries, 8 MiB artifact, 1 MiB payload, 64 KiB/entry and 2 MiB/bundle authorization budgets;
- decode order per audit: bounded outer CBOR → canonical entry → canonical capability → fixed signature → payload length/digest → Meadowcap authorisation; tampered signature and payload-digest mismatch rejected with distinct codes; wrong magic and trailing bytes rejected;
- digest vocabulary implemented: `bundle_digest` (SHA-256 of artifact), domain-separated `entry_digest` (`riot/entry-digest/v1` with length framing — reordering inputs changes the digest), `object_digest` (SHA-256 of payload).

## Deviations and notes

- WU0R (dependency revision) executed inside this unit's wall time when Revision 5 landed; see the WU0 report's completion section.
- `encode_bundle_raw` is deliberately public and unvalidated so WU4's hostile corpus can frame structurally-valid-but-cryptographically-invalid bundles; `encode_bundle` always re-verifies before export.
- Owned publication namespaces, delegated curation, and annotation objects remain stretch evidence, untouched.

## Next action

WU2 — core import: synchronous preview, bounded copy-on-write snapshot transaction, `Applied`/`Dominated`/`AlreadyPresent` dispositions with Willow join semantics (prefix pruning, digest/length tie-breaks), receipts, duplicate handling, hostile cases, and arbiter concurrency tests. The alpha.3 `MemoryStore` serves as the conformance oracle for join permutations.
