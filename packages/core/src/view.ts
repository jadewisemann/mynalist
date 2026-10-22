import { ROOT, type NodeId, type Tree } from './tree.ts';

/** What the user is currently looking at; drives visibility. */
export interface View {
  zoom: NodeId;
  showChecked: boolean;
  /**
   * Active search filter: ids that match or are ancestors of a match.
   * When set, collapse is ignored for these nodes and everything else hides.
   */
  filter?: Set<NodeId> | null;
}

export const defaultView = (): View => ({ zoom: ROOT, showChecked: true, filter: null });

export function isHiddenByCheck(t: Tree, id: NodeId, v: View): boolean {
  return !v.showChecked && !!t.get(id)?.props.ck;
}

/** Children shown under `id` (empty if collapsed, except for the zoom root). */
export function visibleChildren(t: Tree, id: NodeId, v: View): NodeId[] {
  const n = t.get(id);
  if (!n) return [];
  if (v.filter) return n.children.filter((c) => v.filter!.has(c) && !isHiddenByCheck(t, c, v));
  if (n.collapsed && id !== v.zoom) return [];
  if (v.showChecked) return n.children;
  return n.children.filter((c) => !t.get(c)?.props.ck);
}

export function hasVisibleChildren(t: Tree, id: NodeId, v: View): boolean {
  return visibleChildren(t, id, v).length > 0;
}

/** All visible items under the zoom root in document order (zoom root excluded). */
export function flatten(t: Tree, v: View): NodeId[] {
  const out: NodeId[] = [];
  const walk = (id: NodeId) => {
    for (const c of visibleChildren(t, id, v)) {
      out.push(c);
      walk(c);
    }
  };
  walk(v.zoom);
  return out;
}

/** Previous visible item in document order (the zoom root counts as the item before its first child). */
export function prevVisible(t: Tree, id: NodeId, v: View): NodeId | null {
  const n = t.get(id);
  if (!n || n.parent === null || id === v.zoom) return null;
  const sibs = visibleChildren(t, n.parent, v);
  const i = sibs.indexOf(id);
  if (i <= 0) return n.parent;
  return lastVisibleDescendant(t, sibs[i - 1]!, v);
}

export function nextVisible(t: Tree, id: NodeId, v: View): NodeId | null {
  const kids = visibleChildren(t, id, v);
  if (kids.length) return kids[0]!;
  return nextVisibleNonDescendant(t, id, v);
}

export function nextVisibleNonDescendant(t: Tree, id: NodeId, v: View): NodeId | null {
  let cur: NodeId = id;
  while (cur !== v.zoom) {
    const n = t.get(cur);
    if (!n || n.parent === null) return null;
    const sibs = visibleChildren(t, n.parent, v);
    const i = sibs.indexOf(cur);
    if (i >= 0 && i < sibs.length - 1) return sibs[i + 1]!;
    cur = n.parent;
  }
  return null;
}

export function lastVisibleDescendant(t: Tree, id: NodeId, v: View): NodeId {
  let cur = id;
  for (;;) {
    const kids = visibleChildren(t, cur, v);
    if (!kids.length) return cur;
    cur = kids[kids.length - 1]!;
  }
}

export function prevVisibleSibling(t: Tree, id: NodeId, v: View): NodeId | null {
  const n = t.get(id);
  if (!n || n.parent === null) return null;
  const sibs = visibleChildren(t, n.parent, v);
  const i = sibs.indexOf(id);
  return i > 0 ? sibs[i - 1]! : null;
}

export function nextVisibleSibling(t: Tree, id: NodeId, v: View): NodeId | null {
  const n = t.get(id);
  if (!n || n.parent === null) return null;
  const sibs = visibleChildren(t, n.parent, v);
  const i = sibs.indexOf(id);
  return i >= 0 && i < sibs.length - 1 ? sibs[i + 1]! : null;
}

/** Depth relative to the zoom root (its children are depth 0). */
export function depth(t: Tree, id: NodeId, v: View): number {
  let d = 0;
  let n = t.get(id);
  while (n && n.parent !== null && n.parent !== v.zoom) {
    d++;
    n = t.get(n.parent);
  }
  return d;
}

/**
 * Normalize a block selection between `a` and `b` to contiguous visible
 * siblings under their deepest common ancestor (Dynalist "select between").
 */
export function selectBetween(t: Tree, a: NodeId, b: NodeId, v: View): NodeId[] {
  if (a === b) return [a];
  const pa = [...t.ancestors(a), a];
  const pb = [...t.ancestors(b), b];
  let i = 0;
  while (i < pa.length && i < pb.length && pa[i] === pb[i]) i++;
  if (i === pa.length) return [a];
  if (i === pb.length) return [b];
  const parent = pa[i - 1]!;
  const x = pa[i]!;
  const y = pb[i]!;
  const sibs = visibleChildren(t, parent, v);
  const ix = sibs.indexOf(x);
  const iy = sibs.indexOf(y);
  if (ix < 0 || iy < 0) return [a];
  return sibs.slice(Math.min(ix, iy), Math.max(ix, iy) + 1);
}

/** Sort ids into document order and drop ids that are descendants of other ids. */
export function normalizeBlock(t: Tree, ids: NodeId[]): NodeId[] {
  const set = new Set(ids);
  const top = ids.filter((id) => !t.ancestors(id).some((a) => set.has(a)));
  const order = new Map<NodeId, number>();
  let k = 0;
  const walk = (id: NodeId) => {
    order.set(id, k++);
    for (const c of t.get(id)?.children ?? []) walk(c);
  };
  walk(ROOT);
  return [...new Set(top)].sort((x, y) => (order.get(x) ?? 0) - (order.get(y) ?? 0));
}
