# Android app-to-app distribution: hand Riot to the phone next to you

**Date:** 2026-07-29
**Status:** Design, pending review gate
**Scope:** Android only. No `riot-core` / `riot-ffi` logic change.

## The problem

Riot's whole premise is that a community can keep publishing when the
infrastructure is gone or hostile. That premise has a hole at the front door:
**you can only get Riot over the internet.** App stores can delist, networks can
be throttled or cut, and the person standing next to you who most needs the app
is exactly the person who cannot reach a store to get it.

Every other transport in Riot already assumes the network is missing — BLE
nearby sync, local-IP sync, carry-a-bundle-by-hand. Distribution of the app
itself is the one link still assuming the network is present.

This design closes that: **a phone running Riot can install Riot onto a nearby
phone that has no app and no internet**, and can hand that phone an invite to
the community the sender is already on.

Prior art: [bitchat-android](https://github.com/permissionlesstech/bitchat-android)
(`app/src/main/java/com/bitchat/android/hotspot/`), which credits Briar. We take
their transport shape and deliberately drop most of their machinery — see
"What we are not building".

## What the user does

**Sender** opens Nearby → *Share Riot* → *Start sharing*. Screen shows a Wi-Fi
name, a password, and a URL, each with a QR code. Screen stays awake.

**Receiver** joins that Wi-Fi from system settings, opens the URL in their
browser, taps Download, taps Install. Then taps the invite link on the same page
and lands in the sender's community, pending first sync.

**Sender** taps *Stop sharing*. Group, server, and wake locks all tear down.

## Architecture

New package `net.protest.riot.share` inside the existing app module. Nothing
outside it changes except the manifest, `build.gradle.kts`, and one button on the
Nearby tab.

```
apps/android/app/src/main/kotlin/net/protest/riot/share/
  ShareApkSource.kt      # which APK can we hand over, and its fingerprints
  ShareHttpServer.kt     # pure-Kotlin ServerSocket HTTP/1.1, two routes
  ShareLandingPage.kt    # HTML string builder
  WifiDirectHotspot.kt   # WifiP2pManager group lifecycle + credentials
  ShareController.kt     # state machine wiring the above
  ShareRiotScreen.kt     # Compose UI, Riot design system
  SharePermissions.kt    # mirrors the existing NearbyPermissions.kt
```

The layering is load-bearing, not cosmetic. `ShareHttpServer`,
`ShareLandingPage`, and the fingerprint formatting in `ShareApkSource` are
**plain JVM** — no `android.*` imports beyond a single `Context` seam — so they
run under `src/test` on the host JVM, which is where this module's coverage
actually lives (see `android-host-jvm-no-so`: host-JVM unit tests never load the
`.so`, so anything that avoids the FFI is cheaply testable). `WifiDirectHotspot`
and `ShareRiotScreen` are device-only and are covered by instrumentation plus
manual two-device verification.

### Flow

```
[sender] ShareRiotScreen
   -> ShareController.start()
        -> ShareApkSource.resolve()     -> File, certSha256, fileSha256, versionName
        -> WifiDirectHotspot.create()   -> ssid, password, groupOwnerIp
        -> ShareHttpServer.start(file)  -> :9999
   -> screen shows SSID, password, URL, peer count, fingerprints; QR for each

[receiver] joins that Wi-Fi in system settings, opens the URL
   -> GET /          landing page: version, size, fingerprints, invite, warning
   -> GET /riot.apk  the bytes, application/vnd.android.package-archive
   -> installs, taps the riot:// invite -> joinAdditionalCommunity()
```

## Components

### ShareApkSource

Resolves the artifact to serve and the facts about it.

- Serves `applicationInfo.sourceDir` — **the running APK itself**. No download,
  no cache, no separate artifact to keep in sync.
- If `applicationInfo.splitSourceDirs` is non-empty, the install is a Play-style
  split set whose base APK is not installable elsewhere. Returns
  `Unshareable.SplitInstall`, and the screen says so plainly and points at
  riot's site. This never fires for direct-install users, who are the users this
  feature is for.
- Computes SHA-256 of the signing certificate (`PackageInfo.signingInfo` on API
  28+, `signatures` below) and SHA-256 of the APK file. Formats both as
  colon-separated hex pairs so they can be read aloud.

### ShareHttpServer

Hand-rolled HTTP/1.1 over `ServerSocket`. Roughly 150 lines.

Rationale for not taking NanoHTTPD: this module has four runtime dependencies
and already hand-rolls its wire code (`BleFrameCodec`). We serve two static
routes. A pure-Kotlin server is host-JVM testable; a NanoHTTPD subclass is not.

- `GET /` -> landing page, `text/html; charset=utf-8`
- `GET /riot.apk` -> the file, `application/vnd.android.package-archive`,
  `Content-Disposition: attachment; filename="riot-<version>.apk"`
- Supports a single open-ended `Range: bytes=N-` so an interrupted download
  resumes rather than restarting a 15MB transfer. `Accept-Ranges: bytes`.
- Anything else -> 404. No directory traversal surface: there is no path-to-file
  mapping at all, just two literal string comparisons.
- Thread per connection, hard cap on concurrent connections, `Connection: close`,
  fixed read timeout. Binds to the Wi-Fi Direct group-owner address, not
  `0.0.0.0`.

### WifiDirectHotspot

`WifiP2pManager.createGroup()`, following bitchat's and Briar's shape:

- SSID `DIRECT-RIOT-<8 chars>`, 16-char password, both from `SecureRandom` over
  an alphabet excluding look-alikes (`0O5S1lI`).
- `PowerManager` wake lock + `WifiManager` Wi-Fi lock while active.
- Retries group creation up to 5 times; gives up 15s after a creation that never
  forms a group.
- `BroadcastReceiver` on `WIFI_P2P_CONNECTION_CHANGED_ACTION` to refresh peer
  count. Registration is tracked with a flag so it cannot leak on double-start.
- `stop()` is idempotent and releases receiver, locks, server, and group in that
  order.

### SharePermissions

Mirrors the existing `NearbyPermissions.kt`. `NEARBY_WIFI_DEVICES` on API 33+,
`ACCESS_FINE_LOCATION` below it, `ACCESS_LOCAL_NETWORK` on API 36+. Riot's
`minSdk` is 26 and `targetSdk` is 36, so all three tiers are live.

### Share sheet (secondary path)

A second button emits `ACTION_SEND` with a `FileProvider` URI for the same APK,
handing it to Nearby Share / Bluetooth / Signal / whatever the sender has. About
40 lines, no permissions, useful when the receiver already has a working
transfer channel. Requires a `<provider>` and `res/xml/file_paths.xml`, neither
of which the module has today.

## The universal APK

`build.gradle.kts` currently sets `abiFilters += listOf("arm64-v8a", "x86_64")`.
A received APK that does not carry the receiver's ABI fails to install with a
message that explains nothing.

**Corrected 2026-07-31.** The original estimate here — 5.0M per ABI from
`build/native/android/jniLibs`, giving a ~15–17M universal APK — measured a DEV
build and was wrong by a factor of three. Numbers from the actual signed 0.1.2
release APK:

| | |
|---|---|
| `lib/arm64-v8a/libriot_ffi.so` | 19.1 MB uncompressed |
| `lib/x86_64/libriot_ffi.so` | 20.9 MB uncompressed |
| `classes.dex` + `classes2.dex` | 19.2 MB |
| **signed 2-ABI release APK** | **46 MB** |

Adding `armeabi-v7a` therefore lands the universal APK near **60–65 MB**, not
17 MB. Always measure a release artifact; the Rust staticlib is where the size
is, and the debug jniLibs tree is not representative of it.

**Decision stands: one universal APK — `arm64-v8a`, `armeabi-v7a`, `x86_64` — on
every channel**, but the reasoning changes and the UX consequences are real.

The decision survives because the saving was never the point. Splitting per-ABI
buys a variant matrix, ABI-mismatch UX, and — when the installed build is not
the shareable one — bitchat's entire download-resume-and-pin subsystem. One
universal APK makes "the app I am running" and "the app anyone can install" the
same sentence, which is what keeps `ShareApkSource` at twenty lines. That is
worth ~20 MB.

What the real size DOES change:

- **60 MB over Wi-Fi Direct is a minute or two, not seconds.** The transfer
  screen needs honest progress and a byte count, not a spinner. Both phones
  must stay awake for it — the wake locks are load-bearing, not belt-and-braces.
- **`Range` resume stops being a nicety.** A dropped 60 MB transfer that
  restarts from zero is a feature people give up on.
- **Storage matters on the cheap phones this targets.** The landing page should
  state the download size before the tap, so nobody starts it with 40 MB free.
- **A future size cut is worth real effort** — `armeabi-v7a` bundled with a
  release-mode `opt-level="z"` staticlib, or splitting the Rust core's debug
  symbols, would move a number that people feel while standing in a street.

Requires adding the `armv7-linux-androideabi` Rust target to
`scripts/build-native-core.sh` (or its Android equivalent) and to the ABI list
in `build.gradle.kts`.

## Invite payload

The landing page carries the sender's newswire share reference — the existing
`riot://newswire/join/v1/...` string from `newswireShareReference()` — as
tappable link, plain text, and QR.

A new `<intent-filter>` on `MainActivity` for scheme `riot` routes a tap into
the existing `RiotController.joinAdditionalCommunity()` path. That path already
handles author displacement, inline sealing under the wrapping key (Risk 13),
and carrying the descriptor handle so Home can reproject (Risk 15). We add a
deep-link entry point to it and nothing else.

The reference carries coordinates only, so a fresh install reads "pending first
sync" until content arrives over the existing BLE / local-IP sync. That is the
correct and already-built behaviour.

- Invite **defaults on** when a public communal community is active. Handing
  someone the app in order to bootstrap them onto your public wire is the point
  of the feature, and these wires are public by construction.
- A toggle on the start screen turns it off.
- With no active community the toggle is absent and the page is APK-only.

## Honest disclosure

The receiver is installing a store-unsigned binary handed over by a person,
having tapped through "install from unknown sources." We do not paper over that.

- Certificate and file fingerprints appear on **both** the sender's screen and
  the landing page.
- The landing page states that the file came from the phone next to you, not
  from Riot, and that the fingerprint can be checked against the one published
  on riot's site.
- The sender's screen shows the connected-peer count, so the sender can see if
  more devices joined than expected.
- One sentence before the sender starts: **the SSID `DIRECT-RIOT-xxxxxxxx` is
  broadcast and visible to anyone scanning for Wi-Fi.** In the contexts Riot is
  built for, "this phone is running Riot and sharing it" is a meaningful signal,
  and the sender should know it before, not after.

The 16-char password — shown only on the sender's screen and its QR — is the
only gate on who can pull the file. *Stop sharing* is the primary action
whenever the group is live.

## What we are not building

Deliberately dropped from the bitchat design, each because the universal-APK
decision removes its reason to exist:

- GitHub release polling, universal-APK download, resumable WorkManager
  download, download-progress persistence, cache metadata JSON.
- Publisher certificate pinning of a downloaded artifact
  (`BITCHAT_GITHUB_RELEASE_CERT_SHA256`).
- The shareable-variant matrix (`ShareableApkVariant`, universal-vs-arm64
  detection).
- Any APK transfer over BLE. A 15MB payload over BLE is hours, and the receiver
  would need Riot installed to receive Riot.

## Testing

Host JVM (`src/test`, where this module's coverage lives):

- `ShareHttpServer`: routes, 404, `Range` handling, `Content-Type` and
  `Content-Disposition` headers, concurrent-connection cap, traversal attempts
  against both literal routes, byte-exactness of a served file against its
  source.
- `ShareLandingPage`: fingerprints rendered in readable form, invite present
  when supplied and absent when not, HTML-escaping of the community title.
- `ShareApkSource`: split-install detection, fingerprint formatting.

Instrumentation (`src/androidTest`): permission gating, controller state
machine, teardown idempotence.

Manual, two physical devices, recorded in the PR body: sender starts, receiver
joins and installs from a phone that has never had Riot, invite opens into the
community, stop tears down. Emulators cannot exercise Wi-Fi Direct, so this step
is not optional and not automatable.

Coverage floors come from `.coverage-thresholds.json` as usual.

## Risks

1. **Wi-Fi Direct is inconsistent across OEMs.** Briar and bitchat both carry
   retry loops and timeouts for exactly this. We copy that shape and surface a
   real error state rather than a spinner.
2. **The sender's own Wi-Fi drops** when the P2P group forms on many devices.
   The screen must say this will happen before the sender starts.
3. **Split-install users cannot share.** Handled by detection and an honest
   message; a download fallback is the obvious v2 and is explicitly out of scope
   here.
4. **`armeabi-v7a` is a new Rust target** and may surface 32-bit build issues in
   the dependency tree that arm64 never exercised. It is a separate work unit so
   it cannot block the rest.
5. **Deep-link handling is a new untrusted entry point.** `riot://` input is
   attacker-supplied; it must go through the same decode-and-validate path as a
   pasted reference, with no shortcut. Reuse the canonical validator rather than
   hand-rolling a subset (see `riot-reuse-canonical-gate`).

## Marketing site (ships in the same PR, not before)

The site is under an enforced honesty regime
(`scripts/marketing/protocol-page-contracts.mjs` pins boundary phrases, a
`What is not available yet` list, and byte-identical `marketing/<page>/` ↔
`marketing/public/<page>/` mirrors). **No site copy describing this feature may
land before the feature does.** The copy below is drafted now and lands in the
implementation PR, in the same commit range as the code.

Every edit below must be applied twice, byte-identically, to
`marketing/<page>/index.html` and `marketing/public/<page>/index.html`.

### 1. `/guide/` § 6 "Bring people in" — new task, after "Invite someone with a link or QR code"

```html
        <div class="task">
          <h3>Hand Riot to a phone that doesn't have it (Android)</h3>
          <p><span class="chip needs">Needs a connection or permission</span> — nearby-Wi-Fi permission on your phone; no internet on either phone.</p>
          <ol>
            <li>Open <b>“Nearby”</b>, then <b>“Share Riot”</b>.</li>
            <li>Read the note about what other people can see, then tap <b>“Start sharing”</b>. Your own Wi-Fi disconnects while you share.</li>
            <li>Read them the Wi-Fi name and password from your screen, or let them scan the QR code.</li>
            <li>On their phone: join that Wi-Fi, open the web address shown, tap <b>Download</b>, then <b>Install</b>. Android will ask them to allow installing from this source.</li>
            <li>Tap <b>“Stop sharing”</b> when they are done.</li>
          </ol>
          <p class="expect"><strong>Expect:</strong> they install the same app you are running. The page shows that app's fingerprint so they can compare it against the one published on this site. If you left the community invite switched on, tapping it joins them to your community — empty until its first exchange with a member.</p>
          <p class="recover"><strong>If it fails:</strong> “Sharing is unavailable on this build” means this copy of Riot came from a store that installs it in per-device pieces, which cannot be passed on; download the app from this site instead. If their phone says the app was not installed, their Android is older than this build supports.</p>
          <p class="expect"><strong>Worth knowing:</strong> the Wi-Fi name your phone creates is visible to anyone scanning for networks nearby, and it says Riot in it. Only someone with the password can take the file, and sharing stops the moment you tap Stop.</p>
        </div>
```

### 2. `/guide/` § 10 "What differs by platform" — Android

The existing Android paragraphs are **stale and must be corrected in this same
change**: they describe the eight-tab demo shell and state that joining by link
or QR and sharing a community are "not present on Android at all." The reshape in
`apps/android/.../design/RiotDestination.kt` moved Android to the same four
places as iOS (Home, Tools, People, Nearby) with the other surfaces reachable as
actions. Adding an Android task to a page that tells readers Android cannot do
any of this would make the page contradict itself.

Rewrite from the current source, then add:

> Android alone can hand the app to a phone that does not have it, over a Wi-Fi
> connection it makes itself. Apple does not permit an iPhone or Mac to install
> an app onto another device, so there is no equivalent flow there and will not
> be one.

### 3. `/guide/` § 9 "When something fails" — new entry

```html
          <h3>“Sharing is unavailable on this build”</h3>
          <p>Riot can only pass on a copy of itself when it was installed as a single file. A store that delivers the app in per-device pieces leaves nothing that another phone can install. Download Riot from this site and share that.</p>
```

### 4. `/guide/` § 11 not-yet list

No entry is added — the capability ships with this PR. If the PR lands the site
copy without the feature, that is the failure mode this section exists to
prevent.

### 5. `/releases/` — one line by the Android download

> The Android download is one file that runs on every supported phone, and Riot
> can pass that same file to a phone beside it with no network at all. That is
> deliberate: an app you can only get from a store is an app that can be taken
> away.

### 6. `/why-riot/` — one paragraph, Solnit register

Placed with the existing mutual-aid material, matching the voice pinned by the
`Rebecca Solnit` / `A Paradise Built in Hell` assertion:

> The ordinary way to get an app is to ask a company for it. That works until the
> company is told no, or the network is cut, or the store is simply out of reach
> of the person standing next to you who needs it most. So on Android, Riot
> spreads the way everything else in a crisis actually spreads: hand to hand.
> One phone makes a small network of its own and gives the app to another. No
> store, no signal, no permission from anyone.

Do not touch the homepage. Its five-beat story block is pinned to
`apps/ios/Riot/CommunityShell.swift`; nothing here belongs in it.

### 7. Extend the contract gate

New assertions in `scripts/marketing/protocol-page-contracts.mjs`, so this claim
cannot later drift into a lie:

- `/guide/` contains `Hand Riot to a phone that doesn't have it` and
  `visible to anyone scanning for networks`.
- `/guide/` no longer contains `demo shell` or
  `not present on Android at all` (the stale claims).
- `/releases/` contains `pass that same file to a phone beside it`.
- Byte-identical mirrors for every edited page — already enforced generically.

Run `node scripts/marketing/protocol-page-contracts.mjs` before commit; it is a
blocking editorial gate, not a lint.

## Out of scope

iOS and macOS. Sideloading does not exist on iOS; this is structurally an
Android-only capability, and the design does not pretend otherwise.
