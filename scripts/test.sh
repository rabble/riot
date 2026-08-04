#!/usr/bin/env sh
# Run a SLICE of the test suite, fast.
#
# The whole suite is slow for one reason: the `net` feature pulls iroh, which
# is ~30s of compilation before a 0.09s test runs. Almost nothing under active
# development needs it, so the default slices leave it out and stay in seconds.
#
#   sh scripts/test.sh              # quick: core + ffi, no net. The edit loop.
#   sh scripts/test.sh core         # riot-core only
#   sh scripts/test.sh ffi          # riot-ffi only, no net
#   sh scripts/test.sh journeys     # the durable Swift journeys (macOS, ~1s)
#   sh scripts/test.sh swift        # every Swift unit test (macOS)
#   sh scripts/test.sh net          # the iroh/anchor slice — slow, on purpose
#   sh scripts/test.sh all          # everything, including net
#   sh scripts/test.sh watch        # re-run `quick` whenever a file changes
#
# Any extra arguments are passed through to the underlying runner, so a single
# test is:
#   sh scripts/test.sh ffi newswire_contract
#   sh scripts/test.sh journeys testAReplySurvives
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

SLICE="${1:-quick}"
[ $# -gt 0 ] && shift || true

# TWO things keep this fast, and the second is the one people miss.
#
# 1. No `--all-features`, so iroh stays out of the dependency graph entirely.
# 2. A SEPARATE target directory per feature shape. Cargo does not rebuild
#    dependencies for fun — it rebuilds them when the feature graph changes,
#    and alternating `--features net` with plain `cargo test` in ONE target dir
#    invalidates iroh every single switch. Giving each shape its own directory
#    means both stay warm and neither is ever rebuilt for the other's sake.
#    Costs disk, saves minutes per edit.
cargo_fast() {
    crate="$1"
    shift
    # riot-core's tests need its optional features (entropy/author helpers) and
    # it has no iroh edge, so --all-features is both required and still fast.
    # riot-ffi is where `net` lives, so it stays lean.
    features=""
    [ "$crate" = "riot-core" ] && features="--all-features"
    CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target/nonet}" \
        cargo test -p "$crate" $features "$@"
}

swift_tests() {
    only="$1"
    shift
    # The RiotKit scheme on macOS links build/native/ios-simulator. On Apple
    # Silicon that slice must be aarch64-apple-ios-macabi (Mac Catalyst), NOT
    # aarch64-apple-ios-sim, or the link fails with a target mismatch.
    if [ ! -f build/native/ios-simulator/libriot_ffi.a ]; then
        echo "ERROR: build/native/ios-simulator/libriot_ffi.a is missing." >&2
        echo "       cargo build -p riot-ffi --lib --release --features net \\" >&2
        echo "         --target aarch64-apple-ios-macabi" >&2
        echo "       cp target/aarch64-apple-ios-macabi/release/libriot_ffi.a \\" >&2
        echo "         build/native/ios-simulator/libriot_ffi.a" >&2
        exit 1
    fi
    set -- xcodebuild test -project apps/ios/Riot.xcodeproj -scheme RiotKit \
        -destination 'platform=macOS' "$@"
    [ -n "$only" ] && set -- "$@" -only-testing:"$only"
    # xcodebuild is extremely noisy; keep the result lines and the failures.
    "$@" 2>&1 | grep -E "Test Case.*(passed|failed)|Executed .* test|error:|XCTAssert.*failed" || true
}

case "$SLICE" in
quick)
    echo "==> quick slice (riot-core + riot-ffi, no net)"
    cargo_fast riot-core "$@"
    cargo_fast riot-ffi "$@"
    ;;
core) cargo_fast riot-core "$@" ;;
ffi) cargo_fast riot-ffi "$@" ;;
journeys) swift_tests "RiotTests/DurableJourneyTests" "$@" ;;
swift) swift_tests "RiotTests" "$@" ;;
net)
    echo "==> net slice (iroh — slow the first time, cached after)"
    CARGO_TARGET_DIR="$ROOT/target/net" cargo test -p riot-ffi --features net "$@"
    CARGO_TARGET_DIR="$ROOT/target/net" cargo test -p riot-transport "$@"
    ;;
all)
    cargo test --workspace --all-features "$@"
    swift_tests "RiotTests"
    ;;
watch)
    command -v fswatch >/dev/null 2>&1 || {
        echo "ERROR: fswatch not installed (brew install fswatch)." >&2
        exit 1
    }
    echo "==> watching crates/ and apps/ios — re-running the quick slice"
    sh "$0" quick || true
    fswatch -o crates apps/ios | while read -r _; do
        clear
        sh "$0" quick || true
    done
    ;;
*)
    echo "unknown slice: $SLICE" >&2
    sed -n '3,20p' "$0" >&2
    exit 2
    ;;
esac
