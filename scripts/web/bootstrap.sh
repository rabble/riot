#!/bin/sh
set -eu

SCRIPT_DIRECTORY=$(CDPATH='' cd -P "$(dirname "$0")" && pwd)
REPOSITORY_ROOT=$(CDPATH='' cd -P "$SCRIPT_DIRECTORY/../.." && pwd)
cd "$REPOSITORY_ROOT"

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

require_exact_output() {
  if [ "$1" != "$2" ]; then
    fail "$3"
  fi
}

has_component() {
  rustup component list --toolchain "$1" --installed \
    | grep -Eq "^$2(-[^[:space:]]+)?$"
}

has_target() {
  rustup target list --toolchain "$1" --installed \
    | grep -Eq "^$2$"
}

command -v rustup >/dev/null 2>&1 || fail "rustup is required"

stable_version=$(rustc +1.95.0 --version 2>/dev/null || true)
case "$stable_version" in
  "rustc 1.95.0 "*) ;;
  *) rustup toolchain install 1.95.0 --profile minimal ;;
esac
stable_version=$(rustc +1.95.0 --version 2>/dev/null || true)
case "$stable_version" in
  "rustc 1.95.0 "*) ;;
  *) fail "Rust 1.95.0 is required" ;;
esac
require_exact_output "$(cargo +1.95.0 --version | awk '{print $2}')" "1.95.0" "Cargo 1.95.0 is required"

for component in rustfmt clippy; do
  if ! has_component 1.95.0 "$component"; then
    rustup component add --toolchain 1.95.0 "$component"
  fi
  has_component 1.95.0 "$component" || fail "Rust 1.95.0 component $component is required"
done
if ! has_target 1.95.0 wasm32-unknown-unknown; then
  rustup target add --toolchain 1.95.0 wasm32-unknown-unknown
fi
has_target 1.95.0 wasm32-unknown-unknown \
  || fail "Rust 1.95.0 target wasm32-unknown-unknown is required"

nightly_version=$(rustc +nightly-2026-07-01 --version 2>/dev/null || true)
case "$nightly_version" in
  "rustc 1.98.0-nightly "*) ;;
  *) rustup toolchain install nightly-2026-07-01 --profile minimal ;;
esac
nightly_version=$(rustc +nightly-2026-07-01 --version 2>/dev/null || true)
case "$nightly_version" in
  "rustc 1.98.0-nightly "*) ;;
  *) fail "Rust nightly-2026-07-01 is required" ;;
esac
if ! has_component nightly-2026-07-01 llvm-tools; then
  rustup component add --toolchain nightly-2026-07-01 llvm-tools-preview
fi
has_component nightly-2026-07-01 llvm-tools \
  || fail "nightly-2026-07-01 component llvm-tools-preview is required"

if [ "$(cargo llvm-cov --version 2>/dev/null || true)" != "cargo-llvm-cov 0.8.7" ]; then
  cargo +1.95.0 install cargo-llvm-cov --version 0.8.7 --locked
fi
require_exact_output "$(cargo llvm-cov --version 2>/dev/null || true)" \
  "cargo-llvm-cov 0.8.7" "cargo-llvm-cov 0.8.7 is required"

tarpaulin_version=$(cargo tarpaulin --version 2>/dev/null || true)
case "$tarpaulin_version" in
  "cargo-tarpaulin 0.37.0"|"cargo-tarpaulin-tarpaulin 0.37.0") ;;
  *) cargo +1.95.0 install cargo-tarpaulin --version 0.37.0 --locked ;;
esac
tarpaulin_version=$(cargo tarpaulin --version 2>/dev/null || true)
case "$tarpaulin_version" in
  "cargo-tarpaulin 0.37.0"|"cargo-tarpaulin-tarpaulin 0.37.0") ;;
  *) fail "cargo-tarpaulin 0.37.0 is required" ;;
esac

if [ "$(wasm-bindgen --version 2>/dev/null || true)" != "wasm-bindgen 0.2.126" ]; then
  cargo +1.95.0 install wasm-bindgen-cli --version 0.2.126 --locked
fi
require_exact_output "$(wasm-bindgen --version 2>/dev/null || true)" \
  "wasm-bindgen 0.2.126" "wasm-bindgen-cli 0.2.126 is required"

command -v node >/dev/null 2>&1 || fail "Node is required"
command -v npm >/dev/null 2>&1 || fail "npm is required"
expected_node=$(node --input-type=module --eval 'import packageJson from "./package.json" with { type: "json" }; console.log(packageJson.engines.node)')
expected_npm=$(node --input-type=module --eval 'import packageJson from "./package.json" with { type: "json" }; console.log(packageJson.engines.npm)')
require_exact_output "$expected_node" "26.4.0" "package.json must pin Node 26.4.0"
require_exact_output "$expected_npm" "11.17.0" "package.json must pin npm 11.17.0"
require_exact_output "$(node --version)" "v$expected_node" "Node $expected_node is required"
require_exact_output "$(npm --version)" "$expected_npm" "npm $expected_npm is required"
