# Relay-Published Community Directory — Discover on a Real Signed Feed

**Status:** Design (not yet built)
**Date:** 2026-07-24
**Author:** Designer agent
**Supersedes seed:** `apps/ios/Riot/Discover/CommunityDirectory.swift` → `SeededCommunityDirectory`

## Problem

The Discover front door (`CommunityDirectory` protocol, `CommunityDiscoveryModel`)
renders communities today from `SeededCommunityDirectory` — six clearly-marked
`isSeed: true` rows with `joinReference: nil`. There is no way to browse a
community you have no link for and actually walk into it. The seed's own header
already names the fix: a `RelayDirectoryFeedSource: CommunityDirectory` that
serves *verified* rows off the anchor relay, swapping in behind the protocol with
the surface and view model unchanged.

## What already exists (do not rebuild)

Reading `origin/main` shows the directory is much further along than "unbuilt":

- **Record model.** `CommunityListingV1` (`crates/riot-anchor-protocol/src/records.rs:379`)
  already carries `root_id`, the three namespace ids, `manifest_digest`/`manifest_version`,
  **`ticket_core_bytes` = canonical `RootSignedTicketCoreEnvelopeV2`** (a verifiable
  O-root signature, independent of the entry — this *is* the ReadCommitted join
  ticket), `listing_epoch`/`listing_revision`, `listed: bool` (tombstone),
  `title` (≤120B), `summary` (≤512B), `topic_tags` (sorted set ≤8), `languages`
  (BCP-47 ≤8), `region` (optional), `issued_unix_seconds`, `expiry_unix_seconds`.
- **Canonical admission gate.** `SubmitListingService::verify_submission`
  (`crates/riot-anchor/src/listing.rs`) does crypto-**before**-admit: real
  Meadowcap+Ed25519 via `verify_anchor_item_parts → riot_core::willow::verify_entry`,
  coordinate binding at `O:/directory/listing`, root-owned zero-delegation vs.
  root-signed delegate grant, and an internal-consistency re-verify of the
  embedded `ticket_core_bytes`. It is *the only* constructor of
  `AdmittedListingEnvelopeV1` and the sole feeder of `resolve_listing`
  (`crates/riot-anchor-protocol/src/authority.rs:409`). **This is the canonical
  gate — reuse it, do not hand-roll a subset** (memory: `riot-reuse-canonical-gate`).
- **Read wire protocol.** `ControlOperation::PullDirectoryFeed` /
  `PullDirectorySnapshot` with `FeedPullSuccessV1` / `SnapshotPullSuccessV1`
  (`checkpoint_bytes`, `snapshot_record_bytes`, `next_cursor_bytes`, `done`) and
  `SnapshotCursorV1` — all defined in `control.rs`.
- **Mobile net pull.** `NetRuntime::sync_with_anchor`
  (`crates/riot-ffi/src/net/anchor.rs:538`) already dials the deployed relay by
  bare NodeId, runs the canonical `admit_public_site_ticket` transport-floor gate
  **before any packet**, drives `riot/sync/2` ReadCommitted for the ticket's
  O/C/W namespaces, `verify_entry`s every item, and imports through the
  preview→plan→commit boundary. Exposed to Swift as
  `MobileNetRuntime::sync_with_anchor` under the off-by-default `net` feature
  (`crates/riot-ffi/src/net/ffi.rs`).
- **Swift plumbing.** `AnchorRelayDefaults.relayNodeId =
  60ab7b…2432d8` and a hardcoded `communityTicketHex` (River City Wire) drive
  `RiotAppModel.syncFromRelay()` / `bindNetRuntime()` today (`apps/ios/Riot/AppModel.swift:16`).
- **Discover surface.** `CommunityDirectory`, `SeededCommunityDirectory`,
  `CommunityDiscoveryModel`, `CommunityJoinRoute` (branch `feat/ux-times-person-discover`,
  `apps/ios/Riot/Discover/CommunityDirectory.swift`). `commitJoin` /
  `syncWithAnchor` + follow already exist in `AppModel.swift`.

