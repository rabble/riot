<!-- source-sha256: 68bbdfcd6ee983d4c355f82189a31798bb7cf1f30916a01e3f92f47b28d918e4 -->
# permissions

## Permission justification inventory

| Permission ID | Platform | Store justification | Code evidence |
| --- | --- | --- | --- |
| camera | apple | Scan a community QR code selected by the user. | apps/ios/Riot/QRScannerView.swift |
| bluetooth | all | Discover a nearby Riot peer selected by the user. | apps/ios/Riot/Transport/LocalNetworkNearby.swift |
| local-network | apple | Exchange community records directly with a nearby peer. | apps/ios/Riot/Info.plist |
| notifications | all | Show local alerts for community updates when enabled. | apps/android/app/src/main/kotlin/org/riot/evidence/LocalNotifier.kt |
| android-internet | android | Android uses INTERNET for user-directed LAN and followed-site connections. | apps/android/app/src/main/AndroidManifest.xml |

# Gate summary

## inventory.privacy-answers

- State: **PASS**
- Observed: {"apple":1,"google":1}
- Expected: exactly one Apple and one Google privacy answer
- Recovery: Restore the exact canonical Apple and Google answer inventory.

## inventory.network-rows

- State: **PASS**
- Observed: {"first-launch":1,"denied-permission":1,"granted-permission":1,"nearby-sync":1,"followed-site-refresh":1}
- Expected: exactly one of: first-launch, denied-permission, granted-permission, nearby-sync, followed-site-refresh
- Recovery: Restore the exact canonical outbound-network scenario inventory.

## network.first-launch

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical evidence row
- Recovery: Add exactly one first-launch evidence row and remove duplicates or substitutions.

## network.denied-permission

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical evidence row
- Recovery: Add exactly one denied-permission evidence row and remove duplicates or substitutions.

## network.granted-permission

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical evidence row
- Recovery: Add exactly one granted-permission evidence row and remove duplicates or substitutions.

## network.nearby-sync

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical evidence row
- Recovery: Add exactly one nearby-sync evidence row and remove duplicates or substitutions.

## network.followed-site-refresh

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical evidence row
- Recovery: Add exactly one followed-site-refresh evidence row and remove duplicates or substitutions.

## privacy.apple

- State: **PASS**
- Observed: first-launch,denied-permission,granted-permission,nearby-sync,followed-site-refresh
- Expected: all exact scenarios with store-platform code evidence agree with position, fields, destination ownership, retention, and user direction
- Recovery: Correct the store answer or its canonical platform-specific network evidence.

## privacy.google

- State: **PASS**
- Observed: first-launch,denied-permission,granted-permission,nearby-sync,followed-site-refresh
- Expected: all exact scenarios with store-platform code evidence agree with position, fields, destination ownership, retention, and user direction
- Recovery: Correct the store answer or its canonical platform-specific network evidence.

## permission.camera

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical permission entry
- Recovery: Add exactly one camera permission justification and remove substitutions or duplicates.

## permission.bluetooth

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical permission entry
- Recovery: Add exactly one bluetooth permission justification and remove substitutions or duplicates.

## permission.local-network

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical permission entry
- Recovery: Add exactly one local-network permission justification and remove substitutions or duplicates.

## permission.notifications

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical permission entry
- Recovery: Add exactly one notifications permission justification and remove substitutions or duplicates.

## permission.android-internet

- State: **PASS**
- Observed: 1
- Expected: exactly one canonical permission entry
- Recovery: Add exactly one android-internet permission justification and remove substitutions or duplicates.

## privacy.required-reason-audit

- State: **HUMAN ACTION**
- Observed: 0
- Expected: completed submitted-dependency API inventory
- Recovery: Complete the Apple required-reason API audit before archive production.
