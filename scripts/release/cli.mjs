// The release CLI's stable diagnostic surface. `runCli` is a pure function of
// its arguments and injected dependencies — it never reads the process, clock,
// or filesystem directly — so every command is deterministically testable.
// Later work units register commands by extending the dispatch table without
// changing `status`/`generate` behavior. Every path fails closed: a blocked
// policy and an unknown or absent command both exit non-zero.
import { evaluatePolicy } from './policy.mjs';
import { canonicalize } from './canonical-json.mjs';

const USAGE = 'usage: release <status [--json] | generate>';

function runStatus(argv, deps) {
  const readiness = evaluatePolicy(deps.loadPolicy());
  const code = readiness.status === 'READY' ? 0 : 2;

  if (argv.includes('--json')) {
    return { code, stdout: canonicalize(readiness) };
  }

  const detail = readiness.missing.length === 0
    ? ''
    : ` (missing: ${readiness.missing.join(', ')})`;
  return { code, stdout: `status: ${readiness.status}${detail}` };
}

function runGenerate(_argv, deps) {
  const artifacts = deps.generate();
  return { code: 0, stdout: `generated ${artifacts.length} artifacts` };
}

const COMMANDS = new Map([
  ['status', runStatus],
  ['generate', runGenerate],
]);

/**
 * Execute one CLI invocation. Returns `{ code, stdout }`; callers own process
 * exit and printing.
 */
export function runCli(argv, deps) {
  const [command, ...rest] = argv;

  if (command === undefined) {
    return { code: 2, stdout: USAGE };
  }

  const handler = COMMANDS.get(command);
  if (handler === undefined) {
    return { code: 2, stdout: `unknown command: ${command}\n${USAGE}` };
  }

  return handler(rest, deps);
}
