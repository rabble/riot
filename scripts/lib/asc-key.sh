# shellcheck shell=sh
# Install an App Store Connect API key where altool/notarytool look for it.
#
# Sourced by scripts/testflight-release.sh and scripts/macos-release.sh, and by
# scripts/test-asc-key-install.sh.

# riot_install_asc_key SRC KEY_ID DEST_DIR
#
# altool locates a key by ID under ./private_keys, ~/.appstoreconnect/private_keys,
# ~/.private_keys, so the file has to be named AuthKey_<ID>.p8 in one of those.
#
# Must be a no-op when SRC is already that file. Keeping the key at the
# canonical path is the documented setup, so a bare `cp` there fails with
# "are identical (not copied)" and exit 1 — which under `set -e` aborts the
# release one line before the upload, after a successful-looking build.
riot_install_asc_key() {
    _src="$1"; _key_id="$2"; _dest_dir="$3"
    _dest="$_dest_dir/AuthKey_${_key_id}.p8"

    mkdir -p "$_dest_dir"
    # Compare canonical paths so a symlink or a trailing-slash spelling of the
    # same file is still recognised as already-installed.
    if [ "$(cd "$(dirname "$_src")" && pwd -P)/$(basename "$_src")" \
       != "$(cd "$_dest_dir" && pwd -P)/$(basename "$_dest")" ]; then
        cp "$_src" "$_dest"
    fi
    chmod 600 "$_dest"
}
