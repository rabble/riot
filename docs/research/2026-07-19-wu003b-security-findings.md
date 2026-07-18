# WU-003B security findings + queued hardening (2026-07-19)

Adversarial review of the ticket/listing authority (commit `584b5a6`, `admit_public_site_ticket` +
`resolve_listing` in `crates/riot-anchor-protocol/src/authority.rs`). Verdict: **ticket half strong;
listing half is a pure state machine whose safety is 100% contingent on the WU-004+/WU-015 caller.**
No revert needed — `admit` couldn't be forged/downgraded/replayed. Actions below.

## Confirmed strong (no action)
Signature-before-dial; signature binds RE-DERIVED canonical bytes (non-canonical wire can't help an
attacker); `verify_strict` (rejects S-malleability + small-order keys); transport downgrade closed
(`require_arti` refused, never downgraded); inclusive expiry; correct epoch rollback; cross-community
manifest binding when a manifest is present; invented digest labels are domain-separated and fail
CLOSED (not exploitable); `u32::MAX` delegate-pin prevented; no cross-protocol sig reuse (ticket vs
grant domains diverge at byte 5).

## QUEUED — apply in a hardening commit AFTER WU-004 lands (do NOT edit the crate while WU-004 builds)
1. **Close 3 test gaps in `tests/authority_records.rs`** (the "every hostile encoding rejected" claim
   is currently weaker than named):
   - 90-day lifetime cap: add a test with `expiry - issued > 90d` asserting `InvalidTicket(Structure)`.
   - Real indefinite-length rejection for the ticket records (current test only pushes a trailing byte).
   - Drive `admit`'s oversize/structure branch at the `admit` level, not just `decode_canonical` with a
     tiny limit.
2. **`resolve_listing` internal self-check** — re-decode the embedded `ticket_core_bytes` and assert the
   listing's `root_id`/O/C/W/`manifest_digest`/`manifest_version` equal the embedded signed ticket's.
   This is a defense the dep-neutral crate CAN do itself (closes the "listing internal-consistency
   unverified" gap) even before the caller-side crypto exists.
3. **Phantom guard (LOW):** the 768-byte core bound in `admit` is unreachable for the fixed-size
   `PublicSiteTicketV2Core` (matches [[riot-phantom-guards]]). Leave as defense-in-depth OR remove;
   either way the misleading "covers oversize" test is fixed by item 1.

## HIGH — WU-015 (listing service) ACCEPTANCE CRITERIA (the trust root of the directory)
`resolve_listing` decides authority from bytes it never authenticates: authority-class = "grant absent",
the delegate grant carries NO signature in `AdmittedListingEnvelopeV1` (body only), the Meadowcap
`capability_chain_bytes` is ignored, and the entry signature is not checked (dep-neutral crate can't
parse a willow entry). A single omission in the caller ⇒ trivial impersonation + epoch-seizure
(submit a root-owned envelope with `delegate_grant_bytes: None`, `root_id = victim`, epoch 0 → shown +
SEALS the epoch, locking out the legitimate delegate; or same-coords/different-digest → forced
`Equivocation` censors the real listing).

**Required in WU-015 before any `resolve_listing` output is trusted:**
- The willow25-owning caller MUST verify the entry signature, the delegate-grant signature, and the
  Meadowcap capability chain, and confirm the listing entry was signed by `root_id`, BEFORE constructing
  an `AdmittedListingEnvelopeV1`.
- Make the precondition LOAD-BEARING IN THE TYPE SYSTEM: `resolve_listing` should accept an
  already-verified proof token (a `RootId`/`DelegateProof` produced only by the verifier), not raw
  `*_bytes`. A doc comment is not enough for the directory trust root.

## MEDIUM — `admit` two-phase manifest binding (WU-004+/WU-011B caller)
`admit(env, None, …)` returns an `AdmittedTicket` without validating the ticket's self-asserted
coordinates against a real manifest. The caller MUST re-run `admit` with `Some(manifest)` before trusting
served content, OR the two phases should be distinct types so only the manifest-bound result unlocks
serving. Also consider a not-before check (`issued <= now`) — a post-dated ticket currently admits.
