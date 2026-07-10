# Phase 0A — WU1 Report: Alert Codec and Communal Willow Authority

- **Status:** REVISE (the implementation is a useful first slice, but Revision 5 review invalidated the G1 PASS claim)
- **Owning work unit:** WU1
- **Date:** 2026-07-10
- **Elapsed agent-hours:** ~2.0 combined WU0R+WU1 baseline, charged once in the authoritative ledger; future repair time is separate

## Provisional G1 evidence

All 23 `public_` tests pass under the corrected pins (`willow25 =0.6.0-alpha.3`, `bab_rs =0.8.1`). Command: `cargo test -p riot-core public_`.

These green tests prove implemented behavior, not the complete revised gate. G1 remains REVISE because:

- the cross-subspace denial creates two different communal namespaces instead of two subspaces within one namespace;
- author generation is infallible and cannot return the specified `ENTROPY_UNAVAILABLE` result;
- `ClockSnapshot` and separately labelled UTC/TAI conversion evidence are absent;
- the bundle suite lacks the complete canonical-CBOR, exact-boundary, cumulative-limit, sibling-isolation, and structured-diagnostic matrix;
- hostile raw constructors are public release APIs rather than conformance/test-only helpers;
- the release profile aborts on panic, contradicting the catch/quarantine contract.

Task 2 and Task 3 of `docs/superpowers/plans/2026-07-10-riot-phase0a-public-kernel.md` define the repair. WU2 is blocked until the repaired suite and report PASS.

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
- baseline digest vocabulary implemented `bundle_digest`, proof-bound `entry_digest`, and `object_digest`; the repair replaces proof-bound `entry_digest` with canonical `entry_id` plus separate `evidence_digest` so Willow value identity is not conflated with its authorization proof.

## Deviations and notes

- WU0R (dependency revision) executed inside this unit's wall time when Revision 5 landed; see the WU0 report's completion section.
- `encode_bundle_raw` and `BundleItem::from_raw_parts` are currently public/unvalidated. Review requires moving hostile framing to `riot-conformance` or `cfg(test)` so release callers cannot bypass encode-side validation.
- Owned publication namespaces, delegated curation, and annotation objects remain stretch evidence, untouched.

## Next action

Execute the reopened G0/G1 repair tasks and rerun their reports. WU2 remains blocked until both gates PASS.
