# Personal spaces and pages design

## Status

Approved in product brainstorming on 2026-07-12. Revised 2026-07-12 after the
design review gate (Architect, Designer, Security, CTO blockers resolved; PM
approved). Revision notes are inline where a decision changed.

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
   knowing to code; WHEN creating or revising a page. *(Slice 1.5.)*
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
   page to be unable to harm them, exfiltrate anything, learn who they are, or
   record the visit; SO THAT looking at a page is never an attack; WHEN viewing
   any page.
8. **Second device / replacement:** WHO gets a new phone; WANTS to decide in
   advance whether their personal space can be recovered; SO THAT loss is a
   choice they made, not a surprise; WHEN setting up the space and when the old
   device is gone. *(Recovery mechanism Slice 1.5; the *decision* and its storage
   consequences are fixed in Slice 1 — see Root key custody.)*
9. **App maker:** WHO builds something useful on their page; WANTS to publish it
   as an app others can install; SO THAT tools spread person to person; WHEN
   sharing an app. *(Slice 3.)*
10. **App chooser:** WHO is deciding whether to install a stranger's app; WANTS to
    see what independent auditors found in it; SO THAT the choice is informed; WHEN
    browsing the directory. *(Slice 3.)*

## Decisions

These were settled in brainstorming and the review gate and are load-bearing.
Reopening any of them changes the architecture.

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

This convergence is why template, hand, and (Slice 1.5) model authoring all
produce one artifact.

**Path-layout consequence (review gate — Architect blocker).** The app paths are
*not* reused verbatim. Today `APPS_COMPONENT`/`APP_INDEX_COMPONENT` are hardcoded
as the **first** path component (`apps/entry.rs:12`, `apps/index.rs:26`) and the
admission classifiers `is_app_data_path` (`entry.rs:44`) and
`classify_app_index_path` (`index.rs:586`) reject anything whose first component
is not exactly `apps`/`app-index`. The visibility segment (`pub`/`con`) sits
*in front* of these. Therefore Slice 1 **generalizes the path grammar to carry a
leading visibility segment**, and applies that change **in lockstep** across
three gates that must never drift: the local-write path, `classify_app_index_path`
/ `is_app_data_path`, and the import pipeline's `verify_frame`. This is one
admission source of truth, matching the anti-drift discipline those modules
already document. Getting local-write and import-admit out of step silently
breaks page replication, which is Slice 1's whole demo.

### D3 — Reads are metered, writes are consent-mediated, and Slice 1 denies both to foreign pages

Two things a page's JS can do are dangerous, and the review gate showed both are
live in the *existing* runtime today, not only in the future guestbook:

1. **Writes.** `AppDataBridge::put` signs with the *calling person's own key*
   (`apps/bridge.rs:22`), and the current bridge path has **no consent gate**
   (`AppBridgeController.swift:213`). Untrusted page JS calling `put()`
   unattended would let a page you merely *looked at* write a
   **cryptographically attributable, non-repudiable, replicating** record that
   you were there — a tracking beacon strictly worse than a web one.
2. **Identifying reads.** `whoami()` returns the *viewer's* stable subspace ID,
   display name, and tag (`RiotJS.swift:51`, `AppBridgeController.swift:97`). A
   stranger's page you merely view would learn exactly who you are. "Reads are
   free" must not mean "reads deanonymize the visitor."

The network was never the exfiltration channel here. **The visitor's own signing
key and identity are.** So:

- **Slice 1 (review gate — Security & CTO blocker): a `kind: page` bundle
  rendered from a namespace the viewer does not own mounts with NO bridge —
  no `put`, no `whoami`, no `profile`, deny-closed.** A foreign page in Slice 1
  is inert HTML/CSS/JS behind the sandbox and can neither write nor identify its
  reader. There is no consent UI in Slice 1, so there is no write path to gate;
  the safe default is absence, not a silent live bridge. The beacon test asserts
  this defined behavior.
- **Slice 3** introduces the consent-mediated write bridge. A `put()` requires an
  explicit native confirmation, authored by the **host** (never page-supplied
  text), naming the concrete record being signed and its permanence, per-write or
  per-explicitly-enumerated-batch. A guestbook you consciously sign still works;
  the sheet costs nothing anyone wanted. Any identifying read exposed to a
  foreign page in Slice 3 is likewise behind consent.

