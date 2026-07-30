<!-- source-sha256: 67fe4537351790ebc6c50ac285433e42db77e79c080d8aa1c7a8d3c4f8f3b9c6 -->
# review instructions

## first-launch

- Instruction: Launch into built-in synthetic community content.
- Evidence: apps/ios/Riot/RiotApp.swift

## demo-content

- Instruction: Built-in Riverside content is synthetic demo content.
- Evidence: fixtures/demo/riverside/content.json

## no-login

- Instruction: No developer account or login is required.
- Evidence: apps/ios/Riot/RiotApp.swift

## create

- Instruction: Create a local community from the community switcher.
- Evidence: apps/ios/Riot/AppModel.swift

## join

- Instruction: Join with a valid Riot reference or scan a QR code.
- Evidence: apps/ios/Riot/Core/RiotDeepLink.swift

## publish

- Instruction: Compose, review, sign, and persist an update.
- Evidence: apps/ios/Riot/PostUpdateView.swift

## restart

- Instruction: Restart and confirm the local community remains available.
- Evidence: apps/ios/Riot/Core/ProfileRepository.swift

## offline

- Instruction: Read locally retained community records without a network.
- Evidence: crates/riot-core/src/newswire/store.rs

## local-permissions

- Instruction: Camera and nearby permissions are requested only for their named actions.
- Evidence: apps/ios/Riot/Info.plist

## nearby-testing

- Instruction: Nearby claims require named physical-device pair evidence before publication.
- Evidence: apps/ios/RiotTests/LocalNetworkNearbyTests.swift

## permission-denial

- Instruction: Denying permission leaves an actionable retry path.
- Evidence: apps/ios/Riot/QRScannerView.swift

## invalid-join

- Instruction: Invalid join input is rejected with recovery guidance.
- Evidence: apps/ios/Riot/Core/RiotDeepLink.swift

## no-peers

- Instruction: Nearby discovery with no peers remains a recoverable empty state.
- Evidence: apps/ios/RiotTests/LocalNetworkNearbyTests.swift

# Gate summary

## review.first-launch

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add truthful review instructions for first-launch.

## review.demo-content

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add truthful review instructions for demo-content.

## review.no-login

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add truthful review instructions for no-login.

## review.create

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add truthful review instructions for create.

## review.join

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add truthful review instructions for join.

## review.publish

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add truthful review instructions for publish.

## review.restart

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add truthful review instructions for restart.

## review.offline

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add truthful review instructions for offline.

## review.local-permissions

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add truthful review instructions for local-permissions.

## review.nearby-testing

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add truthful review instructions for nearby-testing.

## review.permission-denial

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add truthful review instructions for permission-denial.

## review.invalid-join

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add truthful review instructions for invalid-join.

## review.no-peers

- State: **PASS**
- Observed: present
- Expected: present
- Recovery: Add truthful review instructions for no-peers.
