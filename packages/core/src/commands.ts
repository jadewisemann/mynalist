// Editor commands. Each command runs against a Tx (ops applied to the live
// tree immediately, recorded for sync + undo) and returns the selection to
// show afterwards, or null when it does nothing.

import { advanceRecurrence } from './dates.ts';
import { newId } from './ids.ts';
import { metaOf } from './item.ts';
import { ROOT, type NodeId, type Op, type Props, type Tree } from './tree.ts';
import {
  type View, nextVisible, nextVisibleNonDescendant, nextVisibleSibling, prevVisible,
  prevVisibleSibling, visibleChildren,
} from './view.ts';

export type Sel =
  | { k: 'none' }
  | { k: 'text'; id: NodeId; f: 'c' | 'n'; s: number; e: number }
  | { k: 'block'; ids: NodeId[]; anchor: NodeId; focus: NodeId };

export interface CmdResult {
  sel: Sel;
  /** change zoom (e.g. outdent out of the zoom root) */
  zoom?: NodeId;
}

export const caret = (id: NodeId, s: number, f: 'c' | 'n' = 'c'): Sel => ({ k: 'text', id, f, s, e: s });

/** Applies ops to a tree as they are issued, recording them and their inverses. */
export class Tx {
  readonly ops: Op[] = [];
  /** inverse ops in undo order */
  readonly inverse: Op[] = [];
  readonly ts: number;
  readonly tree: Tree;
  constructor(tree: Tree, ts = Date.now()) {
    this.tree = tree;
    this.ts = ts;
  }

  do(op: Op): boolean {
    const inv = this.tree.apply(op);
    if (!inv) return false;
    this.ops.push(op);
    this.inverse.unshift(inv);
    return true;
  }

  create(parent: NodeId, after: NodeId | null, props: Props, id = newId()): NodeId {
    this.do({ t: 'create', id, parent, after, props, ts: this.ts });
    return id;
  }

  set(id: NodeId, props: Props) {
    return this.do({ t: 'set', id, props, ts: this.ts });
  }

  move(id: NodeId, parent: NodeId | null, after: NodeId | null) {
    return this.do({ t: 'move', id, parent, after, ts: this.ts });
  }

  collapse(id: NodeId, v: boolean) {
    return this.do({ t: 'collapse', id, v });
  }

  /** Undo everything done in this tx (used when a command bails out midway). */
  rollback() {
    for (const op of this.inverse) this.tree.apply(op);
    this.ops.length = 0;
    this.inverse.length = 0;
  }
}

const text = (t: Tree, id: NodeId): string => (t.get(id)?.props.c as string | undefined) ?? '';

/** Replace all non-text props of `id` with `meta`. */
function setMeta(tx: Tx, id: NodeId, meta: Props) {
  const cur = metaOf(tx.tree.get(id)!.props);
  const patch: Props = {};
  for (const k of Object.keys(cur)) if (!(k in meta)) patch[k] = null;
  Object.assign(patch, meta);
  tx.set(id, patch);
}

// ---- Enter / Backspace / Delete ------------------------------------------

export function enter(tx: Tx, v: View, id: NodeId, s: number, e: number): CmdResult | null {
  const t = tx.tree;
  const n = t.get(id);
  if (!n) return null;
  const str = text(t, id);
  const cb = n.props.cb ? { cb: true } : {};

  if (str === '' && id !== ROOT && id !== v.zoom && n.parent !== v.zoom && !nextVisibleSibling(t, id, v)) {
    return outdent(tx, v, [id], caret(id, 0));
  }
  if (id === v.zoom || n.parent === null) {
    // title: head stays, tail becomes the first child
    const tail = str.slice(e);
    if (tail) tx.set(id, { c: str.slice(0, s) });
    const nid = tx.create(id, null, { c: tail });
    return { sel: caret(nid, 0) };
  }
  if (s === e && s >= str.length) {
    const asChild = visibleChildren(t, id, v).length > 0;
    const nid = asChild ? tx.create(id, null, { ...cb }) : tx.create(n.parent, id, { ...cb });
    return { sel: caret(nid, 0) };
  }
  const head = str.slice(0, s);
  const tail = str.slice(e);
  const nid = tx.create(n.parent, t.prevSibling(id), { c: head, ...cb });
  tx.set(id, { c: tail });
  return { sel: s === 0 ? caret(nid, 0) : caret(id, 0) };
}

/**
 * Merge `a` (earlier) and `b` (later). The one with children survives; if
 * neither has, `a` does; if both have, refuse. Returns the survivor and the
 * caret offset at the join.
 */
