// Self-contained HTML report: summary per group, then each scenario with Dynalist | ours side by side.

import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { outline } from './state.mjs';

const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);

function sideBySide(a, b) {
  const A = outline(a);
  const B = outline(b);
  const n = Math.max(A.length, B.length);
  const col = (L, O) => L.map((l, i) => `<div class="${l === O[i] ? '' : 'bad'}">${esc(l) || '&nbsp;'}</div>`).join('') + '<div>&nbsp;</div>'.repeat(n - L.length);
  return `<div class="sbs"><div><h4>Dynalist</h4>${col(A, B)}</div><div><h4>Ours</h4>${col(B, A)}</div></div>`;
}

// verdicts that count as parity (live: match/geometry/known; replay adds fixed)
const OK = new Set(['match', 'geometry', 'known', 'fixed', 'skipped']);

const stepText = (s) => (typeof s === 'string' ? s : s.type != null ? `type "${s.type}"` : s.at ? `caret ${JSON.stringify(s.at)}` : JSON.stringify(s));

export function writeReport(dir, suite, results) {
  const groups = new Map();
  for (const r of results) {
    const g = groups.get(r.group) ?? { match: 0, total: 0 };
    g.total++;
    if (OK.has(r.status)) g.match++;
    groups.set(r.group, g);
  }
  const total = results.length;
  const match = results.filter((r) => OK.has(r.status)).length;
  const rows = [...groups].map(([g, c]) => `<tr><td>${esc(g)}</td><td>${c.match}/${c.total}</td><td><div class="bar"><span style="width:${(100 * c.match) / c.total}%"></span></div></td></tr>`).join('');
  const cards = results
    .map((r) => {
      const extra = [];
      if (r.dyn.clip != null || r.ours.clip != null) extra.push(`<div class="sbs"><pre>${esc(r.dyn.clip ?? '')}</pre><pre>${esc(r.ours.clip ?? '')}</pre></div>`);
      for (const [n, x] of [['Dynalist', r.dyn], ['Ours', r.ours]]) {
        if (x.fatal) extra.push(`<p class="err">${n} fatal: ${esc(x.fatal)}</p>`);
        if (x.errors?.length) extra.push(`<p class="err">${n} page errors: ${esc(x.errors.join(' | '))}</p>`);
      }
      return `<section class="sc ${r.status}" id="${esc(r.file)}">
<h3><span class="st">${r.status}</span> ${esc(r.id)} ${r.kinds.map((k) => `<span class="k">${k}</span>`).join('')}</h3>
<p class="steps">fixture: <code>${esc((r.fx ?? '').replace(/\n/g, ' ⏎ '))}</code>${r.setup?.length ? ` · setup: <code>${esc(JSON.stringify(r.setup))}</code>` : ''}<br>steps: ${r.steps.map((s) => `<code>${esc(stepText(s))}</code>`).join(' → ')}</p>
${r.status === 'setup' ? `<h4>before (setup differs)</h4>${sideBySide(r.dyn.pre, r.ours.pre)}` : ''}
${sideBySide(r.dyn.post, r.ours.post)}
${extra.join('')}
<details${r.status === 'match' ? '' : ' open'}><summary>screenshots</summary><div class="sbs"><img loading="lazy" src="shots/${r.file}.dyn.jpg" onerror="this.replaceWith('(no Dynalist screenshot in a replay)')"><img loading="lazy" src="shots/${r.file}.ours.jpg"></div></details>
</section>`;
    })
    .join('\n');
  const html = `<!doctype html><meta charset="utf-8"><title>QA ${esc(suite)}: ${match}/${total}</title>
<style>
body{font:14px/1.45 system-ui,sans-serif;margin:24px;color:#222;max-width:1400px}
table{border-collapse:collapse}td{padding:2px 10px}.bar{width:200px;height:8px;background:#f3d6d6}.bar span{display:block;height:8px;background:#3a9d5d}
.sc{border:1px solid #ddd;border-radius:6px;padding:8px 14px;margin:14px 0}.sc.match{border-color:#cfe8d6}
.sc h3{margin:4px 0;font-size:15px}.st{display:inline-block;min-width:52px;font-size:12px;text-transform:uppercase;padding:1px 6px;border-radius:4px;background:#eee}
.diff .st,.setup .st,.regression .st,.unrecorded .st{background:#f8d7d7}.known-diff .st{background:#f3e3f3}.fixed .st{background:#d7f0de}.geometry .st,.known .st{background:#e3ecfa}.match .st{background:#d7f0de}.error .st,.flaky .st{background:#fde9b8}
.k{font-size:12px;background:#eef;margin-left:6px;padding:1px 5px;border-radius:4px}
.steps{color:#555;margin:4px 0}.sbs{display:grid;grid-template-columns:1fr 1fr;gap:12px}
.sbs>div{font:13px/1.5 ui-monospace,monospace;white-space:pre;background:#fafafa;padding:6px;overflow:auto}.sbs h4{margin:0 0 4px;font:600 12px system-ui}
.bad{background:#ffe3e3}.err{color:#b00}img{max-width:100%;border:1px solid #eee}pre{background:#fafafa;padding:6px;margin:0}
#f{position:sticky;top:0;background:#fff;padding:6px 0}
</style>
<h1>Dynalist parity — ${esc(suite)}: ${match}/${total} ok</h1>
<table>${rows}</table>
<div id="f"><label><input type="checkbox" id="hide" checked> hide matches</label></div>
<style id="hs">.sc.match{display:none}</style>
<script>document.getElementById('hide').onchange=e=>document.getElementById('hs').disabled=!e.target.checked</script>
${cards}`;
  writeFileSync(join(dir, 'report.html'), html);
}
