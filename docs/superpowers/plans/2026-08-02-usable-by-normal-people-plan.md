# Making Riot usable by normal people

**Date:** 2026-08-02
**Status:** Plan, in progress
**Rule for this plan:** every fix lands red/green. No fix without a test that
failed first.


## What the app is actually for

Riot exists to let a group **write things down, read them, add to each other's
work, carry it to other people, and come back to all of it later** — across many
uses of the app, on more than one device, by more than one person.

That sentence is the specification. Everything below is a cell in it, and a cell
is only real when a test walks it end to end. The bugs in this plan are not
independent defects; they are the cells where that loop breaks.

### The grid

Rows are what a person does. Columns are how far the data has to travel to still
count. `✅` has a journey test that walks it; `⚠️` is partially covered or
covered only against a substitute; `❌` has nothing.

| | same session | after relaunch | after upgrade | to a person in the room | to a person elsewhere |
|---|---|---|---|---|---|
| create a community | ✅ | ✅ | ❌ | ⚠️ | ❌ unbuilt |
| post an update | ✅ | ✅ | ❌ | ⚠️ | ❌ unbuilt |
| reply to someone | ✅ | ✅ | ❌ | ❌ | ❌ unbuilt |
| react to a post | ✅ | ✅ | ❌ | ❌ | ❌ unbuilt |
| join by link / QR | ✅ | ❌ | ❌ | ⚠️ | ❌ unbuilt |
| join from a relay pull | ❌ | ❌ | ❌ | — | ⚠️ pull only |
| edit or retract | ❌ | ❌ | ❌ | ❌ | ❌ unbuilt |
| carry a tool / app | ⚠️ | ⚠️ | ❌ | ⚠️ | ❌ unbuilt |
| follow another site | ⚠️ | ❌ | ❌ | ❌ | ❌ unbuilt |

Two things this makes plain that a bug list does not:

1. **The "after relaunch" column is a lie in the field.** Those `✅`s pass
   against a *stable test-double key store*. On a real device the identity does
   not survive, so every one of those cells is actually broken for a real person.
   A green column that disagrees with the product is worse than a red one.
2. **The right-hand column is nearly empty**, and it is the reason people would
   use this at all. Collaboration between people who are not in the same room is
   the product, and it does not exist (issue #107).

The definition of done for this plan is that grid, filled in — each cell walked
by a test that uses the real substrate, not a substitute.

## The cell that blocks the whole grid

**Riot has destroyed a person's identity and community three times.** (Not a
continuous loop — see the correction below. It fired on 25 and 31 July and has
not fired since.)

Evidence from a real machine on 2026-08-01, not a hypothesis:

`~/Library/Containers/net.protest.riot/…/quarantine/recovery.log`

```
2026-07-25T06:10:23Z  profile-open  InvalidInput   ← app 0.1.1
2026-07-31T03:25:57Z  profile-open  InvalidInput   ← app 0.1.2
```

And, decisively, feeding each quarantined sealed identity plus the wrapping key
currently in the login keychain to the CURRENT core:

| identity | result |
|---|---|
| sealed 25 July (oldest) | **OPENED** |
| sealed 31 July | `InvalidInput` |
| **the live profile, in use right now** | `InvalidInput` |

The live profile cannot be opened by the key on this machine. **The next launch
will quarantine it again.** This is not an upgrade-migration bug that fires once
per release — it is a permanent loop, and every pass through it costs the person
their identity, their community, and their authorship of everything they posted.

That is also the true cause of "reactions and replies don't work": each loop
mints a NEW author, and a new author has no standing in a community they
appeared to have joined.

### What is already ruled out

- Not a JSON schema change. The Swift decoder ignores the legacy
  `trustedAppIDs` field and every property is `decodeIfPresent`; all three
  profiles decode.
- Not a malformed envelope. All are exactly 112 bytes with the `RIOTID` magic.
- Not a bug in `KeychainWrappingKeyStore.create()`. It handles
  `errSecDuplicateItem` by re-reading, and throws rather than returning a key it
  failed to persist.
- Not a panic. The FFI panic hook is compiled into the shipped dylib and never
  fires.
- Not the core write path. In-memory readers, durable readers, and durable
  self-authored journeys all pass.

### CORRECTION, 2026-08-02: the key hypothesis is DISPROVEN

Ran the diagnostic. Two consecutive launches of the current build:

