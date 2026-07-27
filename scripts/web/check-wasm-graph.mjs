#!/usr/bin/env node
// Wasm dependency-graph contract for the Riot browser client.
//
// The Wasm-shaped graph (riot-core consumed with default features disabled,
// resolved for wasm32-unknown-unknown) must not contain willow25's
// filesystem-backed store chain. WU-003 extends FORBIDDEN_WASM with the full
// transport/FFI list from the browser-client spec.
//
// Usage: node scripts/web/check-wasm-graph.mjs
// Exit 0 = contract holds. Exit 1 = forbidden crate present (or cargo tree failed).

import { execFileSync } from 'node:child_process';

export const FORBIDDEN_WASM = ['fjall', 'lsm-tree', 'async-fs'];

// cargo tree prefixes dependency lines with drawing glyphs (│ ├── └──), so
// match "name v<digit>" anywhere on a line. Crate names are distinctive
// enough that substring matching cannot false-positive within this graph.
export function findForbidden(treeOutput, forbidden = FORBIDDEN_WASM) {
  return forbidden.filter((name) =>
    treeOutput.split('\n').some((line) => line.includes(`${name} v`)),
  );
}

export function cargoTree(args) {
  return execFileSync('cargo', ['tree', ...args], {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  });
}

export function wasmShapeArgs() {
  // riot-core with default features off (no sqlite, no persistent-storage),
  // resolved for the browser target. This is the riot-web consumption shape.
  return [
    '-p',
    'riot-core',
    '--no-default-features',
    '--target',
    'wasm32-unknown-unknown',
    '-e',
    'normal',
  ];
}

export function nativeDefaultArgs() {
  // Native default-feature graph: persistent-storage must remain enabled so
  // the filesystem store chain stays exactly as before the vendor patch.
  return ['-p', 'riot-core', '-e', 'normal'];
}

export function check(cargoTreeFn = cargoTree) {
  const wasmGraph = cargoTreeFn(wasmShapeArgs());
  const leaks = findForbidden(wasmGraph);
  const nativeGraph = cargoTreeFn(nativeDefaultArgs());
  const nativeMissing = FORBIDDEN_WASM.filter(
    (name) => !findForbidden(nativeGraph, [name]).includes(name),
  );
  return { leaks, nativeMissing };
}

export function main(argv = process.argv.slice(2), cargoTreeFn = cargoTree) {
  void argv;
  let result;
  try {
    result = check(cargoTreeFn);
  } catch (error) {
    console.error(`wasm graph contract: cargo tree failed: ${error.message}`);
    return 1;
  }
  if (result.leaks.length > 0) {
    console.error(
      `wasm graph contract FAILED: forbidden crates in wasm-shaped graph: ${result.leaks.join(', ')}`,
    );
    return 1;
  }
  if (result.nativeMissing.length > 0) {
    console.error(
      `wasm graph contract FAILED: native default graph lost expected crates: ${result.nativeMissing.join(', ')}`,
    );
    return 1;
  }
  console.log('wasm graph contract OK: no fjall/lsm-tree/async-fs in wasm-shaped graph; native graph unchanged');
  return 0;
}

const isDirectRun =
  process.argv[1] != null && import.meta.url === new URL(`file://${process.argv[1]}`).href;
if (isDirectRun) {
  process.exit(main());
}