### D4 — Containment is by construction, not by trust

Every page runs identically sandboxed regardless of who authored it, whether a
model generated it, and how many auditors endorsed it. The walls are, in order
of what they stop:

- **No network.** CSP is `default-src 'none'; script-src 'self'; style-src
  'self'; img-src 'self' data:` (`AppSchemeHandler.swift:9`). No fetch, XHR, or
  WebSocket.
- **A second wall on the network, required for parity.** Android already backs
  CSP with `blockNetworkLoads=true`, service-worker denial, Safe Browsing off,
  and DOM-storage disable (`AppWebViewHost.kt:52`). iOS today relies on **CSP
  alone**. Slice 1 must give iOS an independent network backstop equivalent to
  Android's, or the design must record an explicit, justified exception. The
  containment suite enumerates and tests the covert channels CSP does not
  reliably govern per-engine: `<link rel=dns-prefetch>`, WebRTC/STUN, form
  submission, `window.open`, `<link>` subresources. "iOS/Android-identical"
  must be provable, not asserted.
- **Own origin per page.** iOS serves `riot-app://<app_id_hex>/<path>`; Android
  a synthetic origin. The app ID is the origin host, so no page reaches another
  page's storage. Because the Android synthetic origin is a *secure context*
  (unlocking service workers, push, background sync), Slice 1 enumerates every
  powerful secure-context API and confirms each is independently denied, so the
  two platforms' capability sets are provably equal.
- **Navigation locked.** Top-level navigation away from the app scheme is refused
  in the navigation delegate, because CSP does not constrain it
  (`AppRuntimeView.swift:203`).
- **No device permissions.** `kind: page` bundles get no camera, microphone,
  location, or photo access in any slice.
- **No key material, no bridge for foreign pages in Slice 1** (D3).

### D5 — Auditor endorsements are signal, never capability

An auditor — including an automated one running a model — is an ordinary
subspace with an opinion. It publishes a signed attestation bound to an exact
bundle digest, at the existing path family
`app-index/<app_id>/endorsements/<auditor_subspace>` (plural, matching
`app_index_endorsement_path`, `index.rs:538`).

Endorsements are **plural** (anyone may run an auditor; you choose whom to
follow), **replicated** (they arrive over BLE with the bundle, so they work in a
blackout), and **advisory**.

