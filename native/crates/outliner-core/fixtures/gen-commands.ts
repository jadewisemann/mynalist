// Records what the TS editor commands and view helpers do on random trees, views and arguments, for
// crates/outliner-core/tests/commands.rs. Run from the repo root after changing packages/core:
//
//   node native/crates/outliner-core/fixtures/gen-commands.ts
//
// Deterministic: a seeded PRNG drives everything. New items get random ids (newId); each case records the
// ids its tx created, in order, and the Rust side hands them out in the same order.

import { writeFileSync } from 'node:fs';
import {
  ROOT, Tree, Tx, backspaceAtStart, caret, cycleLevel, deleteAtEnd, deleteItems, depth, duplicate, enter,
  expandToLevel, flatten, indent, lastVisibleDescendant, merge, moveDown, moveUp, nextVisible,
  nextVisibleNonDescendant, nextVisibleSibling, normalizeBlock, outdent, pasteItems, prevVisible, prevVisibleSibling,
  replaceWithEmpty, selectBetween, setChildrenCheckbox, setLevel, sortChildren, toPortable, toggleChecked,
  toggleCollapse, toggleProp, toggleSiblings, toggleWrap, toggleWrapItems, uncheckDescendants, visibleChildren,
  wrapLink, type NodeId, type Portable, type Props, type Sel, type SortMode, type View,
} from '../../../../packages/core/src/index.ts';

