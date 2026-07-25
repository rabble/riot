#!/usr/bin/env bash
# Release the Riot MACOS app, on either of two channels.
#
# The macOS analogue of scripts/testflight-release.sh. Riot is a native Xcode
# app that links a Rust static lib (riot_ffi), so this does: build the macOS
# native-core slice -> archive -> export signed -> (upload | notarize + staple).
#
#   CHANNEL=appstore    Mac App Store. Exports a signed .pkg and uploads it to
#                       App Store Connect. Sandboxed (already in Riot.entitlements).
#   CHANNEL=developerid Direct download. Exports a Developer ID signed .app,
#                       wraps it in a .dmg, notarizes it, and staples the ticket
#                       so it opens cleanly offline. No Apple review.
#
# Prerequisites (one-time, done by a human — not scriptable here):
#   1. An App Store Connect app record for net.protest.riot with the macOS
#      platform enabled (appstore channel only).
#   2. Certificates on team GZCZBKH7MY:
#        appstore    -> "Apple Distribution" — Xcode automatic signing can
#                       create this for you on first archive.
#        developerid -> "Developer ID Application" — Xcode will NOT create this.
#                       An Account Holder/Admin must make it once at
#                       developer.apple.com -> Certificates. Note Apple's hard
#                       limit of 5 Developer ID Application certs per team.
#   3. An App Store Connect API key for uploading/notarizing without 2FA prompts:
#        export ASC_KEY_ID=XXXXXXXXXX
#        export ASC_ISSUER_ID=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
#        export ASC_KEY_PATH=/absolute/path/AuthKey_XXXXXXXXXX.p8
#
# Usage:
#   sh scripts/macos-release.sh                          # appstore: archive + export .pkg
#   UPLOAD=1 sh scripts/macos-release.sh                 # also upload (needs ASC_*)
#   CHANNEL=developerid sh scripts/macos-release.sh      # export .app + build .dmg
#   CHANNEL=developerid UPLOAD=1 sh scripts/macos-release.sh   # also notarize + staple
#
# UNIVERSAL (arm64 + x86_64), so Intel Macs can run it too. That costs a second
# Rust slice per release; `rustup target add x86_64-apple-darwin` is required and
# is checked below. Only the Release configs are universal — Debug stays
# single-arch (ONLY_ACTIVE_ARCH) so the dev loop does not pay for it.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# shellcheck source=scripts/lib/asc-key.sh
. "$ROOT/scripts/lib/asc-key.sh"

CHANNEL="${CHANNEL:-appstore}"
SCHEME="Riot-macOS"
PROJECT="apps/macos/Riot.xcodeproj"
OUT="build/macos-release"
ARCHIVE="$OUT/Riot-macOS.xcarchive"

case "$CHANNEL" in
  appstore)    EXPORT_OPTS="apps/macos/ExportOptions-AppStore.plist" ;;
  developerid) EXPORT_OPTS="apps/macos/ExportOptions-DeveloperID.plist" ;;
  *) echo "ERROR: CHANNEL must be 'appstore' or 'developerid' (got '$CHANNEL')" >&2; exit 2 ;;
esac

# App Store Connect rejects a build number it has already seen, so it must
# increase on every upload. Commit count is monotonic and needs no clock — the
# same rule scripts/testflight-release.sh uses, and the two apps share the
# bundle id, so their build numbers advance together.
BUILD_NUMBER="$(git rev-list --count HEAD)"

# Refuse to ship an unknown mix. The shared checkout is often mid-edit by other
# sessions; xcodebuild builds the WORKING TREE, not HEAD. Pass ALLOW_DIRTY=1 only
# if you deliberately want to ship uncommitted changes.
if [ -n "$(git status --porcelain -- apps/macos apps/ios crates)" ] && [ "${ALLOW_DIRTY:-0}" != "1" ]; then
  echo "ERROR: apps/macos, apps/ios or crates has uncommitted changes." >&2
  echo "       xcodebuild archives the working tree — you'd ship an unknown state." >&2
  echo "       Commit/stash first, or archive from a clean checkout, or ALLOW_DIRTY=1." >&2
  git status --short -- apps/macos apps/ios crates >&2
  exit 1
fi

echo "==> Riot macOS, channel $CHANNEL, build $BUILD_NUMBER, at $(git rev-parse --short HEAD)"

echo "==> native core (macOS universal: arm64 + x86_64, net-enabled)"
# The app links the FFI-owned iroh runtime (bindNetRuntime / MobileNetRuntime /
# sync_with_anchor). scripts/conference/build-native-core.sh builds those
# NET-FREE, so its staticlib is missing those symbols and the archive fails to
# link. Build with the `net` feature and net bindings so the archived app is
# actually network-capable.
#
# UNIVERSAL: the app target is ARCHS = "arm64 x86_64", so a single-arch
# staticlib fails to link the x86_64 slice. Both Rust slices are built and
# lipo'd together. `rustup target add x86_64-apple-darwin` is a prerequisite —
# checked here rather than discovered as a confusing linker error.
for t in aarch64-apple-darwin x86_64-apple-darwin; do
  rustup target list --installed | grep -qx "$t" || {
    echo "ERROR: rust target not installed: $t" >&2
    echo "       rustup target add $t" >&2
    exit 1
  }
