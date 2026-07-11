# App directory design

## Purpose and scope

A storefront-style directory for discovering, trusting, sharing, endorsing,
and publishing signed JS microapps in Riot. This builds directly on the
signed JS apps platform (`docs/superpowers/specs/2026-07-11-signed-js-apps-design.md`
and its implementation plan) and **depends on that plan landing first** —
manifest/bundle codecs, `app_id` derivation, trust-list evaluation, and the
`AppDataBridge` are all prerequisites this design reuses, not reinvents.

Phasing agreed during brainstorming: v1 serves an **organizer equipping a
space** first, but the data model is deliberately shaped so member browsing
and cross-community spread work from day one, and so dedicated catalog
spaces (phase 2) need no new mechanism.

v1 includes the full publishing loop: a developer-facing CLI that packs a
folder of HTML/JS into a signed bundle, and in-app share/endorse flows that
move apps between communities.

## The core idea: content-addressing makes zine-passing safe

`app_id` is already content-derived (hash of manifest + bundle digest, per
the platform design). This cleanly separates two roles that centralized app
stores conflate:

- **Author** — the identity inside the signed manifest. Tamper with the app
  and the `app_id` changes; you haven't forked someone's reputation, you've
  made a different (untrusted) app.
- **Carrier** — whoever moved the bytes into your space. Like the friend who
  photocopied a zine: their identity signs the *entries*, not the app.

Anyone can carry an app anywhere; nobody can modify one in transit. Trust
markers and endorsements reference the `app_id`, so they stay meaningful
across every space the app travels to.

## Data model

Three subtrees of ordinary signed Willow entries, plus one compiled-in
catalog. Nothing here bypasses the existing
`inspect → plan_all → commit` pipeline.

- `app-index/<app_id>/manifest` — the app's manifest entry.
- `app-index/<app_id>/bundle` — the app's resource bundle entry.
- `app-index/<app_id>/endorsements/<endorser-subspace>` — one small signed
  "we use this" marker per endorsing organizer: CBOR struct of `app_id`,
  optional short note (size-capped), timestamp. Last-write-wins per path, so
  an endorser can update or blank their marker.

`app-index/` is deliberately distinct from `apps/<app_id>/...` (reserved by
the platform plan for the app's own runtime data) so an app writing a data
key named `manifest` can never collide with its distribution entries.

Trust markers are unchanged from the platform design: organizer-signed,
last-write-wins, only honored from a space's known-organizer subspace list.

**Starter catalog**: a small set of built-in apps (first: the checklist)
embedded in the binary via `include_bytes!` as manifest+bundle pairs signed
by a fixed Riot project author identity — the same fixed-public-author
precedent as the conference fixture. Built-ins run through the exact same
decode/verify path as synced apps and get zero special treatment in code;
"Built into Riot" is a provenance label, not a trust shortcut. Like any
other app, a built-in is only launchable in a space once an organizer
trusts it there.

**Provenance is derived, not declared.** Two verifiable sources, no free
text: "Shared by Ana" comes from the carrier's signature on the
`app-index` entries themselves (synced, cryptographically checked), and
"Built into Riot" / "Arrived from Jail Support" from the phone's own local
import context at the moment entries arrive. Neither is a claim an app
author writes into circulation — there is no forgeable provenance field
anywhere in the data model.

**The directory is computed, not stored.** A pure function assembles
listings from: all valid `app-index` entries across synced spaces + the
starter catalog + trust markers + endorsement markers. Invalid signatures
are silently excluded (existing import pattern). There is no directory
database to migrate or corrupt.

## Storefront UI

One **global storefront** across all the person's spaces (chosen over a
per-space utility list during brainstorming), endorsement-led:

- A featured card up top ("most endorsed"), then compact cards for the rest.
- Each card: name, author display name, provenance label, endorsement
  count, trust state ("On in 2 spaces" / "New").
- Tapping opens the **review page** — the trust-decision moment: author
  ("also made Ride Board"), provenance chip, named endorsing groups,
  plain-language description, a "THIS APP CAN" permissions box rendered
  from the manifest's permission list ("Keep its own notes in this space.
  Nothing else — no internet, no photos"), and the action button.
