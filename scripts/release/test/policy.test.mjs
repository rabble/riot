// The policy stop gate is the hard precondition for producing any store
// candidate. If a required user-safety control — content filtering, in-app
// content and author reporting, local blocking, moderator/tombstone handling,
// response ownership, or a public contact — is absent, readiness is BLOCKED and
// no downstream candidate work may begin. Unknown controls fail closed so the
// gate can never be widened by a typo.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { evaluatePolicy, REQUIRED_CONTROLS } from '../policy.mjs';

function fullPolicy(overrides = {}) {
  const base = Object.fromEntries(REQUIRED_CONTROLS.map((control) => [control, true]));
  return { ...base, ...overrides };
}

test('every required control present yields READY with nothing missing', () => {
  const result = evaluatePolicy(fullPolicy());
  assert.equal(result.status, 'READY');
  assert.deepEqual(result.missing, []);
});

test('a single absent control BLOCKS and names it', () => {
  const { publicContact, ...withoutContact } = fullPolicy();
  const result = evaluatePolicy(withoutContact);
  assert.equal(result.status, 'BLOCKED');
  assert.deepEqual(result.missing, ['publicContact']);
});

test('a control explicitly set to false counts as missing', () => {
  const result = evaluatePolicy(fullPolicy({ localBlocking: false }));
  assert.equal(result.status, 'BLOCKED');
  assert.deepEqual(result.missing, ['localBlocking']);
});

test('multiple missing controls are reported sorted', () => {
  const result = evaluatePolicy(fullPolicy({ filtering: false, authorReporting: false }));
  assert.equal(result.status, 'BLOCKED');
  assert.deepEqual(result.missing, ['authorReporting', 'filtering']);
});

test('an unknown control fails closed rather than being ignored', () => {
  assert.throws(() => evaluatePolicy(fullPolicy({ surpriseControl: true })), /unknown/i);
});

test('the required-control set is exactly the seven safety obligations', () => {
  assert.deepEqual(
    [...REQUIRED_CONTROLS].sort(),
    [
      'authorReporting',
      'contentReporting',
      'filtering',
      'localBlocking',
      'moderatorTombstone',
      'publicContact',
      'responseOwnership',
    ],
  );
});
