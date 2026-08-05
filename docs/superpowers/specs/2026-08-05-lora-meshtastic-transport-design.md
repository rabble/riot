# Riot over LoRa / Meshtastic — transport design

Status: **design, unbuilt.** Written 2026-08-05.

Motivating case: [Burning Mesh](https://www.burningmesh.org/), a solar-powered
Meshtastic deployment across Black Rock City. The same shape appears in a
disaster: no cell, no power, and whoever can raise a repeater on a mast gives
the whole area coverage. Riot is already offline-first and account-free; the
missing piece is a radio it can speak over.

This document says what is possible, what is not, and why — with the numbers
measured rather than assumed.

---

## 1. The two ceilings

**Measured in this repo (2026-08-05):**

| Riot artifact | Bytes |
|---|---|
| Signed alert bundle (real headline + description + source claim) | **508** |
| Namespace id | 32 |
| Share/join reference (namespace + descriptor + digest) | ~96 |
| Sync inventory at `MAX_SYNC_IDS` (64 × 32 B) | 2 048 |
| `MAX_SYNC_FRAME_BYTES` | **8 MiB + 128** |

**From Meshtastic's documentation:**

| Property | Value |
|---|---|
| Max payload per packet | **237 bytes** |
| LongFast data rate | **1.07 kbps** theoretical |
| ShortTurbo (fastest) | 21.88 kbps |
| LongModerate | 0.34 kbps |
| Hop limit | 3 bits, 0–7 |
| Routing | managed flooding; next-hop for DMs since 2.6 |
| Private PortNum range | **256–511**, no registration needed |

Two conclusions fall straight out.

**The sync wire is built for fat links.** `MAX_SYNC_FRAME_BYTES` is 8 MiB — four
orders of magnitude past a LoRa packet. Bulk newswire sync over LoRa is not a
tuning problem; it is the wrong protocol for the link.

**Packet size is NOT the blocker.** Riot's BLE transport already chunks to
`max(20, maximumWriteValueLength)` — twenty bytes. The codec tolerates tiny
MTUs today. **Airtime is the blocker**, and it is shared by everyone on the
mesh.

---

## 2. Riot already has the tinySSB primitive

tinySSB makes SSB work on LoRa by amortising signatures: one signed log entry
commits to a side-chain of unsigned content packets, each verified by hashing
back to the signed head. One Ed25519 signature covers many packets.

**Riot gets the same property natively, and does not need to invent it.**

- A Willow entry is a *commitment*: it carries `payload_digest` and
  `payload_length`, with the payload bytes as a separate object. (This
  separation is exactly what produced the digest-only payload bug fixed in
  #169 — the same property, previously unexploited.)
- `payload_digest` is **WILLIAM3**, computed by `bab_rs`, which Riot already
  pins with the `william3` feature.
- Bab supports **verified streaming**: a binary Merkle tree where each chunk
  arrives preceded by sibling labels on its path to the root, so a verifier
  holding only the root digest authenticates data incrementally.

So one signature over the entry covers an arbitrarily long payload delivered as
many tiny packets. No side-chains, no new crypto.

### The catch: Bab's chunk size is 1024 bytes

Verification is **sequential**, with a delay of `chunk_size − 1` bytes — a
verifier buffers ~1 KiB before it can trust anything. On a 1.07 kbps link that
is ~8 seconds, and a 1 KiB chunk is ~5 Meshtastic packets, any one of which
lost invalidates the set until re-requested.

Instantiating Bab with a smaller chunk via its `generic` module would change the
digest and break WILLIAM3 compatibility. **Not an option** — the digest is
protocol.

**Design rule that follows:**

> Every payload Riot sends over LoRa MUST be ≤ 1024 bytes, so it is exactly one
> Bab chunk and therefore one verification unit.

The 508-byte alert already satisfies this. So does a batch of ~40 positions, and
a short dispatch. Anything larger belongs on BLE, local IP, or iroh.

---

## 3. Why this must NOT implement `Dialer`

`riot-transport`'s abstraction is stream-oriented:

```rust
pub trait Dialer: Send {
    fn connect(&mut self) -> impl Future<Output = Result<(BoxWrite, BoxRead), TransportError>>;
}
```

`run_dial` then drives `pump` over an ordered, reliable byte stream.

Meshtastic is the opposite: **datagram, unordered, lossy, broadcast.** Managed
flooding gives no ordering guarantee; ACKs are implicit-by-overhearing for
broadcast, and retries cap at three.

Implementing `Dialer` over that means building a reliable ordered stream on top
of a 1 kbps shared flood mesh — reinventing TCP on the worst possible substrate,
and burning airtime on retransmissions that stall every other user.

**Instead: a datagram-native profile.** LoRa carries self-contained, verifiable
units that need no session and no ordering. Each transmission is independently
useful; loss costs exactly that unit.

---

## 4. What Riot sends over LoRa

Four message types on a single private PortNum (256–511, no registration).
Every one fits the ≤1 KiB single-chunk rule.

### 4.1 Join beacon — 1 packet

A share reference: namespace + descriptor + content digest, ~96 bytes plus
framing. **This is the highest-value message in the design.**

Riot currently has *no way to reach a community you have no prior contact
with* — every working path needs an out-of-band artifact (a link, a QR, a
person beside you, or the one baked-in relay ticket). That is the seeker→in
failure documented in #170.

A periodic low-rate beacon carrying a join reference **is** that missing
artifact, and it is one packet. Someone arriving on the playa or in a disaster
zone can discover and join a community with no internet and nobody to ask.

### 4.2 Alert — ~3 packets

508 bytes measured / ~225 usable per packet after framing. Fragmented, reassembled,
verified as one Bab chunk against the entry's digest.

This is Riot's reason to exist under these conditions: a signed, attributable
urgent message that survives with no infrastructure.

### 4.3 Batched track — 1 chunk per batch

A single signed entry whose payload is **N position fixes**, not one entry per
fix.

A position is ~20 bytes of real data. One Willow entry costs ~450 bytes of
signature, entry bytes, and capability — **95% overhead** if sent per fix, and
at 1 kbps shared it would swamp the mesh. Batching ~40 fixes into one ≤1 KiB
payload pays the signature once.

This is the one place Riot differs structurally from SSB, whose per-feed
append-only log batches naturally. Willow entries are individually signed and
addressed, so batching is a payload-shape decision, not a protocol change.

### 4.4 Inventory digest — 1 packet

Not the full 2 KiB inventory. A compact summary (count + rolling digest, or a
small Bloom filter) that lets a listener decide whether it is missing anything
and should seek a fatter link. LoRa says *what exists*; BLE/iroh moves *the
bytes*.

---

## 5. Airtime budget — the real constraint

At LongFast (1.07 kbps theoretical, less in practice after headers and hops):

- One alert ≈ 3 packets ≈ 711 bytes ≈ **~5 seconds of airtime**, before
  rebroadcast.
- Managed flooding means every node within the hop limit rebroadcasts, so
  multiply by the rebroadcast factor.

**The mesh supports a handful of alerts per minute network-wide — not per
user.** Any design that lets each user emit freely will collapse it.

Non-negotiable consequences:

1. **Alerts are rate-limited and prioritised.** Urgency already exists on the
   alert record; use it to gate transmission.
2. **Beacons are slow** — minutes apart, jittered.
3. **Tracks are batched**, never per-fix.
4. **Riot never initiates bulk sync over LoRa.** Ever.
5. **Prefer a fatter link when present.** LoRa is the fallback, not the default.

Repeaters help coverage, not capacity. A `ROUTER`/`REPEATER` node on a mast
extends reach — which is exactly the emergency story — but every rebroadcast
consumes the same shared airtime. More repeaters means *further*, not *more*.

---

## 6. Integration path

No new radio hardware. The phone pairs with a Meshtastic node over its BLE (or
serial) API, and **Riot already has a complete BLE stack** —
`CoreBluetoothNearby`, `FrameCodec`, chunking, permission handling.

```
Riot (iOS/Android)
   └── BLE ──> Meshtastic node ──> LoRa mesh ──> Meshtastic node ──> BLE ──> Riot
```

Meshtastic's channel encryption is orthogonal to Riot's trust model: Riot's
signatures do not depend on transport security, so a Riot payload is equally
verifiable whether the channel is encrypted or open. The mesh is a carrier, not
an authority.

---

## 7. What is explicitly out of scope

- Bulk newswire sync
- App bundles (1 MiB cap — three orders of magnitude too large)
- Media of any kind
- Payloads > 1024 bytes (breaks the single-chunk rule)
- Per-fix location updates
- Implementing `Dialer` / reliable ordered streams over the mesh

---

## 8. Open questions

1. **Reassembly under flooding.** Fragments arrive out of order and lossy;
   Bab verification is sequential. Needs a bounded reorder buffer with a
   discard policy — and Bab's own rationale is precisely that discarding
   *bounded* unverified data is safe.
2. **Beacon privacy.** A join beacon is a public "this community exists here"
   broadcast, unencrypted and direction-findable. For Burning Man that is
   fine. **For a protest it may be exactly wrong.** This needs an explicit
   opt-in per community and should never be default-on. Riot's threat model
   deserves its own pass here before any code is written.
3. **Duty cycle by region.** US 915 MHz ISM has dwell-time rules; EU 868 has a
   1% duty cycle. Rate limits must be region-aware.
4. **PortNum choice** in 256–511, and whether to seek a registered number
   (64–127) later.
5. **Does the Meshtastic BLE API expose arbitrary PortNum send/receive** on
   both iOS and Android? Assumed, unverified.

---

## 9. Confidence

**Verified in this repo:** the 508-byte alert measurement, `payload_digest` =
WILLIAM3 via `bab_rs`, `bab_rs` verified-streaming support, `MAX_SYNC_IDS` = 64,
`MAX_SYNC_FRAME_BYTES` = 8 MiB + 128, BLE chunking to 20 bytes, the `Dialer`
trait shape.

**From Meshtastic documentation (2026-08-05):** 237-byte payload, modem preset
data rates, hop limit 0–7, managed flooding, PortNum ranges.

**From bab-hash.org:** 1024-byte WILLIAM3 chunks, sequential verification,
`chunk_size − 1` verification delay, ~1.6% metadata overhead.

**Unverified:** Burning Mesh's own specifics — both burningmesh.org and
docs.burningmesh.org rendered essentially empty when fetched, so the deployment's
frequency band, preset, and any MQTT bridging remain unknown. Confirm before
designing to them.
