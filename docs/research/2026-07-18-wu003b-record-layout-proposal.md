# WU-003B canonical record-layout proposal (for review)

**Status:** PROPOSAL — not implemented. Written by the overnight anchor-protocol session
(`overnight/2026-07-18-anchor-protocol`) to unblock WU-003B without autonomously freezing the
protocol wire format.

**Why this exists:** WU-003B defines the on-the-wire canonical records for the authority/ticket/
listing trust layer (`PublicSiteTicketV2Core`, `CommunityListingV1`, `ListingDelegateGrantV1`,
their envelopes, plus `admit_public_site_ticket` / `resolve_listing`). The design doc
(`2026-07-18-public-community-anchor-network-design.md`) specifies **field lists and signing
preimages, but no CDDL / positional byte layout** for these records. The layout is a load-bearing
decision (WU-004/005/006 and the Swift/Kotlin/TS conformance vectors all build on it), so it should
be reviewed rather than minted autonomously. Below is a concrete proposal that follows the WU-002
canonical conventions already implemented in `crates/riot-anchor-protocol/src/codec.rs`. **Decision
points are marked ▶.** Once approved, a session implements `records.rs` + `authority.rs` directly.

## Conventions (already implemented, WU-002)

Positional definite arrays, minimal ints, `snake_case` textual discriminants, `null`-or-value
optionals (fixed array length), sets sorted by canonical element bytes, `_bytes` fields are byte
strings of separately-canonical bytes. Canonicality is enforced by decode→re-encode byte-identity.
`digest_v1(label, canonical) = BLAKE3(u16be(len) || label || u64be(len) || canonical)`. IDs,
digests, keys = 32-byte `bstr`; signatures = 64-byte `bstr`; times = unsigned Unix seconds.

## Envelopes (design gives these EXACTLY — lines 496-505; low risk)

```
RootSignedTicketCoreEnvelopeV2 = [2, PublicSiteTicketV2Core, bstr .size 64]
AdmittedListingEnvelopeV1      = [1, signed_listing_entry_bytes, capability_chain_bytes,
                                    null / delegate_grant_bytes]
```
`listing_digest = digest_v1("riot/admitted-listing-envelope/v1", AdmittedListingEnvelopeV1)`.
`root_signed_ticket_core_digest = digest_v1("riot/public-site-ticket-signed-core/v2",
RootSignedTicketCoreEnvelopeV2, excluding replaceable hints)` — hints live OUTSIDE this core.

## TransportFloor (design lines 377-390)

Closed enum, total order `require:none < require:arti`. ▶ **Decision:** the design writes
`require:none` / `require:arti` (colon), but a colon is not valid in a `snake_case` discriminant.
**Proposed wire tokens:** `"require_none"` and `"require_arti"`. MVP admits only
`transport_floor == require_none && manifest_required_transport == require_none`; anything else
fails **before dialing** as `unsupported_transport`.

## PublicSiteTicketV2Core (design lines 365-404)

Signed by the `O` root: `Sign("riot/public-site-ticket/v2" || canonical_cbor(ticket_body))`.
Body fields, in the design's stated order (lines 374-383):

```
PublicSiteTicketV2Core = [
  root_id_bytes,                 ; 32  (full O root key)
  o_namespace_id_bytes,          ; 32
  c_namespace_id_bytes,          ; 32
  w_namespace_id_bytes,          ; 32
  manifest_digest_bytes,         ; 32
  manifest_version,              ; uint
  min_sync_version,              ; uint, MUST == 2
  manifest_required_transport,   ; "require_none" / "require_arti"
  transport_floor,               ; "require_none" / "require_arti"
  transport_epoch,               ; uint
  issued_unix_seconds,           ; uint
  expiry_unix_seconds,           ; uint
]
```
▶ **Decision:** the design says the body "binds schema/version." I read the type name `...V2` +
envelope tag `2` + signing domain `riot/public-site-ticket/v2` as carrying schema/version, so the
core body itself has NO leading schema/version field (version-scoped record ⇒ `[...]`, not
`[n, ...]`). If instead schema/version must be explicit fields inside the body, prepend
`schema_tstr, version_uint`. **Bounds:** core ≤ 768 bytes (line 2412). Expiry ≤ 90 days from
admission; expiry is inclusive (`now >= expiry` ⇒ expired).

## CommunityListingV1 (design lines 345-363)

