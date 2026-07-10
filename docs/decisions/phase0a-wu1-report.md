# Phase 0A — WU1 Report: Alert Codec, Communal Authority, Evidence Bundle

- **Status:** PASS (G1) — reopened findings individually repaired via Tasks 1–3 of the public-kernel plan, 2026-07-10
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

## Next action

G1 PASS. Proceed to Task 4 (WU2A — namespace-local Willow join with the alpha.3 `MemoryStore` differential oracle).
