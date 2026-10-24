// Differential QA runner: node qa/run.mjs <suite> [--only=substr] [--workers=3] [--stability]
// Runs every scenario on Dynalist (live demo) and on our build, compares normalized state, writes
// qa/out/<suite>/results.json + report.html (side-by-side outlines and screenshots).

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { closeBrowser, runScenario } from './lib/runner.mjs';
import { diffKinds, outline, same, withinCaretTol } from './lib/state.mjs';
import { writeReport } from './lib/report.mjs';

const args = Object.fromEntries(process.argv.slice(3).map((a) => a.replace(/^--/, '').split('=')).map(([k, v]) => [k, v ?? true]));
const suiteName = process.argv[2] ?? 'editing';
const all = (await import(`./suites/${suiteName}.mjs`)).default;
const scenarios = all.filter((s) => !args.only || String(args.only).split(',').some((o) => s.id.includes(o)));
const workers = +(args.workers ?? 3);
const outDir = join(dirname(fileURLToPath(import.meta.url)), 'out', suiteName);
mkdirSync(join(outDir, 'shots'), { recursive: true });

// Divergences that are Dynalist quirks rather than editing behaviour (documented in qa/PLAN.md).
// Each gets the steps, the index of the first divergent step and the (shared) state before it.
const KNOWN = [
  {
    id: 'tab-without-focus',
    why: 'Dynalist lets Tab/Shift+Tab through when nothing is focused; the browser moves focus to its focus receiver, which selects an item',
    test: (steps, k, pre) => /^(Shift\+)?Tab$/.test(steps[k]) && !pre.focus && !pre.selected.length,
  },
  {
    id: 'untitled-placeholder',
    why: 'editing an empty title (typing, or merging an item into it), Dynalist keeps its "Untitled" placeholder as real text',
    test: (steps, k, pre, dyn) => pre.title === '' && dyn.title !== 'Untitled' && dyn.title.includes('Untitled'),
  },
  {
    id: 'title-start-enter-undo',
    why: 'Enter at the very start of the title moves it into a new item; Dynalist\'s undo does not revert that',
    test: (steps, k, pre, dyn, errors, trace) =>
      steps[k] === 'Control+z' && steps[k - 1] === 'Enter' && (k >= 2 ? trace[k - 2] : pre)?.focus?.i === -1 && (k >= 2 ? trace[k - 2] : pre).focus.s === 0,
  },
  {
    id: 'demo-reset-in-undo',
    why: 'harness: the demo document is reset by deleting its content, and Dynalist\'s undo can bring it back',
    test: (steps, k, pre, dyn) => steps[k] === 'Control+z' && dyn.items.some((it) => it.c.startsWith('Hi there!')),
  },
  {
    id: 'dynalist-exception',
    why: 'Dynalist threw an exception during the scenario',
    test: (steps, k, pre, dyn, errors) => errors.length > 0,
  },
];

