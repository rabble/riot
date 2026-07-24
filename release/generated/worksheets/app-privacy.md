<!-- source-sha256: 68bbdfcd6ee983d4c355f82189a31798bb7cf1f30916a01e3f92f47b28d918e4 -->
# app privacy

## App Privacy position

- Store answer: no-collection
- Evidence rows: first-launch, denied-permission, granted-permission, nearby-sync, followed-site-refresh
- No developer account: true
- No ads, tracking, or developer analytics: true
- No hidden collection: true

## first-launch

- Initiator: application
- Destination: none
- Transmitted fields: none
- Redirect handling: No request or redirect is made on first launch.
- Retention: No remote retention.
- Developer operated: false
- Developer retention: none
- User directed: false
- iOS code evidence: apps/ios/Riot/RiotApp.swift
- iPadOS code evidence: apps/ios/Riot/RiotApp.swift
- macOS code evidence: apps/macos/Riot/RiotMacApp.swift
- Android code evidence: apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt

## denied-permission

- Initiator: user
- Destination: none
- Transmitted fields: none
- Redirect handling: Denied permission does not initiate a request.
- Retention: Permission state remains platform-local.
- Developer operated: false
- Developer retention: none
- User directed: true
- iOS code evidence: apps/ios/Riot/QRScannerView.swift
- iPadOS code evidence: apps/ios/Riot/QRScannerView.swift
- macOS code evidence: apps/ios/Riot/QRScannerView.swift, apps/macos/Riot.xcodeproj/project.pbxproj
- Android code evidence: apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt, apps/android/app/src/main/kotlin/org/riot/evidence/transport/NearbyPermissions.kt

## granted-permission

- Initiator: user
- Destination: local-device-subsystem
- Transmitted fields: permission-scoped sensor or discovery data
- Redirect handling: Platform permission APIs do not follow web redirects.
- Retention: Riot does not operate a remote retention service.
- Developer operated: false
- Developer retention: none
- User directed: true
- iOS code evidence: apps/ios/Riot/QRScannerView.swift, apps/ios/Riot/Transport/LocalNetworkNearby.swift
- iPadOS code evidence: apps/ios/Riot/QRScannerView.swift, apps/ios/Riot/Transport/LocalNetworkNearby.swift
- macOS code evidence: apps/ios/Riot/QRScannerView.swift, apps/ios/Riot/Transport/LocalNetworkNearby.swift, apps/macos/Riot.xcodeproj/project.pbxproj
- Android code evidence: apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt, apps/android/app/src/main/kotlin/org/riot/evidence/transport/NearbyPermissions.kt, apps/android/app/src/main/kotlin/org/riot/evidence/transport/AndroidNearbyController.kt

## nearby-sync

- Initiator: user
- Destination: user-selected nearby peer
- Transmitted fields: signed community records, peer discovery metadata
- Redirect handling: Direct local-network transport does not follow HTTP redirects.
- Retention: The receiving peer retains records under its local community state.
- Developer operated: false
- Developer retention: peer-local
- User directed: true
- iOS code evidence: apps/ios/Riot/Transport/LocalNetworkNearby.swift, crates/riot-transport/src/lib.rs
- iPadOS code evidence: apps/ios/Riot/Transport/LocalNetworkNearby.swift, crates/riot-transport/src/lib.rs
- macOS code evidence: apps/ios/Riot/Transport/LocalNetworkNearby.swift, apps/macos/Riot.xcodeproj/project.pbxproj, crates/riot-transport/src/lib.rs
- Android code evidence: apps/android/app/src/main/kotlin/org/riot/evidence/transport/AndroidNearbyController.kt, apps/android/app/src/main/kotlin/org/riot/evidence/transport/NearbyTransport.kt, crates/riot-transport/src/lib.rs

## followed-site-refresh

- Initiator: user
- Destination: followed public site
- Transmitted fields: HTTP request metadata
- Redirect handling: Transport policy validates the followed destination; redirect behavior remains subject to candidate audit.
- Retention: Destination-server retention is outside Riot developer control.
- Developer operated: false
- Developer retention: destination-controlled
- User directed: true
- iOS code evidence: crates/riot-core/src/site/follow.rs, apps/ios/Riot/FollowSiteModel.swift
- iPadOS code evidence: crates/riot-core/src/site/follow.rs, apps/ios/Riot/FollowSiteModel.swift
- macOS code evidence: crates/riot-core/src/site/follow.rs, apps/ios/Riot/FollowSiteModel.swift, apps/macos/Riot.xcodeproj/project.pbxproj
- Android code evidence: crates/riot-core/src/site/follow.rs, apps/android/app/src/main/kotlin/org/riot/evidence/FollowSite.kt, apps/android/app/src/main/kotlin/org/riot/evidence/MainActivity.kt

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