## The four real gaps

1. **Daemon serves neither op.** `AnchorRepositoryService::handle` dispatch
   (`crates/riot-anchor/src/control.rs:325`) routes only `Describe`,
   `GetWorkChallenge`, `PrepareHost`, `CommitHost`, `GetOperation`. **`SubmitListing`,
   `PullDirectoryFeed`, and `PullDirectorySnapshot` all fall to the `_ =>
   ProtocolFailure::Unsupported` arm.** So no community can be listed on the live
   relay, and nothing can read a directory back — even though the listing service
   is fully unit-tested.
2. **No publish path.** `demo_host.rs` hosts a site (Prepare→sync/2→Commit) but
   never submits a `CommunityListingV1`. The owner has no command to publish or
   redeploy the directory on the live relay.
3. **No client directory pull + verifier.** The FFI can pull *one* community by
   ticket (`sync_with_anchor`); there is no `pull_directory` that returns the
   *list of listings*, and no client-side verifier that re-checks a pulled feed
   without trusting the anchor.
4. **No `RelayDirectoryFeedSource`.** The Swift `CommunityDirectory`
   implementation that maps verified rows into `DiscoverableCommunity` does not
   exist.

## Design

### Data model — reuse `CommunityListingV1`, map the display fields

The seed's `DiscoverableCommunity` wants: display name, one-line about, category,
steward name, people/activity hint, open-vs-invite, join ticket. Map onto the
**existing signed record** rather than inventing a parallel one:

| `DiscoverableCommunity` | Source |
|---|---|
| `name` | `CommunityListingV1.title` |
| `about` | `CommunityListingV1.summary` (first line) |
| `category` | derived from `topic_tags` — reserve four canonical tags `b"events"`, `b"help"`, `b"info"`, `b"guides"` and map to `DiscoverCategory`; default `.info` |
| `joinReference` / join ticket | `CommunityListingV1.ticket_core_bytes` (the `RootSignedTicketCoreEnvelopeV2`) — routed into the existing relay pull + `commitJoin` (below) |
| `isOpen` | see "open vs invite" |
| `isSeed` | `false` for every relay row |
| `id` | hex of `root_id` |

Two fields have no home in `CommunityListingV1` today:

- **Steward display name.** Recommendation: **add `steward_name: String` (≤64 UTF-8
  bytes) to `CommunityListingV1`.** It is owner-authored, root-signed, and cheap.
  Alternative (no record change): read the steward from the site's owner-signed
  `/manifest`/newswire descriptor — but that is only available *after* the person
  pulls the site, i.e. after joining, which defeats browse-before-join. Prefer the
  field. Cost: golden-vector regeneration (see contract caveat), no new dependency.
- **People / activity hint.** This is **not authenticatable** — the anchor cannot
  verify a self-reported member count, and a signed `peopleCount` would launder a
  lie into a trusted-looking number. Recommendation: **do not carry a raw count.**
  Derive a coarse, honest liveness hint on the *client* from what the relay
  legitimately proves — the site's committed hosting generation / last-commit
  recency in the pulled checkpoint — rendered as `"active recently"` / `"quiet"`.
  `peopleCount` on the relay row is `0` and the surface shows the hint, not a
  figure. (The seed keeps its illustrative counts; only relay rows change.)

**Open vs invite.** Recommendation for v1: **the public directory is the open
set.** A community that wants to be walk-in-able publishes a listing; an
invite-only community simply does *not* publish one and is reached only by a
shared `riot://newswire/join/v1/...` link or QR — the honest model, and it needs
no new field. Relay rows are therefore `isOpen: true`. If "invite-only but
listed" is wanted later, add `access: ListingAccessV1 { Open | Invite }` to the
record; do not overload `listed` (which is a tombstone).

### Relay side — dispatch, serve, publish