async function runPair(sc) {
  const run = (app) => runScenario(app, sc, { shot: true });
  let [dyn, ours] = await Promise.all([run('dynalist'), run('ours')]);
  if (dyn.fatal) dyn = await run('dynalist'); // network hiccup on the live demo: retry once
  let flaky = false;
  if (args.stability) {
    const again = await run('dynalist');
    flaky = !same(dyn.post, again.post) || dyn.clip !== again.clip;
  }
  let kinds = [...new Set([...diffKinds(dyn.post, ours.post), ...(dyn.clip !== ours.clip ? ['clipboard'] : [])])];
  // typed text lands one char off when the caret is: accept both if that's the only difference
  if (sc.caretTol && kinds.every((k) => ['caret', 'tree', 'zoom'].includes(k)) && withinCaretTol(dyn.post, ours.post, sc.caretTol)) kinds = [];
  let tolerated = false;
  let known;
  if (sc.trace && dyn.trace && ours.trace) {
    // first step after which the two apps disagree
    const k = dyn.trace.findIndex((st, i) => !same(st, ours.trace[i]));
    if (k >= 0) {
      const key = typeof sc.steps[k] === 'string' ? sc.steps[k] : JSON.stringify(sc.steps[k]);
      kinds.unshift(`step ${k + 1}/${sc.steps.length}: ${key}`);
      // a vertical move landing one character off is font geometry, not behavior; later steps inherit it
      const d = diffKinds(dyn.trace[k], ours.trace[k]);
      tolerated = /^(Shift\+)?Arrow(Up|Down)$/.test(key) && d.every((x) => x === 'caret') && Math.abs(dyn.trace[k].focus.s - ours.trace[k].focus.s) <= 1;
      const pre = k ? dyn.trace[k - 1] : dyn.pre;
      known = KNOWN.find((q) => pre && q.test(sc.steps, k, pre, dyn.trace[k], dyn.errors ?? [], dyn.trace));
      if (known) kinds.unshift(`known:${known.id}`);
    }
  }
  const preKinds = diffKinds(dyn.pre, ours.pre).filter((k) => k !== 'focus' && k !== 'caret' && k !== 'selection');
  const status = dyn.fatal ? 'error' : flaky ? 'flaky' : preKinds.length ? 'setup' : tolerated ? 'geometry' : known ? 'known' : kinds.length ? 'diff' : 'match';
  const file = sc.id.replace(/[^\w-]+/g, '_');
  for (const [n, r] of [['dyn', dyn], ['ours', ours]]) {
    if (r.shot) writeFileSync(join(outDir, 'shots', `${file}.${n}.jpg`), r.shot);
    delete r.shot;
  }
  return { ...sc, status, kinds: status === 'setup' ? preKinds : kinds, file, dyn, ours };
}

const results = [];
let next = 0;
const t0 = Date.now();
await Promise.all(
  Array.from({ length: workers }, async () => {
    while (next < scenarios.length) {
      const sc = scenarios[next++];
      const r = await runPair(sc);
      results.push(r);
      const mark = { match: 'ok  ', diff: 'DIFF', setup: 'SETUP', error: 'ERR ', flaky: 'FLAKY', geometry: 'GEOM', known: 'KNOWN' }[r.status];
      console.log(`${mark} ${r.id}${r.kinds.length ? ` [${r.kinds.join(',')}]` : ''}${r.dyn.fatal ? ` dyn: ${r.dyn.fatal}` : ''}${r.ours.fatal ? ` ours: ${r.ours.fatal}` : ''}`);
      if (args.verbose && r.status !== 'match') {
        const a = outline(r.dyn.post), b = outline(r.ours.post);
        for (let i = 0; i < Math.max(a.length, b.length); i++) console.log(`   ${(a[i] ?? '').padEnd(40)} │ ${b[i] ?? ''}`);
      }
    }
  }),
);
// confirm pass: Dynalist's timing (undo grouping above all) shifts under parallel load, so a divergence only
// counts if it reproduces when the scenario runs alone
const suspects = results.filter((r) => r.status === 'diff');
if (suspects.length && !args['no-confirm']) {
  console.log(`\nconfirming ${suspects.length} divergence(s) serially…`);
  for (const r of suspects) {
    const again = await runPair(r);
    if (again.status === 'match') {
      Object.assign(r, { status: 'flaky', kinds: ['matched when re-run alone (load-dependent timing)', ...r.kinds] });
      console.log(`FLAKY ${r.id}`);
    }
  }
}
await closeBrowser();
results.sort((a, b) => scenarios.indexOf(scenarios.find((s) => s.id === a.id)) - scenarios.indexOf(scenarios.find((s) => s.id === b.id)));
writeFileSync(join(outDir, 'results.json'), JSON.stringify(results, null, 1));
writeReport(outDir, suiteName, results);
const count = (st) => results.filter((r) => r.status === st).length;
console.log(`\n${suiteName}: ${count('match')}/${results.length} match, ${count('geometry')} geometry(±1 caret), ${count('known')} known quirk, ${count('diff')} diff, ${count('setup')} setup, ${count('flaky')} flaky, ${count('error')} error  (${((Date.now() - t0) / 1000).toFixed(0)}s)`);
console.log(`report: ${join(outDir, 'report.html')}`);
process.exit(0);
