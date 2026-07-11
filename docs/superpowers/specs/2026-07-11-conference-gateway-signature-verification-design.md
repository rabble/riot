# Conference gateway per-entry signature verification design

## Current state

`apps/gateway/riot_gateway.py` renders one pinned public export
(`fixtures/conference/gateway-space/public-export-v1.json`) with a single
document-level `verification_status`, always `"fixture_unverified"`. The
source fixture (`fixtures/conference/incident-space-v1.json`) carries, per
entry, a field literally named
`opaque_package_shape_placeholder_not_a_signature` — documented in
`docs/superpowers/specs/2026-07-11-riot-conference-native-demo-design.md` as
"not a cryptographic signature and not evidence that this fixture is signed
or authentic." Nothing in the gateway path checks a signature; the badge is
honest but permanently negative.

`crates/riot-core` already has the primitives this needs:
`willow::verify_entry(entry: &Entry, token: &AuthorisationToken) -> bool`
(`crates/riot-core/src/willow/mod.rs:118`), signing via `EvidenceAuthor` and
`create_signed_alert` (`willow/identity.rs`, `willow/entry.rs`), and
`SignedWillowEntry { entry_bytes, capability_bytes, signature: [u8; 64],
payload_bytes }`. This design wires the existing gateway fixture to those
primitives; it adds no new cryptography.

## Goal

Each rendered entry shows a real, per-entry cryptographic verification
result — computed once by Rust at export-build time — instead of one
hardcoded, always-negative document-level status.

## Non-goals (deferred to later sub-projects)

- Live updates to the board (sub-project 2). This design keeps the export a
  static, pinned file; freshness of the *verification result* is bounded by
  when the export was last built, same as every other field in it today.
- Submitting new reports through the page (sub-project 3). No write path,
  no new HTTP methods, no change to the "no mutation API" boundary.
- Loading Rust into the Python process, or shelling out to a compiled
  binary from the running server. The serving path in `server.py` and
  `riot_gateway.py` stays exactly as dependency-free as it is today —
  verification happens earlier, in the export-build step, not per request.

## Data flow

