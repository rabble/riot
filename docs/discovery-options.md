# Local peer discovery: options and recommendation

How should two Riot devices find each other nearby — and what does each choice
leak to someone else on the same network? This synthesizes how established
systems (Briar, Berty, Secure Scuttlebutt, BitTorrent, Apple) solve it, then
recommends a path for Riot.

Riot's audience is activists on networks they do not control — conference Wi-Fi,
a venue, a protest where an adversary may be on the same LAN. So the question is
not just "does discovery work" but "what can a passive observer on the link learn
about who is present."

## Where Riot is today

Two transports run side by side:

- **Bluetooth LE** (`CoreBluetoothNearbyService`) — the original path. Cannot find
  a peer on the *same machine* (one radio never hears its own advertisement), so
  it cannot serve multi-instance testing or desktop-to-desktop sync.
- **Bonjour / mDNS** (`LocalNetworkNearbyService`) — added so same-host and
  same-LAN peers can find each other. Advertises `_riot-sync._tcp` with a TXT
  record. The channel is held link-local (`LocalEndpoint.isLocalAddress` refuses
  any routable address) and pairing requires explicit two-sided confirmation.

What the Bonjour path leaks today, to any passive observer on the subnet:

- **Presence**: "a Riot device is here," and how many. mDNS is cleartext multicast
  to the whole link (RFC 6762/6763); the service *type* alone reveals the app.
- **Not** a stable identity: the service instance name is a per-session random
  UUID and the friendly name is nonce-derived, so a device is *not* linkable
  across networks or sessions. (This is already better than SSB — see below —
  which broadcasts a permanent public key.)
- The device/owner name is **not** leaked, since we set the service name
  explicitly instead of letting Network.framework default it to the computer name.

So the residual leak is **presence on the local subnet**, not identity. Closing
that is what "group-gated discovery" below is about.

## How other systems do it

Two philosophies, and they map cleanly onto our options.

### Broadcast a stable identifier (what NOT to do)

- **Secure Scuttlebutt**: UDP-broadcasts `net:<ip>:<port>~shs:<ed25519 pubkey>`
  to `255.255.255.255` **once per second**. The key is the user's permanent
  global identity, and the SSB social graph is public — so one captured packet
  deanonymizes the device and ties it to a known follow graph. No fix beyond
  "turn LAN discovery off." (SSB Protocol Guide, *Discovery*.)
- **BitTorrent Local Service Discovery** (BEP 14): multicasts the torrent
  infohash in cleartext to `239.192.152.143:6771` every 5 min — anyone on the LAN
  learns exactly what you have. Off by default in Transmission for this reason.
- **Apple Bonjour / AWDL**: the discovery layer is cleartext even when the session
  is encrypted (`MCEncryptionPreference` covers only the data channel). Default
  service names leak the owner's real name ("Jane's iPhone"); the "Billion Open
  Interfaces" (USENIX Sec 2019) and PrivateDrop (USENIX Sec 2021) papers showed
  AWDL/AirDrop broadcasts carry long-term identifiers and brute-forceable contact
  hashes. Lesson: never let the service name default; never assume the
  advertisement layer is private.

### Don't broadcast, or broadcast only a rotating secret-derived token (the good designs)

- **Briar** — the strongest local-privacy posture, and the model for an activist
  tool. Briar's LAN plugin does **not** multicast or broadcast **anything**
  (verified: no `MulticastSocket`/`DatagramSocket` in the codebase). Instead each
  device's IP:port and Bluetooth UUID are **transport properties exchanged over
  the already-encrypted, authenticated contact channel**, and connection is a
  direct same-subnet TCP dial. A passive LAN observer sees only a direct
  connection carrying random-looking (BTP-tagged) bytes — no service name, no
  beacon, no identity. Trade-off: this requires the peers to **already be
  contacts** (addresses exchanged in a prior encrypted session or QR pairing);
  there is no open "who's nearby" discovery.
- **Briar QR pairing (BQP)**: when two people *first* meet, the Bluetooth UUID is
  derived from the scanned key commitment — `UUID(SHA-256('…/COMMIT', pubkey)[:16])`
  — so only the two devices that scanned the QR know which UUID to look for. A
  bystander cannot. Ephemeral to the pairing session.
