# Phase 0A — WU1 Report: Alert Codec, Communal Authority, Evidence Bundle

- **Status:** G1 REOPENED — independent review on 2026-07-10 found release-factory containment and bundle-gate failures; WU2 is blocked
- **Owning work unit:** WU1 (repair executed under the reopened-G0/G1 budget)
- **Date:** 2026-07-10
- **Elapsed agent-hours:** ~2.0 combined WU0R+WU1 baseline (charged once, earlier) + repair time in the ledger

## G1 PASS evidence

39 `public_` tests pass; workspace clippy is clean with zero warnings. Commands:

```
cargo test -p riot-core public_        # 13 alert + 12 willow + 14 bundle
cargo test -p riot-conformance william3_
cargo clippy --workspace --all-targets -- -D warnings
cargo xtask validate-contracts
```

### Each reopened finding, closed

1. **Same-namespace cross-subspace denial.** `public_cross_subspace_denial_within_one_namespace` builds two subspaces under one communal namespace (deterministic second secret) and proves both directions: the second secret cannot mint a token for the first subspace, and the first author's token does not verify an entry in the second subspace. Two independently generated namespaces are no longer used for this claim.
2. **Fallible author factory.** `generate_communal_author(&mut dyn EntropySource)` draws every byte from a fallible source; failure returns `ENTROPY_UNAVAILABLE` and constructs no author (`public_author_generation_fails_closed_without_entropy`). Production uses `OsEntropy` (`rand_core::OsRng::try_fill_bytes`); failing/deterministic sources exist only in tests.
3. **ClockSnapshot with labelled views.** One snapshot carries `unix_seconds` (UTC, signed alert), `tai_j2000_micros` (Willow join recency via pinned willow25+hifitime conversion), and `uncertainty_seconds`. `create_signed_alert` uses one snapshot for both views (`public_signed_alert_uses_one_snapshot_for_both_time_views`); pre-Unix-epoch, pre-J2000, and conversion-range failures return `CLOCK_UNAVAILABLE` with no partial entry (`public_clock_rejects_pre_epoch_and_out_of_range`, `public_signed_alert_fails_closed_on_clock_and_entropy`).
4. **Complete bundle matrix.** The pure codec returns `BundleDecodeOutcome::Decoded|Rejected` with frozen fatal precedence (size → magic → malformed/non-canonical outer → unsupported codec → cumulative limits in encounter order → duplicate `entry_id`). Covered: 64-entry success / 65 rejection (encode and hand-framed decode); 8 MiB+1 rejected before parsing with exact-8 MiB proving precedence; wrong magic; unknown codec version; unknown/duplicate outer keys; indefinite containers; trailing bytes; non-shortest outer integers (re-framing canonicality proof); signature length 63 and 65; non-canonical entry and capability bytes; payload length and digest mismatches; forged-signature authorization failure; cumulative authorization budget crossing at parse time; duplicate canonical entry IDs rejecting globally; **mixed valid/invalid siblings with the valid item unaffected**; and sanitized `BundleDiagnostic {code, component}` values proven to never embed hostile payload bytes.
5. **Hostile framing removed from the release API.** `BundleItem::from_raw_parts` and `encode_bundle_raw` no longer exist; `encode_bundle` always re-verifies before export, and hostile frames are hand-built inside the test suite with minicbor.
6. **Value vs proof identity.** `entry_id` (`riot/willow-entry-id/v1` over canonical entry bytes) is separate from `evidence_digest` (`riot/evidence-digest/v1` over entry‖capability‖signature); `public_entry_id_is_value_identity_not_proof_identity` proves a signature change moves the evidence digest but not the entry ID. Duplicate detection and join identity use `entry_id`.
7. **Release profile unwinds.** `panic = "unwind"` landed in Task 0 and is enforced structurally by `cargo xtask validate-contracts`.
8. **Capability profile enforcement.** The bundle layer accepts only a communal namespace with a zero-delegation communal capability (`is_owned()` false, `delegations()` empty); anything else is `UNSUPPORTED_CAPABILITY` on the Authorization component.

### Deterministic fixtures