1. **Wire `SubmitListing` into the daemon.** Add the arm to
   `control.rs` dispatch that hands the raw submission to `SubmitListingService`
   (the `RawListingSubmission` → `verify_submission` → `resolve_listing`
   transaction). The wire→service adapter is the `FLAGGED` item in
   `listing.rs`'s `RawListingSubmission` doc ("the wire `SubmitListingV1` body
   cannot yet carry the entry/grant signatures; the wire→service adapter is a
   later work unit") — this design lands it.
2. **Serve `PullDirectorySnapshot` (and `PullDirectoryFeed`).** Add a read
   service in `crates/riot-anchor/src/` (`directory_read.rs`) that answers from
   the durable directory projection the listing transaction already maintains
   (the signed inclusion feed + current per-root listing state), paginated via
   `SnapshotCursorV1`, returning `SnapshotPullSuccessV1 { checkpoint_bytes,
   snapshot_record_bytes, next_cursor_bytes, done }`. Snapshot pull (whole current
   set) is what Discover needs; feed pull (incremental) is a follow-on.
3. **Owner publish command.** Extend `demo_host.rs` (or add
   `crates/riot-anchor/examples/demo_publish_listing.rs`) so that after `CommitHost`
   succeeds it mints a `CommunityListingV1` for the just-hosted site (reusing the
   `hosting_common` / `listing_common` fixtures: `genuine_listing`,
   `root_owned_item`, `root_signed_ticket`) and sends a `SubmitListing` control
   frame. The owner runs, against the live GCP relay (**describe only, do not
   run**):

   ```sh
   ANCHOR_NODE_ID=60ab7b416b0ef0b8088cd64a3ef01edd598dcc5bb7a4df03145f957fec2432d8 \
     cargo run -p riot-anchor --features daemon --example demo_host   # host + publish listing
   ```

   Redeploy = re-run with a fresh seed (new revision, monotonic manifest version;
   `resolve_listing` floors prevent rollback). The listing carries a durable
   multi-day ticket exactly as the current `communityTicketHex` was re-baked
   2026-07-24.

### App side — `RelayDirectoryFeedSource: CommunityDirectory`

- **New FFI (net feature).** `NetRuntime::pull_directory(anchor_addr, now) ->
  Vec<DirectoryListingRow>` in `crates/riot-ffi/src/net/anchor.rs`, exported as
  `MobileNetRuntime::pull_directory(anchorHint) -> [DirectoryListingRow]`
  (`uniffi::Record`) in `net/ffi.rs`. It dials the relay's control plane, drives
  `PullDirectorySnapshot` to `done`, and **verifies every row on the client
  without trusting the anchor**:
  1. verify the operator-signed `checkpoint_bytes` against the relay's **pinned
     operator key** (the relay identity — pinned alongside `relayNodeId`);
  2. for each `CommunityListingV1`, re-run the canonical
     `admit_public_site_ticket` over its embedded `ticket_core_bytes`
     (root-signature + coordinate binding + transport floor + expiry) — the same
     gate `sync_with_anchor` uses. A row whose ticket does not verify, whose
     coordinates disagree with the listing, or that is expired is **dropped**.
  This client verifier belongs in `riot-anchor-protocol` (pure, no iroh/tokio) as
  a `verify_pulled_listing` helper reusing `admit_public_site_ticket` +
  `verify_entry`, callable from the FFI which already deps
  `riot-anchor-protocol` under `net`. `DirectoryListingRow` carries the mapped
  display fields plus `ticket_core_hex` for the join route.
