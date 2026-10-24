// Differential QA runner.
//
//   node qa/run.mjs <suite> [--only=a,b] [--workers=3] [--stability] [--no-confirm] [--verbose]
//       live: every scenario on Dynalist (public demo) and on our build, compared side by side
//   node qa/run.mjs <suite> --record [...]
//       live, and save Dynalist's results to qa/golden/<suite>.json (merged when --only is given)
//   node qa/run.mjs <suite> --replay [--accept] [--workers=4]
//       our build only, against the recording: no network, deterministic. Exits 1 on a regression (a
//       scenario that matched when recorded and doesn't now) or an unrecorded/changed scenario.
//       --accept stores the current verdicts as the new baseline (after an intended change).
//
// Writes qa/out/<suite>/results.json + report.html (side-by-side outlines and screenshots).

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { closeBrowser, QA_NOW, runScenario } from './lib/runner.mjs';
import { compare } from './lib/compare.mjs';
import { fingerprint, goldenDyn, loadGolden, saveGolden } from './lib/golden.mjs';
import { outline, same } from './lib/state.mjs';
import { writeReport } from './lib/report.mjs';

const args = Object.fromEntries(process.argv.slice(3).map((a) => a.replace(/^--/, '').split('=')).map(([k, v]) => [k, v ?? true]));
const suiteName = process.argv[2] ?? 'editing';
const all = (await import(`./suites/${suiteName}.mjs`)).default;
const scenarios = all.filter((s) => !args.only || String(args.only).split(',').some((o) => s.id.includes(o)));
const replay = !!args.replay;
const workers = +(args.workers ?? (replay ? 4 : 3));
const outDir = join(dirname(fileURLToPath(import.meta.url)), 'out', suiteName);
mkdirSync(join(outDir, 'shots'), { recursive: true });
const golden = loadGolden(suiteName);
const OK = new Set(['match', 'geometry', 'known']);

function saveShots(file, pairs) {
  for (const [n, r] of pairs) {
    if (r.shot) writeFileSync(join(outDir, 'shots', `${file}.${n}.jpg`), r.shot);
    delete r.shot;
  }
}

async function runPair(sc) {
  const run = (app) => runScenario(app, sc, { shot: true });
  let [dyn, ours] = await Promise.all([run('dynalist'), run('ours')]);
  if (dyn.fatal) dyn = await run('dynalist'); // network hiccup on the live demo: retry once
  let flaky = false;
  if (args.stability) {
    const again = await run('dynalist');
    flaky = !same(dyn.post, again.post) || dyn.clip !== again.clip;
  }
  let { status, kinds } = compare(sc, dyn, ours);
  if (flaky && status !== 'error') status = 'flaky';
  const file = sc.id.replace(/[^\w-]+/g, '_');
  saveShots(file, [['dyn', dyn], ['ours', ours]]);
  return { ...sc, status, kinds, file, dyn, ours };
}

/** Our build against the recording; the verdict is relative to how the scenario fared when recorded. */
async function runReplay(sc) {
  const file = sc.id.replace(/[^\w-]+/g, '_');
  const g = golden.scenarios[sc.id];
  if (!g || g.fp !== fingerprint(sc)) {
    return { ...sc, status: 'unrecorded', kinds: [g ? 'scenario changed since recording' : 'not recorded'], file, dyn: {}, ours: {} };
  }
  if (g.skip) return { ...sc, status: 'skipped', kinds: [g.skip], file, dyn: {}, ours: {} };
  const ours = await runScenario('ours', sc, { shot: true });
  saveShots(file, [['ours', ours]]);
  const { status, kinds } = compare(sc, g.dyn, ours);
  const ok = OK.has(status);
  // ratchet: only a scenario that used to be fine can regress; one that never matched is a known diff
  const verdict = ok ? (g.ok ? status : 'fixed') : g.ok || ours.fatal ? 'regression' : 'known-diff';
  return { ...sc, status: verdict, kinds: [...(verdict === status ? [] : [status]), ...kinds], file, dyn: g.dyn, ours, base: g.status };
}