/**
 * Join b's text onto a's. The item with children survives (in its place); when neither has any, `keep` decides:
 * Backspace keeps a (the merged item stays where the caret was), Delete keeps b (Dynalist).
 */
export function merge(tx: Tx, a: NodeId, b: NodeId, keep: 'a' | 'b' = 'a'): { id: NodeId; at: number } | null {
  const t = tx.tree;
  const na = t.get(a)!;
  const nb = t.get(b)!;
  const aKids = na.children.some((c) => c !== b);
  const bKids = nb.children.length > 0;
  if (aKids && bKids) return null;
  // b is a's first child and has children of its own: b can't survive (it would go down with a), and a
  // can't take b's children without reordering them, so nothing happens (Dynalist)
  if (bKids && nb.parent === a) return null;
  const ta = text(t, a);
  const tb = text(t, b);
  const ma = metaOf(na.props);
  const mb = metaOf(nb.props);
  const meta = ta && !tb ? ma : tb && !ta ? mb : { ...ma, ...mb };
  const noteA = (na.props.n as string | undefined) ?? '';
  const noteB = (nb.props.n as string | undefined) ?? '';
  const survivor = nb.parent === a || aKids ? a : bKids || keep === 'b' ? b : a;
  const victim = survivor === a ? b : a;
  tx.move(victim, null, null);
  tx.set(survivor, { c: ta + tb, n: survivor === a ? noteA || noteB : noteB || noteA });
  setMeta(tx, survivor, meta);
  return { id: survivor, at: ta.length };
}

export function backspaceAtStart(tx: Tx, v: View, id: NodeId): CmdResult | null {
  const t = tx.tree;
  const n = t.get(id);
  if (!n || id === v.zoom || n.parent === null) return null;
  const prev = prevVisible(t, id, v);
  if (prev === v.zoom) {
    // a childless, note-less first item merges into the title (Dynalist); the title keeps its own props
    if (n.children.length || n.props.n) return null;
    const at = text(t, prev).length;
    tx.move(id, null, null);
    tx.set(prev, { c: text(t, prev) + text(t, id) });
    return { sel: caret(prev, at) };
  }
  if (n.children.length === 0 && prev) {
    const r = merge(tx, prev, id);
    return r && { sel: caret(r.id, r.at) };
  }
  const ps = prevVisibleSibling(t, id, v);
  if (ps) {
    const r = merge(tx, ps, id);
    return r && { sel: caret(r.id, r.at) };
  }
  return null;
}

export function deleteAtEnd(tx: Tx, v: View, id: NodeId): CmdResult | null {
  const t = tx.tree;
  const next = nextVisible(t, id, v);
  if (!next) return null;
  const n = t.get(id)!;
  if (id !== v.zoom && !text(t, id) && !n.children.length) {
    // an empty item goes away instead of pulling the next one in (which keeps its own place, as Dynalist)
    tx.move(id, null, null);
    return { sel: caret(next, 0) };
  }
  const r = merge(tx, id, next, 'b');
  return r && { sel: caret(r.id, r.at) };
}

// ---- structure -----------------------------------------------------------

export function indent(tx: Tx, v: View, ids: NodeId[], sel: Sel): CmdResult | null {
  const t = tx.tree;
  const prev = prevVisibleSibling(t, ids[0]!, v);
  if (!prev) return null;
  tx.collapse(prev, false);
  for (const id of ids) {
    const kids = t.get(prev)!.children;
    tx.move(id, prev, kids.length ? kids[kids.length - 1]! : null);
  }
  return { sel };
}

export function outdent(tx: Tx, v: View, ids: NodeId[], sel: Sel): CmdResult | null {
  const t = tx.tree;
  const parent = t.get(ids[0]!)?.parent;
  if (!parent) return null;
  const gp = t.get(parent)?.parent;
  if (!gp) return null;
  let after: NodeId = parent;
  for (const id of ids) {
    tx.move(id, gp, after);
    after = id;
  }
  return parent === v.zoom ? { sel, zoom: gp } : { sel };
}

