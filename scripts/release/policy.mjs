// The policy stop gate. Producing a store candidate is forbidden until every
// required user-safety control is present and true. Evaluation is fail-closed:
// a control that is absent or false BLOCKS, and an unrecognized control key is a
// hard error so the obligation set can never be silently widened or bypassed by
// a typo.

/** The seven safety obligations every release candidate depends on. */
export const REQUIRED_CONTROLS = Object.freeze([
  'filtering',
  'contentReporting',
  'authorReporting',
  'localBlocking',
  'moderatorTombstone',
  'responseOwnership',
  'publicContact',
]);

const REQUIRED_SET = new Set(REQUIRED_CONTROLS);

/**
 * Evaluate a policy source record. Returns `{ status, missing }` where status is
 * `READY` only when all required controls are present and true, otherwise
 * `BLOCKED` with the sorted list of missing controls. Throws on any control key
 * outside the required set.
 */
export function evaluatePolicy(policy) {
  for (const key of Object.keys(policy)) {
    if (!REQUIRED_SET.has(key)) {
      throw new Error(`unknown policy control: ${key}`);
    }
  }

  const missing = REQUIRED_CONTROLS.filter((control) => policy[control] !== true).sort();
  return {
    status: missing.length === 0 ? 'READY' : 'BLOCKED',
    missing,
  };
}