const MARK = {
  match: 'ok  ', diff: 'DIFF', setup: 'SETUP', error: 'ERR ', flaky: 'FLAKY', geometry: 'GEOM', known: 'KNOWN',
  regression: 'REGRESSION', 'known-diff': 'KDIFF', fixed: 'FIXED', unrecorded: 'UNREC', skipped: 'SKIP',
};
const results = [];
let next = 0;
const t0 = Date.now();
await Promise.all(
  Array.from({ length: workers }, async () => {
    while (next < scenarios.length) {
      const sc = scenarios[next++];
      const r = replay ? await runReplay(sc) : await runPair(sc);
      results.push(r);
      console.log(`${MARK[r.status]} ${r.id}${r.kinds.length ? ` [${r.kinds.join(',')}]` : ''}${r.dyn.fatal ? ` dyn: ${r.dyn.fatal}` : ''}${r.ours.fatal ? ` ours: ${r.ours.fatal}` : ''}`);
      if (args.verbose && r.status !== 'match' && r.dyn.post) {
        const a = outline(r.dyn.post), b = outline(r.ours.post);
        for (let i = 0; i < Math.max(a.length, b.length); i++) console.log(`   ${(a[i] ?? '').padEnd(40)} │ ${b[i] ?? ''}`);
      }
    }
  }),
);
// confirm pass: Dynalist's timing (undo grouping above all) shifts under parallel load, so a divergence only
// counts if it reproduces when the scenario runs alone; that lone run is also what gets recorded
const suspects = replay ? [] : results.filter((r) => r.status === 'diff');
if (suspects.length && !args['no-confirm']) {
  console.log(`\nconfirming ${suspects.length} divergence(s) serially…`);
  for (const r of suspects) {
    const again = await runPair(r);
    if (again.status === 'match') {
      Object.assign(r, { status: 'flaky', kinds: ['matched when re-run alone (load-dependent timing)', ...r.kinds], recordDyn: again.dyn });
      console.log(`FLAKY ${r.id}`);
    }
  }
}
await closeBrowser();
const order = all.map((s) => s.id);
results.sort((a, b) => order.indexOf(a.id) - order.indexOf(b.id));

if (args.record) {
  golden.meta = { source: 'https://dynalist.io/demo', recorded: new Date().toISOString(), now: new Date(QA_NOW).toISOString() };
  let n = 0;
  for (const r of results) {
    if (r.status === 'error') continue; // keep the previous recording, if any
    const sc = scenarios.find((s) => s.id === r.id);
    // Dynalist's own demo content (brought back by undo) is not ours to store: keep only the verdict
    if (r.kinds.includes('known:demo-reset-in-undo')) {
      golden.scenarios[r.id] = { fp: fingerprint(sc), ok: true, status: r.status, skip: 'known:demo-reset-in-undo (not stored)' };
      n++;
      continue;
    }
    golden.scenarios[r.id] = { fp: fingerprint(sc), ok: OK.has(r.status) || r.status === 'flaky', status: r.status, dyn: goldenDyn(r.recordDyn ?? r.dyn, sc) };
    n++;
  }
  console.log(`\nrecorded ${n} scenario(s) → ${saveGolden(suiteName, golden, order)}`);
}
if (replay && args.accept) {
  for (const r of results) {
    const g = golden.scenarios[r.id];
    if (!g || r.status === 'unrecorded' || r.status === 'skipped') continue;
    g.ok = r.status !== 'regression' && r.status !== 'known-diff';
    if (g.ok) g.status = r.status === 'fixed' ? r.kinds[0] : r.status;
  }
  console.log(`\nbaseline updated → ${saveGolden(suiteName, golden, order)}`);
}

writeFileSync(join(outDir, 'results.json'), JSON.stringify(results, null, 1));
writeReport(outDir, suiteName, results);
const count = (st) => results.filter((r) => r.status === st).length;
const secs = `(${((Date.now() - t0) / 1000).toFixed(0)}s)`;
if (replay) {
  const okN = count('match') + count('geometry') + count('known');
  console.log(`\n${suiteName} (replay of ${golden.meta.recorded ?? '—'}): ${okN}/${results.length} ok, ${count('fixed')} fixed, ${count('known-diff')} known diff, ${count('regression')} REGRESSION, ${count('unrecorded')} unrecorded, ${count('skipped')} skipped  ${secs}`);
} else {
  console.log(`\n${suiteName}: ${count('match')}/${results.length} match, ${count('geometry')} geometry(±1 caret), ${count('known')} known quirk, ${count('diff')} diff, ${count('setup')} setup, ${count('flaky')} flaky, ${count('error')} error  ${secs}`);
}
console.log(`report: ${join(outDir, 'report.html')}`);
process.exit(replay && !args.accept && (count('regression') || count('unrecorded')) ? 1 : 0);
