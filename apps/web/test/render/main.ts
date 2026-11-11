// Render tests: drive the real editor through its public paths (commands, keys, typing, undo, zoom, search,
// remote sync) and check after every step that the DOM shows exactly what the model says (verify.ts), and now
// and then that it equals a from-scratch render of the same state. Driven by apps/web/e2e/render.mjs.

import '../../src/styles.css';
import {
  DocSession, type NodeId, type Op, ROOT, type Sel, Tx, backspaceAtStart, caret, compileQuery, enter, flatten,
  searchTree, selectBetween, visibleChildren, type Tree,
} from '@outliner/core';
import { Editor, type EditorHost, type LazyOpts } from '../../src/editor/editor.ts';
import { sameLine, serializeRender, verifyRender } from '../../src/editor/verify.ts';

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
/** the "visible" band lazy rendering works around (client coordinates); moved by the scroll action */
let band: [number, number] = [0, 60];

/** Lazy rendering for tests: no timers (steps call lazyTick), around `band`. */
type Lazy = Partial<Pick<LazyOpts, 'first' | 'keep'>>;

function freshEditor(session: DocSession, chunkSize = 100, lazy: Lazy = {}): Editor {
  ed?.dispose();
  stage.textContent = '';
  backlinks.clear();
  band = [0, 60];
  ed = new Editor(host);
  ed.chunkSize = chunkSize;
  ed.lazy = { ...ed.lazy, ...lazy, auto: false, zone: () => band };
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
    f.chunkSize = e.chunkSize;
    f.lazy = { ...e.lazy, first: Infinity, keep: Infinity, auto: false };
    f.applyPrefs(e.prefs);
    f.setDoc(e.docSession, e.view.zoom);
    f.view = { ...e.view };
    f.setFilter(e.view.filter ?? null, x.searchHl);
    f.refreshText(); // as the app does after a search
    const a = serializeRender(e).split('\n');
    const b = serializeRender(f).split('\n');
    const errs = verifyRender(f).map((m) => `fresh editor: ${m}`);
    for (let i = 0; i < Math.max(a.length, b.length) && errs.length < 6; i++) {
      if (!sameLine(a[i] ?? '', b[i] ?? '') || (a[i] === undefined) !== (b[i] === undefined)) errs.push(`line ${i}: incremental ${JSON.stringify(a[i])} != fresh ${JSON.stringify(b[i])}`);
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

/** How much of the lazy-rendering machinery the fuzz exercised (printed by render.mjs, so it can't quietly drop). */
const stats = { steps: 0, pendingSteps: 0, evictions: 0, madeOnDemand: 0 };
/** Editing `before` (the field), and the item is still shown: it must still be edited (moves keep the caret). */
function keepsEditing(e: Editor, before: { id: NodeId; f: 'c' | 'n' } | null, what: string) {
  if (!before || (before.id !== e.view.zoom && !visible(e).includes(before.id))) return;
  const now = e.debugInternals().editing;
  if (now?.id !== before.id || now.f !== before.f) throw new Error(`${what}: was editing ${before.id}.${before.f}, still shown, now editing ${now ? `${now.id}.${now.f}` : 'nothing'}`);
}
const editingNow = (e: Editor) => {
  const x = e.debugInternals().editing;
  return x && { id: x.id, f: x.f };
};
const pendingNow = (e: Editor) => [...e.debugInternals().pending].filter((el) => el.isConnected).length;

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
    const had = e.debugInternals().rows.has(id);
    e.applySel(caret(id, int(r, len + 1), f));
    const now = e.debugInternals().editing;
    // any shown item can be edited, whether or not its row was made
    if (!e.readOnly && (now?.id !== id || now.f !== f)) throw new Error(`edit ${id}.${f}: editing ${now ? `${now.id}.${now.f}` : 'nothing'}`);
    if (!had && now) stats.madeOnDemand++;
    return `edit ${id}.${f}${had ? '' : ' (row made on demand)'}`;
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
    const before = editingNow(e);
    target.dispatchEvent(new KeyboardEvent('keydown', { key, code: key, bubbles: true, cancelable: true, ...mods }));
    const name = `key ${Object.keys(mods).join('+')}${Object.keys(mods).length ? '+' : ''}${key}`;
    // moving the edited item (indent, outdent, swap) keeps editing it
    if (key === 'Tab' || (mods.ctrlKey && !mods.shiftKey)) keepsEditing(e, before, name);
    return name;
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
    // another client's ops, generated against our live tree, arriving with the sync response; half the time our
    // own changes reached the server first (else theirs apply before ours, and some of them miss our new items)
    if (chance(r, 0.5)) {
      s.beginSync();
      s.endSync({ version: s.version + 1, remote: [] });
    }
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
    const before = editingNow(e);
    s.beginSync();
    s.endSync({ version: s.version + 1, remote: ops });
    keepsEditing(e, before, 'remote change');
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
  [10, 'tick', (r, e) => {
    const before = e.debugInternals().rows.size;
    const more = e.lazyTick(Infinity);
    if (e.debugInternals().rows.size < before) stats.evictions++;
    return `lazy tick: ${e.debugInternals().rows.size} rows, ${more ? 'more to do' : 'done'}`;
  }],
  [6, 'scroll', (r, e) => {
    const box = e.el.getBoundingClientRect();
    // somewhere in the document, or far away from it (everything is evictable then)
    const top = chance(r, 0.25) ? Math.round(box.bottom + 5000) : Math.round(box.top + int(r, Math.max(1, box.height) + 200) - 100);
    band = [top, top + pick(r, [10, 30, 100, 400])];
    return `scroll: band ${band.join('..')}`;
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
  /** rows per chunk (default: a small random size, so small documents exercise chunking) */
  chunkSize?: number;
  /** lazy rendering budgets (default: random, mostly tiny so small documents exercise it) */
  lazy?: Lazy;
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
  const chunkSize = o.chunkSize ?? pick(r, [2, 3, 5, 8, 100]);
  const lazy = o.lazy ?? pick<Lazy>(r, [{}, { first: 0, keep: 0 }, { first: 0, keep: 6 }, { first: 0, keep: 20 }, { first: 3, keep: 12 }, { first: 1, keep: 40 }, { first: 10, keep: Infinity }]);
  const s = new DocSession();
  buildDoc(s, r, size, shape);
  const e = freshEditor(s, chunkSize, lazy);
    const log = [`seed ${seed}: ${shape} doc of ${size}, chunks of ${chunkSize}, lazy ${JSON.stringify(lazy)}`];
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
    stats.steps++;
    if (pendingNow(e)) stats.pendingSteps++;
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

/**
 * Another client's change arriving with a sync response. Everything local is confirmed first (a sync round),
 * so that the ops apply against a server state that has the scenario's items.
 */
function remoteChange(s: DocSession, fn: (tx: Tx) => void) {
  s.beginSync();
  s.endSync({ version: s.version + 1, remote: [] });
  const tx = new Tx(s.live.clone());
  fn(tx);
  s.beginSync();
  s.endSync({ version: s.version + 1, remote: tx.ops });
}
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
  const remote = (fn: (tx: Tx) => void) => remoteChange(c.s, fn);
  await c.step('edit b31', () => c.e.applySel(caret(c.ids.b31!, 1)));
  await c.step('remote edits b31 text', () => remote((tx) => tx.set(c.ids.b31!, { c: 'remote text' })));
  c.expect(c.e.tree.get(c.ids.b31!)?.props.c === 'remote text', 'the remote change was not applied');
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

scenario('an empty note takes no line', SPEC, async (c) => {
  const noteOf = (id: NodeId) => c.e.debugInternals().rows.get(id)?.note ?? null;
  const noteShown = (id: NodeId) => {
    const n = noteOf(id);
    return !!n && !n.hidden && getComputedStyle(n).display !== 'none';
  };
  const key = (k: string, o: Partial<KeyboardEventInit> = {}) => document.activeElement!.dispatchEvent(new KeyboardEvent('keydown', { key: k, code: k, bubbles: true, cancelable: true, ...o }));
  // opening the note makes room for it; leaving it empty must give the room back
  await c.step('edit a3 text', () => c.e.applySel(caret(c.ids.a3!, 2)));
  await c.step('Shift+Enter opens an empty note', () => key('Enter', { shiftKey: true }));
  c.expect(c.e.debugInternals().editing?.f === 'n', 'Shift+Enter did not focus the note');
  c.expect(noteShown(c.ids.a3!), 'the note being edited is not shown');
  await c.step('Shift+Enter back to the text', () => key('Enter', { shiftKey: true }));
  c.expect(!noteShown(c.ids.a3!), 'an empty note still takes a line after leaving it');
  await c.step('Escape out of an empty note', () => {
    c.e.applySel(caret(c.ids.a3!, 0, 'n'));
  });
  await c.step('Escape', () => key('Escape'));
  c.expect(!noteShown(c.ids.a3!), 'an empty note still takes a line after Escape');
  // whitespace alone is not a note either (Dynalist only shows a note with text)
  await c.step('note with a space', () => c.e.run((tx) => {
    tx.set(c.ids.a3!, { n: ' ' });
    return { sel: { k: 'none' } };
  }));
  c.expect(!noteShown(c.ids.a3!), 'a whitespace-only note takes a line');
  await c.step('note with a newline', () => c.e.run((tx) => {
    tx.set(c.ids.a3!, { n: '\n' });
    return { sel: { k: 'none' } };
  }));
  c.expect(!noteShown(c.ids.a3!), 'a newline-only note takes a line');
  // ... but its text is kept, and a real note shows again
  await c.step('type into the whitespace note', () => c.e.run((tx) => {
    tx.set(c.ids.a3!, { n: '\nreal' });
    return { sel: { k: 'none' } };
  }));
  c.expect(noteShown(c.ids.a3!), 'a note with text is not shown');
  // editing the item text of an item whose note is blank must not reveal the note
  await c.step('blank note again', () => c.e.run((tx) => {
    tx.set(c.ids.a3!, { n: '  ' });
    return { sel: { k: 'none' } };
  }));
  await c.step('edit its text', () => c.e.applySel(caret(c.ids.a3!, 0)));
  c.expect(!noteShown(c.ids.a3!), 'editing the item text revealed a blank note');
  await c.step('leave', () => c.e.applySel({ k: 'none' }));
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

scenario('lazy rendering: the edited item keeps its row', `
a
  a1
  a2
  a3
b
  b1
    b11
c
d`, async (c) => {
  const key = (k: string, o: Partial<KeyboardEventInit> = {}) => document.activeElement!.dispatchEvent(new KeyboardEvent('keydown', { key: k, code: k, bubbles: true, cancelable: true, ...o }));
  const editing = (id: string, what: string) => c.expect(c.e.debugInternals().editing?.id === c.ids[id], `${what}: ${id} is no longer edited`);
  const made = (id: string) => c.e.debugInternals().rows.has(c.ids[id]!);
  const farTick = () => {
    const box = c.e.el.getBoundingClientRect();
    band = [box.bottom + 5000, box.bottom + 5010];
    c.e.lazyTick(Infinity);
  };
  await c.step('nothing below the top list is made', () => {
    c.e.lazy = { ...c.e.lazy, first: 0, keep: 0 };
    farTick();
    c.e.render();
    if (made('a1') || made('b1')) throw new Error('children made despite a zero budget');
  });
  await c.step('edit c', () => c.e.applySel(caret(c.ids.c!, 1)));
  await c.step('Tab: c moves under b, whose children are pending', () => key('Tab'));
  editing('c', 'indent into a pending block');
  await c.step('evict everything far away', farTick);
  editing('c', 'eviction');
  await c.step('Ctrl+Up: swap c above b1', () => key('ArrowUp', { ctrlKey: true }));
  editing('c', 'swap');
  await c.step('Shift+Tab: c back to the top', () => key('Tab', { shiftKey: true }));
  editing('c', 'outdent');
  await c.step('edit a2 (its row is made on demand)', () => c.e.applySel(caret(c.ids.a2!, 0)));
  editing('a2', 'edit on demand');
  await c.step('evict around it', () => {
    farTick();
    if (made('b1')) throw new Error('b1 still made after eviction');
  });
  editing('a2', 'eviction');
  await c.step('remote: a moves under b1 (not made), and a gets a child', () => {
    remoteChange(c.s, (tx) => {
      tx.move(c.ids.a!, c.ids.b1!, null);
      tx.create(c.ids.a!, c.ids.a3!, { c: 'a4' });
      tx.set(c.ids.a1!, { c: 'a1 changed' });
    });
    if (c.e.tree.get(c.ids.a!)?.parent !== c.ids.b1) throw new Error('the remote change was not applied');
  });
  editing('a2', 'remote move of its parent into a pending block');
  await c.step('evict again', farTick);
  await c.step('fill everything near', () => {
    band = [0, 4000];
    c.e.lazyTick(Infinity);
  });
  await c.step('evict again, then print: everything is made, and stays so until printing is done', () => {
    farTick();
    window.dispatchEvent(new Event('beforeprint'));
    if (c.e.debugInternals().pending.size && [...c.e.debugInternals().pending].some((el) => el.isConnected)) throw new Error('pending blocks while printing');
    farTick();
    if ([...c.e.debugInternals().pending].some((el) => el.isConnected)) throw new Error('emptied while printing');
    window.dispatchEvent(new Event('afterprint'));
  });
  await c.step('after printing, far blocks are emptied again', farTick);
});

async function runScenario(name: string, spec: string, fn: (c: Ctx) => Promise<void>, chunkSize: number, lazy?: Lazy): Promise<Failure | null> {
  const { s, ids } = docFromSpec(spec);
  const e = freshEditor(s, chunkSize, lazy);
  let k = 0;
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
      let errs = check(e, s, true);
      if (!errs.length && lazy) {
        // and after a round of lazy filling / emptying around a band that moves down the document step by step
        const box = e.el.getBoundingClientRect();
        const top = box.top + ((k++ * 97) % Math.max(1, box.height));
        band = [top, top + 40];
        e.lazyTick(Infinity);
        errs = check(e, s, true).map((m) => `after lazy tick at ${Math.round(top)}: ${m}`);
      }
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

/**
 * Every scenario with unchunked lists (the app's chunk size, larger than these documents), with tiny chunks, and
 * with tiny lazy-rendering budgets (most of the document pending, filled and emptied between steps).
 */
const VARIANTS: [number, Lazy | undefined][] = [[100, undefined], [2, undefined], [3, undefined], [2, { first: 1, keep: 6 }], [100, { first: 0, keep: 4 }]];
async function scenarios(only?: string): Promise<{ ran: number; failures: Failure[] }> {
  const failures: Failure[] = [];
  let ran = 0;
  for (const [chunkSize, lazy] of VARIANTS) {
    for (const [name, spec, fn] of SCENARIOS) {
      if (only && !name.includes(only)) continue;
      ran++;
      const f = await runScenario(`${name} [chunks of ${chunkSize}${lazy ? `, lazy ${JSON.stringify(lazy)}` : ''}]`, spec, fn, chunkSize, lazy);
      if (f) failures.push(f);
    }
  }
  return { ran, failures };
}

/**
 * Lazy rendering as the app runs it (timers, frames, window scrolling) on a document of a few thousand items:
 * the first render makes about a screenful, what scrolls into view gets filled within frames, idle time fills
 * up to the limit, far-away blocks are emptied, and the DOM matches the model at every stage.
 */
async function lazyAuto(): Promise<{ failures: Failure[]; report: string[] }> {
  const failures: Failure[] = [];
  const report: string[] = [];
  const frames = async (n: number) => {
    for (let i = 0; i < n; i++) await new Promise((r) => requestAnimationFrame(r));
  };
  for (const shape of ['flat', 'mixed', 'deep'] as const) {
    const log: string[] = [];
    const fail = (m: string) => failures.push({ name: `lazy auto (${shape})`, step: log.length, errors: [m], log: [...log] });
    const s = new DocSession();
    buildDoc(s, rng(7), 4000, shape);
    const open = new Tx(s.live);
    for (const id of flattenAll(s.live)) if (s.live.get(id)!.collapsed) open.collapse(id, false);
    s.commit(open, { before: null, after: null }, { undoable: false });
    window.scrollTo(0, 0);
    stage.textContent = '';
    ed?.dispose();
    const e = (ed = new Editor(host));
    e.lazy = { ...e.lazy, first: 150, keep: 1500, auto: true, zone: null };
    stage.append(e.el);
    e.setDoc(s);
    const x = () => e.debugInternals();
    const pend = () => [...x().pending].filter((el) => el.isConnected);
    // nothing pending may show in the viewport once the frames for it ran
    const blank = () => pend().filter((el) => {
      const r = el.getBoundingClientRect();
      return r.bottom > 0 && r.top < window.innerHeight;
    }).length;
    const verify = (what: string) => {
      log.push(what);
      const errs = [...verifyRender(e), ...compareFresh(e)];
      if (errs.length) fail(`${what}: ${errs.slice(0, 5).join('; ')}`);
    };
    // frames until nothing pending shows (at most 60)
    const untilShown = async () => {
      let n = 0;
      while (blank() && n < 60) {
        await frames(1);
        n++;
      }
      return n;
    };
    const made0 = x().rows.size;
    log.push(`first render: ${made0} rows, ${pend().length} pending blocks`);
    if (made0 > 150 + 2 * e.chunkSize) fail(`first render made ${made0} rows (budget 150)`);
    if (!pend().length) fail('nothing pending after the first render of 4000 items');
    verify('first render');
    let n = await untilShown();
    log.push(`first screen shown after ${n} frames`);
    if (n > 3) fail(`pending blocks in view for ${n} frames after the first render`);
    verify('after a few frames');
    for (const at of [0.5, 0.25, 0.9, 1, 0.6, 0.1]) {
      window.scrollTo(0, (document.documentElement.scrollHeight - window.innerHeight) * at);
      await frames(1); // the scroll event
      n = await untilShown();
      log.push(`scrolled to ${at}: shown after ${n} frames`);
      if (n > 8) fail(`scrolled to ${at}: pending blocks in view for ${n} frames`);
      verify(`scrolled to ${at}`);
    }
    // idle time: fill until 3/4 of keep, empty far blocks above keep
    for (let i = 0; i < 100 && e.lazyTick(0) !== false; i++) await sleep(60);
    await sleep(300);
    log.push(`settled: ${x().rows.size} rows, ${pend().length} pending blocks`);
    if (x().rows.size > e.lazy.keep + 150 + 2 * e.chunkSize) fail(`settled at ${x().rows.size} rows (keep ${e.lazy.keep})`);
    verify('settled');
    // edit something far away: its row is made on demand, kept while far from the viewport
    const far = visible(e)[Math.floor(visible(e).length * 0.1)]!;
    window.scrollTo(0, document.documentElement.scrollHeight);
    await frames(2);
    e.applySel(caret(far, 0));
    if (e.debugInternals().editing?.id !== far) fail('could not edit a far-away item');
    await frames(4);
    await sleep(200);
    if (e.debugInternals().editing?.id !== far || document.activeElement !== e.debugInternals().editing?.el) fail('lost the edited far-away item');
    verify('editing far away');
    report.push(`${shape}: ${log.filter((l) => /frames|render:|settled:/.test(l)).join('; ')}`);
    e.dispose();
    stage.textContent = '';
    window.scrollTo(0, 0);
  }
  return { failures, report };
}

/** For looking at things: show a spec document with the given chunk size and edit the item with text `edit`. */
function show(spec: string, chunkSize: number, edit?: string, lazy?: Lazy): string[] {
  const { s, ids } = docFromSpec(spec);
  const e = freshEditor(s, chunkSize, lazy);
  if (edit) e.applySel(caret(ids[edit]!, 0));
  return verifyRender(e);
}

declare global {
  interface Window {
    renderTests: { fuzz: typeof fuzz; scenarios: typeof scenarios; lazyAuto: typeof lazyAuto; show: typeof show; stats: typeof stats; ready: boolean };
  }
}
window.renderTests = { fuzz, scenarios, lazyAuto, show, stats, ready: true };