- `fixtures/objects/alert-golden-1.cbor` + diagnostic JSON projection (hash-linked, `public_alert_golden_json_projection_matches_cbor`).
- `fixtures/willow/bundle-golden-1.riot-evidence` — a deterministic one-item bundle (fixed namespace/subspace secrets, counting entropy, fixed clock) frozen and decoded Valid on every run.
- `fixtures/willow/william3-vectors.json` — independently cross-checked digest basis (see WU0 report).

## Notes

- The Willow module split (`clock`, `identity`, `entry`, `digest`) keeps concrete Willow generics private; the signer type is neither `Clone` nor `Debug` and exposes no key accessor. `EvidenceAuthor::from_parts_for_tests` remains available for fixtures and is excluded from the FFI surface in Task 6.
- Owned publication namespaces, delegated curation, and annotation objects remain stretch evidence, untouched.

## Independent gate review (2026-07-10)

The required commands were rerun from `main` at reviewed HEAD `deb847a5741d6fbbeb598e6a10e8e99c67f1daa6`: `cargo xtask validate-contracts` PASS; `cargo test -p xtask` 7/7; `cargo test -p riot-conformance william3_` 2/2; `cargo test -p riot-core public_` 39/39; workspace clippy PASS with warnings denied. Those green commands do not prove all reopened claims.

The same-namespace denial evidence is **PASS**: both authors share Alice's namespace, the second fixed subspace secret differs from the first author and produces a distinct subspace ID, and the first authorization assertion fails if the area check is removed.

The fallible-factory claim is **REOPENED**. `EntropySource`, `ClockSource`, `generate_communal_author`, `create_signed_alert`, `snapshot_from_unix_seconds`, and `EvidenceAuthor::from_parts_for_tests` are public in the normal `riot-core` release build. Any Rust caller can therefore supply deterministic entropy/clock sources despite the report's claim that those sources are test/conformance-only. `riot-ffi` is currently empty, so no secret/testing constructor crosses FFI today, but Task 6 must expose only production wrappers backed by `OsEntropy` and `SystemClock` and must prove that injected/test constructors are absent from the release feature closure. Repair by moving injection traits/functions and `from_parts_for_tests` behind `cfg(test)` or a non-default conformance feature unavailable to `riot-ffi`, adding non-injectable production factories, adding a release-API regression check, and explicitly zeroizing temporary namespace/subspace secret byte arrays after construction.

The bundle claim is **REOPENED**:

1. Fatal precedence is wrong. A hand-framed bundle with both a non-shortest outer key and an unsupported codec returns `UnsupportedCodec`; the frozen order requires malformed/noncanonical outer framing to win. Repair with a bounded canonical-frame validation pass before semantic codec/limit decisions and combined-violation tests for every adjacent precedence pair.
2. Diagnostics are not sanitized as a whole. `BundleItemFrame`, `DecodedItem`, `DecodedBundle`, and `BundleDecodeOutcome` derive `Debug` while retaining untrusted bytes. Formatting a decoded outcome exposes the hostile marker as its exact decimal byte sequence. Remove or redact `Debug` for all byte-bearing public result/frame types and test formatting of the entire outcome, not only `ItemStatus`.
3. The decoder uses a 64 KiB limit for canonical Entry bytes, while Revision 5 freezes a 4 KiB Entry ceiling. Add and enforce separate exact Entry, capability, and signature ceilings rather than reusing `MAX_AUTH_BYTES_PER_ENTRY` for Entry bytes.
4. Blocking Task 3 cases remain absent: a valid canonical bundle at exactly 8 MiB, invalid UTF-8 in the codec string, combined fatal-precedence violations, and direct indefinite byte/text-string cases. Add exact/one-over tests and make each rejection deterministic. Nesting/node-ceiling and exact authorization-boundary corpus cases may be completed with WU4 only if the implementation first documents why the fixed-shape parser makes them unreachable; otherwise add them before re-closing G1.

Item-scoped failures do preserve valid siblings; hostile framing helpers are test-private; the `entry_id` and `evidence_digest` byte domains match the plan; and the current capability/signature/payload diagnostic variants themselves contain no hostile strings. Those passing subclaims do not offset the fatal gate failures above.

## Next action

Keep the alert codec and same-namespace evidence PASS, repair Task 2 release containment and Task 3 bundle failures, and do not start Task 4/WU2 until an independent rerun records both G0 and G1 PASS.