```
fixtures/conference/incident-space-v1.json
  Each entry's opaque_package_shape_placeholder_not_a_signature field is
  replaced with a real signature produced by riot-core's signing path
  (EvidenceAuthor::write_capability + create_signed_alert or equivalent),
  over the entry's canonical encoding.

  IMPORTANT consequence: the two fixture authors' current identifiers
  (Avery Brooks' and Jordan Lee's nostr_pubkey / willow_subspace_id, and
  the incident's own PUBLIC_NAMESPACE, which today equals Jordan Lee's
  subspace id) are illustrative hex, not real public keys — the same way
  the signature field was an illustrative placeholder. A real Ed25519 /
  Willow public key cannot be produced to match an arbitrary pre-chosen
  32 bytes; it is only ever the output of generating a real keypair. So
  this design cannot keep today's exact identifier values AND add real
  signatures — it must regenerate real keypairs for both fixture authors
  and let the new public identifiers (namespace_id, subspace_id / signer
  hashes) replace the current pinned hex throughout the fixture chain.
  This ripples to every place that currently hardcodes those exact
  strings: PUBLIC_NAMESPACE and the per-entry signer assertions in
  riot_gateway.py, apps/gateway/tests/test_gateway.py, and
  scripts/conference/gateway-smoke.sh (including the QR code's encoded
  `riot://open?namespace=...` value and PINNED_QR_SVG_SHA256, since the
  QR's payload text changes). The identifiers are cosmetic fixture data
  either way — nothing outside this fixture chain depends on today's
  specific hex values — so this is a mechanical regeneration, not a
  design risk, but it must happen atomically with the signature change
  or the fixture becomes internally inconsistent (claimed signer whose
  public key doesn't match any real keypair).
        |
        v
new xtask subcommand, e.g.
  cargo run -p xtask -- verify-conference-export
This is a small Rust binary that:
  1. loads incident-space-v1.json,
  2. for each entry, reconstructs the Entry + AuthorisationToken it claims,
  3. calls willow::verify_entry(entry, token) -> bool,
  4. writes public-export-v1.json with one new field per entry:
     "verification_status": "signature_verified" | "signature_invalid"
No signature bytes, capability bytes, or entry bytes are copied into the
public export — only the boolean outcome. This keeps the public export
inside its existing boundary: _FORBIDDEN_FIELD_PARTS in riot_gateway.py
already refuses any field whose name contains "capability", "secret",
"receipt", etc.; this design does not need to loosen that list, because
the proof material never crosses into the public file.
        |
        v
fixtures/conference/gateway-space/public-export-v1.json (regenerated,
  new PINNED_EXPORT_SHA256)
        |
        v
apps/gateway/riot_gateway.py (validation + rendering logic changes; the
  HTTP serving path in server.py is untouched)
```

## Schema changes

`EXPORT_SCHEMA` bumps from `riot-public-gateway-export/1` to `.../2` (the
document shape changes: verification moves from document-level to
per-entry). Concretely, in `riot_gateway.py`:

- `_ENTRY_FIELDS` gains `verification_status`.
- `_parse_entry` validates it against a fixed two-value enum
  (`{"signature_verified", "signature_invalid"}`), same pattern as the
  existing `ALLOWED_KINDS` check for `kind`.
- `PublicEntry` gains a `verification_status: str` field.
- The document-level `VERIFICATION_STATUS` / `verification_status` check in
  `_validate_document` is removed; there is no longer one status for the
  whole document. (If a document-level summary is wanted later, it can be
  derived — "all entries verified" / "some entries unverified" — rather
  than being a second source of truth to keep in sync.)
- `PINNED_EXPORT_SHA256`, `SOURCE_FIXTURE_SHA256`, `SOURCE_MANIFEST_SHA256`,
  `PUBLIC_NAMESPACE`, and `PINNED_QR_SVG_SHA256` all get regenerated: the
  namespace changes (see the identifier-regeneration consequence above),
  and `package-manifest-v1.json` also carries that same `namespace` field
  (checked in `_validate_document` against the incident fixture's
  namespace), so its hash changes too. The QR's encoded
  `riot://open?namespace=...` value and thus the QR SVG bytes change as
  well.

## Rendering changes

- `_render_entry`'s current static line, `Claimed author (unverified
  fixture): <code>{signer}</code>`, is replaced with two things: the
  claimed-author line stays (that's still just a claim, the signer id is
  not itself proof), and a new badge renders the real result:
  - `signature_verified` → a green ("anchor") filled or outlined badge,
    e.g. "Signature verified".
  - `signature_invalid` → a red ("hazard") badge, e.g. "Signature invalid"
    — this must be at least as visually loud as the `alert` kind badge,
    since a failed verification is itself a warning regardless of the
    entry's `kind`.
- The page-level "Fixture verification: fixture unverified" banner is
  replaced with a summary line derived from the entries at render time,
  e.g. "2 of 2 entries signature-verified", so there is exactly one place
  (`_render_entry`'s per-entry field) that holds the ground truth.
- No changes to `_render_page`'s ticket/QR/namespace block or to the CSS
  design already in place — this design only touches the verification
  badge and the page-level banner text.

## Testing / migration

- Regenerate `public-export-v1.json` via the new xtask subcommand and
  re-pin all SHA-256 constants (`PINNED_EXPORT_SHA256`,
  `SOURCE_FIXTURE_SHA256`, `SOURCE_MANIFEST_SHA256` only if the manifest
  changes shape — it should not need to).
- Update `apps/gateway/tests/test_gateway.py` and
  `scripts/conference/gateway-smoke.sh`: replace assertions on the literal
  string `"Claimed author (unverified fixture):"` context and
  `fixture_unverified` with the new per-entry wording.
- Add a negative-path test: construct a candidate export where one entry's
  claimed signature does not verify (e.g. tampered `entry_bytes` or wrong
  signer), and assert `_parse_entry`/rendering surfaces
  `signature_invalid` for that entry — not silently upgraded, and not
  rejected outright (an unverified claim is still real content on a
  mutual-aid board; it must render, clearly marked, not disappear).
- Add a Rust-side test for the new xtask subcommand: given a fixture with
  one valid and one tampered entry, it must emit exactly one
  `signature_verified` and one `signature_invalid`.

## Resolved: fixture key material is ephemeral, not checked in

`EvidenceAuthor`'s own doc comment already establishes the pattern this
should follow: "the communal namespace secret is discarded (zeroized) at
generation." The xtask subcommand generates a fresh keypair per fixture
author in-process, signs the fixture entries, calls `verify_entry` to
confirm they check out, writes the resulting public identifiers and
signature-derived `verification_status` into the fixture files, and lets
the private key fall out of scope (zeroized, per existing `Drop`/zeroize
behavior). Nothing private is ever written to disk or committed — the
regenerated fixtures are deterministic *artifacts* of a one-time signing
run, the same way the existing CBOR/SHA-256 pinning already treats fixture
bytes as fixed, reviewable, checked-in outputs rather than reproducible
recomputation. Re-running the subcommand later (e.g. to add a new fixture
entry) will naturally mint new keypairs and new identifiers; that's fine,
since the identifiers are cosmetic fixture data with no meaning outside
this chain (see the ripple note above).

## Open question for implementation

- Exact enum wording (`signature_invalid` vs `signature_unverified` vs
  other) — pick during implementation; keep it a closed, validated set
  either way.