done
RIOT_FFI_NET_BINDINGS=1 cargo run --locked --package xtask -- generate-bindings
cargo build --locked -p riot-ffi --lib --release --features net --target aarch64-apple-darwin
cargo build --locked -p riot-ffi --lib --release --features net --target x86_64-apple-darwin
mkdir -p build/native/macos
lipo -create \
  target/aarch64-apple-darwin/release/libriot_ffi.a \
  target/x86_64-apple-darwin/release/libriot_ffi.a \
  -output build/native/macos/libriot_ffi.a
echo "==> staticlib: $(lipo -archs build/native/macos/libriot_ffi.a)"

echo "==> archive (Release, macOS)"
rm -rf "$ARCHIVE"
xcodebuild archive \
  -project "$PROJECT" \
  -scheme "$SCHEME" \
  -configuration Release \
  -destination 'platform=macOS' \
  -archivePath "$ARCHIVE" \
  -allowProvisioningUpdates \
  CURRENT_PROJECT_VERSION="$BUILD_NUMBER"

# Two things App Store Connect rejects an archive for, both silent until upload.
# Check them here, where the error names itself.
APP="$ARCHIVE/Products/Applications/Riot.app"
/usr/libexec/PlistBuddy -c 'Print :LSApplicationCategoryType' "$APP/Contents/Info.plist" >/dev/null 2>&1 || {
  echo "ERROR: archived app has no LSApplicationCategoryType — App Store validation will reject it." >&2
  exit 1
}
[ -d "$ARCHIVE/dSYMs/Riot.app.dSYM" ] || {
  echo "ERROR: archive has no dSYM (DEBUG_INFORMATION_FORMAT must be dwarf-with-dsym in Release)." >&2
  exit 1
}
echo "==> archive OK: category $(/usr/libexec/PlistBuddy -c 'Print :LSApplicationCategoryType' "$APP/Contents/Info.plist"), dSYM present"

echo "==> export signed ($CHANNEL)"
rm -rf "$OUT/export"
xcodebuild -exportArchive \
  -archivePath "$ARCHIVE" \
  -exportOptionsPlist "$EXPORT_OPTS" \
  -exportPath "$OUT/export" \
  -allowProvisioningUpdates

if [ "$CHANNEL" = appstore ]; then
  PKG="$(ls "$OUT"/export/*.pkg 2>/dev/null | head -1 || true)"
  if [ -z "$PKG" ]; then echo "ERROR: no .pkg produced in $OUT/export" >&2; exit 1; fi
  echo "==> exported: $PKG"

  if [ "${UPLOAD:-0}" = "1" ]; then
    : "${ASC_KEY_ID:?set ASC_KEY_ID}"; : "${ASC_ISSUER_ID:?set ASC_ISSUER_ID}"; : "${ASC_KEY_PATH:?set ASC_KEY_PATH}"
    riot_install_asc_key "$ASC_KEY_PATH" "$ASC_KEY_ID" "$HOME/.appstoreconnect/private_keys"
    echo "==> uploading to App Store Connect"
    xcrun altool --upload-app --type macos --file "$PKG" \
      --apiKey "$ASC_KEY_ID" --apiIssuer "$ASC_ISSUER_ID"
    echo "==> uploaded. Build $BUILD_NUMBER appears in App Store Connect after processing (~10-60 min)."
  else
    echo
    echo "Not uploaded (UPLOAD!=1). To upload, either:"
    echo "  A) Xcode Organizer GUI: open $ARCHIVE, Distribute App -> App Store Connect (handles 2FA)."
    echo "  B) API key: set ASC_KEY_ID / ASC_ISSUER_ID / ASC_KEY_PATH, then:"
    echo "       UPLOAD=1 sh scripts/macos-release.sh"
  fi
  exit 0
fi

# --- developerid: .app -> notarize+staple -> .dmg -> notarize+staple ----------
# shellcheck source=scripts/lib/macos-dmg.sh
. "$ROOT/scripts/lib/macos-dmg.sh"

APP_EXPORT="$OUT/export/Riot.app"
[ -d "$APP_EXPORT" ] || { echo "ERROR: no Riot.app produced in $OUT/export" >&2; exit 1; }
DMG="$OUT/Riot-$BUILD_NUMBER.dmg"
STAGE="$OUT/dmg-stage"

if [ "${UPLOAD:-0}" != "1" ]; then
  echo "==> building $DMG"
  riot_build_dmg "$APP_EXPORT" "$DMG" "$STAGE"
  echo "==> built: $DMG"
  echo
  echo "NOT NOTARIZED (UPLOAD!=1). An un-notarized .dmg is Gatekeeper-blocked on"
  echo "every Mac but this one — do not publish it. To notarize:"
  echo "  set ASC_KEY_ID / ASC_ISSUER_ID / ASC_KEY_PATH, then:"
  echo "    CHANNEL=developerid UPLOAD=1 sh scripts/macos-release.sh"
  exit 0
fi

: "${ASC_KEY_ID:?set ASC_KEY_ID}"; : "${ASC_ISSUER_ID:?set ASC_ISSUER_ID}"; : "${ASC_KEY_PATH:?set ASC_KEY_PATH}"
# Two notarization round trips: the .app, then the .dmg that contains the
# stapled .app. Both are needed — see scripts/lib/macos-dmg.sh for why a
# stapled .dmg alone still leaves the installed app needing the network.
echo "==> notarizing app, then dmg (each waits for Apple; typically 1-15 min)"
riot_package_developerid "$APP_EXPORT" "$DMG" "$STAGE" \
  "$ASC_KEY_PATH" "$ASC_KEY_ID" "$ASC_ISSUER_ID"
echo "==> done: $DMG is notarized, stapled, and safe to publish."
