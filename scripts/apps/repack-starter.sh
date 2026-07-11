#!/bin/sh
# Repack the starter checklist into the committed catalog artifacts.
# Run from the repo root after editing fixtures/apps/checklist/.
# NOTE: interim generator — switches to `riot-app pack` once that CLI lands.
set -eu
cargo run -p riot-core --example pack_checklist
echo "Packed. Commit the two fixtures/apps/checklist.*.cbor files."
