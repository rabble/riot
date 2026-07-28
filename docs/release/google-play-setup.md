# Getting Riot onto Google Play

Everything here is either a human Play Console step or a release step that uses
credentials this repo must never hold. The build side is ready:
`scripts/android-release.sh` produces a signed AAB for Play and a signed APK for
direct download.

## What already exists

- **The Play Console account**: Verse Communication PBC, an ORGANISATION
  account (ID 7492500512941694166), already shipping Divine, Agora and bitchat.
  Two consequences worth stating, because both are commonly assumed the other
  way: there is no 25 USD registration step left, and the 12-testers-for-14-days
  closed-test gate does NOT apply — that rule is for personal accounts only.
  Riot can go to a track directly.
- **The upload keystore**: `~/riot-release-key.jks`, alias `riot`. Its password
  is in the macOS keychain under service `riot-android-keystore`, account
  `riot-release-key`. The certificate SHA-256 fingerprint begins
  `85:52:C6:2A`. Back up this keystore securely; do not create a replacement.

## What does not exist yet

- **An app record** for `net.protest.riot`.
- **A service account** for automated uploads (only needed for CI).

## The signing decision, made once

Enrol in **Play App Signing**: Google holds the real app signing key, and this
machine only holds an *upload* key. That is the recoverable option — losing an
upload key is a support request, losing an app signing key means the app can
never be updated again, by anyone.

The upload key already exists. Verify its public certificate before the first
upload:

```sh
keytool -list -v -keystore ~/riot-release-key.jks -alias riot
```

`scripts/android-release.sh` reads the password from the macOS keychain and
passes the keystore coordinates through the release process environment.
Nothing secret is written inside the checkout or exposed in the process
argument list. The release script fails closed when either the keystore or
keychain entry is absent; contributors can still invoke Gradle directly for an
unsigned local build.

## Building what Play wants

Run the release script from the repository root:

```sh
sh scripts/android-release.sh
```

It derives a monotonic version code from the Git commit count, generates
net-enabled bindings, builds the native library for both shipped ABIs, signs
both artifacts, and verifies both artifacts against the expected upload
certificate. Play takes
`apps/android/app/build/outputs/bundle/release/app-release.aab`; the APK is for
the GitHub release.

## Store listing, before review

- **Data safety form.** Riot collects nothing and has no analytics; say so
  precisely. The form is a legal declaration.
- **Target API level.** Play enforces a floor that rises every August;
  `targetSdk = 36` is current.
- **Export compliance / encryption.** Riot uses XChaCha20-Poly1305. This is the
  same declaration made for the App Store and it is a legal statement, not a
  checkbox to guess at.
- **A privacy policy URL** is mandatory, even for an app that collects nothing.

## Only if you want CI uploads

Create a service account in Google Cloud, grant it release permissions in Play
Console, download the JSON, and keep it out of the repo. `fastlane supply` or
the Gradle Play Publisher plugin then uploads to the internal track. Not needed
for a first manual release.
