# Closing the gaps: what's missing and in what order

Date: 2026-08-04
Status: Rev 2, after a plan review gate that failed the first draft on all three
axes (feasibility, completeness, scope). Rev 1's ordering rested on a false
premise and is not preserved.

## Product decision, 2026-08-04 (rabble)

**The newswire is public broadcast. The goal is saturation — getting information
out and spread as widely as possible. It is deliberately leaky. It is not, and
must not be confused with, the encrypted private-groups mode.**

This resolves the design questions rev 2 said were blocking item 1:

- **Who may write to an anchor?** Anyone. Open publishing, as indymedia's open
  wire was. No authorization gate on the open newswire.
- **What stops a flood?** Nothing may stop it by authority. Flooding is a
  CAPACITY problem (bounded stores, fair-share, eviction), not an admission
  problem. Refusing writes to prevent abuse would defeat the point.
- **Does accepting imply endorsement or liability?** No — an anchor carrying open
  wire content is a carrier, not an editor. Worth writing down somewhere durable
  before an operator is asked the question under pressure.

⚠️ **Leaky CONTENT is the goal. Leaky AUTHORS is not.** These are different axes
and it would be easy to collapse them while chasing reach. Communal authors are
random per community and unlinkable by design; that is an author-safety property,
not a content-secrecy one, and saturation does not require giving it up. A person
posting from a protest wants their report everywhere and their location nowhere.
Every reach item below must be read with that split in mind — in particular the
stable-identity question in item 3 and the IP-exposure note.

Private groups / MLS remain a separate mode and stay deferred; nothing here
applies to them.

## The one-sentence state of the product

**A post you write never leaves your phone unless another device is physically
nearby, that device only carries it onward while that community is selected, and
the whole community caps at 64 records.**

Everything below follows from that sentence.

## What rev 1 got wrong

Recorded because the errors are instructive, not for penance.

- **The relay was listed as "verified working" in both directions.** It is
  read-only by design: `crates/riot-ffi/src/net/anchor.rs` — *"a ReadCommitted
  pull sends nothing"*, *"The one-way ReadCommitted initiator repository"*.
  Seeing `imported=4` in production logs proves INBOUND only. Reading a one-way
  pipe as two-way inverted the whole priority order.
- **The wrong ceiling was cited.** Rev 1 said "caps at ~1024 entries… not
  urgent". The binding cap is `MAX_SYNC_IDS = 64` **per namespace**
  (`crates/riot-core/src/sync/wire.rs:7`), and it refuses **writes and sync**,
  offline included. 1024 is `MAX_STORE_ENTRIES`, a looser and different limit.
  This contradicted `docs/decisions/2026-08-03-store-scaling-status.md`, written
  the day before by the same author.
- **"Real Bonjour is covered by no automated test" was false.**
  `apps/ios/RiotTests/LocalNetworkNearbyTests.swift` contains
  `TwoPeerNearbySyncTests`: two real `NearbyTransportController`s, a real
  `NWListener` advertising `_riot-sync._tcp`, a real `NWBrowser`, real TCP, the
  real handshake and sync, driven through the same public API the UI calls. Only
  **BLE** is uncovered.
- **CI was described as absent.** `.github/workflows/apple-compile-gate.yml`
  already builds iOS and macOS on every PR, including the
  `aarch64-apple-ios-sim` slice. It contains **zero** `xcodebuild test`
  invocations — it compiles the app and never runs one of the 549 tests.
- **A prior audit was never read.**
  `docs/superpowers/plans/2026-08-02-state-of-the-app-audit.md` catalogues the
  built-but-unreachable surfaces. Rev 1 silently dropped all of them.

## Verified working (2026-08-04)

| capability | evidence |
|---|---|
| Two devices sync nearby, incl. real Bonjour discovery | `LocalNetworkNearbyTests` (`TwoPeerNearbySyncTests`) + `TransportContractTests`, 62/62 |
| Two profiles replicate through the app's own coordinator | `AppSyncReplicationTests` 11/11, real sockets, converges under concurrent edits |
| Multi-hop forwarding | Verified in code: `sync_commit` merges a peer's entries into the receiver's own inventory with **no author filter**, and `install_sync_inventory` enforces `inventory_ids == live_ids` exactly — forwarding is structurally mandatory |
| Two peers over iroh QUIC | `riot-transport/iroh_sync` 3/3 — but loopback with literal IPs; proves QUIC carries frames, nothing about NAT |
| Relay **pull** | Production 2026-08-03 |
| Reactions, replies, posts, relaunch | `newswire_contract`, `NewswireSurfaceTests`, verified on the real macOS app |

