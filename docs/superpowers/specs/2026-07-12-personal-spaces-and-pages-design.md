# Personal spaces and pages design

## Status

Approved in product brainstorming on 2026-07-12.

Riot today has one kind of space: a communal namespace shared by a group
(`Berlin Mutual Aid`). This design adds a second kind: a **personal space** —
an owned namespace where one person holds root, publishes a page they authored
themselves, and decides who may read it.

The reference points are GeoCities and MySpace: a place that is *yours*, that
looks like *you made it*, that has things happening on it, and that you can
make public or show only to people you have connected with.

This design depends on, and does not restate:

- `2026-07-12-multi-space-sqlite-store-design.md` — **hard blocker.** Personal
  spaces are inherently multi-space (your page plus every page you read).
- `2026-07-11-full-meadowcap-management-design.md` — the authority layer. This
  design uses its Managed Space profile, its replication read gate, and its
  `InviteRequestV1` shape. It does not supersede it.
- `2026-07-11-signed-js-apps-design.md` and `2026-07-11-app-directory-design.md`
  — the manifest/bundle format, runtime, and endorsement paths reused here.

Normative external reference:
[Meadowcap](https://willowprotocol.org/specs/meadowcap/index.html) (final as of
2025-11-21).

## Purpose

People need a place of their own in Riot that is not a group. Making it should
be the first thing the app invites you to do, it should take under a minute,
and the result should be unmistakably authored rather than templated.

The design answers four questions:

1. What is a personal space, in Willow terms?
2. What is a page, and how do you make one on a phone?
3. What does "public" and "private" actually mean, cryptographically, and what
   do they *not* mean?
4. How can a stranger's code render on your device without endangering you?

## Use cases

1. **New arrival:** WHO is a person opening Riot for the first time; WANTS to
   be told to make a space and a page, and to have one within a minute; SO THAT
   they have an identity and a place before they are asked to join anything;
   WHEN the app first launches.
2. **Page author:** WHO has a personal space; WANTS to start from a template
   and then edit the actual HTML and CSS; SO THAT the page is theirs and not a
   filled-in form; WHEN they set the page up or change it later.
3. **Page author, assisted:** WHO cannot write HTML; WANTS to describe the page
   and have a model generate the bundle; SO THAT authorship is not gated on
   knowing to code; WHEN creating or revising a page.
4. **Public publisher:** WHO wants to be found; WANTS anyone nearby to read
   their page with no negotiation; SO THAT the page works like a public
   homepage; WHEN the space is set public.
5. **Connections-only publisher:** WHO wants a page only for people they know;
   WANTS strangers to see nothing but a calling card and a request button; SO
   THAT the content reaches only granted readers; WHEN the space is set to
   connections-only. *(Slice 2.)*
6. **Requester:** WHO finds someone's calling card; WANTS to request a
   connection and receive read access when granted; SO THAT access is a decision
   the owner makes, not a secret that leaks; WHEN meeting someone nearby.
   *(Slice 2.)*
7. **Visitor:** WHO opens a page authored by someone they barely know; WANTS the
   page to be unable to harm them, exfiltrate anything, or record the visit; SO
   THAT looking at a page is never an attack; WHEN viewing any page.
8. **App maker:** WHO builds something useful on their page; WANTS to publish it
   as an app others can install; SO THAT tools spread person to person; WHEN
   sharing an app. *(Slice 3.)*
9. **App chooser:** WHO is deciding whether to install a stranger's app; WANTS to
   see what independent auditors found in it; SO THAT the choice is informed; WHEN
   browsing the directory. *(Slice 3.)*

## Decisions

These were settled in brainstorming and are load-bearing. Reopening any of them
changes the architecture.

### D1 — Private means a read gate, not encryption

"Connections only" is enforced at the **replication layer**: a peer serves
protected entries only after the requester proves possession of the receiver
secret named in a valid, covering `ReadCapability`. Possession of copied
capability bytes is insufficient.