let seed = 0x2545f491;
function rand(): number {
  seed |= 0;
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
const int = (n: number) => Math.floor(rand() * n);
const pick = <T>(xs: readonly T[]): T => xs[int(xs.length)]!;

// Plain letters, digits and spaces where sorting may compare them (Rust compares titles without ICU), plus
// Korean, an emoji and markup for the text commands.
const TITLES = ['', 'apple', 'Banana', 'item 2', 'Item 10', 'item 1', '한글', '가나다', 'zeta', 'Alpha beta'];
const TEXTS = [...TITLES, 'emoji 😀 x', '**bold**', 'a **b** c', ' padded ', '__x__', 'line'];

/** A UTF-16 offset in `s` that does not split a surrogate pair. */
function offset(s: string): number {
  const cuts = [0];
  let at = 0;
  for (const ch of s) cuts.push((at += ch.length));
  return pick(cuts);
}

function randomTree(n: number): Tree {
  const t = new Tree({ c: pick(['Title', '', 'Doc 한글']) }, 1000);
  const ids: NodeId[] = [ROOT];
  for (let i = 0; i < n; i++) {
    const parent = rand() < 0.35 ? ROOT : pick(ids);
    const kids = t.get(parent)!.children;
    const after = kids.length && rand() < 0.7 ? pick(kids) : null;
    const p: Props = { c: pick(TEXTS) };
    if (rand() < 0.15) p.n = pick(['note', 'two\nlines', '']);
    if (rand() < 0.2) p.ck = true;
    if (rand() < 0.15) p.cb = true;
    if (rand() < 0.1) p.h = 1 + int(3);
    if (rand() < 0.1) p.cl = 1 + int(6);
    const id = `n${i}`;
    t.apply({ t: 'create', id, parent, after, props: p, ts: 1000 + int(5000) });
    ids.push(id);
  }
  for (const id of ids) if (id !== ROOT && rand() < 0.2) t.apply({ t: 'collapse', id, v: true });
  if (rand() < 0.1) t.apply({ t: 'move', id: pick(ids.slice(1)), parent: null, after: null, ts: 9000 });
  return t;
}

function randomView(t: Tree): View {
  const attached = [...t.nodes.keys()].filter((id) => t.isAttached(id));
  const withKids = attached.filter((id) => t.get(id)!.children.length);
  const zoom = rand() < 0.75 || !withKids.length ? ROOT : pick(withKids);
  const v: View = { zoom, showChecked: rand() < 0.7, filter: null };
  if (rand() < 0.15) {
    // a search result: some matches and their ancestors
    const set = new Set<NodeId>();
    for (let i = 0; i < 4; i++) {
      const m = pick(attached);
      set.add(m);
      for (const a of t.ancestors(m)) set.add(a);
    }
    v.filter = set;
  }
  return v;
}

const viewJson = (v: View) => ({ zoom: v.zoom, showChecked: v.showChecked, filter: v.filter ? [...v.filter].sort() : null });

const text = (t: Tree, id: NodeId) => (t.get(id)?.props.c as string | undefined) ?? '';

function randomPortable(depthLeft: number): Portable {
  const kids = depthLeft > 0 && rand() < 0.4 ? Array.from({ length: 1 + int(2) }, () => randomPortable(depthLeft - 1)) : [];
  const props: Props = { c: pick(TEXTS) };
  if (rand() < 0.3) props.ck = true;
  return { props, collapsed: kids.length > 0 && rand() < 0.3, children: kids };
}

const SORTS: SortMode[] = [
  'title-asc', 'title-desc', 'unchecked-first', 'checked-first', 'edited-new', 'edited-old', 'created-new', 'created-old', 'reverse',
];

interface Case { tree: unknown; view: unknown; cmd: string; args: unknown; ids: string[]; ts: number; result: unknown; ops: unknown; inverse: unknown; canonical: string }
const cases: Case[] = [];
const views: unknown[] = [];

for (let c = 0; c < 1500; c++) {
  const t = randomTree(4 + int(18));
  const v = randomView(t);
  const visible = flatten(t, v);
  if (!visible.length) continue;
  const before = structuredClone(t.toJSON()); // toJSON shares the live nodes, which the command changes
  const one = () => pick(visible);
  const block = () => selectBetween(t, one(), one(), v);
  const textSel = (id: NodeId): Sel => {
    const s = offset(text(t, id));
    return { k: 'text', id, f: 'c', s, e: s };
  };
  const selFor = (ids: NodeId[]): Sel =>
    rand() < 0.6 ? textSel(ids[0]!) : { k: 'block', ids, anchor: ids[0]!, focus: ids[ids.length - 1]! };
  const tx = new Tx(t, 50_000 + c);
  let cmd: string;
  let args: Record<string, unknown>;
  let result: unknown;
  switch (int(23)) {
    case 0: case 1: case 2: {
      const id = rand() < 0.1 ? v.zoom : one();
      const str = text(t, id);
      let s = offset(str);
      let e = rand() < 0.7 ? s : offset(str);
      if (e < s) [s, e] = [e, s];
      cmd = 'enter'; args = { id, s, e };
      result = enter(tx, v, id, s, e);
      break;
    }
    case 3: case 4: {
      const id = one();
      cmd = 'backspaceAtStart'; args = { id };
      result = backspaceAtStart(tx, v, id);
      break;
    }
    case 5: {
      const id = rand() < 0.1 ? v.zoom : one();
      cmd = 'deleteAtEnd'; args = { id };
      result = deleteAtEnd(tx, v, id);
      break;
    }
    case 6: { const ids = block(); const sel = selFor(ids); cmd = 'indent'; args = { ids, sel }; result = indent(tx, v, ids, sel); break; }
    case 7: {
      const ids = block(); const sel = selFor(ids); const inPlace = rand() < 0.4;
      cmd = 'outdent'; args = { ids, sel, inPlace }; result = outdent(tx, v, ids, sel, inPlace); break;
    }
    case 8: { const ids = block(); const sel = selFor(ids); cmd = 'moveUp'; args = { ids, sel }; result = moveUp(tx, v, ids, sel); break; }
    case 9: { const ids = block(); const sel = selFor(ids); cmd = 'moveDown'; args = { ids, sel }; result = moveDown(tx, v, ids, sel); break; }
    case 10: {
      const ids = block(); const dir = pick(['left', 'right'] as const);
      cmd = 'deleteItems'; args = { ids, dir }; result = deleteItems(tx, v, ids, dir); break;
    }
    case 11: { const ids = block(); cmd = 'replaceWithEmpty'; args = { ids }; result = replaceWithEmpty(tx, v, ids); break; }
    case 12: {
      const ids = block(); const sel = selFor(ids); const stay = rand() < 0.3;
      cmd = 'toggleChecked'; args = { ids, sel, stay }; result = toggleChecked(tx, v, ids, sel, { stay }); break;
    }
    case 13: {
      const ids = block(); const sel = selFor(ids); const key = pick(['cb', 'num', 'ck']);
      cmd = 'toggleProp'; args = { ids, key, sel }; result = toggleProp(tx, ids, key, sel); break;
    }
    case 14: {
      const ids = block(); const sel = selFor(ids); const key = pick(['h', 'cl'] as const); const value = int(4);
      cmd = 'setLevel'; args = { ids, key, value, sel }; result = setLevel(tx, ids, key, value, sel); break;
    }
    case 15: {
      const ids = block(); const sel = selFor(ids); const key = pick(['h', 'cl'] as const); const max = key === 'h' ? 3 : 6;
      cmd = 'cycleLevel'; args = { ids, key, max, sel }; result = cycleLevel(tx, ids, key, max, sel); break;
    }
    case 16: {
      const ids = block(); const sel = selFor(ids);
      if (rand() < 0.5) { cmd = 'uncheckDescendants'; args = { ids, sel }; result = uncheckDescendants(tx, ids, sel); }
      else { cmd = 'setChildrenCheckbox'; args = { ids, sel }; result = setChildrenCheckbox(tx, ids, sel); }
      break;
    }
    case 17: {
      const ids = rand() < 0.15 ? [v.zoom] : block(); const r = int(4);
      if (r === 0) { const recursive = rand() < 0.5; cmd = 'toggleCollapse'; args = { ids, recursive }; toggleCollapse(tx, v, ids, recursive); }
      else if (r === 1) { const level = int(3); cmd = 'expandToLevel'; args = { ids, level }; expandToLevel(tx, ids, level); }
      else { const id = ids[0]!; cmd = 'toggleSiblings'; args = { id }; toggleSiblings(tx, id); }
      result = null;
      break;
    }
    case 18: { const ids = block(); cmd = 'duplicate'; args = { ids }; result = duplicate(tx, ids); break; }
    case 19: {
      const id = rand() < 0.1 ? v.zoom : one();
      const str = text(t, id);
      let s = offset(str);
      let e = rand() < 0.7 ? s : offset(str);
      if (e < s) [s, e] = [e, s];
      const items = Array.from({ length: 1 + int(3) }, () => randomPortable(2));
      cmd = 'pasteItems'; args = { id, s, e, items };
      result = pasteItems(tx, v, id, s, e, items);
      break;
    }
    case 20: {
      const parents = [...new Set([v.zoom, ...visible.filter((id) => t.get(id)!.children.length)])];
      const ids = [pick(parents)]; const mode = pick(SORTS); const sel = textSel(ids[0]!);
      cmd = 'sortChildren'; args = { ids, mode, sel }; result = sortChildren(tx, ids, mode, sel); break;
    }
    case 21: {
      const ids = block(); const d = pick(['**', '__', '`']);
      cmd = 'toggleWrapItems'; args = { ids, d }; toggleWrapItems(tx, ids, d); result = null; break;
    }
    default: {
      const a = one(); const b = rand() < 0.6 ? (nextVisible(t, a, v) ?? one()) : one();
      if (a === b) continue;
      const keep = pick(['a', 'b'] as const);
      cmd = 'merge'; args = { a, b, keep }; result = merge(tx, a, b, keep);
      break;
    }
  }
  const ids = tx.ops.filter((o) => o.t === 'create').map((o) => o.id);
  cases.push({ tree: before, view: viewJson(v), cmd, args, ids, ts: tx.ts, result: result ?? null, ops: tx.ops, inverse: tx.inverse, canonical: t.canonical() });
}

// Pure helpers: the view functions and the text edits.
for (let c = 0; c < 400; c++) {
  const t = randomTree(4 + int(18));
  const v = randomView(t);
  const visible = flatten(t, v);
  const all = [...t.nodes.keys()].filter((id) => t.isAttached(id));
  const per = all.map((id) => ({
    id,
    visibleChildren: visibleChildren(t, id, v),
    prevVisible: prevVisible(t, id, v),
    nextVisible: nextVisible(t, id, v),
    nextVisibleNonDescendant: nextVisibleNonDescendant(t, id, v),
    lastVisibleDescendant: lastVisibleDescendant(t, id, v),
    prevVisibleSibling: prevVisibleSibling(t, id, v),
    nextVisibleSibling: nextVisibleSibling(t, id, v),
    depth: depth(t, id, v),
  }));
  const pairs = Array.from({ length: 6 }, () => {
    const a = pick(all); const b = pick(all);
    return { a, b, between: selectBetween(t, a, b, v) };
  });
  const sample = Array.from({ length: 1 + int(6) }, () => pick(all));
  const portableOf = pick(all);
  views.push({
    tree: t.toJSON(), view: viewJson(v), flatten: visible, per, pairs,
    normalize: { ids: sample, out: normalizeBlock(t, sample) },
    portable: { id: portableOf, plain: toPortable(t, portableOf), shown: toPortable(t, portableOf, v) },
  });
}

const edits = Array.from({ length: 400 }, () => {
  const str = pick([...TEXTS, '**x**', 'a**b**c', '[x](y)', 'ab  cd']);
  let s = offset(str);
  let e = offset(str);
  if (e < s) [s, e] = [e, s];
  const d = pick(['**', '__', '`', '~~']);
  return { str, s, e, d, wrap: toggleWrap(str, s, e, d), link: wrapLink(str, s, e) };
});

const out = new URL('./commands.json', import.meta.url);
writeFileSync(out, JSON.stringify({ cases, views, edits }));
console.log(`wrote ${out.pathname}: ${cases.length} command cases`);