Multi-hop is bounded three ways: **64 records**, **only the selected community**
(nothing iterates joined communities in the background), and **durable stores
only** (`build_followed_site_offer` returns `Internal` on a memory profile).

---

## 1. A post can leave the phone at all

**Today it cannot, except to someone in the room.** This is the product working
or not working, for anyone not physically present.

It is not unwired plumbing. `riot/sync/2` has three modes; the anchor
(`crates/riot-anchor/src/sync_service.rs:649-658`) serves `ReadCommitted`,
token-gates `HostReconcileStaged` behind an `operation_id` + `namespace_token`
(an organizer staging operation), and **refuses `ReplicaIntoStaged` outright**.
An ordinary member publishing to a relay is a capability nobody has designed.

**The design questions are now answered** (see the product decision above):
anyone may publish, flooding is handled as capacity rather than admission, and a
carrying anchor is a carrier and not an editor. What remains is engineering: an
outbound mode the anchor will accept, reusing the endpoint, ticket admission and
transport-floor gate already proven inbound.

**Done when:** someone posts on a phone with nobody nearby, and someone else on a
different network sees it.

**Size:** estimable now that the policy is settled. The anchor currently refuses
`ReplicaIntoStaged` outright, so this is a serve-side change plus an FFI dial,
not a token-gated organizer flow.

## 2. A device carries only the community you are looking at

`open_sync_session` builds ONE session for the ACTIVE namespace, from the active
community's inventory. Nothing iterates joined communities. So a phone holding a
tenants union, a mutual aid network and a protest wire passes along only whichever
happens to be selected when it meets someone.

For a saturation goal this is the single biggest miss: every device is a carrier
for everything it holds, and today it behaves as a carrier for one thing at a
time, chosen by what is on screen. Sneakernet propagation — the property that
makes this work during a shutdown — is mostly not happening.

**Done when:** two devices that meet exchange everything they both hold and are
entitled to, without anyone selecting a community first.

⚠️ This must not widen what a peer is offered ACROSS a community boundary: the
`inventory_ids == live_ids` equality guard is marked LOAD-BEARING for exactly
that isolation. Carrying more communities means more sessions, not a wider
inventory in one session.

## 3. The 64-record ceiling

**64 is not a Willow limit, and it contradicts what Willow is for.** Willow is
designed for large stores reconciled by 3D range fingerprints with recursive
splitting — cost proportional to DIFFERENCES, not to store size. Scale is the
point of the data model. Nobody should read this ceiling as inherited from the
protocol Riot is built on; it is the opposite of it.

What happened is that `willow25` implements no reconciliation at all — the string
`fingerprint` appears **zero** times in the crate — so Riot hand-rolled
`org.riot.conference-sync/1`, which puts the COMPLETE entry-id list in one
`Summary` frame with no paging and no rounds. 64 is simply how many ids that
frame is allowed to carry.

And it is placeholder-grade even for that naive protocol: 64 ids × 32 bytes is
2 KB against a `MAX_SYNC_FRAME_BYTES` of 8 MB — **0.02% of the frame it is
allowed to use**. The same frame could carry ~262,000 ids.

`MAX_SYNC_IDS = 64` is per namespace and counts records of every kind —
descriptor, posts, comments, reactions, profile cards. It refuses **writes**
(offline included), **nearby sync**, and any future peer sync. A tenants union
reaches it in a week; reactions are the highest-frequency write and each tap is a
permanent record (see 12).

**Two separable problems, and only one is hard:**

1. *The number.* Raising it is a contract amendment (`fixtures/manifest.json` +
   `crates/xtask/src/main.rs`) and buys orders of magnitude — but it lands on the
   next wall almost immediately: `MAX_STORE_ENTRIES = 1024`, then an unlocated
   quadratic that makes 12,000 entries take 444 seconds to write
   (`docs/decisions/2026-08-03-store-scaling-status.md`). Expect a few thousand
   records from this, not a city newswire.
2. *The protocol.* Paged frames, then fingerprint range reconciliation — the
   actual Willow design. `willow25` does not provide it, so it must be built,
   arguably upstream rather than in Riot.

Do 1 to stop the bleeding, having first instrumented the write path so the
quadratic is found rather than guessed at. Do 2 to make store size stop
mattering.

The proposed fix already exists and went through the design gate:
`docs/superpowers/specs/2026-08-03-area-scoped-windowed-sync-design.md`. It came
back NEEDS_REVISION from all five reviewers — **read the corrections before
building from it**, particularly the moderation-exemption rule (a windowed-out
tombstone silently un-redacts content).

