// Mobile parity on a phone viewport (390×844, touch, Android Chrome user agent): the header buttons, the
// bottom editing toolbar (which buttons, in which order, where, how big, when it shows) and a few touch
// interactions both apps support (tap to edit, tap a bullet, toolbar commands, header undo/redo, select
// multiple, read mode), compared on the normalized editor state. Writes qa/out/mobile/report.txt.
// node qa/mobile.mjs [--record | --replay]   (--record saves Dynalist's side to qa/golden/mobile.json;
// --replay runs only our build against it and exits 1 on any difference not covered by OURS_EXTRA)

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { APPS } from './lib/apps.mjs';
import { GOLDEN_DIR } from './lib/golden.mjs';
import { closeBrowser, getBrowser, newPage, PHONE, press, step } from './lib/runner.mjs';
import { outline, same } from './lib/state.mjs';

const FX = 'alpha\nbeta\n  gamma\ndelta';

/**
 * Our intentional extras, ignored when comparing button lists:
 * - toolbar `customize`: Dynalist has it too, but only for Pro accounts (hidden in the demo); ours is always on.
 */
const OURS_EXTRA = { toolbar: new Set(['customize']), header: new Set() };

/** Intentional differences: check path → what ours shows instead, and why. Ours must show exactly that. */
const OURS_DIFFERENT = {
  'lock › locked › ui › header': {
    ours: 'menu, undo:disabled, select, search, lock:active, more',
    why: 'Undo is disabled in read mode, where nothing can change (Dynalist leaves it enabled)',
  },
};

/**
 * Dynalist's toolbar option classes (`mod-*`) and header option classes, mapped to our command / button ids.
 * Our toolbar buttons carry `data-cmd`, our header buttons `data-hb`.
 */
const DYN_TOOLBAR = {
  unindent: 'unindent', indent: 'indent', 'zoom-in': 'zoom-in', delete: 'delete-left', check: 'toggle-checked',
  note: 'goto-note', date: 'add-date', 'move-up': 'swap-up', 'move-down': 'swap-down', 'move-item': 'move-to',
  upload: 'upload', checklist: 'toggle-checkbox', color: 'color-menu', heading: 'heading-menu', 'get-link': 'get-link',
  customize: 'customize',
};
const DYN_HEADER = {
  mainMenuIcon: 'menu', undo: 'undo', redo: 'redo', selectMultiple: 'select', searchInDocument: 'search',
  toggleReadMode: 'lock', moreOptions: 'more',
};

const SEL = {
  dynalist: {
    toolbar: '.node-toolbar.mod-not-ios',
    tbButton: (id) => `.node-toolbar.mod-not-ios .node-toolbar-option.mod-${Object.keys(DYN_TOOLBAR).find((k) => DYN_TOOLBAR[k] === id)}`,
    hdButton: (id) => {
      const k = Object.keys(DYN_HEADER).find((x) => DYN_HEADER[x] === id);
      return k === 'mainMenuIcon' ? '.MobileHeader.is-active .MobileHeader-mainMenuIcon' : `.MobileHeader.is-active .MobileHeader-option--${k}`;
    },
    bullet: (page, i) => page.locator('.Node.is-currentRoot .Node:not(.is-currentRoot)').filter({ visible: true }).nth(i).locator(':scope > .Node-self .Node-bullet'),
  },
  ours: {
    toolbar: '.mobile-bar',
    tbButton: (id) => `.mobile-bar button[data-cmd="${id}"]`,
    hdButton: (id) => `header.topbar [data-hb="${id}"]`,
    bullet: (page, i) => page.locator('.o-items .o-node').filter({ visible: true }).nth(i).locator(':scope > .o-row > .o-bullet'),
  },
};

// ---- in-page readers (run in the browser; each gets the app name and the maps) ---------------------------

