// Render tests: drive the real editor through its public paths (commands, keys, typing, undo, zoom, search,
// remote sync) and check after every step that the DOM shows exactly what the model says (verify.ts), and now
// and then that it equals a from-scratch render of the same state. Driven by apps/web/e2e/render.mjs.

import '../../src/styles.css';
import {
  DocSession, type NodeId, type Op, ROOT, type Sel, Tx, backspaceAtStart, caret, compileQuery, enter, flatten,
  searchTree, selectBetween, visibleChildren, type Tree,
} from '@outliner/core';
import { Editor, type EditorHost } from '../../src/editor/editor.ts';
import { serializeRender, verifyRender } from '../../src/editor/verify.ts';

// ---- deterministic randomness ----------------------------------------------------------------------------

export type Rng = () => number;
export function rng(seed: number): Rng {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const int = (r: Rng, n: number) => Math.floor(r() * n);
const pick = <T>(r: Rng, xs: readonly T[]): T => xs[int(r, xs.length)]!;
const chance = (r: Rng, p: number) => r() < p;
function weighted<T>(r: Rng, xs: readonly [number, T][]): T {
  let x = r() * xs.reduce((s, [w]) => s + w, 0);
  for (const [w, v] of xs) if ((x -= w) < 0) return v;
  return xs[xs.length - 1]![1];
}

const WORDS = ['alpha', 'beta', 'gamma', 'delta', 'omega', 'kiwi', 'lime', 'plum', 'item', 'task', 'note', 'list'];
const MARKUP = ['**bold**', '__it__', '~~gone~~', '==mark==', '`code`', '#tag', '@who', '[link](https://e.com)', '!(2026-11-03)', 'http://x.io'];
function text(r: Rng): string {
  const n = 1 + int(r, 5);
  const out: string[] = [];
  for (let i = 0; i < n; i++) out.push(chance(r, 0.2) ? pick(r, MARKUP) : pick(r, WORDS));
  return out.join(' ');
}

// ---- the editor under test --------------------------------------------------------------------------------

const backlinks = new Map<NodeId, number>();
const host: EditorHost = {
  onZoom() {},
  onTitleEdit() {},
  onAppCommand: () => false,
  openLink() {},
  onTagClick() {},
  backlinkCounts: () => (backlinks.size ? new Map(backlinks) : null),
};

const stage = document.getElementById('stage')!;
let ed: Editor | null = null;

function freshEditor(session: DocSession): Editor {
  ed?.dispose();
  stage.textContent = '';
  backlinks.clear();
  ed = new Editor(host);
  ed.applyPrefs({ bulletThreading: true, highlightCurrent: true });
  stage.append(ed.el);
  ed.setDoc(session);
  return ed;
}

/** Problems with the tree itself (parent/children links). */
function verifyTree(t: Tree): string[] {
  const errs: string[] = [];
  const seen = new Set<NodeId>();
  const walk = (id: NodeId) => {
    if (seen.has(id)) return errs.push(`tree: ${id} reachable twice`);
    seen.add(id);
    for (const c of t.get(id)?.children ?? []) {
      const n = t.get(c);
      if (!n) errs.push(`tree: missing child ${c}`);
      else if (n.parent !== id) errs.push(`tree: ${c}.parent=${n.parent}, listed under ${id}`);
      else walk(c);
    }
  };
  walk(ROOT);
  return errs;
}

/** From-scratch render of the same state in a second editor; differences as text. */
function compareFresh(e: Editor): string[] {
  const x = e.debugInternals();
  const f = new Editor(host);
  // rendering needs a connected editor (rows that aren't connected are dropped)
  const box = document.createElement('div');
  box.className = 'doc-wrap';
  box.style.cssText = 'position: absolute; left: -10000px; top: 0; width: 900px;';
  box.append(f.el);
  document.body.append(box);
  try {
    f.applyPrefs(e.prefs);
    f.setDoc(e.docSession, e.view.zoom);
    f.view = { ...e.view };
    f.setFilter(e.view.filter ?? null, x.searchHl);
    f.refreshText(); // as the app does after a search
    const a = serializeRender(e).split('\n');
    const b = serializeRender(f).split('\n');
    const errs = verifyRender(f).map((m) => `fresh editor: ${m}`);
    for (let i = 0; i < Math.max(a.length, b.length) && errs.length < 6; i++) {
      if (a[i] !== b[i]) errs.push(`line ${i}: incremental ${JSON.stringify(a[i])} != fresh ${JSON.stringify(b[i])}`);
    }
    return errs;
  } finally {
    f.dispose();
    box.remove();
  }
}

// ---- documents --------------------------------------------------------------------------------------------

export type Shape = 'mixed' | 'flat' | 'deep' | 'wide';

/** Build a random document straight into the session (not undoable). */
function buildDoc(s: DocSession, r: Rng, n: number, shape: Shape) {
  const tx = new Tx(s.live);
  tx.set(ROOT, { c: `Doc ${shape}` });
  const stack: NodeId[] = [ROOT];
  const last = new Map<NodeId, NodeId>();
  for (let i = 0; i < n; i++) {
    const d = stack.length - 1;
    let to = d;
    if (shape === 'flat') to = chance(r, 0.95) ? Math.min(d, 1) : d + 1;
    else if (shape === 'deep') to = chance(r, 0.6) ? d + 1 : Math.max(1, d - int(r, 3));
    else if (shape === 'wide') to = chance(r, 0.15) ? d + 1 : Math.max(1, d - (chance(r, 0.1) ? 1 : 0));
    else to = Math.max(1, Math.min(d + 1, d + int(r, 3) - 1));
    if (i === 0) to = 1;
    while (stack.length - 1 >= to && stack.length > 1) stack.pop();
    while (stack.length - 1 < to - 1) stack.push(last.get(stack[stack.length - 1]!)!);
    const parent = stack[stack.length - 1]!;
    const props: Record<string, unknown> = { c: text(r) };
    if (chance(r, 0.12)) props.n = text(r);
    if (chance(r, 0.12)) props.cb = true;
    if (chance(r, 0.12)) props.ck = true;
    if (chance(r, 0.05)) props.h = 1 + int(r, 3);
    if (chance(r, 0.05)) props.cl = 1 + int(r, 6);
    if (chance(r, 0.04)) props.num = true;
    const id = tx.create(parent, last.get(parent) ?? null, props);
    last.set(parent, id);
    stack.push(id);
  }
  for (const [p] of last) if (p !== ROOT && chance(r, 0.1)) tx.collapse(p, true);
  s.commit(tx, { before: { k: 'none' }, after: { k: 'none' } }, { undoable: false });
}

/** Indented text → document. Markers: "[ ] " checkbox, "[x] " checked, "+" collapsed, "# " h1, "1. " numbered, " // note". */
function docFromSpec(spec: string): { s: DocSession; ids: Record<string, NodeId> } {
  const s = new DocSession();
  const tx = new Tx(s.live);
  tx.set(ROOT, { c: 'Spec' });
  const ids: Record<string, NodeId> = {};
  const stack: { d: number; id: NodeId }[] = [{ d: -1, id: ROOT }];
  const last = new Map<NodeId, NodeId>();
  const collapse: NodeId[] = [];
  for (const raw of spec.split('\n')) {
    if (!raw.trim()) continue;
    const d = raw.search(/\S/) / 2;
    let line = raw.trim();
    const props: Record<string, unknown> = {};
    let col = false;
    for (;;) {
      if (line.startsWith('[ ] ')) (props.cb = true), (line = line.slice(4));
      else if (line.startsWith('[x] ')) (props.cb = props.ck = true), (line = line.slice(4));
      else if (line.startsWith('+')) (col = true), (line = line.slice(1));
      else if (line.startsWith('# ')) (props.h = 1), (line = line.slice(2));
      else if (line.startsWith('1. ')) (props.num = true), (line = line.slice(3));
      else break;
    }
    const [c, n] = line.split(' // ');
    props.c = c;
    if (n) props.n = n;
    while (stack[stack.length - 1]!.d >= d) stack.pop();
    const parent = stack[stack.length - 1]!.id;
    const id = tx.create(parent, last.get(parent) ?? null, props);
    last.set(parent, id);
    ids[c!] = id;
    stack.push({ d, id });
    if (col) collapse.push(id);
  }
  for (const id of collapse) tx.collapse(id, true);
  s.commit(tx, { before: { k: 'none' }, after: { k: 'none' } }, { undoable: false });
  return { s, ids };
}

// ---- actions ----------------------------------------------------------------------------------------------

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const visible = (e: Editor) => flatten(e.tree, e.view);

const COMMANDS = [
  'indent', 'unindent', 'collapse', 'toggle-siblings', 'expand-1', 'expand-2', 'expand-4', 'delete-left', 'delete-right',
  'toggle-checked', 'toggle-checked-here', 'uncheck-children', 'toggle-checkbox', 'toggle-children-checkbox',
  'toggle-numbered', 'duplicate', 'swap-up', 'swap-down', 'sort-asc', 'sort-desc', 'delete-checked', 'collapse-all',
  'zoom-in', 'zoom-out', 'goto-note-of', 'select-all',
];
const KEYS: [string, Partial<KeyboardEventInit>][] = [
  ['Enter', {}], ['Backspace', {}], ['Delete', {}], ['Tab', {}], ['Tab', { shiftKey: true }], ['ArrowUp', {}],
  ['ArrowDown', {}], ['ArrowDown', { shiftKey: true }], ['ArrowUp', { shiftKey: true }], ['Escape', {}],
  ['ArrowUp', { ctrlKey: true }], ['ArrowDown', { ctrlKey: true }], ['ArrowUp', { altKey: true, shiftKey: true }],
];

/** Some visible items next to each other (a selection-like target). */
function someIds(r: Rng, e: Editor): NodeId[] {
  const vis = visible(e);
  if (!vis.length) return [];
  const a = int(r, vis.length);
  const b = Math.min(vis.length - 1, a + (chance(r, 0.7) ? 0 : int(r, 4)));
  return selectBetween(e.tree, vis[a]!, vis[b]!, e.view);
}

type Action = (r: Rng, e: Editor, s: DocSession) => string | Promise<string>;

const ACTIONS: [number, string, Action][] = [
  [16, 'command', (r, e) => {
    const ids = someIds(r, e);
    const cmd = pick(r, COMMANDS);
    if (!ids.length) return `${cmd}: nothing visible`;
    e.execOn(ids, cmd);
    return `${cmd} on ${ids.join(',')}`;
  }],
  [10, 'edit', (r, e) => {
    const vis = visible(e);
    const id = vis.length && chance(r, 0.95) ? pick(r, vis) : e.view.zoom;
    const f = id !== e.view.zoom && chance(r, 0.15) ? 'n' : 'c';
    const len = String(e.tree.get(id)?.props[f] ?? '').length;
    e.applySel(caret(id, int(r, len + 1), f));
    return `edit ${id}.${f}`;
  }],
  [14, 'type', (r, e) => {
    if (e.editState().f === null) return 'type: not editing';
    const t = chance(r, 0.15) ? pick(r, MARKUP) : pick(r, WORDS).slice(0, 1 + int(r, 4));
    document.execCommand('insertText', false, t);
    return `type ${JSON.stringify(t)}`;
  }],
  [14, 'key', (r, e) => {
    const [key, mods] = pick(r, KEYS);
    const target = document.activeElement && e.el.contains(document.activeElement) ? document.activeElement : e.el;
    target.dispatchEvent(new KeyboardEvent('keydown', { key, code: key, bubbles: true, cancelable: true, ...mods }));
    return `key ${Object.keys(mods).join('+')}${Object.keys(mods).length ? '+' : ''}${key}`;
  }],
  [6, 'enter-split', (r, e) => {
    const vis = visible(e);
    if (!vis.length) return 'enter: empty';
    const id = pick(r, vis);
    const len = String(e.tree.get(id)?.props.c ?? '').length;
    const at = int(r, len + 1);
    e.run((tx) => enter(tx, e.view, id, at, at));
    return `enter ${id}@${at}`;
  }],
  [4, 'merge-up', (r, e) => {
    const vis = visible(e);
    if (!vis.length) return 'merge: empty';
    const id = pick(r, vis);
    e.run((tx) => backspaceAtStart(tx, e.view, id));
    return `backspace-at-start ${id}`;
  }],
  [8, 'set-props', (r, e) => {
    const vis = visible(e);
    if (!vis.length) return 'set: empty';
    const id = pick(r, vis);
    const key = pick(r, ['c', 'n', 'n', 'cb', 'ck', 'h', 'cl', 'num']);
    const val = key === 'c' || key === 'n' ? (chance(r, 0.3) ? '' : text(r)) : key === 'h' ? int(r, 4) || null : key === 'cl' ? int(r, 7) || null : chance(r, 0.5) || null;
    e.run((tx) => {
      tx.set(id, { [key]: val });
      return { sel: e.currentSel() };
    });
    return `set ${id}.${key}=${JSON.stringify(val)}`;
  }],
  [4, 'block-select', (r, e) => {
    const ids = someIds(r, e);
    if (!ids.length) return 'block: empty';
    e.applySel({ k: 'block', ids, anchor: ids[0]!, focus: ids[ids.length - 1]! } as Sel);
    return `block ${ids.join(',')}`;
  }],
  [5, 'undo', (r, e) => {
    if (chance(r, 0.3)) {
      e.redo();
      return 'redo';
    }
    e.undo();
    return 'undo';
  }],
  [4, 'zoom', (r, e) => {
    const vis = visible(e);
    const to = chance(r, 0.3) || !vis.length ? (chance(r, 0.5) ? ROOT : (e.tree.get(e.view.zoom)?.parent ?? ROOT)) : pick(r, vis);
    e.setZoom(to);
    return `zoom ${to}`;
  }],
  [3, 'search', (r, e) => {
    if (e.view.filter && chance(r, 0.5)) {
      e.el.classList.remove('o-searching');
      e.setFilter(null);
      e.refreshText();
      return 'search off';
    }
    const q = chance(r, 0.2) ? `-${pick(r, WORDS)}` : pick(r, WORDS).slice(0, 2 + int(r, 3));
    const m = compileQuery(q, Date.now(), { typing: chance(r, 0.5) });
    const res = searchTree(e.tree, m, e.view.zoom, { showChecked: e.view.showChecked });
    e.el.classList.add('o-searching');
    e.setFilter(res.visible, { terms: m.terms, ids: new Set(res.matches) });
    e.refreshText();
    return `search ${JSON.stringify(q)}`;
  }],
  [2, 'show-checked', (r, e) => {
    e.setShowChecked(!e.view.showChecked);
    return `show checked ${e.view.showChecked}`;
  }],
  [1, 'notes-pref', (r, e) => {
    const v = pick(r, ['show', 'teaser', 'hide'] as const);
    e.applyPrefs({ showNotes: v });
    return `notes ${v}`;
  }],
  [2, 'backlinks', (r, e) => {
    const vis = visible(e);
    if (chance(r, 0.3)) backlinks.clear();
    for (let i = 0; i < 3 && vis.length; i++) {
      const id = pick(r, vis);
      if (chance(r, 0.3)) backlinks.delete(id);
      else backlinks.set(id, 1 + int(r, 9));
    }
    e.render();
    return 'backlinks + full render';
  }],
  [5, 'remote', (r, e, s) => {
    // another client's ops, generated against our live tree, arriving with the sync response
    const t = s.live.clone();
    const tx = new Tx(t);
    const all = [...flattenAll(t)];
    for (let i = 0, n = 1 + int(r, 4); i < n && all.length; i++) {
      const id = pick(r, all);
      const k = int(r, 5);
      if (k === 0) tx.create(chance(r, 0.3) ? ROOT : id, null, { c: `remote ${text(r)}` });
      else if (k === 1) tx.set(id, { c: text(r) });
      else if (k === 2) {
        const p = pick(r, [ROOT, ...all]);
        if (p !== id && !t.ancestors(p).includes(id)) tx.move(id, p, null);
      } else if (k === 3) tx.move(id, null, null);
      else tx.collapse(id, !t.get(id)?.collapsed);
    }
    const ops: Op[] = tx.ops.filter((o) => o.t !== 'collapse');
    s.beginSync();
    s.endSync({ version: s.version + 1, remote: ops });
    return `remote ${ops.map((o) => `${o.t}:${o.id}`).join(' ')}`;
  }],
  [4, 'paste', (r, e) => {
    // plain text through the editor's paste handler: one line pastes into the text, several become items
    const n = chance(r, 0.4) ? 1 : 2 + int(r, 4);
    const lines = Array.from({ length: n }, (_, i) => `${'  '.repeat(i ? int(r, 3) : 0)}${chance(r, 0.3) ? '- ' : ''}${text(r)}`);
    const dt = new DataTransfer();
    dt.setData('text/plain', lines.join('\n'));
    const target = document.activeElement && e.el.contains(document.activeElement) ? document.activeElement : e.el;
    target.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
    return `paste ${n} line(s)`;
  }],
  [3, 'copy-cut', (r, e) => {
    const cut = chance(r, 0.5);
    const dt = new DataTransfer();
    const target = document.activeElement && e.el.contains(document.activeElement) ? document.activeElement : e.el;
    target.dispatchEvent(new ClipboardEvent(cut ? 'cut' : 'copy', { clipboardData: dt, bubbles: true, cancelable: true }));
    return `${cut ? 'cut' : 'copy'} (${dt.getData('text/plain').split('\n').length} line(s))`;
  }],
  [1, 'read-only', (r, e) => {
    const on = !e.readOnly && chance(r, 0.5);
    e.setReadOnly(on);
    return `read-only ${on}`;
  }],
  [2, 'render', (r, e) => {
    if (chance(r, 0.5)) e.refreshText();
    else e.render();
    return 'full render';
  }],
  [3, 'wait', async (r) => {
    const ms = pick(r, [0, 50, 250]);
    await sleep(ms);
    return `wait ${ms}`;
  }],
];

function* flattenAll(t: Tree, id: NodeId = ROOT): Generator<NodeId> {
  for (const c of t.get(id)?.children ?? []) {
    yield c;
    yield* flattenAll(t, c);
  }
}

// ---- runners ----------------------------------------------------------------------------------------------

export interface FuzzOptions {
  steps?: number;
  size?: number;
  shape?: Shape;
  /** compare with a from-scratch render every N steps */
  freshEvery?: number;
}

export interface Failure {
  name: string;
  step: number;
  errors: string[];
  log: string[];
}

function check(e: Editor, s: DocSession, fresh: boolean): string[] {
  const errs = [...verifyTree(s.live), ...verifyRender(e)];
  // DOM rearrangements must not take the caret away from the item being edited
  const ed$ = e.debugInternals().editing;
  if (ed$ && document.hasFocus() && document.activeElement !== ed$.el) {
    errs.push(`editing ${ed$.id}.${ed$.f} but focus is on ${document.activeElement?.className || document.activeElement?.tagName}`);
  }
  if (!errs.length && fresh) errs.push(...compareFresh(e));
  return errs;
}

async function fuzz(seed: number, o: FuzzOptions = {}): Promise<Failure | null> {
  const r = rng(seed);
  const shape = o.shape ?? pick(r, ['mixed', 'flat', 'deep', 'wide'] as const);
  const size = o.size ?? pick(r, [0, 3, 20, 60, 200]);
  const steps = o.steps ?? 200;
  const freshEvery = o.freshEvery ?? 10;
  const s = new DocSession();
  buildDoc(s, r, size, shape);
  const e = freshEditor(s);
  const log = [`seed ${seed}: ${shape} doc of ${size}`];
  let errs = check(e, s, true);
  if (errs.length) return { name: `fuzz ${seed}`, step: 0, errors: errs, log };
  for (let step = 1; step <= steps; step++) {
    const [, label, act] = weighted(r, ACTIONS.map(([w, l, a]) => [w, [w, l, a] as const]));
    try {
      log.push(`${step}. ${await act(r, e, s)}`);
    } catch (err) {
      return { name: `fuzz ${seed}`, step, errors: [`${label} threw: ${(err as Error).stack ?? err}`], log: log.slice(-25) };
    }
    errs = check(e, s, step % freshEvery === 0 || step === steps);
    if (errs.length) return { name: `fuzz ${seed}`, step, errors: errs, log: log.slice(-25) };
  }
  return null;
}

// ---- scenarios: named, deterministic, each step checked ----------------------------------------------------

interface Ctx {
  e: Editor;
  s: DocSession;
  ids: Record<string, NodeId>;
  /** run a step, then check the render (throws with the step name on problems) */
  step(name: string, fn: () => unknown): Promise<void>;
  /** extra expectation */
  expect(cond: boolean, msg: string): void;
  texts(): string;
}

const SCENARIOS: [string, string, (c: Ctx) => Promise<void>][] = [];
const scenario = (name: string, spec: string, fn: (c: Ctx) => Promise<void>) => SCENARIOS.push([name, spec, fn]);

const SPEC = `
a
  a1
  a2 // note of a2
  a3
b
  [ ] b1
  [x] b2
  b3
    b31
    b32
c
+d
  d1
  d2
1. e
  e1
  e2
  e3
f`;

scenario('indent and outdent', SPEC, async (c) => {
  await c.step('indent a2', () => c.e.execOn([c.ids.a2!], 'indent'));
  await c.step('indent a2 again (no previous sibling)', () => c.e.execOn([c.ids.a2!], 'indent'));
  await c.step('outdent a2', () => c.e.execOn([c.ids.a2!], 'unindent'));
  await c.step('outdent a2 to top level', () => c.e.execOn([c.ids.a2!], 'unindent'));
  await c.step('indent b, c, d into a', () => c.e.execOn([c.ids.b!, c.ids.c!, c.ids.d!], 'indent'));
  await c.step('outdent them back', () => c.e.execOn([c.ids.b!, c.ids.c!, c.ids.d!], 'unindent'));
});

scenario('removing every child', SPEC, async (c) => {
  await c.step('delete a1..a3', () => c.e.execOn([c.ids.a1!, c.ids.a2!, c.ids.a3!], 'delete-left'));
  c.expect(!c.e.el.querySelector(`[data-id="${c.ids.a}"]`)?.classList.contains('o-has-kids'), 'a lost o-has-kids');
  await c.step('undo', () => c.e.undo());
  await c.step('redo', () => c.e.redo());
  await c.step('add a child back', () => c.e.run((tx) => enter(tx, c.e.view, c.ids.a!, 1, 1)));
});

scenario('collapse and expand (animated and not)', SPEC, async (c) => {
  await c.step('collapse b', () => c.e.execOn([c.ids.b!], 'collapse'));
  await c.step('wait for animation', () => sleep(250));
  await c.step('expand d', () => c.e.execOn([c.ids.d!], 'collapse'));
  await c.step('collapse d mid-animation', () => c.e.execOn([c.ids.d!], 'collapse'));
  await c.step('expand b while d animates', () => c.e.execOn([c.ids.b!], 'collapse'));
  await c.step('wait', () => sleep(250));
  await c.step('collapse all', () => c.e.execOn([c.ids.a!], 'collapse-all'));
  await c.step('expand to level 2', () => c.e.execOn([c.ids.b!], 'expand-2'));
  await c.step('toggle siblings', () => c.e.execOn([c.ids.b3!], 'toggle-siblings'));
  await c.step('wait', () => sleep(250));
});

scenario('numbering follows visible children', SPEC, async (c) => {
  await c.step('delete e2', () => c.e.execOn([c.ids.e2!], 'delete-left'));
  await c.step('enter after e1', () => c.e.run((tx) => enter(tx, c.e.view, c.ids.e1!, 2, 2)));
  await c.step('move e3 up', () => c.e.execOn([c.ids.e3!], 'swap-up'));
  await c.step('check e1, hide checked', () => {
    c.e.execOn([c.ids.e1!], 'toggle-checked');
    c.e.setShowChecked(false);
  });
  await c.step('show checked', () => c.e.setShowChecked(true));
  await c.step('numbering off', () => c.e.execOn([c.ids.e!], 'toggle-numbered'));
  await c.step('numbering on b', () => c.e.execOn([c.ids.b!], 'toggle-numbered'));
});

scenario('hiding checked items', SPEC, async (c) => {
  await c.step('hide checked', () => c.e.setShowChecked(false));
  await c.step('check b1 while hidden', () => c.e.execOn([c.ids.b1!], 'toggle-checked'));
  await c.step('check all of b', () => c.e.execOn([c.ids.b!], 'toggle-checked'));
  await c.step('undo', () => c.e.undo());
  await c.step('delete checked', () => c.e.execOn([c.ids.b!], 'delete-checked'));
  await c.step('show checked', () => c.e.setShowChecked(true));
});

scenario('search filter and highlight', SPEC, async (c) => {
  const search = (q: string) => {
    const m = compileQuery(q, Date.now());
    const res = searchTree(c.e.tree, m, c.e.view.zoom, { showChecked: c.e.view.showChecked });
    c.e.setFilter(res.visible, { terms: m.terms, ids: new Set(res.matches) });
    c.e.refreshText();
  };
  await c.step('search d1 (inside a collapsed item)', () => search('d1'));
  await c.step('search 3', () => search('3'));
  await c.step('edit a match while filtered', () => c.e.applySel(caret(c.ids.b31!, 1)));
  await c.step('type into it', () => document.execCommand('insertText', false, 'zz'));
  await c.step('search nothing found', () => search('nomatchxyz'));
  await c.step('clear', () => {
    c.e.setFilter(null);
    c.e.refreshText();
  });
});

scenario('zoom', SPEC, async (c) => {
  await c.step('zoom into b', () => c.e.setZoom(c.ids.b!));
  await c.step('zoom into a leaf', () => c.e.setZoom(c.ids.f!));
  c.expect(c.e.el.classList.contains('o-empty'), 'leaf zoom is empty');
  await c.step('add first item in the empty zoom', () => c.e.run((tx) => enter(tx, c.e.view, c.ids.f!, 1, 1)));
  await c.step('zoom into collapsed d', () => c.e.setZoom(c.ids.d!));
  await c.step('outdent d1 out of the zoom', () => c.e.execOn([c.ids.d1!], 'unindent'));
  await c.step('zoom out', () => c.e.execOn([], 'zoom-out'));
  await c.step('zoom to root', () => c.e.setZoom(ROOT));
});

scenario('typing keeps the edited element', SPEC, async (c) => {
  await c.step('edit a3', () => c.e.applySel(caret(c.ids.a3!, 2)));
  const el = c.e.debugInternals().editing!.el;
  for (const t of ['x', 'y', ' **b', 'old**', ' #t']) await c.step(`type ${t}`, () => document.execCommand('insertText', false, t));
  c.expect(c.e.debugInternals().editing?.el === el, 'edited element replaced while typing');
  await c.step('edit its note', () => c.e.applySel(caret(c.ids.a3!, 0, 'n')));
  await c.step('type a note', () => document.execCommand('insertText', false, 'fresh note'));
  await c.step('leave', () => c.e.applySel({ k: 'none' }));
  await c.step('edit title', () => c.e.applySel(caret(ROOT, 4)));
  await c.step('type in title', () => document.execCommand('insertText', false, '!'));
});

scenario('remote changes while editing', SPEC, async (c) => {
  const remote = (fn: (tx: Tx) => void) => {
    const tx = new Tx(c.s.live.clone());
    fn(tx);
    c.s.beginSync();
    c.s.endSync({ version: c.s.version + 1, remote: tx.ops });
  };
  await c.step('edit b31', () => c.e.applySel(caret(c.ids.b31!, 1)));
  await c.step('remote edits b31 text', () => remote((tx) => tx.set(c.ids.b31!, { c: 'remote text' })));
  await c.step('remote moves b3 to top', () => remote((tx) => tx.move(c.ids.b3!, ROOT, null)));
  await c.step('remote adds items', () => remote((tx) => {
    tx.create(ROOT, null, { c: 'r1' });
    tx.create(c.ids.a!, c.ids.a1!, { c: 'r2' });
  }));
  await c.step('remote deletes the edited item', () => remote((tx) => tx.move(c.ids.b31!, null, null)));
  await c.step('remote deletes the zoom root', async () => {
    c.e.setZoom(c.ids.a!);
    remote((tx) => tx.move(c.ids.a!, null, null));
  });
});

scenario('undo and redo of structure', SPEC, async (c) => {
  await c.step('move b down twice', () => {
    c.e.execOn([c.ids.b!], 'swap-down');
    c.e.execOn([c.ids.b!], 'swap-down');
  });
  await c.step('duplicate a', () => c.e.execOn([c.ids.a!], 'duplicate'));
  await c.step('sort', () => c.e.execOn([c.ids.b3!], 'sort-desc'));
  for (let i = 0; i < 4; i++) await c.step(`undo ${i}`, () => c.e.undo());
  for (let i = 0; i < 4; i++) await c.step(`redo ${i}`, () => c.e.redo());
});

scenario('row parts come and go', SPEC, async (c) => {
  const set = (id: string, props: Record<string, unknown>) => c.e.run((tx) => {
    tx.set(c.ids[id]!, props);
    return { sel: c.e.currentSel() };
  });
  await c.step('add checkbox', () => set('a1', { cb: true }));
  await c.step('check it', () => set('a1', { ck: true }));
  await c.step('remove checkbox', () => set('a1', { cb: null, ck: null }));
  await c.step('add note', () => set('c', { n: 'a note' }));
  await c.step('remove note', () => set('c', { n: '' }));
  await c.step('heading + color', () => set('c', { h: 2, cl: 3 }));
  await c.step('plain again', () => set('c', { h: null, cl: null }));
  await c.step('backlinks on', () => {
    backlinks.set(c.ids.a!, 2).set(c.ids.f!, 5);
    c.e.render();
  });
  await c.step('backlinks change', () => {
    backlinks.delete(c.ids.a!);
    backlinks.set(c.ids.b1!, 1);
    c.e.render();
  });
  await c.step('backlinks off', () => {
    backlinks.clear();
    c.e.render();
  });
});

scenario('enter and backspace', SPEC, async (c) => {
  await c.step('edit end of a', () => c.e.applySel(caret(c.ids.a!, 1)));
  const key = (k: string, o: Partial<KeyboardEventInit> = {}) => document.activeElement!.dispatchEvent(new KeyboardEvent('keydown', { key: k, code: k, bubbles: true, cancelable: true, ...o }));
  await c.step('Enter at end of a parent', () => key('Enter'));
  await c.step('type', () => document.execCommand('insertText', false, 'new'));
  await c.step('Enter', () => key('Enter'));
  await c.step('Tab', () => key('Tab'));
  await c.step('Shift+Tab', () => key('Tab', { shiftKey: true }));
  await c.step('Backspace on empty', () => key('Backspace'));
  await c.step('edit start of b', () => c.e.applySel(caret(c.ids.b!, 0)));
  await c.step('Backspace merges b into the item above', () => key('Backspace'));
  await c.step('ArrowDown x5', () => {
    for (let i = 0; i < 5; i++) key('ArrowDown');
  });
  await c.step('Shift+ArrowDown x3', () => {
    for (let i = 0; i < 3; i++) key('ArrowDown', { shiftKey: true });
  });
  await c.step('Tab the block', () => key('Tab'));
});

scenario('long sibling lists', '', async (c) => {
  await c.step('300 top-level items', () => c.e.run((tx) => {
    let after: NodeId | null = null;
    for (let i = 0; i < 300; i++) after = tx.create(ROOT, after, { c: `item ${i}` });
    return { sel: { k: 'none' } };
  }));
  const vis = () => visibleChildren(c.e.tree, ROOT, c.e.view);
  await c.step('insert at the top', () => c.e.run((tx) => {
    tx.create(ROOT, null, { c: 'top' });
    return { sel: { k: 'none' } };
  }));
  await c.step('insert in the middle', () => c.e.run((tx) => {
    tx.create(ROOT, vis()[150]!, { c: 'middle' });
    return { sel: { k: 'none' } };
  }));
  await c.step('delete a range across the middle', () => c.e.execOn(vis().slice(90, 210), 'delete-left'));
  await c.step('undo', () => c.e.undo());
  await c.step('indent a range under item 49', () => c.e.execOn(vis().slice(50, 180), 'indent'));
  await c.step('collapse item 49', () => c.e.execOn([vis()[50]!], 'collapse'));
  await c.step('expand it', () => c.e.execOn([vis()[50]!], 'collapse'));
  await c.step('move the last item to the top', () => {
    for (let i = 0; i < 20; i++) c.e.execOn([vis()[vis().length - 1 - i]!], 'swap-up');
  });
  await c.step('sort all', () => c.e.execOn([ROOT], 'sort-asc'));
  await c.step('numbered', () => c.e.execOn([ROOT], 'toggle-numbered'));
  await c.step('hide half (checked)', () => {
    c.e.execOn(vis().filter((_, i) => i % 2 === 0), 'toggle-checked');
    c.e.setShowChecked(false);
  });
  await c.step('show', () => c.e.setShowChecked(true));
});

async function runScenario(name: string, spec: string, fn: (c: Ctx) => Promise<void>): Promise<Failure | null> {
  const { s, ids } = docFromSpec(spec);
  const e = freshEditor(s);
  const log: string[] = [];
  let failure: Failure | null = null;
  const c: Ctx = {
    e,
    s,
    ids,
    async step(label, fn) {
      if (failure) return;
      log.push(label);
      try {
        await fn();
      } catch (err) {
        failure = { name, step: log.length, errors: [`threw: ${(err as Error).stack ?? err}`], log };
        return;
      }
      const errs = check(e, s, true);
      if (errs.length) failure = { name, step: log.length, errors: errs, log };
    },
    expect(cond, msg) {
      if (!cond && !failure) failure = { name, step: log.length, errors: [`expectation: ${msg}`], log };
    },
    texts: () => serializeRender(e),
  };
  await c.step('initial render', () => {});
  try {
    await fn(c);
  } catch (err) {
    // the scenario body itself (between steps) broke, usually because an earlier step already failed
    failure ??= { name, step: log.length, errors: [`scenario threw: ${(err as Error).stack ?? err}`], log };
  }
  await sleep(0);
  return failure;
}

async function scenarios(only?: string): Promise<{ ran: number; failures: Failure[] }> {
  const failures: Failure[] = [];
  let ran = 0;
  for (const [name, spec, fn] of SCENARIOS) {
    if (only && !name.includes(only)) continue;
    ran++;
    const f = await runScenario(name, spec, fn);
    if (f) failures.push(f);
  }
  return { ran, failures };
}

declare global {
  interface Window {
    renderTests: { fuzz: typeof fuzz; scenarios: typeof scenarios; ready: boolean };
  }
}
window.renderTests = { fuzz, scenarios, ready: true };