**Done when:** a community can hold a year of ordinary use without refusing a
write.

## 4. Phone-to-phone directly

`riot-transport` has `sync_accept` and `accept_with_router`; grep for either in
`crates/riot-ffi/src/` returns **zero**. `NetRuntime` is documented as *"a pure
dialer"*. What this actually costs:

- **NAT physics.** QUIC hole-punching needs a third-party rendezvous by
  construction. `bind()` is `N0DisableRelay` — "direct only, for local/LAN";
  `bind_public()` is `N0` — "relay + pkarr/DNS discovery … reachable from
  anywhere across NAT". *"Different cities"* and *"no relay reachable"* cannot
  both hold.
- **Identity conflict.** Each bind mints a **fresh ephemeral NodeId** by design.
  A dialable peer needs a stable one. Given the public-broadcast decision, a
  stable TRANSPORT identity is likely acceptable — but it must not become a
  stable AUTHOR identity: the per-community random author is what keeps a person
  from being linked across communities, and it is independent of the NodeId a
  socket uses. Keep them separate deliberately, and write down why.
- **A phone cannot currently tell anyone its own NodeId** — `NetRuntime::node_id`
  is not exported over uniffi.
- **iOS gives a backgrounded app no listening UDP socket**, so both people must
  have the app foregrounded simultaneously — which removes most of the
  store-and-forward value the anchor path was for.
- **The FFI's only iroh dial speaks `ReadCommitted`.** The bidirectional
  reconcile is `sync/1`/`ByteSyncSession`, which `riot-ffi` never drives over
  iroh. And `sync/1` caps at 64 (see 2).

**Done when (split, because one half is achievable much sooner):**
- **4a** — two profiles on the same LAN reconcile bidirectionally over
  direct-addressed QUIC with **no Riot anchor** in the path.
- **4b** — two profiles on different networks reconcile with **no Riot anchor**,
  using iroh's public relay + discovery for rendezvous and hole-punching only.
  This is the honest form of the argument: it removes *Riot's* relay as a single
  point of failure and surveillance; the rendezvous server sees connection
  metadata but no content.

**Size:** 4a ~1–2 weeks. 4b 4–8 weeks, and blocked on the identity decision.

⚠️ Item 4 exposes each phone's IP to every peer it dials, which the relay
currently masks. `docs/discovery-options.md` reasons carefully about link-local
presence leaks; routable dialing is owed the same rigor before it ships.

## 5. Correcting the record

`NewswireEditorial.swift:687` — `EditorialActionKind.allCases.filter { $0 != .retract }`.
Retract is implemented in core, modelled in Swift, and **deliberately filtered
out of the UI**. Per the 08-02 audit: *"Updating and correcting the record is in
the one-sentence goal for this app, the core supports it, and there is no way to
do it."*

For a tool descended from indymedia, being unable to correct or withdraw what you
published is a hole in the premise. Also worth deciding: retract is editor-gated,
not author-gated — should an author be able to correct their own post?

**Size:** small, if the gating question is answered. Mostly surfacing existing
core.

## 6. Turn the test suite on in CI

`apple-compile-gate.yml` builds and never tests. Four separate stale-test
failures were found on 2026-08-03, from four different PRs, each silently red
since it landed — the same shape as the outage.

- **Extend** `apple-compile-gate.yml`; do not add a job (toolchain, bindings and
  staticlib steps are already there, and macOS runners bill at **10×**).
- Destination `platform=macOS,variant=Mac Catalyst,arch=arm64` against the
  `RiotKit` scheme is **verified working** — used repeatedly on 2026-08-04.
  (`scripts/test.sh` uses `platform=macOS`; either resolves.)
- Needs `generate-bindings` with the net-bindings flag, and the
  `aarch64-apple-ios-macabi` slice copied to `build/native/ios-simulator/` — the
  artifact there today is a macCatalyst build under an iOS-Simulator name.
- Add `cargo build -p riot-core --no-default-features`. A missing
  `#[cfg(feature = "sqlite")]` broke every non-sqlite build for a day because
  every check used `--all-features`, the one combination that cannot see it.
- **Land report-only first**, or fix 9 and 10 before flipping to required —
  otherwise the gate is red on day one.

## 7. Android is a platform behind

`grep -rl "syncWithAnchor|syncWithNextRelay|bindNetRuntime" apps/android/app/src/main`
→ **zero**. Android is BLE + Wi-Fi only, with no relay or anchor surface at all.
Its CI job runs 44 surface-logic test files and, by its own comment, *"never
loads libriot_ffi.so"*; `androidTest/` instrumentation runs nowhere.