function readUi([app, DYN_TOOLBAR, DYN_HEADER]) {
  const shown = (e) => {
    if (!e) return false;
    const r = e.getBoundingClientRect();
    const s = getComputedStyle(e);
    return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none' && r.bottom > 0 && r.top < innerHeight;
  };
  const dyn = app === 'dynalist';
  const bar = document.querySelector(dyn ? '.node-toolbar.mod-not-ios' : '.mobile-bar');
  // the buttons only count while the toolbar is shown
  const toolbar = { shown: shown(bar) };
  if (toolbar.shown) {
    toolbar.buttons = [...bar.querySelectorAll(dyn ? '.node-toolbar-option' : 'button[data-cmd]')]
      .filter((e) => getComputedStyle(e).display !== 'none')
      .map((e) => (dyn ? DYN_TOOLBAR[e.className.match(/\bmod-(\S+)/)?.[1]] ?? '?' + e.className : e.dataset.cmd));
  }
  const hb = dyn
    ? [...document.querySelectorAll('.MobileHeader.is-active :is(.MobileHeader-mainMenuIcon, .MobileHeader-option)')]
    : [...document.querySelectorAll('header.topbar [data-hb]')];
  const header = hb.filter(shown).sort((a, b) => a.getBoundingClientRect().left - b.getBoundingClientRect().left).map((e) => {
    const id = dyn ? (e.classList.contains('MobileHeader-mainMenuIcon') ? 'menu' : DYN_HEADER[e.className.match(/MobileHeader-option--(\w+)/)?.[1]] ?? '?' + e.className) : e.dataset.hb;
    const disabled = dyn ? e.classList.contains('is-disabled') : e.disabled;
    const active = dyn ? e.classList.contains('is-active') : e.getAttribute('aria-pressed') === 'true';
    return id + (disabled ? ':disabled' : '') + (active ? ':active' : '');
  });
  return { toolbar, header };
}

/** Geometry of the header and (if shown) the toolbar, in CSS px. */
function readMetrics([app]) {
  const dyn = app === 'dynalist';
  const rect = (e) => e.getBoundingClientRect();
  const vis = (e) => rect(e).width > 0 && getComputedStyle(e).display !== 'none';
  const out = {};
  const head = document.querySelector(dyn ? 'header.AppHeader' : 'header.topbar');
  const hb = [...document.querySelectorAll(dyn ? '.MobileHeader.is-active :is(.MobileHeader-mainMenuIcon, .MobileHeader-option)' : 'header.topbar [data-hb]')]
    .filter(vis).sort((a, b) => rect(a).left - rect(b).left);
  const sel = document.querySelector(dyn ? '.MobileHeader-option--selectMultiple svg' : 'header.topbar [data-hb="select"] svg');
  out.header = {
    height: rect(head).height,
    'first button left': rect(hb[0]).left,
    'last button right gap': innerWidth - rect(hb[hb.length - 1]).right,
    'smallest button side': Math.min(...hb.map((e) => Math.min(rect(e).width, rect(e).height))),
    'icon size': sel ? rect(sel).width : null,
  };
  const bar = document.querySelector(dyn ? '.node-toolbar.mod-not-ios' : '.mobile-bar');
  if (bar && vis(bar)) {
    const b = rect(bar);
    const btns = [...bar.querySelectorAll(dyn ? '.node-toolbar-option' : 'button[data-cmd]')].filter(vis);
    const b0 = rect(btns[0]);
    const scroller = dyn ? bar.querySelector('.node-toolbar-options-container') : bar;
    const doc = document.querySelector(dyn ? '.DocumentContainer' : '.doc-scroll');
    out.toolbar = {
      'bottom gap': innerHeight - b.bottom,
      left: b.left,
      'width − viewport': b.width - innerWidth,
      height: b.height,
      'first button left': b0.left - b.left,
      'button width': b0.width,
      'button visible height': Math.min(b0.bottom, b.bottom) - Math.max(b0.top, b.top),
      'icon size': rect(btns[0].querySelector('svg') ?? btns[0]).width,
      'scrolls sideways': scroller.scrollWidth > scroller.clientWidth,
      'document ends at toolbar': Math.round(b.top - rect(doc).bottom),
      background: getComputedStyle(bar).backgroundColor,
    };
  }
  return out;
}

function readPicker([app]) {
  if (app === 'dynalist') {
    const m = document.querySelector('.mobile-popup-menu.is-active');
    return m ? [...m.querySelectorAll('.mobile-popup-menu-option, .mobile-popup-menu-cancel-button')].filter((e) => e.getClientRects().length).map((e) => e.innerText.trim()) : null;
  }
  const m = document.querySelector('.sheet');
  return m ? [...m.querySelectorAll('button')].map((e) => e.textContent.trim()) : null;
}

// ---- driving ----------------------------------------------------------------------------------------------

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function setup(name) {
  const app = APPS[name];
  const b = await getBrowser();
  const { ctx, page } = await newPage(b, PHONE.viewport, PHONE.device);
  page.setDefaultTimeout(10000);
  await app.fresh(page);
  await step(app, page, { paste: FX }, {});
  await press(page, 'Escape');
  await press(page, 'Escape');
  await page.evaluate(() => document.activeElement?.blur());
  await sleep(300);
  return { app, ctx, page, S: SEL[name] };
}

