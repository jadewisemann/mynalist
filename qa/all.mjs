// Every parity suite in one mode: node qa/all.mjs [--replay | --record] [extra args for qa/run.mjs]
// Exits non-zero if any suite does (in --replay: a regression or an unrecorded scenario).
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const args = process.argv.slice(2);
const mode = args.filter((a) => a === '--replay' || a === '--record');
const dir = fileURLToPath(new URL('.', import.meta.url));
const runs = [
  ...['editing', 'fuzz', 'render'].map((s) => ['run.mjs', s, ...args]),
  ['layout.mjs', ...mode],
];
let failed = [];
for (const [script, ...rest] of runs) {
  console.log(`\n=== ${script} ${rest.join(' ')}`);
  const r = spawnSync(process.execPath, [dir + script, ...rest], { stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8' });
  // per-scenario lines are in the report; print only what isn't a match
  for (const line of r.stdout.split('\n')) if (line && !line.startsWith('ok ')) console.log(line);
  if (r.status !== 0) failed.push(rest[0] ?? script);
}
console.log(failed.length ? `\nFAILED: ${failed.join(', ')}` : '\nall suites passed');
process.exit(failed.length ? 1 : 0);