Every reach item above is two-platform work costed for one. Decide explicitly
whether Android trails or ships in step.

## 8. "Did it reach anyone?"

Rev 1's premise was false: `ImportAcceptance.accepted_entry_ids`
(`mobile_state.rs:1538-1543`) is built from the **local** commit and never
crosses the wire; `SyncFrame` has six variants and none is an ack.

A real signal exists: the peer's `Request { entry_ids }` names exactly which of
your entries it lacked, and after your `Entries` it sends `Complete` (or
`Reject`). **`Complete`-after-`Entries` is a genuine on-wire acceptance.**
Bundle-granular, not per-entry, and only observable by the side that sent.

⚠️ With forwarding confirmed, a peer that merely relayed also counts. "Reached 2
people" may mean two carriers and zero readers — scope the count to distinct
peers that requested-and-completed, and never call it a read receipt. It is a
count of devices, not identities: do not build a read-receipt graph in an app
used under surveillance.

**Size:** 3–5 days once there is an outbound path to acknowledge.

## 9. Field failure

Nothing in rev 1 covered what a person sees with no relay and no peers, an
interrupted sync mid-bundle, backgrounding or battery death during sync, or
**panic wipe** — a stated safety principle with an Android test and no named iOS
counterpart. For an app whose users' phones get seized, the seizure path deserves
at least a line, even if that line is "done, here is the test."

## 10 & 11. The two red tests

`AnchorProtocolVectorTests` — a `CommunityListingV1` vector expects 19 fields, the
code emits 18 (a steward/contact name). Intermittent across runs and the fixture
embeds timestamps, so rule out a time-dependent fixture first.

`CompactReactionBarNativeSnapshotTests` ×2 — `NSApplication has not been created
yet`. These are the tests the compact-reaction design named as the guard against
exactly the glyph drift that shipped (f81637ae). Both block 6 going required.

## 12. Reactions cost O(taps)

Every reaction and every toggle-off is a distinct unprunable sibling path, so
on/off/on leaves three permanent records while the UI shows one. Putting identity
coordinates in the path (`.../reactions/<parent_entry_id>/<kind>`) would let
Willow's prefix pruning do the supersession `projection.rs` does by hand.

Reclassified: this is not cosmetic debt, it is **an input to item 3** — reactions
are the fastest way a community reaches 64. Record-family migration touching five
sites including the non-compiler-forced `store.rs` prefix scan.

## 13. Store scaling below the sync cap

`docs/decisions/2026-08-03-store-scaling-status.md`. **Do not raise a cap and do
not propose a fix from reading the code** — six predictions in one day, six
wrong, two fixes tried and reverted for no measurable gain. Next step is
instrumenting the write path phase by phase.

---

## Deliberately out of scope, stated so it is a decision and not an oversight

Each is built-or-designed and unreachable; none is planned now:

- **Private groups / MLS and the bridge** — half the dual-mode design. Last, per
  the standing decision.
- **Composite sites, personal spaces, meadowcap capabilities** — design-gate
  passed, core complete, no surface.
- **Follow-a-site** — FFI exists, no screen.
- **Archive / restore communities** — core supports it, no surface.
- **App-to-app APK distribution** — designed 2026-07-29, unimplemented.
- **Local LLM field editor** — a product-brief section with no status.
- **Distribution reach** — iOS TestFlight-only; Play blocked (#151); the APK is
  arm64+x86_64, so 32-bit phones cannot install it.

## Order

Reordered around saturation, per the 2026-08-04 product decision.

1. **Item 1 — a post can leave the phone.** Policy is settled; this is now
   engineering, and nothing else changes what a person can do as much.
2. **Item 2 — carry every community you hold, not just the selected one.**
   The cheapest large multiplier on reach, and pure sneakernet value during a
   shutdown.
3. **Item 3 — the 64-record ceiling.** It caps everything 1 and 2 unlock; you
   cannot saturate anything with 64 records.
4. **In parallel, cheap:** item 5 (retract) and item 6 CI report-only, with 10
   and 11 as prerequisites.
5. **Then** 4a (LAN peer dial), then 8 (reach signal), then 4b (cross-NAT).
6. **Then** 7 (Android parity), 9 (field failure), 12, 13.

## Rules carried forward

1. Measure, do not predict. Every performance intuition here has been wrong.
2. Do not diagnose from UI copy — distinct failures wore one string for a week.
3. Test across two profile lifetimes and with a full store.
4. `--all-features` hides feature-gating breaks; also build `--no-default-features`.
5. State direction when describing sync. "It synced" hid a one-way pipe for a day.