- **`RelayDirectoryFeedSource: CommunityDirectory`** (new
  `apps/ios/Riot/Discover/RelayDirectoryFeedSource.swift`). Holds
  `AnchorRelayDefaults.relayNodeId` + the pinned operator key + a fallback
  `SeededCommunityDirectory`. `discoverableCommunities()` returns the last
  verified pull (offline → the seed rows, still `isSeed: true` so nothing false is
  presented as live). A `refresh()` path calls `bindNetRuntime().pull_directory`
  off the main actor and maps each `DirectoryListingRow` →
  `DiscoverableCommunity(isSeed: false, joinReference: <ticket-bearing route>)`.
  **Drop-in:** change the single `CommunityDiscoveryModel(source:)` construction
  site to pass `RelayDirectoryFeedSource(fallback: SeededCommunityDirectory())`;
  the protocol, model, and view are untouched.
- **Join routing.** A relay row's join must (a) pull the committed O/C/W via
  `syncWithAnchor(ticketHex, relayNodeId)` then (b) route into the existing local
  `commitJoin`/follow so the community becomes a member. Extend
  `CommunityJoinRoute` with `.pullFromRelay(ticketHex: String, anchorNodeId:
  String)`; `route(for:)` returns it when the row carries a ticket. Seed rows
  (`joinReference == nil`) keep `.pasteOrScan`. Riot never fabricates a joinable
  coordinate for a sample row — unchanged invariant.
- **Build implications.** All of the above is behind the `net` Cargo feature
  (iroh/tokio, `riot-transport`, `riot-anchor-protocol`). The app must be built
  **net-enabled** — the default staticlib is iroh/tokio-free and
  `pull_directory`/`bindNetRuntime` do not exist in it. This is the same
  net-enable requirement that already gates `syncFromRelay`; a TestFlight build
  that exercises Discover-over-relay **must** ship the net-enabled core (memory:
  `riot-testflight-must-be-net-enabled`). iroh/tokio stay out of `riot-core`
  entirely (memory: `iroh-1.0-api`, `riot-mobile-transport-local-only`).

## Trust & safety

- **No trust in the anchor.** The relay is a *reach* mechanism, not an authority.
  Every row is only as trustworthy as (a) the relay operator key that signs the
  checkpoint (pinned, = relay identity) and (b) the community's own O-root
  signature over its ticket. Both are re-verified client-side; a compromised or
  lying relay can withhold or reorder rows but cannot forge a listing for a root
  it does not control.
- **Open set only (v1).** Public directory = walk-in communities; invite-only
  reached by link (see above).
- **Plural signed directory feeds.** The research's end state is users adding /
  removing / sharing multiple directories. **Recommendation: v1 ships ONE feed**
  (the deployed relay), because a single pinned operator key keeps the trust root
  singular and the whole path host-testable. Architect for plural additively:
  `RelayDirectoryFeedSource` takes a `DirectorySource { nodeId, pinnedOperatorKey,
  label }`; v1 constructs one, a later WU takes a `[DirectorySource]` and a
  user-managed list. No record or wire change is needed to go plural — only the
  source list and merge/dedupe-by-`root_id` in the Swift source.

## Work-unit decomposition

- **WU-1 (protocol / core).** `verify_pulled_listing` in
  `riot-anchor-protocol::authority` (pure): reuse `admit_public_site_ticket` +
  `verify_entry` + coordinate binding; return a verified display projection.
  Optionally add `steward_name` to `CommunityListingV1` (+ golden-vector
  regen). Tests: valid row, forged ticket, coordinate mismatch, expired, wrong
  operator key. No iroh.
- **WU-2 (anchor daemon + publish).** Dispatch `SubmitListing` (wire→service
  adapter, the `FLAGGED` item); serve `PullDirectorySnapshot`/`PullDirectoryFeed`
  from the projection; extend `demo_host` (or `demo_publish_listing`) to mint +
  submit a `CommunityListingV1`. Tests: submit→snapshot round-trip over the
  in-process loopback anchor (mirror `anchor_e2e`), pagination, tombstone hidden.
- **WU-3 (FFI net).** `NetRuntime::pull_directory` +
  `MobileNetRuntime::pull_directory` returning verified `DirectoryListingRow`s;
  drive snapshot to `done`, verify via WU-1. Tests: `net/anchor_e2e`-style
  loopback pull → verified rows; a served-but-forged row is dropped, not returned.
