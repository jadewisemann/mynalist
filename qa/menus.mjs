// Item context menu parity: the same outline in both apps, right-click a bullet in several item states,
// compare the menu's groups, labels (in order) and shortcut hints. Writes qa/out/menus/report.txt.
// node qa/menus.mjs [--record | --replay]   (--record saves Dynalist's menus to qa/golden/menus.json;
// --replay compares only our build against them and exits 1 on any difference not listed in OURS_EXTRA)

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { APPS } from './lib/apps.mjs';
import { GOLDEN_DIR } from './lib/golden.mjs';
import { closeBrowser, getBrowser, newPage, press, focusAt, step } from './lib/runner.mjs';

const FX = 'parent\n  child\nleaf\ndone\nboxed\nfolded\n  kid';
const SETUP = [['check', 3], ['checkbox', 4], ['collapse', 5]];
/** context name → visible item index to right-click */
const CONTEXTS = [['parent', 0], ['child', 1], ['leaf', 2], ['checked', 3], ['checkbox', 4], ['collapsed', 5]];

/** Our additions (not in Dynalist's menu); ignored when comparing. */
const OURS_EXTRA = new Set(['Insert template…', 'Duplicate', 'Uncheck all children', 'Share…']);

const SEL = {
  dynalist: { bullet: ':scope > .Node-self .Node-bullet', nodes: '.Node.is-currentRoot .Node:not(.is-currentRoot)' },
  ours: { bullet: ':scope > .o-row > .o-bullet', nodes: '.o-items .o-node' },
};

/** Menu as groups of [label, shortcut]; colour / heading pickers become a single marker. */
const READ = {
  dynalist: () => {
    const m = [...document.querySelectorAll('.ContextMenu.NodeMenu')].find((x) => getComputedStyle(x).display !== 'none' && x.getBoundingClientRect().width > 0);
    if (!m) return null;
    const groups = [];
    for (const g of m.querySelectorAll(':scope > .MenuGroup')) {
      const rows = [...g.querySelectorAll('.MenuItem')].filter((e) => e.getClientRects().length);
      if (!rows.length) continue;
      const first = rows[0].innerText.replace(/\s+/g, '');
      if (/^A{5,}/.test(first)) groups.push([['<colors>', '']]);
      else if (/^H1H2H3/.test(first)) groups.push([['<headings>', '']]);
      else groups.push(rows.map((e) => {
        const parts = [...e.children].map((c) => c.innerText.trim()).filter(Boolean);
        const key = parts.length > 1 ? parts[parts.length - 1] : '';
        return [parts[0] ?? '', key === 'Blank' ? '' : key]; // "Blank": Dynalist's placeholder for an unset shortcut
      }));
    }
    return groups;
  },
  ours: () => {
    const m = document.querySelector('body > .menu');
    if (!m) return null;
    const groups = [[]];
    for (const el of m.children) {
      const cur = groups[groups.length - 1];
      if (el.classList.contains('sep')) groups.push([]);
      else if (el.classList.contains('swatches')) {
        if (cur.length) groups.push([]);
        groups[groups.length - 1].push([/H1/.test(el.textContent) ? '<headings>' : '<colors>', '']);
        groups.push([]);
      } else {
        const spans = el.querySelectorAll(':scope > span');
        const key = spans[1]?.textContent.trim() ?? '';
        cur.push([spans[0]?.textContent.trim() ?? '', key === '▸' || /^\d/.test(key) ? '' : key]);
      }
    }
    return groups.filter((g) => g.length);
  },
};

async function collect(name) {
  const app = APPS[name];
  const b = await getBrowser();
  const { ctx, page } = await newPage(b);
  page.setDefaultTimeout(10000);
  const out = {};
  try {
    await app.fresh(page);
    await step(app, page, { paste: FX }, {});
    for (const [op, i] of [...SETUP].sort((x, y) => y[1] - x[1])) {
      if (op === 'check') await focusAt(app, page, i), await press(page, 'Control+Enter');
      if (op === 'checkbox') await focusAt(app, page, i), await press(page, 'Control+Shift+C');
      if (op === 'collapse') await focusAt(app, page, i), await press(page, 'Control+Period');
    }
    await press(page, 'Escape');
    await press(page, 'Escape');
    for (const [ctxName, i] of CONTEXTS) {
      const node = page.locator(SEL[name].nodes).filter({ visible: true }).nth(i);
      await node.locator(SEL[name].bullet).first().click({ button: 'right' });
      await page.waitForTimeout(400);
      out[ctxName] = await page.evaluate(READ[name]);
      await press(page, 'Escape');
      await page.mouse.click(1100, 850);
      await page.waitForTimeout(250);
    }
  } finally {
    await ctx.close();
  }
  return out;
}

const mode = process.argv.includes('--replay') ? 'replay' : process.argv.includes('--record') ? 'record' : 'live';
const goldenFile = join(GOLDEN_DIR, 'menus.json');
const res = {};
if (mode === 'replay') {
  if (!existsSync(goldenFile)) throw new Error('no recording: run node qa/menus.mjs --record first');
  res.dynalist = JSON.parse(readFileSync(goldenFile, 'utf8')).dynalist;
}
for (const name of mode === 'replay' ? ['ours'] : ['dynalist', 'ours']) res[name] = await collect(name);
await closeBrowser();
if (mode === 'record') {
  writeFileSync(goldenFile, JSON.stringify({ meta: { source: 'https://dynalist.io/demo', recorded: new Date().toISOString() }, dynalist: res.dynalist }, null, 1) + '\n');
}

const show = (g) => g.map((grp) => grp.map(([l, k]) => (k ? `${l} [${k}]` : l)).join(' · ')).join('  ‖  ');
const lines = [];
let bad = 0;
for (const [ctxName] of CONTEXTS) {
  const A = res.dynalist[ctxName];
  const B = (res.ours[ctxName] ?? []).map((g) => g.filter(([l]) => !OURS_EXTRA.has(l))).filter((g) => g.length);
  const ok = JSON.stringify(A) === JSON.stringify(B);
  if (!ok) bad++;
  lines.push(`${ok ? 'ok  ' : 'DIFF'} ${ctxName}`);
  if (!ok) lines.push(`     dynalist: ${show(A ?? [])}`, `     ours:     ${show(B)}`);
}
const dir = join(import.meta.dirname, 'out', 'menus');
mkdirSync(dir, { recursive: true });
writeFileSync(join(dir, 'report.txt'), lines.join('\n') + '\n');
console.log(lines.join('\n'));
console.log(`menus: ${CONTEXTS.length - bad}/${CONTEXTS.length} match`);
process.exit(mode !== 'live' && bad ? 1 : 0);
