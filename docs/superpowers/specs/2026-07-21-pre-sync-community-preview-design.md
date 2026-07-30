# Design: Untrusted Pre-Sync Community Preview

**Status:** Draft (awaiting Design Review Gate)
**Date:** 2026-07-21
**Author:** agent session (from user request: "why not join and give me a summary of the group, instead of just sync again")

## Problem

When a user opens a community join link/QR, hits a nearby-adoption prompt, or lands on the
"couldn't be opened" recovery screen, Riot shows bare coordinates and a generic **Retry /
Sync again** button. The user has no sense of **what the group is** before committing. The
two entry points today:

1. **Join-by-link/QR** (`JoinByReferenceSheet`): shows `JoinPreview.shortNamespace`
   ("`a1b2c3d4…`") and no title. `JoinPreview.title` is deliberately `nil`.
2. **Recovery** (`ShellRecoveryState.communityUnavailable`): "`<remembered name> couldn't be
   opened.`" + Retry/Find-nearby. Carries only `name` + `code`, no namespace/digest.

A user faced with "`a1b2c3d4…` + Sync again" cannot make an informed join decision.

## The hard constraint (anti-spoof)

A `riot://newswire/join/v1/...` reference carries **only cryptographic coordinates** —
namespace id, descriptor entry id, content digest (`NewswireShareReferenceV1`,
`crates/riot-core/src/newswire/share.rs:20-34`). It carries **no name, no roster, no posts**,
by design: an attacker who mints a reference must not choose the name a joiner sees
(`JoinReferenceModel.swift:3-9`). The existing test pins this:
`XCTAssertNil(preview.title, "share ref carries no title; UI must not fabricate one")`.

**Therefore a preview name/members/posts can never be trusted straight off the link.**
This design fetches a preview from the community's gateway over untrusted HTTPS — exactly
the architectural twin of the followed-site HTTP-pull model
(`import_followed_site_bundle`, `crates/riot-ffi/src/site_ffi.rs:986-1056`) — and re-verifies
the fetched descriptor against the digest locally before showing anything as factual. A
hostile gateway can serve stale/empty, never forge.

## Design

### Trust ladder (the core idea)

The preview is shown in **two distinct visual tiers**, never blurred:

| Tier | Source | What it shows | Visual treatment |
|---|---|---|---|
| **Untrusted (pre-fetch)** | the link only | namespace short id, honest note "name arrives after sync" | muted, no name field |
| **Descriptor-verified (post-fetch, pre-sync)** | gateway fetch + `verify_descriptor_matches` passes | the community **name** bound to the digest, member count, recent-post titles | clear "**verified summary**" chip, distinct from signed data |
| **Signed (post-sync)** | Willow namespace sync | full members, posts, tools, all signed | authoritative — overrides tier 2 on conflict |

Tier 2 is the new surface. It is **gated on the digest check passing**: if the fetched
descriptor's WILLIAM3 digest (`share.rs:83-91`) does not match the reference's
`content_digest`, the fetch is discarded and the UI falls back to tier 1 (never shows an
unverified name). The chip text and styling make "verified-against-the-link-summary" visually
distinct from "signed-by-the-community."

### Data flow

1. User pastes link / scans QR / hits recovery. App has a `JoinPreview` (link/QR) or, after
   this change, an enriched `CommunityUnavailable` carrying namespace+digest (recovery).
2. App derives the gateway fetch URL from the namespace id via a deterministic mapping
   (documented, e.g. `https://<gateway>/<ns-hex>/preview.json`). **No URL is taken from the
   link** — the link is digest-only; the gateway origin is a Riot-known/pinned base, same as
   the followed-site model.
3. Rust core fetches (over untrusted HTTPS; offline-first: unreachable ⇒ tier 1 fallback,
   no hang), parses a small CBOR/JSON preview bundle, and runs `verify_descriptor_matches`
   against the reference digest.
4. On match: returns a `CommunityPreviewSummary { name, member_count, recent_post_titles[] }`
   to the FFI. On mismatch/empty/offline: returns `nil`; UI shows tier 1.
5. UI renders tier 2 with the verified-summary chip and a primary action **"Sync to join"
   (labeled honestly as what it does)**, not "Sync again."

### Gateway endpoint (new)

Add `/preview/<ns-hex>.json` to the newswire gateway's route set
(`apps/gateway/newswire.py`), serving a signed-export-derived summary for that namespace:
`{ namespace, name, member_count, recent_post_titles[] }`. This is the same data the
newswire renderer already has (`_from_v2` space block, `newswire.py:89-101`) exposed as a
small JSON shape instead of HTML. Conference gateway (`riot_gateway.py`) is intentionally
**not** extended — it's pinned to one fixture; the preview endpoint lives on the newswire
gateway which already serves per-space data.

### Recovery-screen enrichment

`CommunityUnavailable` (CommunityShell.swift:79-92) gains optional `namespaceIdHex` +
`contentDigestHex` (populated wherever the unavailable community's coordinates are known at
the failure site). The recovery view, when those are present, runs the same preview fetch and
renders tier-2 summary above the Retry action — so "couldn't be opened" becomes
"`<verified name>` couldn't be opened. Here's what it is: …" instead of a bare Retry.

### Offline / failure behavior (mandatory)

- Gateway unreachable ⇒ tier 1 only (current behavior, no regression).
- Fetch returns garbage/mismatch ⇒ discard, tier 1 only, no error surfaced as fact.
- Fetch returns a valid-but-stale descriptor ⇒ shown as tier 2 (stale is the accepted v1
  bound, identical to followed-site model).
- All of the above are tested.

## File scope (indicative)

- `crates/riot-core/src/newswire/` — `preview.rs` (new): fetch + verify_descriptor_matches
  gate; `CommunityPreviewSummary` type.
- `crates/riot-ffi/src/newswire_ffi.rs` — new FFI `newswire_fetch_preview(namespace, digest)`.
- `apps/gateway/newswire.py` — new `/preview/<ns>.json` route + tests.
- `apps/ios/Riot/JoinReferenceModel.swift` — tier-2 rendering state + fetch trigger.
- `apps/ios/Riot/JoinByReferenceSheet.swift` — tier-2 summary card.
- `apps/ios/Riot/CommunityShell.swift` — `CommunityUnavailable` gains coordinates;
  `ShellRecoveryState.communityUnavailable` gains preview-backed summary.
- `apps/ios/Riot/ConferenceShellView.swift` — recovery view renders tier-2 when available.
- `apps/ios/RiotTests/` — preview fetch/verify tests, offline fallback, signed-overrides test.

## Out of scope

- Showing verified members/posts **before** sync (impossible without the digest-of-each-item;
  tier 3 stays authoritative).
- A preview for the nearby-adoption flow (peer-supplied title is a separate human-trust model).
- Any change to the §9.3 seizure disclosure (fires only at mint-masthead, unrelated).
- Extending the conference gateway (pinned fixture; newswire gateway only).

## Risks

- **Privacy:** a fetch to `<gateway>/<ns-hex>/preview.json` leaks the user's interest in a
  namespace to the gateway operator. Mitigation: same bound as followed-site (operator sees
  the request); document in the privacy page.
- **Spoofing via stale:** a seized gateway could serve an old-but-valid descriptor.
  Accepted v1 bound (matches followed-site model); tier-3 signed data overrides on conflict.
- **Scope creep into nearby:** deliberately excluded.
