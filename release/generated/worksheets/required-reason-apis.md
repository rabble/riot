<!-- source-sha256: 27a4e0c2c4646635c891ea64af0ae2120b899175b2279c86d97f63bed6a58689 -->
# required reason apis.md

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
