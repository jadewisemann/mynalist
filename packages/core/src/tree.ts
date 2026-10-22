// Deterministic outline tree + operations.
//
// Every replica (server, web, android) applies the same ops in the same order
// with this exact code, so they converge on identical state. `apply` never
// throws: an op that is invalid against the current state (missing node,
// cycle, ...) is dropped and reported as not applied.

export type NodeId = string;
export type Props = Record<string, unknown>;

export const ROOT: NodeId = 'root';

export interface TNode {
  id: NodeId;
  /** null = root, or detached (deleted; kept so undo can reattach it) */
  parent: NodeId | null;
  children: NodeId[];
  props: Props;
  collapsed: boolean;
  created: number;
  edited: number;
}

/**
 * `after` anchors a position among the parent's children: insert right after
 * that sibling, `null` = first child. A stale anchor (no longer a child of the
 * parent) falls back to appending at the end.
 */
export type Op =
  | { t: 'create'; id: NodeId; parent: NodeId; after: NodeId | null; props: Props; ts: number }
  /** Shallow-merge props; a `null` value deletes the key. */
  | { t: 'set'; id: NodeId; props: Props; ts: number }
  /** parent null = detach (delete). */
  | { t: 'move'; id: NodeId; parent: NodeId | null; after: NodeId | null; ts: number }
  | { t: 'collapse'; id: NodeId; v: boolean };

export interface SerializedTree {
  nodes: TNode[];
}

export class Tree {
  readonly nodes = new Map<NodeId, TNode>();

  constructor(rootProps: Props = {}, ts = 0) {
    this.nodes.set(ROOT, {
      id: ROOT, parent: null, children: [], props: { ...rootProps }, collapsed: false, created: ts, edited: ts,
    });
  }

  static fromJSON(s: SerializedTree): Tree {
    const t = new Tree();
    t.nodes.clear();
    for (const n of s.nodes) t.nodes.set(n.id, { ...n, children: [...n.children], props: { ...n.props } });
    return t;
  }

  toJSON(): SerializedTree {
    return { nodes: [...this.nodes.values()] };
  }

  clone(): Tree {
    return Tree.fromJSON(this.toJSON());
  }

  get(id: NodeId): TNode | undefined {
    return this.nodes.get(id);
  }

  root(): TNode {
    return this.nodes.get(ROOT)!;
  }

  /** True if `id` is reachable from the root (i.e. not deleted). */
  isAttached(id: NodeId): boolean {
    let n = this.nodes.get(id);
    while (n) {
      if (n.id === ROOT) return true;
      if (n.parent === null) return false;
      n = this.nodes.get(n.parent);
    }
    return false;
  }

  isAncestor(anc: NodeId, id: NodeId): boolean {
    let n = this.nodes.get(id);
    while (n && n.parent !== null) {
      if (n.parent === anc) return true;
      n = this.nodes.get(n.parent);
    }
    return false;
  }

  index(id: NodeId): number {
    const n = this.nodes.get(id);
    if (!n || n.parent === null) return -1;
    return this.nodes.get(n.parent)!.children.indexOf(id);
  }

  prevSibling(id: NodeId): NodeId | null {
    const n = this.nodes.get(id);
    if (!n || n.parent === null) return null;
    const sibs = this.nodes.get(n.parent)!.children;
    const i = sibs.indexOf(id);
    return i > 0 ? sibs[i - 1]! : null;
  }

  nextSibling(id: NodeId): NodeId | null {
    const n = this.nodes.get(id);
    if (!n || n.parent === null) return null;
    const sibs = this.nodes.get(n.parent)!.children;
    const i = sibs.indexOf(id);
    return i >= 0 && i < sibs.length - 1 ? sibs[i + 1]! : null;
  }

  ancestors(id: NodeId): NodeId[] {
    const out: NodeId[] = [];
    let n = this.nodes.get(id);
    while (n && n.parent !== null) {
      out.unshift(n.parent);
      n = this.nodes.get(n.parent);
    }
    return out;
  }

  /** Anchor describing the current position of `id` (for inverse moves). */
  anchorOf(id: NodeId): { parent: NodeId | null; after: NodeId | null } {
    const n = this.nodes.get(id);
    if (!n || n.parent === null) return { parent: null, after: null };
    return { parent: n.parent, after: this.prevSibling(id) };
  }