- Organizer sees: **"Let everyone here use this"** → space picker (only
  spaces where they're a recognized organizer) → trust marker written.
- Non-organizer sees: "Ask an organizer to turn this on" — v1 keeps this a
  face-to-face ask; no request-message plumbing.

Daily use does not go through the storefront: each space has a **Tools
row** listing its trusted apps. Launch checks the trust list at open time,
so a revoked app quietly disappears rather than erroring.

## Flows

**There is no install step.** Joining a space brings you its trust markers
and app bundles through ordinary sync, so the community's tools appear in
your Tools row automatically — same as its checklists would. The
organizer's one trust decision covers everyone, including future joiners.
This is a headline property; UI copy must never say "install".

**Enable (organizer)**: storefront → review page → "Let everyone here use
this" → space picker → trust marker. Syncs to all members.

**Launch (member)**: space's Tools row → tap → WebView host (platform
plan's runtime) launches the trusted bundle.

**Share (anyone)**: detail page → "Share to a space" → pick one of your
spaces → your phone re-wraps the identical manifest+bundle bytes as
`app-index` entries in that space, signed by you as carrier. Arrives in
members' storefronts under "New", provenance "Shared by <name>". Sharing
never auto-trusts.

**Endorse (organizer)**: on the detail page, an organizer of a space where
the app is already trusted can tap "We use this", writing one endorsement
entry. The storefront counts distinct endorsing *groups* (deduped by
subspace), names them, and ranks endorsements from groups you share spaces
with above ones from identities you've never synced with.

**Reputation is endorsement-only in v1**: no reviews, ratings, or comments.
Forging reputation requires inventing group identities, which carry no
weight with people who don't share spaces with them.

## Phase 2 (designed for, not built): catalog spaces and forking

Because apps, endorsements, and trust are all ordinary entries, **any space
is already a potential app directory**. A community can run a space that
exists purely to curate apps; joining it fills your storefront. Forking a
directory = carrying the `app-index` entries you like into your own space
and layering your own trust/endorsement curation on top. No new mechanism,
no privileged catalog format — phase 2 is UI affordances (subscribe,
fork-this-directory), not architecture.

## Components

**`riot-core`** (new files in the existing `apps/` module):
- `apps/index.rs` — pins the `app-index/...` path scheme; endorsement
  marker codec (manual minicbor style, matching `model/mod.rs`).
- `apps/directory.rs` — pure listing assembly:
  `(app_index_entries, trust_markers, endorsements, starter_catalog) →
  Vec<AppListing>`. `AppListing`: `app_id`, manifest fields, provenance,
  per-space trust state, endorsement summary, `newer_version_of` (grouping
  by author identity + name; **never** merges same-name different-author).
- `apps/starter.rs` — embedded built-ins, verified through the standard
  decode path at load.

**`riot-ffi`**: `directory_listings()`, `endorse_app(app_id, space)`,
`share_app(app_id, space)` — thin `with_active` delegators alongside the
platform plan's trust/bridge surface.

**Native UI (iOS + Android)**: storefront screen, review/detail screen,
space-picker sheet, Tools row per space. No WebView involvement in the
directory itself — browsing and trusting happen entirely in native code
before any app JS can run.

**`riot-app` CLI** (`crates/riot-app-cli/`): `keygen`, `pack <dir>` (reads
`riot-app.json`, validates size caps and entry point, signs, emits a
standard import bundle — the same `encode_bundle` format the import
pipeline already accepts), `inspect <file>`. Reuses `riot-core` as a
library; publishing invents no new transport or format.

## Error handling and plain-language UI

Extends the platform design's table; still no "bundle", "signature",
"namespace", or "sync" in anything a person sees:

| Situation | What the person sees |
|---|---|
| Bundle with bad signature arrives | Never listed anywhere — silently excluded |
| App trusted but bundle bytes not yet synced | "Still arriving from your group…" instead of a launch button |
| Same name, different author (impersonation) | Both listed, authors visually distinct; never merged |
| Newer version of a trusted app arrives | "Update available" on detail page; organizer re-approves (new `app_id` = new trust decision) |
| You leave / are removed from a space | Its tools leave your Tools row; storefront still shows the app if present via other spaces |
| Endorsement from a group you've never met | Counted but ranked lower; tapping shows "You haven't met this group" |
| CLI pack failure | Specific message with the actual limit ("bundle is 1.4 MB; limit is 1 MB") |

## Testing strategy

TDD throughout, matching `riot-core` conventions:

- **Core**: endorsement codec round-trip + tamper rejection;
  `directory.rs` assembly — provenance precedence, impersonation non-merge,
  version grouping, endorsement dedup by subspace, unknown-group ranking;
  starter catalog with a deliberately corrupted embedded bundle must
  exclude it.
- **FFI**: contract tests — share then list from the receiving side;
  endorse then observe the count change; trust then see the listing flip
  to enabled.
- **CLI**: golden-file tests — `pack` output decodes via `riot-core`'s own
  decoder with a stable `app_id` for identical input; adversarial fixtures
  (oversize, missing entry point, path traversal in resource names).
- **Native**: one UI test per platform — storefront renders from a fixture
  store; trust action writes a marker; Tools row appears for a trusted app.

## Dependencies and sequencing

1. Signed JS apps core platform plan (exists, not yet implemented) — hard
   prerequisite.
2. This design's core + FFI + CLI work — testable entirely via `cargo test`.
3. Native storefront/Tools UI — needs the WebView runtime follow-up plan
   from the platform design for launch to work end-to-end, but storefront
   browsing/trusting can land before the runtime exists.
