// The CLI skeleton is the stable diagnostic surface later work units extend
// without changing. It must report the policy stop gate truthfully — a READY
// policy exits zero, a BLOCKED policy exits non-zero and names what is missing —
// expose a machine-readable `--json` form, run `generate`, and fail closed on an
// unknown or absent command.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { runCli } from '../cli.mjs';
import { REQUIRED_CONTROLS } from '../policy.mjs';

function fullPolicy() {
  return Object.fromEntries(REQUIRED_CONTROLS.map((control) => [control, true]));
}

function depsWith(overrides = {}) {
  return {
    loadPolicy: () => fullPolicy(),
    generate: () => ['release/generated/worksheets/app-privacy.md'],
    ...overrides,
  };
}

test('status on a READY policy exits zero and says READY', () => {
  const result = runCli(['status'], depsWith());
  assert.equal(result.code, 0);
  assert.match(result.stdout, /READY/);
});

test('status on a BLOCKED policy exits non-zero and names the missing control', () => {
  const result = runCli(['status'], depsWith({ loadPolicy: () => ({ filtering: true }) }));
  assert.notEqual(result.code, 0);
  assert.match(result.stdout, /BLOCKED/);
  assert.match(result.stdout, /publicContact/);
});

test('status --json emits canonical machine-readable readiness', () => {
  const result = runCli(['status', '--json'], depsWith());
  const parsed = JSON.parse(result.stdout);
  assert.equal(parsed.status, 'READY');
  assert.deepEqual(parsed.missing, []);
  assert.equal(result.code, 0);
});

test('status --json still exits non-zero when BLOCKED', () => {
  const result = runCli(['status', '--json'], depsWith({ loadPolicy: () => ({ filtering: true }) }));
  assert.equal(JSON.parse(result.stdout).status, 'BLOCKED');
  assert.notEqual(result.code, 0);
});

test('generate runs the generator and reports how many artifacts it produced', () => {
  let called = 0;
  const result = runCli(['generate'], depsWith({
    generate: () => {
      called += 1;
      return ['a.md', 'b.md'];
    },
  }));
  assert.equal(called, 1);
  assert.equal(result.code, 0);
  assert.match(result.stdout, /2/);
});

test('an unknown command fails closed', () => {
  const result = runCli(['teleport'], depsWith());
  assert.notEqual(result.code, 0);
  assert.match(result.stdout, /unknown command/i);
});

test('no command prints usage and fails closed', () => {
  const result = runCli([], depsWith());
  assert.notEqual(result.code, 0);
  assert.match(result.stdout, /usage/i);
});
