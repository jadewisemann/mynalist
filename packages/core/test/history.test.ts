import { describe, expect, it } from 'vitest';
import { DocSession, ROOT, Tx, restoreTree, type Op } from '../src/index.ts';

const NO_SEL = { before: null, after: null };

function build(spec: [string, string | null, string][]): Op[] {
  // [id, parent (null = root), text], in order
  const last = new Map<string, string | null>();
  return spec.map(([id, p, c]) => {
    const parent = p ?? ROOT;
    const after = last.get(parent) ?? null;
    last.set(parent, id);
    return { t: 'create', id, parent, after, props: { c }, ts: 1 } as Op;
  });
}

describe('restoreTree', () => {
  it('turns the live tree into the target in one undoable transaction', () => {
    const s = new DocSession();
    s.transact(build([['a', null, 'A'], ['b', null, 'B'], ['b1', 'b', 'B1'], ['c', null, 'C']]), NO_SEL);
    const old = s.live.clone();
    // later edits: rename, move, delete, new item, collapse, title
    s.transact([
      { t: 'set', id: 'a', props: { c: 'A2', ck: true }, ts: 2 },
      { t: 'move', id: 'c', parent: 'a', after: null, ts: 2 },
      { t: 'move', id: 'b1', parent: null, after: null, ts: 2 },
      { t: 'create', id: 'd', parent: ROOT, after: null, props: { c: 'D' }, ts: 2 },
      { t: 'collapse', id: 'a', v: true },
      { t: 'set', id: ROOT, props: { c: 'Title' }, ts: 2 },
    ], NO_SEL);
    const live = s.live.clone();
    const tx = new Tx(live, 3);
    restoreTree(tx, old);
    expect(live.canonical()).toBe(old.canonical());
    for (const id of ['a', 'b', 'c']) expect(live.get(id)?.collapsed).toBe(false);
    // the same ops replayed on another replica converge too
    const other = s.live.clone();
    for (const op of tx.ops) other.apply(op);
    expect(other.canonical()).toBe(old.canonical());
    // and restoring again is a no-op
    const again = new Tx(live, 4);
    restoreTree(again, old);
    expect(again.ops).toEqual([]);
  });

  it('restores through a session; undo brings the newer state back', () => {
    const s = new DocSession();
    s.transact(build([['a', null, 'A'], ['b', null, 'B'], ['b1', 'b', 'B1']]), NO_SEL);
    const old = s.live.clone();
    s.transact([
      { t: 'move', id: 'b1', parent: ROOT, after: null, ts: 2 },
      { t: 'move', id: 'a', parent: null, after: null, ts: 2 },
      { t: 'create', id: 'x', parent: 'b', after: null, props: { c: 'X', h: 2 }, ts: 2 },
    ], NO_SEL);
    const newer = s.live.canonical();
    const tx = new Tx(s.live.clone(), 3);
    restoreTree(tx, old);
    s.transact(tx.ops, NO_SEL);
    expect(s.live.canonical()).toBe(old.canonical());
    expect(s.live.get('x')?.parent).toBe(null); // detached, not destroyed
    s.undo();
    expect(s.live.canonical()).toBe(newer);
  });
});
