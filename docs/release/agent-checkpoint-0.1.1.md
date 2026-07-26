# Agent Checkpoint — Riot release 0.1.1

## Objective

Ship Riot 0.1.1 on three platforms: signed artifacts built, GitHub release page
updated, iOS to TestFlight, Android onto Google Play. **Done** = v0.1.1 release
published with downloadable artifacts, and both store uploads either completed
or handed to the owner with everything they need.

## Current Status

All three signed artifacts are **built and signature-verified** (see Evidence).
Nothing has been uploaded anywhere yet, and the GitHub release for 0.1.1 does
not exist yet. The release scripts have uncommitted changes that must be
committed before anything is published — the tree that produced these artifacts
is dirty (builds ran with `ALLOW_DIRTY=1`).

## Branch / Commit

- Artifacts built from `eae81062` (= `origin/main` tip), in a **detached**
  worktree at `/Users/rabble/code/explorations/riot-wt-release`.
- The main checkout `/Users/rabble/code/explorations/riot` is on
  `feat/tor-arti-dial-transport` — **another session's branch. Do not commit
  release work there.**
- Uncommitted in the release worktree (all mine, all needed):
  `apps/android/app/build.gradle.kts`, `scripts/macos-release.sh`,
  `scripts/testflight-release.sh`, `scripts/android-release.sh` (new),
  `docs/release/google-play-setup.md` (new).

## Build Status `[VERIFIED]`

Ran sequentially, all exit 0:

| Artifact | Path (under the release worktree) | Size |
|---|---|---|
| Android AAB | `apps/android/app/build/outputs/bundle/release/app-release.aab` | 41M |
| Android APK | `apps/android/app/build/outputs/apk/release/app-release.apk` | 44M |
| iOS IPA | `build/testflight/Riot.ipa` | 11M |
| macOS PKG | `build/macos-release/export/Riot.pkg` | 32M |

Version 0.1.1, build/versionCode **1008** (`git rev-list --count HEAD`).

**No macOS DMG was produced.** The App Store channel emits a `.pkg`, which is
only useful for store upload. A GitHub release needs the **notarized DMG** from
`CHANNEL=developerid`, which has not been run this session.

## Test Status `[VERIFIED]`

Not re-run this session — the code is `origin/main` unchanged. Last observed:
PR #140 CI **5/5 green** against `956d3d78`; Android host-JVM **253 passed / 0
failed**; macOS RiotKit **360 passed / 0 failed**.

## Work Completed

- PRs #133, #134, #135, #138, #139, #140 all merged to `main` (secure storage,
  emergency wipe, Android design system, screen migration + IA reshape).
- Android release signing wired to the **pre-existing** keystore (see Mental
  Model — do not create a new one).
- `scripts/android-release.sh` added; `docs/release/google-play-setup.md` added.
- Fixed: Android `versionCode` was hardcoded `1`; iOS release script logged a
  version it wasn't building; all three release scripts could race.

## Current Task

Was about to cut the GitHub v0.1.1 release when the session ended. Nothing
partial is in flight — no upload was started.

## Remaining Work

1. **Commit the release-tooling changes.** They're what produced these
   artifacts; publishing before committing ships an unreproducible build.
   Branch off `origin/main` in the release worktree, PR as usual.
2. **Build the macOS DMG** — `CHANNEL=developerid sh scripts/macos-release.sh`
   in the release worktree. Native libs are cached, so this is minutes.
   Notarization needs ASC credentials (see Blockers) — an un-notarized DMG is
   Gatekeeper-blocked and **must not** be published.
3. **Create the GitHub release** `v0.1.1`. Match the shape of `v0.1.0-alpha`,
   whose assets were `Riot-0.1.0-alpha-universal.dmg` + `Riot-0.1.0-alpha.apk`
   — so: DMG + APK. The IPA and AAB are store-bound and should **not** be
   attached (an IPA is not sideloadable for most people).
4. **iOS → TestFlight**: owner uploads `build/testflight/Riot.ipa` via Xcode
   Organizer → Distribute App. Decision below.
5. **Android → Play**: needs the `net.protest.riot` app record created in Play
   Console first; then upload the AAB to a track.

## Architectural Decisions

- **Release signing reads the keystore password from the macOS keychain**
  (`security find-generic-password -s riot-android-keystore -a riot-release-key`)
  and passes it to Gradle as a project property. Rejected: `keystore.properties`
  on disk (a plaintext secret one `git add -A` away from being committed), and
  env vars in a shell profile (same problem, wider blast radius). Without the
  keychain item the build still runs and emits **unsigned** artifacts with a
  warning, so CI and fresh checkouts are unaffected.
