# Remove the Dead Import Tab

## Problem

The iOS and macOS shell exposes an **Import** destination that does not participate in the working import flow. `RiotAppModel.importEntries` is never populated, the view has no accept or reject actions, and nearby-sync review already happens in the **Connect** destination. Keeping the tab suggests that Riot has a persistent import inbox when it does not.

## Decision

Remove the Import destination until Riot has a real transport-independent import inbox.

The shell will retain these destinations, in order:

1. Spaces
2. Apps
3. Board
4. Compose
5. Connect

Nearby sync remains unchanged. When a peer offers entries, Connect continues to show the pending count and the existing **Add them** and **Not now** actions. This change does not alter FFI import review, sync acceptance, persistence, or Willow data handling.

## Code Changes

- Remove `RiotDestination.importPreview` and its title, tab label, and icon mappings.
- Remove the unused `RiotAppModel.importEntries` state.
- Remove `ImportPreviewView` and its shell destination mapping.
- Update shell-navigation tests to assert the five remaining destinations and their order.

No transport, repository, Rust, Android, or generated-binding code changes are in scope.

## Verification

- First update the navigation expectation so it fails against the six-tab shell.
- Apply the minimal production change and rerun the focused navigation tests.
- Run the complete iOS RiotKit test suite, which also compiles the shared macOS-compatible shell sources.
- Inspect the final diff to ensure nearby-sync behavior is untouched and concurrent edits are preserved.