Willow-entry payload at `O:/directory/listing`. Binds:
```
CommunityListingV1 = [
  schema_tstr,                   ; ▶ "riot/community-listing/1" — Decision: explicit schema
                                 ;   field vs. version-scoped. Proposed: explicit (design names it).
  root_id_bytes,                 ; 32
  o_namespace_id_bytes,          ; 32
  c_namespace_id_bytes,          ; 32
  w_namespace_id_bytes,          ; 32
  manifest_digest_bytes,         ; 32
  manifest_version,              ; uint
  ticket_core,                   ; embedded PublicSiteTicketV2Core (NOT the envelope; the design
                                 ;   says "canonical root-signed PublicSiteTicketV2 core")
                                 ;   ▶ Decision: embed the core value, or the signed-core bytes?
                                 ;   Proposed: the signed-core envelope bytes as `*_bytes` so the
                                 ;   listing carries a verifiable root signature. Confirm.
  listing_epoch,                 ; u32
  listing_revision,              ; u32
  listed,                        ; bool (false = unlisting tombstone)
  title_tstr,                    ; <= 120 UTF-8 bytes
  summary_tstr,                  ; <= 512 bytes
  topic_tags,                    ; ▶ set/list of <=8 bstr, each <=32. Proposed: SORTED SET (dedup,
                                 ;   canonical) to avoid equivocation. Confirm vs. ordered list.
  languages,                     ; <=8 BCP-47 tstr, each <=35. Same set/list decision.
  region,                        ; null / bstr <=16  (optional coarse region)
  issued_unix_seconds,           ; uint
  expiry_unix_seconds,           ; uint  (<=30 days from admission; inclusive)
]
```
The Willow entry signature already covers payload digest, namespace, subspace, path, timestamp,
capability (design lines 410-411) — no second ad hoc signature scheme.

## ListingDelegateGrantV1 (design lines 330-339)

Signed directly by the `O` root: `Sign("riot/listing-delegate-grant/v1" ||
canonical_cbor(grant_body))`. Binds:
```
ListingDelegateGrantV1 = [
  root_id_bytes,                 ; 32  (the O root)
  delegate_key_bytes,            ; 32  (listing key subspace)
  terminal_capability_digest_bytes, ; 32  ▶ Decision: digest of what exactly — the terminal
                                 ;   Meadowcap capability's canonical bytes. Confirm the preimage.
  listing_epoch,                 ; u32  (exactly one; a Meadowcap chain alone can't pick an epoch)
  issued_unix_seconds,           ; uint
  expiry_unix_seconds,           ; uint  (cannot outlive the Meadowcap time range)
]
```

## admit_public_site_ticket (design lines 374-404, 1106-1112) — SECURITY-CRITICAL, needs review

`fn admit_public_site_ticket(envelope: &RootSignedTicketCoreEnvelopeV2,
manifest: Option<&ValidatedManifest>, floor, now) -> Result<AdmittedTicket, AuthorityError>`.
Order (fail-closed, signature BEFORE any dial):
1. bounded canonical decode + re-encode of the envelope (reject duplicate/oversize fields,
   >768-byte core, indefinite/map/trailing).
2. verify the 64-byte root signature over `"riot/public-site-ticket/v2" || core` against `root_id`.
   Reject `invalid_ticket` reason ∈ {signature, root, structure}.
3. `min_sync_version == 2`; reject v1/v2 downgrade.
4. transport: require `transport_floor == require_none && manifest_required_transport ==
   require_none`; else `unsupported_transport` (this rejects `require_arti`). If a manifest is
   present, `transport_floor >= manifest_required_transport` and both `require_none`, else
   `manifest_transport_mismatch` (never downgrade to public iroh).
5. expiry: `now < expiry` (inclusive rejection at equality) else `expired_ticket`.
6. rollback: reject if `transport_epoch < highest_seen_epoch_for_root` (caller supplies the floor).
7. if manifest present: root/O/C/W/manifest-digest/version must match, else `manifest_mismatch`.

▶ **Needs:** riot-core's `ValidatedManifest` type (find it) + `ed25519-dalek` verify. This is the
piece that most warrants adversarial review — the plan's Slice-1 RED list (design lines 4043-4046)
is the test matrix: coordinate disagreement, transport summary mismatch, duplicate fields, oversize
proofs, unsupported Arti, v1/v2 downgrade.

## resolve_listing (design lines 416-436) — pure state transition, testable without a manifest

`fn resolve_listing(durable_floor, candidate: &AdmittedListingEnvelopeV1, now) ->
Result<ListingTransition, AuthorityError>`. Rules, for a given root:
- a delegated entry is admitted only in the epoch named by its valid grant;
- only a root-owned zero-delegation entry/grant may establish the next epoch, advancing by exactly 1;
- higher valid epoch wins;
- within an epoch a root-owned zero-delegation listing unconditionally beats every delegated
  listing regardless of revision, and SEALS that epoch against later delegated changes;
- among the same authority class, higher revision wins;
- identical `(epoch, class, revision)` + digest ⇒ dedupe;
- identical coordinates with different digests ⇒ listing equivocation (show neither);
- a higher-revision root-owned listing in the current/next valid epoch clears equivocation and
  cannot be pinned by a delegate at `u32::MAX`;
- expiry inclusive (`now >= expiry` ⇒ invalid);
- persist root-controlled epoch, sealed status, highest admitted revision, grants, conflict
  evidence — restart/eviction must not roll the listing backward.

This is deterministic and a good first implementation target once the records above are approved.
```
