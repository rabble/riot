<!-- source-sha256: bfc71a9188c6fc1a3d20ee44afc90ac1a944c08c2e1ffec5740ffe9acf76323f -->
# permissions.md

## privacy.apple

- State: **PASS**
- Observed: first-launch,denied-permission,granted-permission,nearby-sync,followed-site-refresh
- Expected: all referenced network evidence rows exist
- Recovery: Correct the store answer or add the missing network evidence.

## privacy.google

- State: **PASS**
- Observed: first-launch,denied-permission,granted-permission,nearby-sync,followed-site-refresh
- Expected: all referenced network evidence rows exist
- Recovery: Correct the store answer or add the missing network evidence.

## privacy.required-reason-audit

- State: **HUMAN ACTION**
- Observed: 0
- Expected: completed submitted-dependency API inventory
- Recovery: Complete the Apple required-reason API audit before archive production.
