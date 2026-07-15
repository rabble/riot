# Dev loop — speed & housekeeping

## Pre-commit check runs in parallel

`sh scripts/green.sh` runs its five checks (rust, iOS build, macOS build, macOS
tests, android) as independent lanes in parallel. Wall time is the slowest lane,
not the sum. `sh scripts/green.sh fast` skips the Rust suite and macOS tests.

Per-lane logs land in `/tmp/green-*.log`; a RED row names the log to read.

## `target/` gets big — clean it when it does

Local builds accumulate across host + iOS + macOS + Android targets; `target/`
routinely grows past 40GB. It is safe to delete at any time (it is fully
regenerated, and gitignored). When disk gets tight:

```sh
cargo clean            # nukes all of target/ — next build is cold
du -sh target          # check size first if unsure
```

CI does not carry this cost: the `Swatinem/rust-cache` action caches and
auto-trims the build artifacts per workflow, so runners stay warm without an
ever-growing tree.

## Why there is no Rust build cache config to tune

Incremental rebuilds are already fast (a touched-file rebuild of `riot-core` is
~9s). The slow path is a *cold* compile of the 252-crate dependency graph
(~9min on a cold CI runner), which is a caching problem, not a config one —
handled by `Swatinem/rust-cache` in `.github/workflows/ci.yml`. No `sccache` is
needed at this size.