- **Berty / weshnet** — the model for "only my group can discover me" **over a
  broadcast medium**. It advertises a **rotating rendezvous point**:
  `point = HMAC-SHA256(key = topic‖seed, msg = bigEndian(floor(now/interval)*interval))`
  (TOTP-style, RFC 6238; default interval 24h with a grace window). `topic` is the
  group id; `seed` is a shared secret. A peer holding the seed recomputes the
  current point and recognizes you; **a third party without the seed cannot link
  successive points or identify you.** Seeds are user-renewable (revocation).

The takeaway: if you must emit *something* to the whole link, emit a **rotating
token derived from a shared group secret**, never a stable name — and prefer to
emit nothing at all when the peers are already known to each other.

## Options for Riot

### Option A — Open mDNS discovery (where we are)

Advertise `_riot-sync._tcp` openly; anyone can browse. Ephemeral names (done), no
stable identity leaked. **Leaks presence + count to the subnet.**

- Best for: same-machine testing, trusted rooms, the demo.
- Weakness: on hostile Wi-Fi, an observer enumerates that Riot users are present
  and how many. Not identity, but a signal.

### Option B — Group-gated ephemeral discovery (the Berty pattern) — recommended default

Only devices that share a **group secret** can recognize each other. Instead of a
fixed service instance name, advertise a rotating token:

```
token(t) = HMAC-SHA256(groupSecret, floor(t / interval))     // truncated to a label
```

- Browsers still see `_riot-sync._tcp` exists, but the **instance name is an
  opaque rotating token**. A peer who holds `groupSecret` computes the same
  token for the current (and previous, for the grace window) interval and
  recognizes only its own group. Everyone else sees uncorrelatable noise.
- The TXT record (identity, friendly name) moves **inside** the pairing handshake,
  which is already gated by two-sided confirmation, so it is no longer in the
  cleartext advertisement at all.
- `groupSecret` comes from the space/namespace the two devices already share, or
  is bootstrapped out-of-band (QR / short code) the first time — the Briar-BQP
  idea. In Willow terms this maps naturally: the space namespace is the `topic`,
  and a capability/secret for that space is the `seed`.
- Cost: a peer can only be *discovered* by its own group. Cross-group "who's
  nearby" goes away — which for an activist tool is a feature, not a loss.

This still leaks that *some* `_riot-sync._tcp` device is present (the service type
is visible), but not who, not which group, and nothing linkable across time.

### Option C — No broadcast; direct dial to known peers (the Briar pattern)

For peers already in a shared space, skip discovery entirely: carry each other's
last-known link-local IP:port as space metadata (exchanged during a prior synced
session), and dial directly. Emits nothing to the link. Falls back to Option B
only when meeting someone new. Strongest, most work; a good eventual target.

## Recommendation

1. **Keep Option A available but behind a mode** — it is the right thing for
   same-machine testing and the demo, where openness is the point. Gate it so it
   is not the production default on an untrusted network.
2. **Make Option B the production default** — group-gated rotating token derived
   from the shared space secret, TXT identity moved into the confirmed handshake.
   This gives "only my group can discover me" with no server, and a defensible
   answer to "can a hostile conference network enumerate Riot users" (they see an
   unidentified service type, nothing linkable).
3. **Grow toward Option C** for known peers — direct dial from space metadata,
   broadcasting nothing, with Option B as the first-contact path.

None of this changes the two invariants already in place: the channel stays
link-local (routable addresses refused), and discovery is never consent —
pairing always requires an explicit yes on both sides.

## Sources

- Briar BTP/BQP specs and source (code.briarproject.org/briar/briar-spec, /briar)
- Berty rendezvous: berty.tech/docs/protocol, github.com/berty/weshnet
  (`pkg/rendezvous`)
- SSB Protocol Guide (ssbc.github.io/scuttlebutt-protocol-guide), *Discovery*
- BitTorrent LSD BEP 14 (bittorrent.org/beps/bep_0014.html)
- Apple: TN3179 Local Network Privacy; "A Billion Open Interfaces for Eve and
  Mallory" (USENIX Security 2019); PrivateDrop (USENIX Security 2021)
- RFC 6762 (mDNS), RFC 6763 (DNS-SD), RFC 6238 (TOTP)
