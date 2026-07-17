# Riot Auto-Fixing / Self-Healing Recovery System — Design

**Goal:** Riot must never brick and never silently lose data. Any corrupt, partial, or
version-incompatible persisted state auto-recovers to a usable app, quarantines what it can't
restore (never deletes), and honestly tells the user what happened. This is a field tool for
activists — a bad sync, a killed write, a phone running out of space, or a schema change in an
update must degrade gracefully, not lock someone out of their community.

## Three invariants (non-negotiable)

1. **Never brick.** Every restore/import boundary reaches a usable state — worst case, a fresh
   profile in onboarding. No dead-end error with only a re-failing RETRY.
2. **Never delete user data.** Un-restorable state is *quarantined* (moved aside, timestamped,
   with a manifest), never destroyed. Recovery is copy-on-write, matching `session.rs`'s
   preview→plan→commit discipline and the FFI panic=unwind quarantine.
3. **Never heal silently.** Every auto-fix is recorded and surfaced. The user sees "we recovered
   / saved X aside," not a mystery empty app.

## The reusable core

### `RecoveryQuarantine`
One component every boundary uses. Not per-call ad-hoc.
- `quarantine(_ artifacts: [URL|Data], reason: RecoveryReason, error: Error?) -> QuarantineRef`
  — moves files / writes blobs to `<storage>/quarantine/<ISO8601>-<reason>/`, writes a
  `manifest.json` (what, why, when, the underlying error string, app version). Atomic move where
  possible; never deletes the source until safely relocated.
- `list() -> [QuarantineRef]` and `open(_ ref)` for the recovery UI (inspect / export / discard-
  by-explicit-user-action only).
- A rolling `recovery.log` (append-only) so auto-fixes are inspectable even headless.

### `recovering(_:onFailure:)` — the resilient-restore helper
The pattern applied at EVERY boundary, so resilience is systematic, not remembered case-by-case:
```
let value = recovering(step: .space) {
    try restoreSpace()
} onFailure: { error in
    quarantine([spaceBlob], reason: .space, error: error)   // record + set aside
    report.dropped(.space, error)                            // surface it
    return nil                                               // degrade, keep going
}
```
Each isolated unit: success → keep it; failure → quarantine that unit + record + continue with a
degraded-but-usable result. One bad unit never takes down its siblings or the whole open.

### `RecoveryReport`
Produced by every open/import. `{ healed: [], quarantined: [QuarantineRef], dropped: [Step] }`.
Exposed from the repository so the shell renders an honest notice and the recovery view lists it.

## Boundaries it covers (phased)

- **Phase 1 (NOW — the launch brick): profile open.** `ProfileRepository.open` — the 3 throwing
  steps (core sealed-identity/DB open; space rejoin; alert replay). Core-open failure → quarantine
  persisted state + SQLite DB, open fresh, land in onboarding. Space/alert failure → skip that
  unit, keep the identity. This is the anchor and it ships first — it also auto-fixes the current
  621-upgrade brick for anyone on the fixed build (no manual delete-reinstall).
- **Phase 2: per-community open/reproject.** A corrupt or unreadable community must not brick the
  registry or the other communities — quarantine that community, keep the rest, mark it
  "unavailable / recover" in the chooser.
- **Phase 3: sync import.** Already copy-on-write (preview→plan→commit). Add: a bundle that fails
  verify/plan is quarantined + logged, never partially applied, and the session continues.
- **Phase 4: app-drop / app data.** A bad app pack or app-data blob is quarantined + skipped (the
  install loops already `try?`-skip; route them through the quarantine so it's recorded, not lost).
- **Phase 5: the protected-storage blob load itself.** If `storage.load()` returns undecodable
  bytes, quarantine the blob + start from empty rather than throwing before open even begins.

## Startup self-check

On launch, open through the recovering path and produce a `RecoveryReport`. If it's non-empty,
show a dismissible, honest banner ("Some data couldn't be restored — it's saved aside") linking to
the **Recovery view** (lists quarantined items + timestamps + reasons; offers export and an
explicit, confirmed "start fresh" that quarantines current state). The launch error surface always
offers **Start fresh** (quarantine + fresh open), never only a re-failing RETRY.

## What this is NOT
- Not silent auto-delete. Not auto-migration of old formats (a separate concern; quarantine buys
  time to write real migrations). Not a way to hide bugs — the recovery log makes auto-fixes
  visible precisely so recurring corruption is caught, not masked.

## Test discipline (per phase)
Feed each boundary a poisoned input (undecodable blob, invalid sealed identity, a bundle the core
rejects, a corrupt community) and assert: open/import RECOVERS (usable result), the bad unit is
quarantined (artifact exists, manifest written), the original is NOT destroyed, and the
`RecoveryReport` names what was dropped. The recovery path is itself covered — resilience you can't
test is resilience you don't have.
