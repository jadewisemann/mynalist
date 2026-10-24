// Visual/layout parity: same fixture in both apps, measure computed styles and geometry of the same
// elements, compare with tolerances; writes qa/out/layout/report.html with full screenshots.
// node qa/layout.mjs [--record | --replay]   (--record saves Dynalist's metrics to qa/golden/layout.json;
// --replay measures only our build against them and exits 1 if a metric is out of tolerance)

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { APPS } from './lib/apps.mjs';
import { GOLDEN_DIR } from './lib/golden.mjs';
import { closeBrowser, getBrowser, newPage, QA_NOW, step } from './lib/runner.mjs';

const FX = [
  'plain item',
  '  child item',
  '    grandchild item',
  'heading one',
  'heading two',
  'heading three',
  'checked item',
  'item with note',
  '**bold** __italic__ `code` [link](https://example.com) #tag !(2099-12-31)',
  'checkbox item',
  'last plain item',
].join('\n');
const SETUP = [['heading', 3, 1], ['heading', 4, 2], ['heading', 5, 3], ['check', 6], ['note', 7, 'a note under the item'], ['checkbox', 9]];

const SEL = {
  dynalist: {
    nodes: '.Node.is-currentRoot .Node:not(.is-currentRoot)',
    self: ':scope > .Node-self',
    content: ':scope > .Node-self .Node-renderedContent',
    note: ':scope > .Node-self .Node-renderedNote',
    bullet: ':scope > .Node-self > .Node-bullet',
    check: ':scope > .Node-self > .Node-checkbox',
    title: '.Node.is-currentRoot > .Node-self .Node-content',
    inline: { bold: '.node-bold', italic: '.node-italics', code: '.node-inline-code', link: '.node-link', tag: '.node-tag', date: '.node-time' },
  },
  ours: {
    nodes: '.o-items .o-node',
    self: ':scope > .o-row',
    content: ':scope > .o-row .o-content',
    note: ':scope > .o-row .o-note',
    bullet: ':scope > .o-row > .o-bullet',
    check: ':scope > .o-row > .o-check',
    title: '.o-title .o-content',
    inline: { bold: '.o-bold', italic: '.o-italic', code: '.o-code', link: '.o-link', tag: '.o-tag', date: '.o-date' },
  },
};

