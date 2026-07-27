import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  FORBIDDEN_WASM,
  findForbidden,
  cargoTree,
  wasmShapeArgs,
  nativeDefaultArgs,
  check,
  main,
} from '../check-wasm-graph.mjs';

test('forbidden list pins the willow25 filesystem chain', () => {
  assert.deepEqual(FORBIDDEN_WASM, ['fjall', 'lsm-tree', 'async-fs']);
});

test('findForbidden flags crates behind cargo tree glyphs', () => {
  const tree = [
    'riot-core v0.1.0',
    '├── willow25 v0.6.0-alpha.3',
    '│   ├── fjall v3.1.6',
    '│   └── async-fs v2.2.0',
    '└── lsm-tree v3.1.6',
  ].join('\n');
  assert.deepEqual(findForbidden(tree), ['fjall', 'lsm-tree', 'async-fs']);
  assert.deepEqual(findForbidden('riot-core v0.1.0\n└── willow25 v0.6.0-alpha.3'), []);
});

test('cargo tree arg shapes encode the wasm and native contracts', () => {
  assert.deepEqual(wasmShapeArgs(), [
    '-p', 'riot-core', '--no-default-features', '--target',
    'wasm32-unknown-unknown', '-e', 'normal',
  ]);
  assert.deepEqual(nativeDefaultArgs(), ['-p', 'riot-core', '-e', 'normal']);
});

test('check reports wasm leaks and native regressions separately', () => {
  const fake = (args) =>
    args.includes('--no-default-features')
      ? 'riot-core v0.1.0\n└── fjall v3.1.6'
      : 'riot-core v0.1.0';
  const result = check(fake);
  assert.deepEqual(result.leaks, ['fjall']);
  assert.deepEqual(result.nativeMissing, ['fjall', 'lsm-tree', 'async-fs']);
});

test('check passes when wasm graph is clean and native keeps the chain', () => {
  const fake = (args) =>
    args.includes('--no-default-features')
      ? 'riot-core v0.1.0\n└── willow25 v0.6.0-alpha.3'
      : 'riot-core v0.1.0\n├── fjall v3.1.6\n├── lsm-tree v3.1.6\n└── async-fs v2.2.0';
  assert.deepEqual(check(fake), { leaks: [], nativeMissing: [] });
});

test('main returns 1 on leak, 1 on native regression, 1 on cargo failure, 0 when clean', () => {
  const leaky = () => 'fjall v3.1.6';
  assert.equal(main([], leaky), 1);
  const nativeRegressed = (args) =>
    args.includes('--no-default-features') ? 'willow25 v0.6.0-alpha.3' : 'riot-core v0.1.0';
  assert.equal(main([], nativeRegressed), 1);
  const boom = () => {
    throw new Error('cargo exploded');
  };
  assert.equal(main([], boom), 1);
  const clean = (args) =>
    args.includes('--no-default-features')
      ? 'willow25 v0.6.0-alpha.3'
      : 'fjall v3.1.6\nlsm-tree v3.1.6\nasync-fs v2.2.0';
  assert.equal(main([], clean), 0);
});

test('cargoTree invokes real cargo (smoke: --version works)', () => {
  const out = cargoTree(['--version']);
  assert.match(out, /cargo/);
});

test('direct run exercises the CLI entry (contract verdict, either direction)', () => {
  const script = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    '../check-wasm-graph.mjs',
  );
  const result = spawnSync(process.execPath, [script], {
    encoding: 'utf8',
    timeout: 120_000,
  });
  // Exit code is graph-state dependent (RED pre-patch, GREEN after WU-000
  // completes); assert only that the CLI ran and printed a verdict line.
  assert.ok(
    result.status === 0 || result.status === 1,
    `unexpected exit ${result.status}: ${result.stderr}`,
  );
  assert.match(result.stdout + result.stderr, /wasm graph contract/);
});