  /** Anchor for inserting at `index` in `parent`'s children. */
  anchorAt(parent: NodeId, index: number): NodeId | null {
    const p = this.nodes.get(parent);
    if (!p || index <= 0) return null;
    return p.children[Math.min(index, p.children.length) - 1] ?? null;
  }

  private insert(parent: TNode, id: NodeId, after: NodeId | null) {
    if (after === null) {
      parent.children.unshift(id);
      return;
    }
    const i = parent.children.indexOf(after);
    if (i < 0) parent.children.push(id);
    else parent.children.splice(i + 1, 0, id);
  }

  private detach(n: TNode) {
    if (n.parent === null) return;
    const p = this.nodes.get(n.parent);
    if (p) {
      const i = p.children.indexOf(n.id);
      if (i >= 0) p.children.splice(i, 1);
    }
    n.parent = null;
  }

  /**
   * Apply an op. Returns the inverse op (for undo) or null if the op was
   * dropped as invalid / no-op.
   */
  apply(op: Op): Op | null {
    switch (op.t) {
      case 'create': {
        if (this.nodes.has(op.id)) return null;
        const p = this.nodes.get(op.parent);
        if (!p) return null;
        const n: TNode = {
          id: op.id, parent: op.parent, children: [], props: cleanProps(op.props),
          collapsed: false, created: op.ts, edited: op.ts,
        };
        this.nodes.set(op.id, n);
        this.insert(p, op.id, op.after);
        return { t: 'move', id: op.id, parent: null, after: null, ts: op.ts };
      }
      case 'set': {
        const n = this.nodes.get(op.id);
        if (!n) return null;
        const old: Props = {};
        let changed = false;
        for (const [k, v] of Object.entries(op.props)) {
          const prev = n.props[k];
          if (v === null || v === undefined || v === '' || v === false || v === 0) {
            if (k in n.props) {
              old[k] = prev;
              delete n.props[k];
              changed = true;
            }
          } else if (prev !== v) {
            old[k] = prev === undefined ? null : prev;
            n.props[k] = v;
            changed = true;
          }
        }
        if (!changed) return null;
        n.edited = op.ts;
        return { t: 'set', id: op.id, props: old, ts: op.ts };
      }
      case 'move': {
        const n = this.nodes.get(op.id);
        if (!n || n.id === ROOT) return null;
        const before = this.anchorOf(op.id);
        if (op.parent === null) {
          if (n.parent === null) return null;
          this.detach(n);
        } else {
          const p = this.nodes.get(op.parent);
          if (!p || op.parent === op.id || this.isAncestor(op.id, op.parent)) return null;
          if (op.after === op.id) return null;
          if (n.parent === op.parent && before.after === op.after) return null;
          this.detach(n);
          n.parent = op.parent;
          this.insert(p, op.id, op.after);
        }
        return { t: 'move', id: op.id, parent: before.parent, after: before.after, ts: op.ts };
      }
      case 'collapse': {
        const n = this.nodes.get(op.id);
        if (!n || n.collapsed === op.v) return null;
        n.collapsed = op.v;
        return { t: 'collapse', id: op.id, v: !op.v };
      }
    }
  }

  /** Drop detached subtrees whose top was last touched before `cutoff`. */
  purgeDetached(cutoff: number) {
    for (const n of [...this.nodes.values()]) {
      if (n.id !== ROOT && n.parent === null && n.edited < cutoff && this.nodes.has(n.id)) {
        const stack = [n.id];
        while (stack.length) {
          const id = stack.pop()!;
          const c = this.nodes.get(id);
          if (!c) continue;
          stack.push(...c.children);
          this.nodes.delete(id);
        }
      }
    }
  }

  /** Stable content hash input (attached nodes only, BFS) for divergence checks. */
  canonical(): string {
    const out: unknown[] = [];
    const queue = [ROOT];
    while (queue.length) {
      const n = this.nodes.get(queue.shift()!)!;
      out.push([n.id, n.children, sortedProps(n.props)]);
      queue.push(...n.children);
    }
    return JSON.stringify(out);
  }
}

/** Falsy values mean "unset" so every replica stores the same shape. */
function cleanProps(p: Props): Props {
  const out: Props = {};
  for (const [k, v] of Object.entries(p)) {
    if (v === null || v === undefined || v === '' || v === false || v === 0) continue;
    out[k] = v;
  }
  return out;
}

function sortedProps(p: Props): Props {
  const out: Props = {};
  for (const k of Object.keys(p).sort()) out[k] = p[k];
  return out;
}
