# Carrying every community without disclosing which ones you're in

Date: 2026-08-04
Status: Design note, for decision. Not a plan; no work is scheduled from this yet.

## The problem, precisely

Saturation wants every device to carry every community it holds and hand them all
onward when it meets someone. To do that, two devices must work out which
communities they share. Working that out requires someone to say what they hold.

That is a different disclosure from the one already decided. The 2026-08-04
product decision made **content** deliberately leaky — posts should spread as far
as possible. This is **membership**: a stranger you sync with at a protest
learning you are in *Riverside Tenants Union*, *Legal Support*, and two others.
Content is information a person chose to publish. Membership is information about
them, and it is the kind that gets people arrested.

## What is already true (and good)

- **Discovery leaks nothing.** The Bonjour TXT record carries `instance`, `name`
  and `tie` only — no namespace ids (`LocalNetworkNearby.swift:19-57`). Someone
  sniffing a protest learns that Riot devices are present, not what they hold.
- **Pairing is consent-first.** `SpacePairing` sends the confirm token before any
  community is named — *"the community stays opaque until the peer consents
  too"*.
- **Identity is already partitioned per community.** Each community gets its own
  random, unlinkable author. There is no cross-community identifier to correlate.

So the question is not "how do we add privacy" — it is "how do we add multi-
community carry without spending the privacy we already have".

## Why per-group keys don't solve it

Riot already has per-group keys, and they are not the leak. A community is named
by its `namespace_id`: a stable 32-byte value, identical for everyone in it,
required to address a sync. Rotating a *personal* key changes nothing, because
the thing being matched on is shared by construction.

Any scheme in which two strangers can determine "do we both hold X" necessarily
lets someone who already knows X test for it. That is not a weakness to engineer
away; it is what matching means. The design question is who can test, how many
guesses they get, and whether two encounters can be linked.

## Options

### A. Disclose the list

Send your namespace ids. Simplest, maximum saturation.

A peer — or anyone who compromises one — learns your full membership set. For
public wires that is near-harmless; for a tenants union with named members it is
a roster. **Not recommended as the only mode.**

### B. Salted rotating hashes

Exchange `H(namespace_id ‖ session_nonce)` per held community, fresh nonce per
encounter. Each side tests its own known namespaces against the peer's set; you
learn the intersection.

- Cheap — hashing only, no new dependency.
- An eavesdropper learns nothing, and cannot link two encounters.
- A peer can still test any namespace they already know. A hostile peer carrying
  a scraped directory of public community ids can ask "is she in any of these
  500?" and get an answer.

### C. DH-based private set intersection

Each side blinds its ids with a private scalar, exchanges, blinds again, compares.

- A peer learns only the intersection for ids they committed to in that round; no
  offline dictionary testing afterwards.
- Standard and well understood; one curve operation per id per side, trivial at
  the scale of "communities a person is in".
- Still cannot prevent someone who knows an id from confirming it — nothing can.

### D. Per-community carry policy — the one I would ship first

Let a person mark each community **carry automatically** or **only when I say
so**. The automatic set participates in whatever intersection scheme; the manual
set is never named without a deliberate act.

Why this first:
- It is the only option that is *legible to the person whose safety is at stake*.
  Nobody at a protest can reason about set-intersection guarantees; everybody can
  reason about "this one spreads, this one doesn't".
- It matches how the communities actually differ. A public protest wire wants
  maximum saturation. A legal-support group does not, and no crypto changes that.
- It shrinks whatever the intersection scheme has to protect, so B becomes
  adequate for most real cases and C stops being urgent.
- Public communities publish their namespace ids in directories anyway. For those,
  hiding the id was never the protection — not being *linked to them, here, now*
  is.

## Recommendation

**D + B.** Per-community carry policy, with salted rotating hashes for the
automatic set. Ship D first — it is a user-facing control, it is cheap, and it
determines how much the cryptography has to carry.

Hold C in reserve for when a real threat model demands it, or when someone wants
automatic carry for a sensitive community. Do not build C first: it is the
satisfying engineering answer to a question that is mostly a product question.

## Open questions

1. **Default for a newly joined community — automatic or manual?** Automatic
   maximises saturation and is the honest default for a public wire. Manual is
   the safe default and will mean most communities never spread. This is the real
   decision; the rest follows from it.
2. **Does the person get told what an exchange disclosed?** "You and this person
   both hold River City Wire" is legible; silently learning it is not.
3. **Does BLE change the answer?** Bluetooth advertisements are broadcast
   continuously to anyone in range, unlike a consented TCP session. Anything
   carried in an advertisement is a different risk class from anything carried
   after pairing, and this note has only considered the latter.
4. **What about a peer who is coerced afterwards?** Nothing here protects against
   a phone seized after a sync. Membership learned in an exchange is retained by
   the peer; whether Riot should retain it at all is worth asking.

## What is NOT at stake

Content spread. All four options carry the same posts to the same people once a
community is shared. This note is only about how two devices decide *which*
communities they share — nothing here slows saturation of a community both
parties have already agreed to carry.
