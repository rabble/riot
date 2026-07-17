# Fast iOS dev loop

The bottleneck in iOS work here is `xcodebuild -destination generic/platform=iOS`
(~5-10 min per change). You almost never need it during iteration.

## The loop

1. **Iterate with `sh scripts/ios-check.sh`** (the `fast` default). It compiles the
   **macOS** scheme (`Riot-macOS`), which builds the *same* `apps/ios` SwiftUI
   sources natively — no simulator, no packaging, no signing. On a warm build it
   returns the compiler's verdict in seconds to ~a minute. This catches ~every
   type error, missing symbol, wrong modifier, and API misuse.
2. **`sh scripts/ios-check.sh test`** for logic — the `RiotKit-macOS` unit tests
   (same tests CI/green.sh run), native and fast.
3. **`sh scripts/ios-check.sh ios` ONCE at the end** to confirm the two things a
   macOS build can't: Xcode **target membership** (a new `.swift` file must be
   added to the iOS target — the classic "committed but not in the target" bug
   `scripts/green.sh` exists to catch) and anything behind `#if os(iOS)`.
   `sh scripts/ios-check.sh sim` (simulator) is an in-between that catches most
   iOS-only bits without a device archive.

## For pure layout / visual iteration: `#Preview`

For tuning a SwiftUI view's look, add a `#Preview { ... }` and use Xcode's canvas —
it renders the view with no full build at all. Reserve `ios-check.sh` for
compile-correctness and the final iOS confirm.

## Why it's fast (two levers)

- **macOS scheme, not the iOS device slice.** Same sources, native toolchain, no
  sim/packaging/signing.
- **Persistent DerivedData** (`build/xcode-dd`, gitignored, never auto-cleaned).
  Only the first build in a worktree is cold; the rest are incremental recompiles
  of just what changed. `ios-check.sh clean` forces a cold rebuild if needed.
  Set `RIOT_DERIVED_DATA=/path` to share one warm cache across worktrees (only
  when you know builds won't overlap — two xcodebuilds writing one DerivedData
  corrupt it).

## Agents

Agents doing iOS SwiftUI changes should compile-loop with `ios-check.sh fast`
(and `test`), and run `ios-check.sh ios` only once before handing off — not a
full iOS device build per edit.
