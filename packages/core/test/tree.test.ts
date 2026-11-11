import { describe, expect, it } from 'vitest';
import { DocSession, ROOT, Tree, type Op } from '../src/index.ts';

const c = (id: string, parent = ROOT, after: string | null = null, text = id): Op =>
  ({ t: 'create', id, parent, after, props: { c: text }, ts: 1 });

describe('Tree', () => {
  it('creates, moves, detaches and inverts', () => {
    const t = new Tree();
    t.apply(c('a'));
    t.apply(c('b', ROOT, 'a'));
    t.apply(c('x', 'a'));
    expect(t.root().children).toEqual(['a', 'b']);
    const inv = t.apply({ t: 'move', id: 'b', parent: 'a', after: 'x', ts: 2 })!;
    expect(t.get('a')!.children).toEqual(['x', 'b']);
    t.apply(inv);
    expect(t.root().children).toEqual(['a', 'b']);
    // cycle dropped
    expect(t.apply({ t: 'move', id: 'a', parent: 'x', after: null, ts: 3 })).toBeNull();
  });

  it('restores multi-delete order via reversed inverses', () => {
    const t = new Tree();
    for (const [id, after] of [['a', null], ['b', 'a'], ['c', 'b'], ['d', 'c']] as const) t.apply(c(id, ROOT, after));
    const inv: Op[] = [];
    for (const id of ['b', 'c']) inv.unshift(t.apply({ t: 'move', id, parent: null, after: null, ts: 2 })!);
    expect(t.root().children).toEqual(['a', 'd']);
    for (const op of inv) t.apply(op);
    expect(t.root().children).toEqual(['a', 'b', 'c', 'd']);
  });
});

describe('DocSession sync', () => {
  it('two clients converge through a simulated server', () => {
    const server = { tree: new Tree(), log: [] as Op[][] };
    const sync = (s: DocSession) => {
      const req = s.beginSync()!;
      const remote = server.log.slice(req.baseVersion).flat();
      if (req.ops.length) {
        for (const op of req.ops) server.tree.apply(op);
        server.log.push(req.ops);
      }
      s.endSync({ version: server.log.length, remote });
    };
    const A = new DocSession();
    const B = new DocSession();
    A.transact([c('a')], { before: null, after: null });
    sync(A);
    sync(B);
    A.transact([{ t: 'set', id: 'a', props: { c: 'from A' }, ts: 5 }], { before: null, after: null });
    B.transact([c('b', ROOT, 'a'), { t: 'move', id: 'a', parent: null, after: null, ts: 6 }], { before: null, after: null });
    sync(A);
    sync(B);
    sync(A);
    expect(A.live.canonical()).toEqual(server.tree.canonical());
    expect(B.live.canonical()).toEqual(server.tree.canonical());
    expect(A.live.root().children).toEqual(['b']);
  });

  it('coalesces typing into one undo step', () => {
    const s = new DocSession();
    s.transact([c('a', ROOT, null, '')], { before: 0, after: 1 });
    for (const text of ['h', 'he', 'hel']) s.transact([{ t: 'set', id: 'a', props: { c: text }, ts: 1 }], { before: 0, after: 0 });
    s.undo();
    expect(s.live.get('a')!.props.c).toBeUndefined();
    s.undo();
    expect(s.live.get('a')!.parent).toBeNull();
    s.redo();
    s.redo();
    expect(s.live.get('a')!.props.c).toBe('hel');
  });
});

describe('kidsRev', () => {
  it('changes when the children change, and with collapse and numbering', () => {
    const t = new Tree();
    t.apply(c('a'));
    const before = t.kidsRev(ROOT);
    expect(t.kidsRev(ROOT)).toBe(before); // stable while nothing changes
    t.apply(c('b', ROOT, 'a'));
    const added = t.kidsRev(ROOT);
    expect(added).not.toBe(before);
    t.apply({ t: 'move', id: 'b', parent: 'a', after: null, ts: 2 });
    expect(t.kidsRev(ROOT)).not.toBe(added);
    // collapse and numbering are part of the stamp: they change how the children block is drawn
    const plain = t.kidsRev('a');
    t.apply({ t: 'collapse', id: 'a', v: true });
    expect(t.kidsRev('a')).not.toBe(plain);
    const shut = t.kidsRev('a');
    t.apply({ t: 'set', id: 'a', props: { num: true }, ts: 4 });
    expect(t.kidsRev('a')).not.toBe(shut);
  });

  it('never repeats a stamp for the same node', () => {
    const t = new Tree();
    const seen = new Set<number>();
    for (let i = 0; i < 50; i++) {
      t.apply(c(`n${i}`));
      const r = t.kidsRev(ROOT);
      expect(seen.has(r)).toBe(false);
      seen.add(r);
    }
  });
});