- **Version code from `git rev-list --count HEAD`**, matching what iOS/macOS
  already do. Play and ASC both reject a previously-seen build number; a
  hardcoded `1` can be uploaded exactly once, ever.
- **A `mkdir`-based lock in all three release scripts.** Rejected: `flock`
  (not on macOS by default). See Mental Model for why the lock exists at all.

## Current Mental Model

- **The Play signing setup already exists on this machine. Do not create a new
  keystore.** `~/riot-release-key.jks`, alias `riot`, `CN=Riot, O=protest.net,
  C=US`, created 2026-07-25, valid to 2053; password in the macOS keychain
  under service `riot-android-keystore`, account `riot-release-key`. A second
  upload key would produce artifacts Play rejects for the rest of the app's
  life. I nearly generated a duplicate before looking — look first.
- **The Play Console account is an ORGANISATION account** (Verse Communication
  PBC, ID 7492500512941694166, already shipping bitchat/diVine/Agora).
  Consequently there is **no** 25 USD registration step and **no**
  12-testers-for-14-days closed-test gate — that rule is personal accounts only.
  Riot can go straight to a track. `docs/release/google-play-setup.md` initially
  had this wrong; it is now corrected.
- **The three release scripts share `build/generated/riot-ffi` and
  `build/native/` within one checkout.** Running them concurrently means one
  regenerates the bindings the other is mid-compile against. The failure does
  not look like a race — it looks like a missing file:
  `error opening input file '.../build/generated/riot-ffi/riot_ffi.swift'`.
  That is exactly what killed the first parallel attempt. The `mkdir` lock now
  refuses the second run rather than corrupting both. **Run them sequentially.**
- **Bindings and native library must be generated/built at the same feature
  set.** Bindings made with `RIOT_FFI_NET_BINDINGS=1` against a `.so`/staticlib
  built without `--features net` link fine and die at the **first FFI call**
  (`undefined symbol: uniffi_riot_ffi_fn_clone_mobilenetruntime`). All three
  release scripts now do both net-enabled. This has bitten iOS, macOS **and**
  Android in one day.
- **Apple-Silicon Android emulators are `arm64-v8a`, not x86_64.** Building the
  x86_64 slice produces a launch crash whose real cause hides in a *suppressed*
  exception (`Native library (android-aarch64/libriot_ffi.so) not found`).
- **Android ships two ABIs.** A bundle missing one ships an app that dies on
  launch on that architecture, and nothing in the build warns you.
- **`ALLOW_DIRTY=1` was used for these builds.** The scripts otherwise refuse a
  dirty tree because xcodebuild/Gradle build the *working tree*, not HEAD. The
  artifacts therefore correspond to `eae81062` **plus** the uncommitted script
  changes. Commit before publishing, or rebuild after committing.

## Known Problems

- Android app is **half-migrated**: shell, Spaces, Incident board, Newswire,
  People, Tools, Nearby are Compose on the design system; the five remaining
  surfaces (app directory, compose & sign, import preview, follow a site,
  connection) still render their original unthemed view code inside
  `AndroidView`. Ships fine, looks inconsistent.
- `RiotTabBar` still scrolls horizontally. That was needed for eight tabs; with
  four it can go back to equal-width shares. Comment in the file says so.
- `KeystoreSecretStore`'s real Keystore I/O, and the `setUnlockedDeviceRequired`
  / StrongBox flags, have **no automated test** — they are device behaviours
  needing an instrumented run, including one device *without* StrongBox to
  exercise the fallback.

## Risks

- Publishing an **un-notarized DMG** would ship something Gatekeeper blocks on
  every Mac but this one. Notarize and staple, then verify, before upload.
- Uploading the AAB before the Play app record exists fails confusingly.
- The emulator on this machine ANR'd repeatedly under parallel builds
  (`system_server` dying, `Can't find service: package`). If a wiped emulator
  ANRs at its own launcher, the host is the problem, not the app.

## Assumptions

- `[ASSUMPTION]` The existing keystore is the one already associated with any
  Play upload for `net.protest.riot`. It was created 2026-07-25 and no upload
  has happened, so there is nothing yet to contradict — but if Play reports a
  certificate mismatch, that assumption is where to look.
