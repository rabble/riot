<!-- source-sha256: e15067e31868e052904fb8a746f2e4999cb8e493868bea49132ec27972af88b7 -->
# account gates

- agreements: human-action; Confirm current Apple and Google developer agreements.; evidence: not recorded
- tax: human-action; Confirm required tax declarations in both stores.; evidence: not recorded
- banking: human-action; Confirm whether either free-app account still requires banking setup.; evidence: not recorded
- trader-status: human-action; Confirm store trader-status declarations.; evidence: not recorded
- signing: human-action; Signing identities are external and are validated in later work units.; evidence: not recorded
- hardware: human-action; Physical-device rehearsals occur after exact store builds exist.; evidence: not recorded
- console: human-action; Authenticated Console actions are performed manually.; evidence: not recorded
# Gate summary

## inventory.account-gates

- State: **PASS**
- Observed: {"agreements":1,"tax":1,"banking":1,"trader-status":1,"signing":1,"hardware":1,"console":1}
- Expected: exactly one of: agreements, tax, banking, trader-status, signing, hardware, console
- Recovery: Restore the exact canonical account/legal gate inventory.

## account.agreements

- State: **HUMAN ACTION**
- Observed: human-action
- Expected: authenticated current evidence
- Recovery: Confirm agreements in the authenticated store account.

## account.tax

- State: **HUMAN ACTION**
- Observed: human-action
- Expected: authenticated current evidence
- Recovery: Confirm tax in the authenticated store account.

## account.banking

- State: **HUMAN ACTION**
- Observed: human-action
- Expected: authenticated current evidence
- Recovery: Confirm banking in the authenticated store account.

## account.trader-status

- State: **HUMAN ACTION**
- Observed: human-action
- Expected: authenticated current evidence
- Recovery: Confirm trader-status in the authenticated store account.

## account.signing

- State: **HUMAN ACTION**
- Observed: human-action
- Expected: authenticated current evidence
- Recovery: Confirm signing in the authenticated store account.

## account.hardware

- State: **HUMAN ACTION**
- Observed: human-action
- Expected: authenticated current evidence
- Recovery: Confirm hardware in the authenticated store account.

## account.console

- State: **HUMAN ACTION**
- Observed: human-action
- Expected: authenticated current evidence
- Recovery: Confirm console in the authenticated store account.