It is **not** encryption. Data is plaintext at rest on device. A connection you
granted can copy and reshare everything they can read. MLS confidentiality
(per the Meadowcap design's Private Managed Space) is out of scope.

**The product never uses the words "private" or "encrypted" for this.** It says
**"connections only."** The distinction is not pedantry: users make safety
decisions on this label, and the label must not promise more than the mechanism
delivers.

### D2 — A page is an app

There is no separate page format. A page is a signed manifest + bundle in the
existing `apps/` format, marked `kind: page` in its manifest, replicated as
ordinary Willow entries, served over the existing `riot-app://<app_id_hex>/`
scheme.

> **A page is an app you published to your own space. An app is a page someone
> else can install.**

This is why LLM authoring, template authoring, and hand authoring all converge:
they are three ways to produce one artifact.

### D3 — Reads are free, writes are consent-mediated

Page JavaScript may read through the bridge without friction. It may **never**
write without an explicit native confirmation naming what is being signed.

`AppDataBridge::put` signs with the *calling person's own key*
(`apps/bridge.rs:1`). Untrusted page JS calling `put()` unattended would let a
page you merely *looked at* write a **cryptographically attributable,
non-repudiable, replicating** record that you were there. That is a tracking
beacon strictly worse than a web one. On a tool used for protest and mutual aid,
"who viewed this page" is the metadata that gets people hurt.

The network is not the exfiltration channel here. **The visitor's own signing key
is.** A guestbook you consciously sign is still a guestbook; the sheet costs
nothing anyone wanted.

### D4 — Containment is by construction, not by trust

Every page runs identically sandboxed regardless of who authored it, whether a
model generated it, and how many auditors endorsed it:

- **No network.** CSP is `default-src 'none'; script-src 'self'; style-src
  'self'; img-src 'self' data:` (`AppSchemeHandler.swift:9`). No fetch, no XHR,
  no WebSocket, no external subresource.
- **Own origin per page.** iOS serves `riot-app://<app_id_hex>/<path>`, so the
  app ID *is* the origin host and no page can reach another page's storage.
- **Navigation locked.** Top-level navigation away from `riot-app://` is refused
  in the navigation delegate, because CSP does not constrain it
  (`AppRuntimeView.swift:203`).
- **No device permissions.** Page bundles get no camera, microphone, location,
  or photo access in any slice of this design.
- **No key material.** JS never sees a secret key or a raw capability. Writes are
  performed natively and signed by the host.

### D5 — Auditor endorsements are signal, never capability

An auditor — including an automated one running a model — is an ordinary
subspace with an opinion. It publishes a signed attestation bound to an exact
bundle digest, at the existing path family
`app-index/<app_id>/endorsement/<auditor_subspace>`.

Endorsements are **plural** (anyone may run an auditor; you choose whom to
follow), **replicated** (they arrive over BLE with the bundle, so they work in a
blackout), and **advisory**.

**An endorsement never unlocks a capability.** It does not grant network access,
does not skip the consent sheet, does not widen the bridge. It informs *what a
person chooses to install*; it never changes *what the code may do*.

The reasoning is the Meadowcap design's own principle 2 — *protocol validity is
not community policy*. Containment is protocol; endorsement is policy. Merging
them yields exactly what that spec warns against: "one unreviewable authorization
mechanism." Concretely: an attacker gets unlimited offline attempts against a
known reviewer, obfuscated JavaScript is the canonical defeat for LLM review, and
Riot's users are targeted by adversaries with budgets. Keeping the sandbox costs
nothing when the audit is right, and is the whole defense when it is wrong.

UI copy says **"reviewed — no findings,"** never **"safe."** A review cannot
certify safety, and users lean hardest on that word exactly when it matters most.

### D6 — The model runs host-side, never in the sandbox

LLM authoring is a native action that produces a bundle. The generated artifact
then runs under D4 like any other, with no network and no special trust. The
generator and the sandbox never touch. A prompt-injected model that emits a
beacon is contained by the same walls as a malicious human author — which is the
entire reason D3 and D4 are non-negotiable.

**Local model is the default.** Remote inference is **opt-in per use, never
sticky**, behind a sheet naming the provider in plain language: *"This sends your
prompt to <provider>. Don't include anything you wouldn't post publicly."* Page
content and Willow data are **never** auto-attached as context; only prompt text
the person typed. Riot's thesis is that it works in a blackout — authoring must
degrade to templates and hand-editing with no network at all.

## Architecture

### Personal space = owned namespace

A personal space is a Willow **owned** namespace
(`randomly_generate_owned_namespace()`). The person holds the root
`NamespaceSecret`; the namespace ID *is* the root public key.

This differs from every namespace Riot creates today. Group spaces are communal:
`NamespaceKind` has one variant, and `create_public_space` deliberately
**zeroizes the namespace secret** (`willow/identity.rs:164`) because it confers
no privilege in a communal namespace. Personal spaces invert this — the secret is
the point, and it is **persisted in platform secure storage** (iOS Keychain,
Android Keystore).

Two keypairs are therefore in play, and must not be conflated:

| Key | Role | Storage |
|---|---|---|
| Namespace root (`NamespaceSecret`) | Mints read/write caps over the space | Secure enclave / Keychain |
| Author subspace (`EvidenceAuthor`) | Signs entries as *you* | Existing profile storage |

The existing `namespace_id == organizer_subspace_id` trick (`identity.rs:239`)
is a **communal**-space device and does not apply here.

**Owned from day one, even though Slice 1 is public-only.** Communal-vs-owned is
a bit flag *in the namespace ID itself* and cannot be changed afterward. Root
replacement is a signed migration to a new namespace, not a rotation. Creating
personal spaces as communal now would mean every early user must migrate — losing
their namespace ID, and every capability anyone holds — the day connections-only
ships. This is the single most expensive mistake available in this design.

### Path profile

Read capabilities narrow by **path prefix**, so the public/protected boundary must
be a *path* boundary, fixed before any entry is written. The first path component
is the visibility segment:

```
<personal owned namespace>/
  pub/                             ← served to anyone, no capability required
    profile/<subspace>/card        ← existing ProfileCard: the calling card
    page/current                   ← manifest digest of the live public page
    app-index/<app_id>/manifest
    app-index/<app_id>/bundle
    app-index/<app_id>/endorsement/<auditor>     (Slice 3)
    apps/<app_id>/**               ← public app data (Slice 3)
  con/                             ← served only to a holder of a read cap over con/**
    page/current                   ← the connections-only page
    app-index/<app_id>/**
    apps/<app_id>/**               ← guestbook and other protected app data (Slice 3)
```

- **Public space:** the real page lives in `pub/`.
- **Connections-only space:** only the calling card is in `pub/`. A stranger sees
  a name, an avatar, and a *Request to connect* button — and nothing else.
  Everything real is in `con/`.

Granting read access is then one call:
`ReadCapability::new_owned(root_keypair, requester_receiver_key)` delegated down
to the `con/**` area, with expiry expressed as the capability's time range.

Human-readable labels never become secret path components (per the Meadowcap
design's PIO guidance): protected path segments use high-entropy identifiers.

### Page = app: authoring and publication

Producing a page, from any of the three authoring modes, is one pipeline:

```
template | hand-edit | LLM  →  bundle (HTML/CSS/JS)
                           →  manifest { kind: page, entry_point, digest }
                           →  sign with author subspace key
                           →  write app-index/<app_id>/{manifest,bundle}
                           →  set page/current = <app_id>
```

`app_id` is the manifest digest, as today — so **a page is immutable and
content-addressed**, and publishing an edit publishes a new `app_id` and
repoints `page/current`. This is what lets an auditor endorsement bind to an exact
bundle: change one byte, the endorsement is void.

Writing into your own owned namespace still requires a write capability —
`WriteCapability::new_owned(root_keypair, your_subspace)` — minted at space
creation and stored alongside the profile.

### Authoring UX

**Templates first.** Ship a small set of deliberately gaudy starting points
(starfield, tiled brick, under-construction, marquee banner). Pick one, edit
name/bio/colors/images in a native form, publish. Under a minute, no code.

**View source, always.** Any template drops into a raw HTML/CSS/JS editor. Editing
the source makes the page yours. This is the GeoCities loop — *view source, steal
it, make it worse* — and it is the reason we chose user-authored bundles over a
fixed profile template.

**Model assist.** Describe the page; a model emits a bundle; you review the
generated source before it is signed. Local model by default, remote opt-in per
D6. Generation is never required: with no network and no local model, templates
and the source editor still work.

### Runtime and containment

Per D4, unchanged from the existing app runtime, with two additions:

1. **Viewer-relative trust.** Trust is evaluated from the *viewer's* perspective.
   A space owner's trust marker over their own page does **not** bind a visitor.
   A bundle living in a namespace you do not own is never trusted by you, so its
   writes always prompt (D3). Existing directory apps, trusted by an organizer you
   follow in a space you joined, keep their current frictionless behavior.
2. **No device permissions for `kind: page` bundles**, in any slice.

### Connection requests (Slice 2)

Reuses `InviteRequestV1` from the Meadowcap design (line 696) rather than
inventing a second protocol. Delivery is **nearby-only** in this slice: an open,
world-writable `pub/requests/**` area would be a spam and enumeration vector, and
nearby-first matches how Riot already meets people.

```
requester → InviteRequestV1 { request_id, receiver signing key, space
                              fingerprint, nonce, signature }        [over BLE]
owner     → sees pending request with the requester's calling card
owner     → accepts: delegate ReadCapability over con/** to receiver key, expiry set
owner     → InviteV1 { recipient-bound, canonical child capability }  [over BLE]
requester → stores cap; proves possession of receiver secret at next protected sync
```

Revocation is a signed revocation record plus capability expiry. It stops
*future* reads once peers learn the new state. **It cannot recall data already
synchronized.** The UI must say so.

### Replication read gate (Slice 2)

The current `/1` `Hello`/`Summary` codec discloses namespace and entry identifiers
before authentication (Meadowcap design, line 238). It remains the **legacy
public-only codec** and serves `pub/**`.

`con/**` requires the protected codec from that design's §Replication read gate:
handshake → prove receiver secret in a domain-separated transcript → bind
receiver-named capabilities → reconcile only inside the granted area. The serving
peer checks `cap.includes_area()` before binding a range and `cap.includes(entry)`
before emitting any entry or payload. Failures close the session without a
distinguishable protected-data response.

## Slices

**Slice 1 — Make your page.** *(This spec's implementation target.)*
Multi-space store; owned namespace creation with root key in secure storage;
first-run flow prompting for profile + space; template gallery; source editor;
model-assisted authoring; page publication; page runtime.
**Public pages only** — no read gate is needed when everything is in `pub/`.
Demoable end to end: create a space, build a hideous page, show it to the phone
next to you.

**Slice 2 — Connections only.** Read capability minting and delegation;
`InviteRequestV1` request/grant flow over nearby; protected sync codec; revocation
and expiry; the calling-card-only view of a connections-only space.

**Slice 3 — Guestbooks, apps, auditors.** Consent-mediated bridge writes; narrow
delegated write caps over `apps/<app_id>/**`; publishing a page as an installable
app; auditor endorsements and follow lists.

## Limitations, stated honestly

These belong in user-facing copy, not only in this document.

1. **Connections-only is not encryption.** Plaintext at rest. A granted connection
   can copy and reshare anything they can read. Device seizure reveals everything.
2. **Existence leaks.** Without PIO/Confidential Sync (upstream proposals, not
   final), an unauthorized peer can still learn your namespace ID and that
   connections-only content exists. They obtain **zero entries and zero payloads**.
   The content is protected; the silhouette is not.
3. **Revocation is forward-only.** Removing a connection stops future reads. It
   cannot un-send what already synced.
4. **Audits are advisory.** "Reviewed — no findings" is a report, not a guarantee,
   and never relaxes the sandbox.
5. **Remote model authoring is network egress.** Opt-in, per-use, provider named.

## Testing strategy

- **Namespace kind is intrinsic.** Property test: a personal space's namespace ID
  always reports `is_owned()`; a group space's always reports `is_communal()`; no
  code path produces an owned namespace whose secret was zeroized.
- **Root key custody.** The root `NamespaceSecret` never appears in
  `riot-profile.json`, in any Willow entry, in any log, or across the FFI boundary.
- **Path gate.** No entry may be written to `con/**` by a caller holding only a
  `pub/**` capability; attenuation is enforced by the capability, not by the caller.
- **Containment (the security-critical suite).** A hostile fixture page attempts,
  and must fail at, each of: `fetch`/XHR/WebSocket to any host; loading an external
  script, style, font, or image; top-level navigation off `riot-app://`; reading
  another page's `localStorage`/IndexedDB via a forged origin; calling `put()`
  without consent; obtaining a secret key or raw capability from the bridge;
  requesting camera/microphone/location.
- **Beacon test.** Opening a hostile page writes **zero** entries signed by the
  visitor. Asserted at the store, not the UI.
- **Endorsement binds the digest.** A single mutated byte in a bundle invalidates
  every endorsement over it.
- **Endorsement grants nothing.** A bundle with a valid endorsement from a followed
  auditor is subject to byte-identical runtime restrictions as an unendorsed one.
  This test exists to make D5 a regression, not a memo.
- **Offline authoring.** With no network and no local model, template selection,
  source editing, and publication all succeed.

## Acceptance criteria

1. First launch prompts for a display name and a space; both exist inside one
   minute, and `set_display_name` finally has a caller.
2. A personal space is an owned namespace whose root secret lives in platform
   secure storage and never leaves it.
3. A person can pick a template, edit its source, and publish — producing a signed
   bundle that another device renders after nearby sync.
4. A model-assisted page can be generated, reviewed as source, and published; and
   authoring still works with the network off.
5. Every hostile-page test above fails closed.
6. Group spaces (communal) and personal spaces (owned) coexist in the multi-space
   store, and the app renders both.
7. Slice 1 writes no entry outside `pub/**`, and the `con/**` prefix is reserved so
   Slice 2 needs no migration.

## Open questions for Slice 2

- Does a connection get a read cap over all of `con/**`, or do we want tiers
  (close friends vs connections) as distinct prefixes? Reserving `con/<tier>/**`
  now costs nothing and is impossible to retrofit.
- Is there a public request path at all, or is nearby-only permanent?
- Per-device receiver keys versus one receiver per person: the Meadowcap design
  says per-device (line 692), which implies a grant fans out to a person's devices.