export function moveUp(tx: Tx, v: View, ids: NodeId[], sel: Sel): CmdResult | null {
  const t = tx.tree;
  const first = ids[0]!;
  const parent = t.get(first)?.parent;
  if (!parent) return null;
  const prev = prevVisibleSibling(t, first, v);
  let target: NodeId;
  let after: NodeId | null;
  if (prev) {
    target = parent;
    after = t.prevSibling(prev);
  } else {
    if (parent === v.zoom) return null;
    const pp = prevVisibleSibling(t, parent, v);
    const gp = t.get(parent)!.parent;
    if (!gp) return null;
    // into the parent's previous sibling (also a childless one), unless it's collapsed (Dynalist)
    if (pp && (visibleChildren(t, pp, v).length > 0 || t.get(pp)!.children.length === 0)) {
      target = pp;
      const kids = t.get(pp)!.children;
      after = kids[kids.length - 1] ?? null;
    } else {
      target = gp;
      after = t.prevSibling(parent);
    }
  }
  for (const id of ids) {
    tx.move(id, target, after);
    after = id;
  }
  return tx.ops.length ? { sel } : null;
}

export function moveDown(tx: Tx, v: View, ids: NodeId[], sel: Sel): CmdResult | null {
  const t = tx.tree;
  const last = ids[ids.length - 1]!;
  const parent = t.get(last)?.parent;
  if (!parent) return null;
  const next = nextVisibleSibling(t, last, v);
  let target: NodeId;
  let after: NodeId | null;
  if (next) {
    target = parent;
    after = next;
  } else {
    if (parent === v.zoom) return null;
    const pn = nextVisibleSibling(t, parent, v);
    const gp = t.get(parent)!.parent;
    if (!gp) return null;
    if (pn && (visibleChildren(t, pn, v).length > 0 || t.get(pn)!.children.length === 0)) {
      target = pn;
      after = null;
    } else {
      target = gp;
      after = parent;
    }
  }
  for (const id of ids) {
    tx.move(id, target, after);
    after = id;
  }
  return tx.ops.length ? { sel } : null;
}

export function deleteItems(tx: Tx, v: View, ids: NodeId[], dir: 'left' | 'right' = 'left'): CmdResult | null {
  const t = tx.tree;
  if (ids.some((id) => id === v.zoom || id === ROOT)) return null;
  const first = ids[0]!;
  const last = ids[ids.length - 1]!;
  const prev = prevVisible(t, first, v);
  const before = prev !== v.zoom ? prev : null; // the title never takes the caret here
  const after = nextVisibleNonDescendant(t, last, v);
  for (const id of ids) tx.move(id, null, null);
  if (before && (dir === 'left' || !after)) return { sel: caret(before, text(t, before).length) };
  if (after) return { sel: caret(after, 0) };
  return { sel: { k: 'none' } };
}

/** Block Enter: replace the selection with one empty item. */
export function replaceWithEmpty(tx: Tx, v: View, ids: NodeId[]): CmdResult | null {
  const t = tx.tree;
  const first = t.get(ids[0]!);
  if (!first?.parent) return null;
  const nid = tx.create(first.parent, t.prevSibling(first.id), {});
  for (const id of ids) tx.move(id, null, null);
  return { sel: caret(nid, 0) };
}

// ---- item-level toggles ----------------------------------------------------

export interface CheckOptions {
  /**
   * Checking an item whose first date recurs: `new_instance` inserts a checked
   * copy above and advances the date in the (still unchecked) original;
   * `update` only advances the date. Omitted: recurrence is ignored.
   */
  recurring?: 'new_instance' | 'update';
  /** completion time for "from completion" recurrences (default: tx time) */
  now?: number;
}

export function toggleChecked(tx: Tx, v: View, ids: NodeId[], sel: Sel, opts: CheckOptions = {}): CmdResult | null {
  const t = tx.tree;
  const targets = ids.filter((id) => id !== ROOT);
  if (!targets.length) return null;
  const all = targets.every((id) => t.get(id)?.props.ck);
  let advanced = 0;
  for (const id of targets) {
    const n = t.get(id);
    if (!all && opts.recurring && n && !n.props.ck && n.parent) {
      const adv = advanceRecurrence(text(t, id), opts.now ?? tx.ts);
      if (adv) {
        if (opts.recurring === 'new_instance') tx.create(n.parent, t.prevSibling(id), { ...n.props, ck: true });
        tx.set(id, { c: adv.text });
        advanced++;
        continue;
      }
    }
    tx.set(id, { ck: all ? null : true });
  }
  if ((sel.k === 'text' || sel.k === 'block') && advanced < targets.length) {
    // checking or unchecking moves on to the start of the next item (Dynalist)
    const next = nextVisibleNonDescendant(t, targets[targets.length - 1]!, v);
    if (next) return { sel: caret(next, 0) };
  }
  if (sel.k === 'text' && advanced) {
    const len = text(t, sel.id).length;
    return { sel: { ...sel, s: Math.min(sel.s, len), e: Math.min(sel.e, len) } };
  }
  return { sel };
}