- `[ASSUMPTION]` The iOS provisioning profile in the IPA
  (`iOS Team Store Provisioning Profile: net.protest.riot`) is the right one for
  TestFlight. It is an App Store profile, so this should hold.
- `[UNVERIFIED]` None of the four artifacts has been **installed and launched**.
  They are signature-verified only. The Android APK in particular has only been
  verified as a debug build on the emulator, not this release build.

## Open Questions

- Which Play track for the first upload — internal testing, or closed?
- Should the GitHub release be marked pre-release like `v0.1.0-alpha`? The repo
  README describes the project as a proof of concept, which argues yes.

## Blockers & Dependencies

- **ASC Issuer ID** — needed for scripted TestFlight upload *and* for notarizing
  the DMG. The API key `AuthKey_M95U99RZY4.p8` is installed at
  `~/.appstoreconnect/private_keys/`, but the Issuer ID is stored nowhere on
  this machine. Owner decided: **build only, upload via Xcode Organizer**. Note
  that this does not solve notarization, which has no GUI path in these scripts.
- **Play app record** for `net.protest.riot` does not exist. Owner action.

## Files Changed

| Path | Role |
|---|---|
| `scripts/android-release.sh` | New. Signed AAB+APK, both ABIs net-enabled, keychain password, `apksigner` verify. |
| `apps/android/app/build.gradle.kts` | Release `signingConfig`; `versionCode` from `-PversionCode`; `versionName` 0.1.1. |
| `scripts/macos-release.sh`, `scripts/testflight-release.sh` | Concurrency lock; iOS version line reads `MARKETING_VERSION`. |
| `docs/release/google-play-setup.md` | New. Play setup state and the signing decision. |

## Important Commands

```sh
# All from /Users/rabble/code/explorations/riot-wt-release — ONE AT A TIME.
sh scripts/android-release.sh                        # signed AAB + APK
sh scripts/testflight-release.sh                     # signed .ipa
sh scripts/macos-release.sh                          # App Store .pkg
CHANNEL=developerid sh scripts/macos-release.sh      # notarizable .dmg  <-- still needed
rmdir build/.release-lock                            # if a run was killed
```

## Evidence `[VERIFIED]`

```
Signer #1 certificate DN: CN=Riot, O=protest.net, C=US
Signer #1 certificate SHA-256 digest: 8552c62a629638292884aafb2431fde89032210c606c2be44d766cf8daeac084
```
Matches the keystore fingerprint `85:52:C6:2A:…:C0:84` — the APK is signed by
the intended upload key.

```
iOS:   Payload/Riot.app — "iOS Team Store Provisioning Profile: net.protest.riot"
macOS: 3rd Party Mac Developer Installer: Verse Communications, Inc. (GZCZBKH7MY)
```

## Constraints / Conventions

- **Shared checkout.** Several agent sessions use
  `/Users/rabble/code/explorations/riot` at once. Work in a worktree, commit with
  explicit pathspecs, never `git stash` (the stack is global), and never commit
  onto another session's branch.
- Owner standing instruction: verified work gets pushed and a PR opened without
  asking; the PR body states the real test story, red included.
- TDD is mandatory; `.coverage-thresholds.json` is the coverage source of truth.

## Important Documents

- `docs/release/google-play-setup.md` — Play state, signing decision, store-listing prerequisites.
- `~/.claude/projects/-Users-rabble-code-explorations-riot/memory/` — `android-native-lib-run-recipe.md`, `macos-signing-traps.md`, `riot-release-pipeline-traps.md`, `shared-checkout-multi-agent.md`.

## Recommended Next Steps

1. In the release worktree: `git checkout -b chore/release-0.1.1 origin/main`,
   commit the five changed/new files, push, open a PR.
2. Get the ASC Issuer ID from the owner, then
   `CHANNEL=developerid UPLOAD=1 sh scripts/macos-release.sh` to produce a
   **notarized, stapled** DMG.
3. `gh release create v0.1.1 --prerelease` with the DMG + APK, renamed to match
   the `v0.1.0-alpha` convention (`Riot-0.1.1-universal.dmg`, `Riot-0.1.1.apk`).

## Confidence

- **High** — artifacts exist and are signed by the intended identities (ran and
  observed).
- **High** — the keystore/keychain setup and the Play account facts (inspected
  directly).
- **Medium** — that these exact artifacts install and run. Signature-verified
  only; none launched.
- **Low** — anything about notarization or store upload succeeding, since
  neither has been attempted and both need a credential this machine lacks.
