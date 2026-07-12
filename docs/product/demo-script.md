# Riot demo script

About three and a half minutes of live beats, plus the setup below. Read this
out loud before you ever do it on a real stage — every line in quotes is a
line you can actually say.

This script is the spec. If a screen, an animation, or a feature isn't
mentioned here, it doesn't need to be ready for the demo, and no amount of
polish on it matters until this script asks for it.

## Setup

You need two iPhones.

- **Both phones in airplane mode, with Bluetooth turned back on.** Airplane
  mode first, then flip Bluetooth on — that's what proves there's no internet
  in the room while still letting the phones find each other directly.
- **Phone A** has demo mode loaded: open **Spaces** and tap **“Load the demo
  space (Riverside Tenants Union).”** This seeds the space with six
  alerts, a partly-done checklist, and one available app in the directory.
  Load it once, well before you're on stage — never in front of the audience.
  **Phone A must not already be in another space.** Demo mode refuses to load
  onto a phone that has one — deliberately, because a phone can only be in one
  space at a time, and quietly swapping it out would take away the very sync
  this demo ends on. Use a phone with no space, or reset the profile first.
- **Phone B** is a fresh install, signed in with its own profile, nothing
  loaded. It starts the demo with nothing on its board.

**This is the only configuration this demo is rehearsed in.** Two phones,
airplane mode, Bluetooth on, demo mode on A only, B fresh. If you change
anything about this setup, rehearse the new version before you show it to
anyone.

## Beat 1 — Open (30s)

Phone A is already open to the Riverside Tenants Union board. You open the
demo standing on this screen — don't tap anything for the first few seconds,
let the audience read it.

Say: *"This is a tenants union's board. Six people posted to it. Nobody's
phone talked to a server to get any of this — it's all sitting right here,
signed by the people who wrote it."*

Read the six alerts, in board order, naming who posted each:

1. **"Court support needed Thursday, 9am — bring folding chairs"** — posted by
   **Ana**
2. **"Diaper and formula drop at the Elm St lot, Saturday 11–2"** — posted by
   **Marcus**
3. **"Reminder: you don't have to open the door for anyone without a
   warrant"** — posted by **Priya**
4. **"Ride offered to Thursday's hearing — 3 seats, leaving the laundromat at
   8:30"** — posted by **Dee**
5. **"Tuesday's general meeting moved to 7pm, same room"** — posted by **Ana**
6. **"Found a ring of keys on the courtyard bench — come by unit 4B"** —
   posted by **Marcus**

Say: *"Ana, Marcus, Priya, Dee — real names people chose, each one tagged with
a few characters tied to their actual key, so nobody can fake being them."*

## Beat 2 — Discover (45s)

Tap into the **App Directory** tab.

Say: *"Riot ships with almost nothing built in — a checklist, that's it.
Everything else is something someone in the network built and other people
vouched for."*

Point at the top of the list: **Checklist**, labeled **"Built into Riot"**,
already switched on. That's the tool the courthouse-support checklist you'll
check later lives in.

Scroll to **Available** and stop on **Shift Signup**.

Say: *"Someone in this network built a shift sign-up tool. It's not on
anyone's phone yet — it's just sitting here, available, because two groups
looked at it and said it's fine to use."*

Read the endorsement line under it: **"Endorsed by Eastside Tenant Council and
Courtyard Mutual Aid."**

Say: *"Those are two real groups in this network, not Riot the company,
telling you they checked this and trust it."*

## Beat 3 — Trust (45s)

Tap **Get Shift Signup**, then **Review**.

> **Two taps, not one — don't be surprised on stage.** The app's bytes have
> arrived, but it isn't installed on this phone yet, so the row offers "Get"
> first and the review page second. That ordering is honest — nothing is
> installed before you've read what it can do — but it is one more tap than a
> phone-store audience expects. If it feels like a stumble, name it: *"It won't
> even unpack it until I've looked at it."*

Say: *"Before anything gets installed, you get to read exactly what it can
touch — no fine print."*

Read the permissions out loud, exactly as shown:

- **"Can read the shift schedule board"**
- **"Can add new shift sign-ups"**
- **"Cannot see your messages"**
- **"Cannot access any other tool"**

Say: *"That's the whole list. It reads and writes shifts. That's it."*

Tap **"Let everyone here use this."**

The stamp slams down, the phone gives one solid thunk in your hand, and
**Shift Signup** appears in the **Tools** tab.

Say: *"Now it's in my Tools, and it's in Tools for everyone else in this space
too — the trust doesn't just apply to me."*

## Beat 4 — Sync finale (90s)

Hand phone B to someone, or hold it up next to phone A.