export function toggleProp(tx: Tx, ids: NodeId[], key: string, sel: Sel): CmdResult {
  const all = ids.every((id) => tx.tree.get(id)?.props[key]);
  for (const id of ids) tx.set(id, { [key]: all ? null : true });
  return { sel };
}

/** Set heading/color to `value`; if every item already has it, clear instead. */
export function setLevel(tx: Tx, ids: NodeId[], key: 'h' | 'cl', value: number, sel: Sel): CmdResult {
  const all = ids.every((id) => (tx.tree.get(id)?.props[key] ?? 0) === value);
  for (const id of ids) tx.set(id, { [key]: all ? null : value || null });
  return { sel };
}

export function cycleLevel(tx: Tx, ids: NodeId[], key: 'h' | 'cl', max: number, sel: Sel): CmdResult {
  const cur = (tx.tree.get(ids[0]!)?.props[key] as number | undefined) ?? 0;
  const next = (cur + 1) % (max + 1);
  for (const id of ids) tx.set(id, { [key]: next || null });
  return { sel };
}

export function uncheckDescendants(tx: Tx, ids: NodeId[], sel: Sel): CmdResult {
  const walk = (id: NodeId) => {
    tx.set(id, { ck: null });
    for (const c of tx.tree.get(id)?.children ?? []) walk(c);
  };
  ids.forEach(walk);
  return { sel };
}

export function setChildrenCheckbox(tx: Tx, ids: NodeId[], sel: Sel): CmdResult {
  const kids = ids.flatMap((id) => tx.tree.get(id)?.children ?? []);
  const all = kids.length > 0 && kids.every((c) => tx.tree.get(c)?.props.cb);
  const walk = (id: NodeId) => {
    for (const c of tx.tree.get(id)?.children ?? []) {
      tx.set(c, { cb: all ? null : true });
      walk(c);
    }
  };
  ids.forEach(walk);
  return { sel };
}

/** Collapse/expand. Not undoable (collapse ops are excluded from undo). */
export function toggleCollapse(tx: Tx, v: View, ids: NodeId[], recursive: boolean): void {
  const t = tx.tree;
  const targets = ids.flatMap((id) => (id === v.zoom ? t.get(id)!.children : [id]));
  const withKids = targets.filter((id) => (t.get(id)?.children.length ?? 0) > 0);
  if (!withKids.length) return;
  const collapse = !withKids.every((id) => t.get(id)!.collapsed);
  const walk = (id: NodeId) => {
    const n = t.get(id);
    if (!n || n.children.length === 0) return;
    tx.collapse(id, collapse);
    if (recursive) n.children.forEach(walk);
  };
  withKids.forEach(walk);
}

export function expandToLevel(tx: Tx, ids: NodeId[], level: number): void {
  const walk = (id: NodeId, d: number) => {
    const n = tx.tree.get(id);
    if (!n || n.children.length === 0) return;
    tx.collapse(id, d >= level);
    for (const c of n.children) walk(c, d + 1);
  };
  for (const id of ids) walk(id, 0);
}

export function toggleSiblings(tx: Tx, id: NodeId): void {
  const p = tx.tree.get(id)?.parent;
  if (!p) return;
  const sibs = tx.tree.get(p)!.children.filter((c) => (tx.tree.get(c)?.children.length ?? 0) > 0);
  const collapse = !sibs.every((c) => tx.tree.get(c)!.collapsed);
  for (const c of sibs) tx.collapse(c, collapse);
}

// ---- copy / insert ---------------------------------------------------------

/** Portable item tree (clipboard / import / duplicate). */
export interface Portable {
  props: Props;
  collapsed?: boolean;
  children: Portable[];
}

export function toPortable(t: Tree, id: NodeId, v?: View): Portable {
  const n = t.get(id)!;
  const kids = v ? visibleChildren(t, id, { ...v, zoom: '' }) : n.children;
  return { props: { ...n.props }, collapsed: n.collapsed, children: kids.map((c) => toPortable(t, c, v)) };
}

/** Insert items as siblings after `after` under `parent`; returns new top-level ids. */
export function insertPortable(tx: Tx, parent: NodeId, after: NodeId | null, items: Portable[]): NodeId[] {
  return insertDeep(tx, parent, after, items).ids;
}