```
LAUNCH 1  wrapping key LOADED  fp=25666804
LAUNCH 2  wrapping key LOADED  fp=25666804
CLI key fingerprint            25666804
quarantines                    0
```

The wrapping key persists, is stable across launches, and is the same key the
CLI reads. No quarantine occurred. **The app is healthy right now**, and the
"permanent loop" framing in the previous revision of this plan was wrong.

A sharper fact falls out of yesterday's data: the identity quarantined on
**25 July opens fine with today's key**. It was never unopenable. So the
recovery ladder blamed `profile-open` for something that was not the identity
at all.

The untested variable is the difference between the diagnostic and the real
path:

- diagnostic called `open_profile_from_sealed_identity` — no database
- the app calls `open_profile_from_sealed_identity_WITH_DATABASE`

so the failure is plausibly the SQLite open or the persisted replay, not the
seal. The 25 July profile also carried `starterCatalogGeneration` ABSENT while
the diagnostic passed `Some(2)`, which is a second untested difference.

**Next:** run the with-database variant against the real quarantined database,
identity, and that profile's actual generation. Three known-good inputs, one
known-bad outcome — that isolates it.

**Kept regardless:** the fingerprint logging. It costs nothing, and if this
recurs the log now says immediately whether the key changed.

## Work units

### WU-1 — The relaunch column becomes true. **Blocks the whole grid.**

1. **Red:** a durable journey that opens a profile, closes it, and reopens it,
   asserting the author id is unchanged and `recovery` is nil. *(This already
   exists and PASSES — `testReopeningAProfileNeverQuarantinesItOrChangesIdentity`
   — so it does not reproduce the field failure. The next red test must use a
   REAL keychain-backed store, not the stable test double, because the test
   double is precisely what hides this bug.)*
2. **Red:** a fixture test built from the two real quarantined profiles, pinned
   into the repo as test data, asserting each opens under current code.
3. Add the key-fingerprint diagnostic, run one launch, identify the cause.
4. **Green:** fix it.
5. **Recovery for people already broken:** the app must try previously
   quarantined identities when the current one fails, and adopt the one that
   opens, rather than minting a stranger. Someone whose community is sitting in
   `quarantine/` should get it back.

**Done when:** a profile written by one launch always opens in the next, on a
real device, with the real keychain — and existing broken profiles recover.

### WU-2 — Walk one full row on real devices

Install → join a community → post → reply → react → quit → relaunch → all of it
is still there. Written down with evidence, on both iOS and Android. Right now
nobody has seen a reply succeed in the running app; until that is recorded,
"post and discuss" is unproven.

### WU-3 — Fill the grid, on the real substrate

The suites are green while the app is broken because they replace exactly what
breaks: `open_local_profile()` is in-memory, the Swift tests pass no
`databasePath`, and `ReactionUITestFixture` is "a bounded IN-MEMORY wire" with a
"stable IN-MEMORY wrapping key". The durable journey harness exists now; the work
is to route the UI fixture through the real substrate and add the missing
journeys — relay-joined community, two-peer sync, upgrade.

### WU-4 — Honest state in the interface

- "Not synced yet" (sidebar) and "Synced just now" (header) contradict each
  other on the same screen.
- The red "Reactions aren't available for this post" persists after a reaction
  visibly succeeds.
- "We couldn't restore your previous data" needs to say what happened and what
  the person can do about it.

### WU-5 — A way in that is not sideloading

iOS is TestFlight-only; Android is a 46MB APK behind "install from unknown
sources" that 32-bit phones refuse outright — the cheap-handset population Riot
exists for. Store listings are blocked (`release:status` BLOCKED, twelve
`policy.*` gates plus account and legal items; Play additionally blocked by
issue #151). Either clear those or publish an honest signed download page, and
shrink the APK.

### WU-6 — Two people who are not in the same room (issue #107)

The guide says it plainly: internet sync between Riot devices is not built.
Mobile carries no iroh; the anchor relay is a pull, not a path between phones.
Until this exists, Riot is a same-room tool and should be described as one.

## Order, and why

1. **WU-1.** Nothing else matters while the app eats people's data.
2. **WU-2.** Prove the basic loop works before building on it.
3. **WU-3.** So fixes stay fixed.
4. **WU-4.** Cheap, and directly what a person sees.
5. **WU-5 / WU-6.** Larger; WU-6 is the real product gap.

## Standing rule

**Ship no further releases until WU-1 is done.** Every 0.1.x published today
costs existing users their community — a worse outcome than not shipping.
