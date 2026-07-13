#!/bin/sh
# Repack all built-in community miniapps into committed catalog artifacts.
# The packer aborts before writing if it would change frozen Checklist bytes.
set -eu
cargo run -p riot-core --example pack_starter
echo "Packed all starter artifacts; frozen Checklist bytes are unchanged."
