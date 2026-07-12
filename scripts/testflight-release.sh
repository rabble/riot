#!/usr/bin/env bash
# TestFlight release for the Riot iOS app.
#
# Riot is a native Xcode app that links a Rust static lib (riot_ffi), so this
# does: build the device native-core slice -> archive -> export a signed .ipa ->
# upload to App Store Connect / TestFlight.
#
# The team's usual iOS release path is Codemagic (see divine-mobile), so ASC
# credentials are NOT on this machine. To upload from here you must provide an
# App Store Connect API key (recommended) or use Xcode Organizer's GUI.
#
# Prerequisites (one-time, done by a human — not scriptable here):
#   1. An App Store Connect app record for bundle id net.protest.riot must exist.
#   2. Export-compliance: set ITSAppUsesNonExemptEncryption in apps/ios/Riot/Info.plist
#      (Riot uses XChaCha20-Poly1305). This is a legal declaration — see README notes.
#   3. Either sign into Xcode on team GZCZBKH7MY, OR create an ASC API key and set:
#        export ASC_KEY_ID=XXXXXXXXXX
#        export ASC_ISSUER_ID=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
#        export ASC_KEY_PATH=/absolute/path/AuthKey_XXXXXXXXXX.p8
#
# Usage:
#   sh scripts/testflight-release.sh            # archive + export, print upload cmd
#   UPLOAD=1 sh scripts/testflight-release.sh   # also upload (needs ASC_* env)
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

SCHEME="Riot"
PROJECT="apps/ios/Riot.xcodeproj"
EXPORT_OPTS="apps/ios/ExportOptions.plist"
OUT="build/testflight"
ARCHIVE="$OUT/Riot.xcarchive"

# TestFlight requires a unique, increasing build number per marketing version.
# Commit count is monotonic and needs no clock.
BUILD_NUMBER="$(git rev-list --count HEAD)"

# Refuse to ship an unknown mix. The shared checkout is often mid-edit by other
# sessions; xcodebuild builds the WORKING TREE, not HEAD. Pass ALLOW_DIRTY=1 only
# if you deliberately want to ship uncommitted changes.
if [ -n "$(git status --porcelain -- apps/ios crates)" ] && [ "${ALLOW_DIRTY:-0}" != "1" ]; then
  echo "ERROR: apps/ios or crates has uncommitted changes." >&2
  echo "       xcodebuild archives the working tree — you'd ship an unknown state." >&2
  echo "       Commit/stash first, or archive from a clean checkout, or ALLOW_DIRTY=1." >&2
  git status --short -- apps/ios crates >&2
  exit 1
fi

echo "==> Riot version $(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' apps/ios/Riot/Info.plist 2>/dev/null || echo 0.1), build $BUILD_NUMBER, at $(git rev-parse --short HEAD)"

echo "==> native core (device arm64 slice)"
sh scripts/conference/build-native-core.sh

echo "==> archive (Release, generic iOS device)"
rm -rf "$ARCHIVE"
xcodebuild archive \
  -project "$PROJECT" \
  -scheme "$SCHEME" \
  -configuration Release \
  -destination 'generic/platform=iOS' \
  -archivePath "$ARCHIVE" \
  -allowProvisioningUpdates \
  CURRENT_PROJECT_VERSION="$BUILD_NUMBER"

echo "==> export signed .ipa"
xcodebuild -exportArchive \
  -archivePath "$ARCHIVE" \
  -exportOptionsPlist "$EXPORT_OPTS" \
  -exportPath "$OUT" \
  -allowProvisioningUpdates

IPA="$(ls "$OUT"/*.ipa 2>/dev/null | head -1 || true)"
if [ -z "$IPA" ]; then echo "ERROR: no .ipa produced in $OUT" >&2; exit 1; fi
echo "==> exported: $IPA"

UPLOAD_CMD="xcrun altool --upload-app --type ios --file \"$IPA\" \
  --apiKey \"\${ASC_KEY_ID}\" --apiIssuer \"\${ASC_ISSUER_ID}\""

if [ "${UPLOAD:-0}" = "1" ]; then
  : "${ASC_KEY_ID:?set ASC_KEY_ID}"; : "${ASC_ISSUER_ID:?set ASC_ISSUER_ID}"; : "${ASC_KEY_PATH:?set ASC_KEY_PATH}"
  # altool finds the key by ID under ./private_keys, ~/.appstoreconnect/private_keys, etc.
  mkdir -p "$HOME/.appstoreconnect/private_keys"
  cp "$ASC_KEY_PATH" "$HOME/.appstoreconnect/private_keys/AuthKey_${ASC_KEY_ID}.p8"
  echo "==> uploading to App Store Connect / TestFlight"
  xcrun altool --upload-app --type ios --file "$IPA" \
    --apiKey "$ASC_KEY_ID" --apiIssuer "$ASC_ISSUER_ID"
  echo "==> uploaded. Build $BUILD_NUMBER will appear in TestFlight after processing (~10-60 min)."
else
  echo
  echo "Not uploaded (UPLOAD!=1). To upload, either:"
  echo "  A) Xcode Organizer GUI: open $ARCHIVE, Distribute App -> App Store Connect (handles 2FA)."
  echo "  B) API key: set ASC_KEY_ID / ASC_ISSUER_ID / ASC_KEY_PATH, then:"
  echo "       UPLOAD=1 sh scripts/testflight-release.sh"
  echo "     (or directly: $UPLOAD_CMD )"
fi
