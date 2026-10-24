// Print side-by-side outlines of non-matching scenarios: node qa/show.mjs <suite> [substr]
import { readFileSync } from 'node:fs';
import { outline } from './lib/state.mjs';
const [suite = 'editing', only] = process.argv.slice(2);
const res = JSON.parse(readFileSync(new URL(`./out/${suite}/results.json`, import.meta.url)));
const st = (s) => (typeof s === 'string' ? s : s.type != null ? `"${s.type}"` : s.at ? `@${s.at.join(':')}` : JSON.stringify(s));
for (const r of res) {
  if (r.status === 'match' || (only && !r.id.includes(only))) continue;
  console.log(`\n## ${r.id} [${r.kinds}]  ${(r.fx ?? '').replace(/\n/g, '⏎')}  ${r.setup ? JSON.stringify(r.setup) : ''}\n   ${r.steps.map(st).join(' → ')}`);
  let A = r.status === 'setup' ? r.dyn.pre : r.dyn.post, B = r.status === 'setup' ? r.ours.pre : r.ours.post;
  if (r.trace && r.dyn.trace) {
    const k = r.dyn.trace.findIndex((st, i) => JSON.stringify(outline(st)) !== JSON.stringify(outline(r.ours.trace[i])));
    if (k > 0) {
      console.log(`   before step ${k + 1} (same in both):`);
      for (const l of outline(r.dyn.trace[k - 1])) console.log('     ' + l);
      console.log(`   after step ${k + 1} ${JSON.stringify(r.steps[k])}:`);
    }
    if (k >= 0) { A = r.dyn.trace[k]; B = r.ours.trace[k]; }
  }
  const a = outline(A), b = outline(B);
  for (let i = 0; i < Math.max(a.length, b.length); i++) console.log(`${a[i] === b[i] ? ' ' : '≠'}  ${(a[i] ?? '').padEnd(38)} │ ${b[i] ?? ''}`);
  if (r.dyn.clip != null || r.ours.clip != null) console.log('   clip dyn:', JSON.stringify(r.dyn.clip), JSON.stringify(r.dyn.clipHtml), '\n   clip ours:', JSON.stringify(r.ours.clip), JSON.stringify(r.ours.clipHtml));
  for (const x of [r.dyn, r.ours]) if (x.errors?.length) console.log('   errors:', x.errors);
}