On **phone B**, open the **Connection** tab. A ring sweeps outward, searching.

Say: *"No wifi, no cell signal, no server anywhere. Phone B is just
listening."*

The sweep finds phone A; a labeled dot pops in on the radar screen.

Say: *"There it is — it found phone A directly, over Bluetooth, because
they're close enough to hear each other."*

Entries start landing on phone B's board — each one stamps down as it
arrives: the six alerts, the checklist, Shift Signup, all appearing live.

Say: *"Everything phone A had, phone B is getting right now, over the air
between these two devices, in the same room."*

On **phone A**, open the courthouse-support checklist and check off **"Bring
folding chairs."**

Watch phone B: a ring pulses out from that line item, and it updates to read
**"checked by Ana · a3f9."**

Say, pointing at the airplane-mode icon on each phone in turn:

*"No internet. No servers. Just these two phones."*

## Known gaps — read this before you rehearse (2026-07-12)

These are things that do **not** work yet, recorded honestly so you don't
discover them on stage. Each is tracked; none is a mystery.

1. **The radar can't put a name on a peer yet.** Riot doesn't know *who* a
   nearby phone belongs to until sync actually opens — before that there is
   only a device nickname, no key. Rather than print a fake key tag, the radar
   currently shows the device, not the person. **This is a product call for
   Rabble**, not a bug: show the nickname untagged, show nothing until identity
   is known, or exchange identity earlier in pairing.
2. **The two-phone finale has never run on two physical iPhones.** Headless
   tests prove discovery, adoption, sync, and live redraw over Bonjour/TCP on
   one Mac; they do not prove BLE discovery or transfer. Rehearse it on real
   hardware twice before anyone watches.

## What can go wrong

**Beat 1 — Open.** The board is momentarily blank or a placeholder while the
seeded space finishes loading on first open after a fresh install of demo
mode. Say: *"Give it half a second — it's local, not a network fetch."* and
keep narrating what you're about to show; it resolves in under a second. Do
not tap around looking for it.

**Beat 2 — Discover.** Shift Signup shows up under the wrong section, or the
endorsement line is momentarily missing while the list re-sorts. Say: *"The
list's still settling — here it is,"* and scroll to find it; the content is
already on the phone, this is just layout catching up. Never re-open the tab
from scratch.

**Beat 3 — Trust.** The stamp animation or haptic doesn't fire (common on a
phone with haptics turned off in Settings, or on a first tap that lands a
half-second early). The permission change itself always lands even if the
flourish doesn't. Say: *"There it is, in Tools,"* and point at the Tools tab
to show it landed — don't re-tap the button hunting for the animation.

**Beat 4 — Sync finale.** This is the beat most likely to wobble, because it
depends on two radios finding each other live.

- *Radar finds nothing after several seconds.* Say: *"It's looking — Bluetooth
  takes a few seconds to find another phone,"* and keep talking about what
  the audience is about to see. Do not toggle airplane mode or Bluetooth to
  "fix" it — that resets the search. Give it real time; it almost always
  resolves within ten to fifteen seconds if the phones are within a few feet
  of each other.
- *Entries arrive out of order, or all at once instead of one by one.* Say:
  *"They're all landing now — it's the same data either way,"* and keep
  going; the order they animate in is cosmetic, not a sign anything's wrong.
- *The checklist ripple on phone B is slow to appear after you check the item
  on phone A.* Say: *"That'll land in a second,"* and finish your closing
  line — by the time you're done saying "just these two phones," it's
  usually there. If it still hasn't appeared, move on; the sync already
  happened for everything else on screen, and that's the point being made.

**The one rule that covers all of the above: never restart the app on
stage.** A restart looks like a crash to the audience even when it isn't one,
and it discards exactly the state — phone A's seeded space, whatever's
already synced to phone B — that the rest of the demo depends on. Every
failure mode above resolves by waiting and narrating, not by resetting
anything.

## What this demo is NOT claiming

This is a local-first prototype, not a finished product making guarantees
about scale, security audits, or long-term reliability. The sync you're
watching is nearby-only — phone A and phone B are talking directly to each
other over Bluetooth in the same room; there is no internet relay standing
behind this, no server that would let these two people sync if they were in
different cities, and that limitation is real, not a demo shortcut. The
display names are self-claimed: anyone can type "Ana" into their own profile,
and Riot does not verify that the person typing it is a particular real-world
person. That is exactly why the app never shows a bare name — it's always
**name · four characters tied to that person's actual key**, so that if two
different people both call themselves "Ana," you can tell them apart, and so
that nobody can quietly impersonate someone already in the space. The
suffix isn't decoration; it's the honest admission that the name alone
proves nothing.