- **WU-4 (Swift).** `RelayDirectoryFeedSource: CommunityDirectory` with seed
  fallback; extend `CommunityJoinRoute` with `.pullFromRelay`; wire the single
  `CommunityDiscoveryModel(source:)` construction to it. Tests: mapping (row →
  `DiscoverableCommunity`), offline fallback to seed, route selection, pure
  `filter` unchanged. Net-enabled build only.

## Test strategy

- **Rust:** TDD per WU; loopback in-process anchor over iroh (reuse
  `net/anchor_e2e.rs` + `hosting_common`/`listing_common` fixtures) for the full
  publish→pull→verify path. Adversarial cases (forged ticket, wrong operator key,
  coordinate disagreement, expiry, tombstone) are first-class, not afterthoughts —
  this is a security surface. `cargo test --workspace --all-features` (a record or
  matched enum change can break `riot-ffi` invisibly under a scoped `-p`; memory:
  `riot-scoped-test-hides-cross-crate-break`).
- **Swift:** hostless `RiotTests` for `RelayDirectoryFeedSource` mapping/fallback/
  route and `CommunityDiscoveryModel.filter` (pure, no FFI). Live-relay pull is a
  manual/net-enabled integration check, not a unit test.
- **Coverage:** `.coverage-thresholds.json` is the floor; verify before commit.

## Cargo.lock contract-pin caveat

`xtask validate-contracts` pins a sha256 of `Cargo.lock` in
`fixtures/manifest.json` (memory: `riot-cargo-lock-contract`). If WU-3's client
verifier or WU-2's read service pulls in **any new crate**, CI fails until the
pin is updated to the printed `actual`. Reusing the existing `riot-anchor-protocol`
(already a `net` dep of `riot-ffi`) and `riot-core` verifiers **adds no
dependency and no Cargo.lock change** — the preferred path. Adding a field to
`CommunityListingV1` changes **golden vectors** (`control_vectors.rs`,
`golden_vectors.rs`), not `Cargo.lock` — regenerate and re-verify those instead.

## Risks & open questions

1. **Operator-key pinning.** The relay operator key must be pinned in the app
   alongside `relayNodeId`. Where does it come from — hardcoded next to the NodeId,
   or read from the relay's `Describe` and trust-on-first-use? Recommend hardcoded
   pin for v1 (single known relay).
2. **`SubmitListing` wire signature transport.** `RawListingSubmission` notes the
   wire body "cannot yet carry the entry/grant signatures." WU-2 must define how
   the signed listing entry + optional delegate grant travel in `SubmitListingV1`
   — confirm the frame can carry the full anchor-item bytes + 64-byte sig within
   `MAX_CONTROL_FRAME_BYTES`.
3. **Activity hint honesty.** Deriving liveness from committed-generation recency
   is honest but coarse; confirm the checkpoint exposes a usable last-commit time.
   If not, show category + "Open" and no liveness hint rather than a fake number.
4. **Steward field vs. manifest.** Adding `steward_name` to the record is the
   clean answer but costs a golden-vector bump and a record-version decision;
   confirm the owner wants it in the signed listing vs. deferring steward display
   to post-join manifest read.
5. **Category vocabulary.** `topic_tags` is a free-ish sorted set; reserving four
   canonical category tags must not collide with editorial topic tags. Confirm a
   reserved namespace (e.g. `b"cat:events"`).
6. **Snapshot size / pagination on mobile.** `PullDirectorySnapshot` page ≤ the
   protocol max; a large directory means multiple round-trips on a phone radio.
   Snapshot-only (whole set) v1; incremental `PullDirectoryFeed` deferred.
7. **Empty/first-run directory.** Until WU-2 publishes real listings, the relay
   snapshot may be empty; the source must fall back to seed rows (clearly marked)
   so Discover is never blank.