/** Tap item i (-1 = title) to the right of its text: the caret goes to the end. */
async function tapItem(t, i) {
  const box = await (await t.app.item(t.page, i)).boundingBox();
  await t.page.touchscreen.tap(box.x + box.width - 15, box.y + Math.min(box.height / 2, 11));
  await sleep(500);
}
const tapSel = async (t, sel) => (await t.page.locator(sel).first().tap(), sleep(600));
const ui = (t) => t.page.evaluate(readUi, [t.app.name, DYN_TOOLBAR, DYN_HEADER]);
const state = (t) => t.app.state(t.page);

/** Each probe runs on a fresh page and returns plain data; the same probe runs on both apps. */
const PROBES = {
  /** header buttons, when the toolbar shows, its buttons and geometry, keyboard emulation, header undo/redo */
  async chrome(t) {
    const out = { idle: await ui(t) };
    await tapItem(t, -1);
    out.title = { ui: await ui(t), state: await state(t) };
    await tapItem(t, 1);
    out.item = { ui: await ui(t), state: await state(t), metrics: await t.page.evaluate(readMetrics, [t.app.name]) };
    // a virtual keyboard shrinks the layout viewport (Android Chrome's default): the toolbar must stay on top of it
    await t.page.setViewportSize({ width: PHONE.viewport.width, height: 500 });
    await sleep(500);
    const kb = await t.page.evaluate(readMetrics, [t.app.name]);
    out.keyboard = { shown: (await ui(t)).toolbar.shown, 'bottom gap': kb.toolbar?.['bottom gap'] ?? null };
    await t.page.setViewportSize(PHONE.viewport);
    await sleep(400);
    await t.page.keyboard.type('xy', { delay: 40 });
    await sleep(400);
    await tapSel(t, t.S.hdButton('undo'));
    out.undo = { ui: await ui(t), state: await state(t) };
    await tapSel(t, t.S.hdButton('redo'));
    out.redo = { ui: await ui(t), state: await state(t) };
    return out;
  },
  /** tapping a bullet zooms in */
  async bullet(t) {
    await t.S.bullet(t.page, 1).tap();
    await sleep(600);
    return { ui: await ui(t), state: await state(t) };
  },
  /** header "select multiple": tapping items selects them, the toolbar acts on the selection */
  async select(t) {
    const out = {};
    await tapSel(t, t.S.hdButton('select'));
    out.on = { ui: await ui(t), state: await state(t) };
    await tapItem(t, 1);
    await tapItem(t, 3);
    out.tapped = { ui: await ui(t), state: await state(t) };
    await tapSel(t, t.S.tbButton('indent'));
    out.indent = { ui: await ui(t), state: await state(t) };
    await tapSel(t, t.S.hdButton('select'));
    out.off = { ui: await ui(t), state: await state(t) };
    return out;
  },
  /** header lock: read mode, tapping an item doesn't edit it */
  async lock(t) {
    const out = {};
    await tapSel(t, t.S.hdButton('lock'));
    await tapItem(t, 1);
    out.locked = { ui: await ui(t), state: await state(t) };
    await tapSel(t, t.S.hdButton('lock'));
    await tapItem(t, 1);
    out.unlocked = { ui: await ui(t), state: await state(t) };
    return out;
  },
  /** the heading and colour buttons open a picker */
  async pickers(t) {
    const out = {};
    await tapItem(t, 1);
    await tapSel(t, t.S.tbButton('color-menu'));
    out.color = await t.page.evaluate(readPicker, [t.app.name]);
    await t.page.getByText('Cancel', { exact: true }).filter({ visible: true }).tap();
    await sleep(500);
    await tapItem(t, 1);
    await tapSel(t, t.S.tbButton('heading-menu'));
    out.heading = await t.page.evaluate(readPicker, [t.app.name]);
    await t.page.getByText('Heading 2', { exact: true }).filter({ visible: true }).tap();
    await sleep(500);
    out.afterHeading = { ui: await ui(t), state: await state(t) };
    return out;
  },
};
/** toolbar commands: [command id, item to tap first] */
const EFFECTS = [
  ['indent', 1], ['unindent', 2], ['zoom-in', 1], ['delete-left', 1], ['toggle-checked', 1], ['goto-note', 1],
  ['swap-up', 1], ['swap-down', 1], ['toggle-checkbox', 1],
];
for (const [cmd, i] of EFFECTS) {
  PROBES[`toolbar ${cmd}`] = async (t) => {
    await tapItem(t, i);
    await tapSel(t, t.S.tbButton(cmd));
    return { ui: await ui(t), state: await state(t) };
  };
}