function measure(S) {
  const nodes = [...document.querySelectorAll(S.nodes)].filter((n) => n.getClientRects().length);
  const q = (n, sel) => n?.querySelector(sel);
  const cs = (el, pseudo) => (el ? getComputedStyle(el, pseudo) : null);
  const rect = (el) => el?.getBoundingClientRect();
  const textRect = (el) => {
    // first line box of the text (not the element box, which may be padded)
    if (!el) return null;
    const r = document.createRange();
    r.selectNodeContents(el);
    return r.getClientRects()[0] ?? el.getBoundingClientRect();
  };
  const font = (el) => {
    const c = cs(el);
    return c && { size: parseFloat(c.fontSize), weight: +c.fontWeight, family: c.fontFamily.split(',')[0].replace(/["']/g, '').trim(), lineHeight: c.lineHeight === 'normal' ? null : parseFloat(c.lineHeight), color: c.color, style: c.fontStyle, decoration: c.textDecorationLine };
  };
  const out = {};
  const content = (i) => q(nodes[i], S.content);
  const t = document.querySelector(S.title);
  out.title = { ...font(t), height: rect(t)?.height };
  out.base = font(content(0));
  const c0 = textRect(content(0));
  const c1 = textRect(content(1));
  const c2 = textRect(content(2));
  out.indent = { level1: c1.left - c0.left, level2: c2.left - c1.left };
  const tops = [0, 1, 2, 10].map((i) => textRect(content(i))?.top);
  out.pitch = { 'item→child': tops[1] - tops[0], 'child→grandchild': tops[2] - tops[1] };
  out.titleGap = { 'title bottom→first item text': c0.top - rect(t).bottom, 'title text left − item text left': textRect(t).left - c0.left };
  const b = q(nodes[0], S.bullet);
  const dot = (() => {
    // the dot is either the element itself or a pseudo element: take whichever is a small round box
    for (const pseudo of [undefined, '::before', '::after']) {
      const c = cs(b, pseudo);
      const w = parseFloat(c.width);
      if (w > 2 && w < 12 && c.backgroundColor !== 'rgba(0, 0, 0, 0)') return { w, h: parseFloat(c.height), color: c.backgroundColor };
    }
    return null;
  })();
  const br = rect(b);
  out.bullet = { dot, 'center x → text left': c0.left - (br.left + br.width / 2), 'center y − text line center': br.top + br.height / 2 - (c0.top + c0.height / 2) };
  for (const [k, i] of [['h1', 3], ['h2', 4], ['h3', 5]]) out[k] = font(content(i));
  out.checked = { ...font(content(6)), opacity: +cs(q(nodes[6], S.self)).opacity * +cs(nodes[6]).opacity };
  const note = q(nodes[7], S.note);
  out.note = { ...font(note), 'gap below content': textRect(note).top - textRect(content(7)).bottom };
  const box = q(nodes[9], S.check);
  out.checkbox = { size: rect(box)?.width, 'box right → text left': textRect(content(9)).left - rect(box).right };
  const line = content(8);
  for (const [k, sel] of Object.entries(S.inline)) {
    const el = line.querySelector(sel);
    const c = cs(el);
    out[k] = el && { ...font(el), background: c.backgroundColor, padding: c.padding, radius: c.borderRadius, border: c.borderTopWidth + ' ' + c.borderTopColor };
  }
  return out;
}

// --- comparison -------------------------------------------------------------------------------
const rgb = (s) => (s?.match(/[\d.]+/g) ?? []).map(Number);
function cmp(a, b, key) {
  if (a == null) return true; // not measurable on Dynalist's side
  if (b == null) return false;
  if (typeof a === 'number') return Math.abs(a - b) <= Math.max(1.5, Math.abs(a) * 0.08);
  if (/rgb/.test(a)) {
    const [x, y] = [rgb(a), rgb(b)];
    if ((x[3] ?? 1) === 0 && (y[3] ?? 1) === 0) return true;
    return Math.hypot(x[0] - y[0], x[1] - y[1], x[2] - y[2]) < 40 && Math.abs((x[3] ?? 1) - (y[3] ?? 1)) < 0.2;
  }
  if (key === 'family') return true; // fonts are deliberately open-source substitutes
  return a === b;
}

const outDir = join(dirname(fileURLToPath(import.meta.url)), 'out', 'layout');
mkdirSync(outDir, { recursive: true });
const mode = process.argv.includes('--replay') ? 'replay' : process.argv.includes('--record') ? 'record' : 'live';
const goldenFile = join(GOLDEN_DIR, 'layout.json');
const b = await getBrowser();
const res = {};
if (mode === 'replay') {
  if (!existsSync(goldenFile)) throw new Error(`no recording: run node qa/layout.mjs --record first`);
  res.dynalist = JSON.parse(readFileSync(goldenFile, 'utf8')).dynalist;
}
for (const name of mode === 'replay' ? ['ours'] : ['dynalist', 'ours']) {
  const app = APPS[name];
  const { ctx, page } = await newPage(b);
  page.setDefaultTimeout(10000);
  const out = {};
  await app.fresh(page);
  await step(app, page, { paste: FX }, out);
  const { focusAt, press } = await import('./lib/runner.mjs');
  for (const [op, i, arg] of [...SETUP].sort((x, y) => y[1] - x[1])) {
    await focusAt(app, page, i);
    if (op === 'heading') for (let k = 0; k < arg; k++) await press(page, 'Control+Shift+H');
    if (op === 'check') await press(page, 'Control+Enter');
    if (op === 'checkbox') await press(page, 'Control+Shift+C');
    if (op === 'note') {
      await press(page, 'Shift+Enter');
      await page.keyboard.type(arg);
      await press(page, 'Shift+Enter');
    }
  }
  await press(page, 'Escape');
  await press(page, 'Escape');
  await page.evaluate(() => document.activeElement?.blur());
  await page.mouse.move(1270, 890);
  await page.waitForTimeout(400);
  res[name] = await page.evaluate(measure, SEL[name]);
  writeFileSync(join(outDir, `${name}.png`), await page.screenshot());
  await ctx.close();
}
await closeBrowser();
if (mode === 'record') {
  mkdirSync(GOLDEN_DIR, { recursive: true });
  const meta = { source: 'https://dynalist.io/demo', recorded: new Date().toISOString(), now: new Date(QA_NOW).toISOString() };
  writeFileSync(goldenFile, JSON.stringify({ meta, dynalist: res.dynalist }, null, 1) + '\n');
}

const rows = [];
for (const group of Object.keys(res.dynalist)) {
  const A = res.dynalist[group] ?? {};
  const B = res.ours[group] ?? {};
  for (const key of new Set([...Object.keys(A ?? {}), ...Object.keys(B ?? {})])) {
    let a = A?.[key];
    let bb = B?.[key];
    if (a && typeof a === 'object') {
      for (const k2 of Object.keys(a)) rows.push({ m: `${group}.${key}.${k2}`, a: a[k2], b: bb?.[k2], ok: cmp(a[k2], bb?.[k2], k2) });
      continue;
    }
    rows.push({ m: `${group}.${key}`, a, b: bb, ok: cmp(a, bb, key) });
  }
}
const fmt = (v) => (typeof v === 'number' ? +v.toFixed(2) : v ?? '—');
const bad = rows.filter((r) => !r.ok);
console.log(`layout: ${rows.length - bad.length}/${rows.length} metrics within tolerance`);
for (const r of bad) console.log(`  ✗ ${r.m.padEnd(40)} dyn=${fmt(r.a)}  ours=${fmt(r.b)}`);
writeFileSync(join(outDir, 'results.json'), JSON.stringify({ res, rows }, null, 1));
const esc = (s) => String(s).replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c]);
writeFileSync(
  join(outDir, 'report.html'),
  `<!doctype html><meta charset="utf-8"><title>layout parity</title><style>body{font:14px system-ui;margin:24px}td{padding:2px 10px;font-family:ui-monospace,monospace;font-size:12px}tr.bad{background:#ffe3e3}.sbs{display:grid;grid-template-columns:1fr 1fr;gap:10px}img{max-width:100%;border:1px solid #ddd}</style>
<h1>Layout parity: ${rows.length - bad.length}/${rows.length} within tolerance</h1>
<div class="sbs"><div><h3>Dynalist</h3><img src="dynalist.png" onerror="this.replaceWith('(no screenshot in a replay)')"></div><div><h3>Ours</h3><img src="ours.png"></div></div>
<table><tr><th>metric</th><th>Dynalist</th><th>ours</th></tr>${rows.map((r) => `<tr class="${r.ok ? '' : 'bad'}"><td>${esc(r.m)}</td><td>${esc(fmt(r.a))}</td><td>${esc(fmt(r.b))}</td></tr>`).join('')}</table>`,
);
console.log(`report: ${join(outDir, 'report.html')}`);
process.exit(mode === 'replay' && bad.length ? 1 : 0);