/** Like insertPortable, also reporting the last created node in document order. */
function insertDeep(tx: Tx, parent: NodeId, after: NodeId | null, items: Portable[]): { ids: NodeId[]; last: NodeId | null } {
  const ids: NodeId[] = [];
  let last: NodeId | null = null;
  let anchor = after;
  for (const it of items) {
    const id = tx.create(parent, anchor, it.props);
    const sub = insertDeep(tx, id, null, it.children);
    if (it.collapsed && it.children.length) tx.collapse(id, true);
    ids.push(id);
    last = sub.last ?? id;
    anchor = id;
  }
  return { ids, last };
}

export function duplicate(tx: Tx, ids: NodeId[]): CmdResult | null {
  const t = tx.tree;
  const last = ids[ids.length - 1]!;
  const parent = t.get(last)?.parent;
  if (!parent) return null;
  const copies = ids.map((id) => toPortable(t, id));
  const nids = insertPortable(tx, parent, last, copies);
  return { sel: { k: 'block', ids: nids, anchor: nids[0]!, focus: nids[nids.length - 1]! } };
}

/**
 * Paste structured items at a caret: first item's text is spliced into the
 * current item at the caret, the remaining text after the caret goes to the
 * end of the last pasted line; first item's children go under the current item.
 */
export function pasteItems(tx: Tx, v: View, id: NodeId, s: number, e: number, items: Portable[]): CmdResult | null {
  const t = tx.tree;
  const n = t.get(id);
  if (!n || !items.length) return null;
  const str = text(t, id);
  const head = str.slice(0, s);
  const tail = str.slice(e);
  const [first, ...rest] = items;
  const firstText = (first!.props.c as string | undefined) ?? '';
  if (id === v.zoom || n.parent === null) {
    // into title: insert everything as first children
    const ids = insertPortable(tx, id, null, items);
    const lastId = ids[ids.length - 1]!;
    return { sel: caret(lastId, text(t, lastId).length) };
  }
  tx.set(id, { c: head + firstText });
  if (!head) setMeta(tx, id, { ...metaOf(n.props), ...metaOf(first!.props) });
  const sub = insertDeep(tx, id, null, first!.children);
  const sibs = insertDeep(tx, n.parent, id, rest);
  const lastId = sibs.last ?? sub.last ?? id;
  const lastText = text(t, lastId);
  if (tail) tx.set(lastId, { c: lastText + tail });
  return { sel: caret(lastId, lastText.length) };
}

// ---- inline formatting on raw text ----------------------------------------

export interface TextEdit {
  text: string;
  s: number;
  e: number;
}

/** Toggle a symmetric delimiter (e.g. "**") around [s, e). */
export function toggleWrap(str: string, s: number, e: number, d: string): TextEdit {
  const L = d.length;
  if (s >= L && str.slice(s - L, s) === d && str.slice(e, e + L) === d) {
    return { text: str.slice(0, s - L) + str.slice(s, e) + str.slice(e + L), s: s - L, e: e - L };
  }
  const sel = str.slice(s, e);
  if (sel.length >= 2 * L && sel.startsWith(d) && sel.endsWith(d)) {
    const inner = sel.slice(L, sel.length - L);
    return { text: str.slice(0, s) + inner + str.slice(e), s, e: s + inner.length };
  }
  let a = s;
  let b = e;
  if (b - a > 1 && str[a] === ' ') a++;
  if (b - a > 1 && str[b - 1] === ' ') b--;
  return { text: str.slice(0, a) + d + str.slice(a, b) + d + str.slice(b), s: a + L, e: b + L };
}

/** Mod+K: `[sel]()` with caret inside `()`; without selection `[]()` with caret in `[]`. */
export function wrapLink(str: string, s: number, e: number): TextEdit {
  const sel = str.slice(s, e);
  const ins = `[${sel}]()`;
  const out = str.slice(0, s) + ins + str.slice(e);
  const at = sel ? s + sel.length + 3 : s + 1;
  return { text: out, s: at, e: at };
}

/** Toggle delimiter around the whole text of every item (block formatting). */
export function toggleWrapItems(tx: Tx, ids: NodeId[], d: string): void {
  const wrapped = (s: string) => s.length >= 2 * d.length && s.startsWith(d) && s.endsWith(d);
  const all = ids.every((id) => wrapped(text(tx.tree, id)));
  for (const id of ids) {
    const s = text(tx.tree, id);
    if (all) tx.set(id, { c: s.slice(d.length, s.length - d.length) });
    else if (!wrapped(s) && s) tx.set(id, { c: d + s + d });
  }
}