async function collect(name, only) {
  const out = {};
  for (const [probe, fn] of Object.entries(PROBES)) {
    if (only && !only.some((o) => probe.includes(o))) continue;
    const t = await setup(name);
    try {
      out[probe] = await fn(t);
    } catch (e) {
      out[probe] = { error: String(e.message ?? e).split('\n')[0] };
    } finally {
      await t.ctx.close();
    }
  }
  return out;
}

// ---- comparison -------------------------------------------------------------------------------------------

const rgb = (s) => (s?.match(/[\d.]+/g) ?? []).map(Number);
function near(a, b) {
  if (a == null) return true;
  if (typeof a === 'number') return typeof b === 'number' && Math.abs(a - b) <= Math.max(1.5, Math.abs(a) * 0.08);
  if (typeof a === 'string' && /rgb/.test(a)) {
    const [x, y] = [rgb(a), rgb(b)];
    return y.length >= 3 && Math.hypot(x[0] - y[0], x[1] - y[1], x[2] - y[2]) < 40;
  }
  return a === b;
}

/** Flatten both sides into [path, dyn, ours, ok] rows. */
function compare(A, B, path = [], rows = []) {
  const key = path[path.length - 1];
  if (A && typeof A === 'object' && 'items' in A && 'focus' in A) {
    rows.push([path.join(' › '), outline(A).join(' / '), outline(B).join(' / '), same(A, B)]);
  } else if (Array.isArray(A)) {
    const kind = path.includes('header') ? 'header' : path.includes('toolbar') || key === 'buttons' ? 'toolbar' : null;
    const b = (B ?? []).filter((x) => !kind || !OURS_EXTRA[kind].has(String(x).split(':')[0]));
    rows.push([path.join(' › '), A.join(', '), b.join(', '), JSON.stringify(A) === JSON.stringify(b)]);
  } else if (A && typeof A === 'object') {
    for (const k of Object.keys(A)) compare(A[k], B?.[k], [...path, k], rows);
  } else {
    rows.push([path.join(' › '), A, B, near(A, B)]);
  }
  return rows;
}

const mode = process.argv.includes('--replay') ? 'replay' : process.argv.includes('--record') ? 'record' : 'live';
const only = process.argv.find((a) => a.startsWith('--only='))?.slice(7).split(',');
const goldenFile = join(GOLDEN_DIR, 'mobile.json');
const res = {};
if (mode === 'replay') {
  if (!existsSync(goldenFile)) throw new Error('no recording: run node qa/mobile.mjs --record first');
  res.dynalist = JSON.parse(readFileSync(goldenFile, 'utf8')).dynalist;
}
for (const name of mode === 'replay' ? ['ours'] : ['dynalist', 'ours']) res[name] = await collect(name, only);
await closeBrowser();
if (only) for (const k of Object.keys(res.dynalist)) if (!only.some((o) => k.includes(o))) delete res.dynalist[k];
if (mode === 'record') {
  if (only) throw new Error('--record needs the full run (no --only)');
  const errs = Object.entries(res.dynalist).filter(([, v]) => v?.error);
  if (errs.length) throw new Error(`Dynalist probes failed, not recorded: ${errs.map(([k, v]) => `${k}: ${v.error}`).join('; ')}`);
  writeFileSync(goldenFile, JSON.stringify({ meta: { source: 'https://dynalist.io/demo', recorded: new Date().toISOString(), viewport: PHONE.viewport }, dynalist: res.dynalist }, null, 1) + '\n');
}

const lines = [];
let bad = 0;
let total = 0;
for (const probe of Object.keys(res.dynalist)) {
  const rows = compare(res.dynalist[probe], res.ours[probe], [probe]);
  for (const [p, a, b, ok] of rows) {
    total++;
    const known = OURS_DIFFERENT[p];
    if (known && !ok && String(b) === known.ours) {
      lines.push(`ok   ${p}  (intentional: ${known.why})`);
      continue;
    }
    if (!ok) bad++;
    lines.push(`${ok ? 'ok  ' : 'DIFF'} ${p}`);
    if (!ok) lines.push(`     dynalist: ${a}`, `     ours:     ${b}`);
  }
}
const dir = join(import.meta.dirname, 'out', 'mobile');
mkdirSync(dir, { recursive: true });
writeFileSync(join(dir, 'report.txt'), lines.join('\n') + '\n');
writeFileSync(join(dir, 'results.json'), JSON.stringify(res, null, 1));
const shown = lines.filter((l) => process.argv.includes('--verbose') || !l.startsWith('ok ') || l.includes('(intentional'));
if (shown.length) console.log(shown.join('\n'));
console.log(`mobile: ${total - bad}/${total} checks match`);
process.exit(mode !== 'live' && bad ? 1 : 0);
