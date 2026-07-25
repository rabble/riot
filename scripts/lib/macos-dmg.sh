# shellcheck shell=sh
# Developer ID packaging for the macOS app: notarize + staple, in the order
# that produces a .dmg which opens offline.
#
# Sourced by scripts/macos-release.sh, and by scripts/test-macos-dmg-package.sh
# which stubs xcrun/hdiutil/ditto to assert the ordering without an Apple
# account.
#
# The ordering is load-bearing. A notarization ticket is stapled to a specific
# artifact, and stapling the .dmg does NOT put a ticket inside the app it
# contains. Ship only a stapled .dmg and Gatekeeper still clears the app —
# right up until the user drags it to /Applications, trashes the .dmg, and
# launches it with no network. So: notarize and staple the .app FIRST, seal
# that stapled copy into the .dmg, then notarize and staple the .dmg too.

# riot_notarize ARTIFACT KEY_PATH KEY_ID ISSUER_ID
riot_notarize() {
    _artifact="$1"; _key_path="$2"; _key_id="$3"; _issuer="$4"
    xcrun notarytool submit "$_artifact" \
        --key "$_key_path" --key-id "$_key_id" --issuer "$_issuer" \
        --wait
}

# riot_staple ARTIFACT
riot_staple() {
    xcrun stapler staple "$1"
    xcrun stapler validate "$1"
}

# riot_build_dmg APP DMG STAGE
# Seals APP into DMG with the conventional drag-to-install layout. Whatever
# state APP is in when this is called is what ships — so it must already be
# stapled.
riot_build_dmg() {
    _app="$1"; _dmg="$2"; _stage="$3"
    rm -f "$_dmg"
    rm -rf "$_stage"; mkdir -p "$_stage"
    cp -R "$_app" "$_stage/Riot.app"
    ln -s /Applications "$_stage/Applications"
    hdiutil create -volname Riot -srcfolder "$_stage" -ov -format UDZO "$_dmg" >/dev/null
    rm -rf "$_stage"
}

# riot_package_developerid APP DMG STAGE KEY_PATH KEY_ID ISSUER_ID
riot_package_developerid() {
    _app="$1"; _dmg="$2"; _stage="$3"; _key_path="$4"; _key_id="$5"; _issuer="$6"

    # notarytool will not take a bare .app — it wants a container. The zip is a
    # transport for the notary service only; the ticket lands on the .app.
    _zip="$(dirname "$_dmg")/Riot.zip"
    rm -f "$_zip"
    ditto -c -k --keepParent "$_app" "$_zip"
    riot_notarize "$_zip" "$_key_path" "$_key_id" "$_issuer"
    # The ticket is issued against the app's cdhash, so it staples to the .app
    # even though the .zip is what was submitted. The zip itself is discarded —
    # stapling it would be pointless, nothing ever opens it.
    riot_staple "$_app"
    rm -f "$_zip"

    riot_build_dmg "$_app" "$_dmg" "$_stage"
    riot_notarize "$_dmg" "$_key_path" "$_key_id" "$_issuer"
    riot_staple "$_dmg"
}