**An endorsement never unlocks a capability.** It does not grant network access,
does not skip the consent sheet, does not widen the bridge. It informs *what a
person chooses to install*; it never changes *what the code may do*. To keep this
airtight against the review gate's finding that trust/install currently *does*
grant the frictionless bridge (`AppReviewSheet` trusts an app "for everyone in the
space"), the endorsement UI is **mechanically separate** from the install/trust
action: an endorsement can never be the one tap that grants a bridge.

The reasoning is the Meadowcap design's own principle 2 — *protocol validity is
not community policy*. Containment is protocol; endorsement is policy. Merging
them yields "one unreviewable authorization mechanism." Concretely: an attacker
gets unlimited offline attempts against a known reviewer, obfuscated JavaScript is
the canonical defeat for LLM review, and Riot's users face adversaries with
budgets. Keeping the sandbox costs nothing when the audit is right and is the
whole defense when it is wrong.

UI copy says **"reviewed — no findings,"** never **"safe."**

### D6 — The model runs host-side, never in the sandbox *(Slice 1.5)*

Model-assisted authoring is deferred to Slice 1.5 (review gate — PM). Templates,
the native name/bio/colors form, and the view-source editor fully satisfy
"authorship is not gated on coding" in Slice 1; the model surface is the heaviest
and riskiest part and its removal does not weaken the Slice 1 demo.

When it lands, its rules are fixed now: LLM authoring is a **native action that
produces a bundle**; the generated artifact then runs under D4 with no network and
no special trust — generator and sandbox never touch. **Local model is the
default.** Remote inference is **opt-in per use, never sticky**, behind a sheet
that names the provider *and* states that the connection itself (IP, timing, "this
device is talking to a model provider") is metadata independent of the prompt:
*"This sends your prompt to <provider>. The fact that you contacted them is also
visible. Don't include anything you wouldn't post publicly."* Page content and
Willow data are **never** auto-attached as context; only typed prompt text.

## Architecture

### Personal space = owned namespace

A personal space is a Willow **owned** namespace
(`randomly_generate_owned_namespace()`). The person holds the root
`NamespaceSecret`; the namespace ID *is* the root public key.

This differs from every namespace Riot creates today. Group spaces are communal:
`NamespaceKind` has one variant, and `create_public_space` deliberately
**zeroizes the namespace secret** (`willow/identity.rs:164`). Personal spaces
invert this — the secret is retained (custody below).

Two keypairs are in play and must not be conflated:

| Key | Role | Storage |
|---|---|---|
| Namespace root (`NamespaceSecret`, ed25519) | Mints read/write caps over the space | Platform Keychain/Keystore blob (see custody) |
| Author subspace (`EvidenceAuthor`) | Signs entries as *you* | Existing profile storage |

The existing `namespace_id == organizer_subspace_id` trick (`identity.rs:239`)
is a **communal**-space device and does not apply here.

**Owned from day one, even though Slice 1 is public-only.** Communal-vs-owned is
a bit flag *in the namespace ID itself* and cannot be changed afterward. Root
replacement is a signed migration to a new namespace, not a rotation. Creating
personal spaces as communal now would force every early user to migrate — losing
their namespace ID and every capability anyone holds — the day connections-only
ships. This is the single most expensive mistake available in this design.

### Root key custody (review gate — Security & CTO blocker)

The root is an **ed25519** secret. It **cannot be Secure-Enclave-bound** — the
enclave holds P-256 only. It is therefore a **Keychain/Keystore blob**, and the
design commits to these properties, all fixed in Slice 1 because the key is minted
in Slice 1 and cannot be retrofitted without the namespace migration named above:

- **Accessibility:** `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` (iOS) /
  device-bound, auth-gated Keystore (Android).
- **No iCloud/Google sync. No inclusion in device backups** by default. A synced
  or backed-up root is a second seizure and coercion surface.
- **Recovery is opt-in, off by default (user decision, 2026-07-12).** A user may
  *deliberately* produce a passphrase-encrypted recovery export (mechanism:
  Slice 1.5). The UI states plainly that the export is itself a seizure/coercion
  surface. With no export and a lost device, the personal space's root is gone
  and the person creates a new space; the app says so before they rely on it.

**Owned-cap minting across FFI.** Cap minting happens in Rust core
(`authorise_entry` → `write_capability`, today hardcoded to
`new_communal`, `identity.rs:65`). Slice 1 adds an owned author/identity path so
the root can mint `WriteCapability::new_owned(root, subspace)` and thread it
through `authorise_entry`/`commit_at`/`publish_app_index`, none of which accept an
externally supplied capability today. The plaintext root secret **never crosses
FFI**: it crosses only as a **sealed/wrapped blob**, unsealed inside core, used,
and zeroized immediately — reusing the existing sealed-identity precedent
(`identity.rs:81`, `seal_identity`). The custody test asserts *"no plaintext root
secret appears across FFI, in `riot-profile.json`, in logs, or in any Willow
entry"* rather than "never crosses FFI."

### Path profile

Read capabilities narrow by **path prefix**, so the public/protected boundary must
be a *path* boundary, fixed before any entry is written. The first path component
is the visibility segment, constrained by the generalized admission classifier
(D2) to a closed set of allowed values:

```
<personal owned namespace>/
  pub/                             ← served to anyone, no capability required
    profile/<subspace>/card        ← existing ProfileCard: the calling card
    page/current                   ← manifest digest of the live public page
    app-index/<app_id>/manifest
    app-index/<app_id>/bundle
    app-index/<app_id>/endorsements/<auditor>      (Slice 3)
    apps/<app_id>/**               ← public app data (Slice 3)
  con/                             ← served only to a holder of a read cap over con/**
    <tier>/                        ← RESERVED now (user decision, 2026-07-12): a
                                     tier segment (e.g. connections, close) so
                                     Slice 2 may offer tiers without migration.
                                     Slice 2 decides whether to populate >1 tier.
      page/current
      app-index/<app_id>/**
      apps/<app_id>/**             ← guestbook and other protected app data (Slice 3)
```

- **Public space:** the real page lives in `pub/`.
- **Connections-only space:** only the calling card is in `pub/`. A stranger sees
  a name, an avatar, and a *Request to connect* button — and nothing else.

Granting read access is then one call:
`ReadCapability::new_owned(root_keypair, requester_receiver_key)` delegated to the
`con/<tier>/**` area, with expiry expressed as the capability's time range.

Human-readable labels never become secret path components (Meadowcap PIO
guidance): protected path segments use high-entropy identifiers.

### Page = app: authoring and publication

```
template | hand-edit | (Slice 1.5) LLM  →  bundle (HTML/CSS/JS)
                                       →  manifest { kind: page, entry_point, digest }
                                       →  sign with author subspace key
                                       →  write pub/app-index/<app_id>/{manifest,bundle}
                                       →  set pub/page/current = <app_id>
```

`app_id` is the manifest digest, so **a page is immutable and content-addressed**;
publishing an edit publishes a new `app_id` and repoints `page/current`. This is
what lets an auditor endorsement bind to an exact bundle: change one byte, the
endorsement is void.

### Navigation and creation UX (review gate — Designer blocker)

- **First run is a full-screen onboarding gate** before the 5-tab shell: pick a
  display name (finally giving `set_display_name` its first caller), then create
  your personal space and land on template selection. Target: name + space inside
  one minute; producing the first page is encouraged in the same flow but the
  one-minute budget covers name + space.
- **The personal space is a first-class card in the existing Spaces tab**, which
  is already the default destination. Editing/republishing a page later uses the
  same authoring surface as first creation, reached from that card.
- **Two space kinds are visibly distinct at creation.** "Make your page"
  (personal, owned, root key in Keychain) and "Create group space" (communal,
  secret zeroized) are separate, unambiguously labeled entry points. Because D1's
  thesis is that users make safety decisions on labels, conflating the kinds at
  creation is a safety hazard, not a polish gap.
- **Slice 1 exposes no privacy-implying control.** No visibility toggle, no
  "connections only" switch that appears to protect while everything is written to
  `pub/`. If the affordance is shown at all it is visibly unavailable
  ("coming soon"), never a live-looking safety switch that does nothing.
- **Defined empty/error states:** owner's space before its first page
  (`page/current` unset); visitor opening a public space with no page set yet
  (card-only vs empty vs loading); publish failures (missing/expired write
  capability, signing failure, store-write failure) each have a defined UX.

### Runtime and containment

Per D4, with the Slice-1 foreign-page posture of D3. Trust is evaluated from the
**viewer's** perspective: a space owner's trust marker over their own page does
not bind a visitor, so a foreign `kind: page` bundle is never auto-trusted and, in
Slice 1, mounts bridge-less. Existing directory apps trusted by an organizer in a
space you joined keep their current behavior.

### Connection requests and read gate (Slice 2)

Reuses `InviteRequestV1` (Meadowcap design line 696); delivery is **nearby-only**
in this slice (an open, world-writable request area is a spam/enumeration vector).
`pub/**` is served by the legacy `/1` codec; `con/**` requires the protected codec
(Meadowcap §Replication read gate): handshake → prove receiver secret in a
domain-separated transcript → bind receiver-named capabilities → reconcile only
inside the granted area, checking `cap.includes_area()` before binding and
`cap.includes(entry)` before emitting anything. Failures close the session with no
distinguishable protected-data response. Revocation stops *future* reads once
peers learn the new state; it cannot recall synchronized data, and the UI says so.

## Slices

**Slice 1 — Make your page.** *(Implementation target.)* Multi-space store; owned
namespace + `NamespaceKind::Owned` + owned cap minting (net-new core work, first
work unit); root key custody; first-run onboarding; template gallery; source
editor; page publication; page runtime with the Slice-1 foreign-page posture
(bridge-less) and the iOS network backstop. **Public pages only** — no read gate.
Demoable end to end: create a space, build a hideous page, show it to the phone
next to you.

**Slice 1.5 — Assisted authoring.** Local-default, remote-opt-in model authoring
(D6); passphrase-encrypted root recovery export mechanism.

**Slice 2 — Connections only.** Read-cap minting/delegation; `InviteRequestV1`
request/grant over nearby; protected sync codec; revocation and expiry;
calling-card-only view; the tier decision for `con/<tier>/**`.

**Slice 3 — Guestbooks, apps, auditors.** Consent-mediated bridge writes
(host-authored copy, per-write/enumerated-batch); narrow delegated write caps over
`apps/<app_id>/**`; publishing a page as an installable app; auditor endorsements
and follow lists, mechanically separate from install/trust.

## Limitations, stated honestly

These belong in user-facing copy, not only here.

1. **Connections-only is not encryption.** Plaintext at rest. A granted connection
   can copy and reshare anything they read. Device seizure reveals the content.
2. **Root compromise is total and permanent.** The root key is an extractable
   Keychain blob (ed25519 cannot be enclave-bound). Extraction — forensics,
   jailbreak, coercion, or an opt-in export — lets the holder mint new read caps
   over `con/**` forever and author as any subspace in the namespace. This is
   strictly worse than limitation 1 and is why the root defaults to no-sync,
   no-backup, this-device-only.
3. **Existence and identity leak.** Without PIO/Confidential Sync (upstream
   proposals, not final), an unauthorized peer learns your namespace ID and that
   `con/` content exists (zero entries, zero payloads served). The namespace ID
   is the root public key — a **stable, unique identifier** disclosed pre-auth by
   the legacy `/1` codec over BLE, enabling passive physical correlation of the
   device across time and place (cf. commit `1187bdd`, "stop leaking device
   name"). The content is protected; the silhouette is not.
4. **Visitor identity was a live leak and is closed in Slice 1** by mounting
   foreign pages bridge-less; when identifying reads return in later slices they
   are consent-gated (D3).
5. **Revocation is forward-only.** It cannot un-send what already synced.
6. **Audits are advisory.** "Reviewed — no findings" is a report, never a
   capability change.
7. **Remote model authoring (Slice 1.5) is network egress**, and the connection
   itself is metadata. Opt-in, per-use, provider named.

## Testing strategy

Each test is tagged with its owning slice; **Slice 1 acceptance (criterion 5)
covers only the Slice-1 subset.**

- **[S1] Namespace kind is intrinsic.** A personal space's namespace ID always
  reports `is_owned()`; a group space's always `is_communal()`; no path produces
  an owned namespace whose secret was zeroized.
- **[S1] Root key custody.** No plaintext root secret appears across FFI, in
  `riot-profile.json`, in logs, or in any Willow entry; the Keychain item carries
  the this-device-only, no-sync accessibility class.
- **[S1] Owned cap minting.** Publishing into the owned namespace authorizes via
  `new_owned` threaded through `authorise_entry`; a communal author cannot write
  the owned namespace and vice versa.
- **[S1] Visibility-segment admission is one source of truth.** A crafted entry
  whose first component is not an allowed visibility value is rejected identically
  by local-write, `classify_app_index_path`/`is_app_data_path`, and `verify_frame`.
- **[S1] Containment (security-critical).** A hostile fixture page attempts, and
  must fail at, each of: `fetch`/XHR/WebSocket; external script/style/font/image;
  `<link rel=dns-prefetch>`; WebRTC/STUN; form submission; `window.open`;
  top-level navigation off the app scheme; reading another page's storage via a
  forged origin; every powerful secure-context API on Android's synthetic origin;
  requesting camera/microphone/location.
- **[S1] Beacon / foreign-page posture.** A foreign `kind: page` bundle has no
  bridge: `put`, `whoami`, and `profile` are absent; opening a hostile page writes
  **zero** entries signed by the visitor and returns nothing identifying.
  Asserted at the store and the bridge, not the UI.
- **[S1] Offline authoring.** With no network and no local model, template
  selection, source editing, and publication all succeed.
- **[S1] iOS network backstop.** With CSP stripped in a test build, the
  independent iOS network backstop still blocks a subresource/network load — CSP
  is not the only wall.
- **[S2] Path gate.** No entry may be written to `con/**` by a caller holding only
  a `pub/**` capability; attenuation is enforced by the capability.
- **[S3] Endorsement binds the digest.** One mutated bundle byte invalidates every
  endorsement over it.
- **[S3] Endorsement grants nothing.** A bundle with a valid endorsement from a
  followed auditor is subject to byte-identical runtime restrictions as an
  unendorsed one, and no endorsement path reaches the trust/install action. This
  test exists to make D5 a regression, not a memo.
- **[S3] Consent copy is host-authored.** The write-consent sheet cannot render
  page-supplied text, and one consent authorizes only the enumerated write(s).

Coverage is enforced with the Rust+Swift command set (e.g. `cargo-llvm-cov`
thresholds and Swift coverage), recorded in `.coverage-thresholds.json`, not the
JS tooling CLAUDE.md describes by default.

## Acceptance criteria (Slice 1)

1. First launch prompts for a display name and a space; both exist inside one
   minute, and `set_display_name` (the FFI setter with no current Swift caller)
   gains its first caller.
2. A personal space is an owned namespace whose root secret lives in
   this-device-only, no-sync, no-backup secure storage and never leaves it in
   plaintext.
3. A person can pick a template, edit its source, and publish — producing a signed
   bundle another device renders after nearby sync.
4. A foreign `kind: page` bundle mounts with no bridge; the beacon and containment
   tests fail closed; the iOS network backstop test passes.
5. Every **Slice-1-tagged** hostile-page and containment test above fails closed.
6. Group spaces (communal) and personal spaces (owned) coexist in the multi-space
   store, are visibly distinct at creation, and both render.
7. Slice 1 writes no entry outside `pub/**`; the `con/<tier>/**` prefix is reserved
   so Slice 2 needs no migration; Slice 1 exposes no live privacy-implying control.

## Open questions

- **Start dependency on the multi-space store.** The store is a confirmed-unbuilt
  hard blocker whose own design already persists owned namespaces + root-key
  references. Can owned-namespace creation, custody, and the page runtime begin in
  parallel against the store's *interface*, or must the store merge first? Biggest
  schedule risk; resolve before sequencing Slice 1 work units.
- **Slice 1 discovery model.** Viewing is nearby-only in Slice 1 (you render the
  page of a phone you synced with). Confirm that is the intended Slice-1 audience
  path, and how someone reaches a page whose author is not physically present
  (deferred, but name it).
- **Tiers (Slice 2).** The `con/<tier>/**` segment is reserved; Slice 2 decides
  whether to populate more than one tier and what the tiers are.

## Carried-forward implementation notes (for the plan, non-blocking)

Raised by the review gate, agreed, and to be resolved at planning time rather
than in this design:

- **Name the iOS network backstop concretely.** WKWebView has no
  `blockNetworkLoads` equivalent to Android's. The likely mechanism is treating
  the custom scheme handler as the *sole* network loader (it already returns
  `notFound` for any non-bundle host, `AppSchemeHandler.swift:31`) plus explicit
  denial of the engine-level covert channels that bypass it (`dns-prefetch`,
  WebRTC/STUN). Pick and pin this in the Slice 1 plan so the [S1] backstop test
  targets a defined mechanism.
- **Sequence the capability-threading refactor first.** `authorise_entry` /
  `commit_at` / `publish_app_index` accept no externally supplied capability
  today, so owned-cap minting is a threading refactor across the write path.
  Land it (with the [S1] owned-cap-minting test) as the first work unit, before
  the authoring UX builds on it. Put `NamespaceKind::Owned` on its own sealed
  envelope path so the existing "authenticated non-communal plaintext is still
  rejected" invariant (`identity.rs:142`) is not loosened, and keep the
  allowed-visibility set in one shared constant consumed by all three admission
  gates.
- **Rendered preview is the primary review surface for assisted authoring
  (Slice 1.5).** A person who cannot write HTML cannot audit it; containment,
  not reading, is what keeps them safe. Show the rendered page first, source
  underneath. Specify the LLM degraded states (generating, failed/timeout,
  offline/model-unavailable) so the assist entry disables cleanly to templates
  and the source editor.
- **Consent copy conveys permanence (Slice 3).** The write-consent sheet says a
  signed, replicating, non-recallable record — not merely "post as you" — and
  the flow is designed for rarity so consent fatigue never trains tap-through.
  When identifying reads (`whoami`/`profile`) later return behind consent, they
  use the same host-authored, non-page-supplied copy rule as writes.
