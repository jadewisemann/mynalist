// Restoring an older version in place: the ops that turn the live tree into `target`, applied as one
// transaction (so the restore is undoable and syncs like any edit; nothing is lost, only detached).

import { ROOT, type NodeId, type Props, type Tree } from './tree.ts';
import type { Tx } from './commands.ts';

function propsPatch(cur: Props, want: Props): Props | null {
  const patch: Props = {};
  let any = false;
  for (const k of Object.keys(cur)) if (!(k in want)) (patch[k] = null), (any = true);
  for (const [k, v] of Object.entries(want)) if (cur[k] !== v) (patch[k] = v), (any = true);
  return any ? patch : null;
}

export function restoreTree(tx: Tx, target: Tree): void {
  const t = tx.tree;
  const keep = new Set<NodeId>();
  const sync = (id: NodeId) => {
    const want = target.get(id)!;
    const patch = propsPatch(t.get(id)!.props, want.props);
    if (patch) tx.set(id, patch);
    if (t.get(id)!.collapsed !== want.collapsed) tx.collapse(id, want.collapsed);
    let prev: NodeId | null = null;
    for (const c of want.children) {
      keep.add(c);
      if (!t.get(c)) tx.create(id, prev, target.get(c)!.props, c);
      else if (t.get(c)!.parent !== id || t.prevSibling(c) !== prev) tx.move(c, id, prev);
      sync(c);
      prev = c;
    }
  };
  sync(ROOT);
  const prune = (id: NodeId) => {
    for (const c of [...t.get(id)!.children]) {
      if (keep.has(c)) prune(c);
      else tx.move(c, null, null);
    }
  };
  prune(ROOT);
}
