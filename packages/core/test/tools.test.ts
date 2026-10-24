import { describe, expect, it } from 'vitest';
import {
  ROOT, Tree, Tx, backlinkCounts, outdent, countMatches, findBacklinks, parseItemUrl, replaceAll, sortChildren, visibleChildren,
  compileQuery, searchTree, defaultView, type Op,
} from '../src/index.ts';

function tree(items: [string, string, Record<string, unknown>?, number?][], parent = ROOT): Tree {
  const t = new Tree();
  let after: string | null = null;
  for (const [id, c, props = {}, ts = 1] of items) {
    t.apply({ t: 'create', id, parent, after, props: { c, ...props }, ts } as Op);
    after = id;
  }
  return t;
}
const kids = (t: Tree, id = ROOT) => t.get(id)!.children;
const NONE = { k: 'none' } as const;

describe('sortChildren', () => {
  const make = () => tree([
    ['a', 'banana !(2026-03-01)', {}, 3],
    ['b', 'Apple', { ck: true }, 1],
    ['c', 'cherry !(2025-01-01)', {}, 2],
    ['d', 'apple 10', { ck: true }, 4],
    ['e', 'apple 9', {}, 5],
  ]);
  const sorted = (mode: Parameters<typeof sortChildren>[2]) => {
    const t = make();
    sortChildren(new Tx(t, 9), [ROOT], mode, NONE);
    return kids(t).join('');
  };
  it('by title, numeric-aware and case-insensitive', () => {
    expect(sorted('title-asc')).toBe('bedac');
    expect(sorted('title-desc')).toBe('cadeb');
  });
  it('by first date; undated items last', () => {
    expect(sorted('date-new')).toBe('acbde');
    expect(sorted('date-old')).toBe('cabde');
  });
  it('by checked state (stable)', () => {
    expect(sorted('unchecked-first')).toBe('acebd');
    expect(sorted('checked-first')).toBe('bdace');
  });
  it('by created time, and reverse', () => {
    expect(sorted('created-new')).toBe('edacb');
    expect(sorted('created-old')).toBe('bcade');
    expect(sorted('reverse')).toBe('edcba');
  });
});

describe('search & replace', () => {
  it('replaces in text and notes under the scope only', () => {
    const t = tree([['a', 'foo Foo', { n: 'foo' }], ['b', 'bar']]);
    t.apply({ t: 'create', id: 'a1', parent: 'a', after: null, props: { c: 'foofoo' }, ts: 1 });
    expect(countMatches(t, ROOT, 'foo', false)).toBe(5);
    expect(countMatches(t, ROOT, 'foo', true)).toBe(4);
    expect(countMatches(t, 'a1', 'foo', true)).toBe(2);
    const tx = new Tx(t, 2);
    expect(replaceAll(tx, ROOT, 'foo', '$1x', true)).toBe(4);
    expect(t.get('a')!.props).toMatchObject({ c: '$1x Foo', n: '$1x' });
    expect(t.get('a1')!.props.c).toBe('$1x$1x');
    expect(tx.ops.length).toBe(2);
  });
});

describe('backlinks', () => {
  it('finds items linking to an item, in any document', () => {
    const target = tree([['x', 'target'], ['self', 'see [me](#d=D1&z=self)']]);
    const other = tree([['l1', 'see [[target]](#d=D1&z=x)'], ['l2', 'none', { n: 'https://h/app/#d=D1&z=x' }], ['l3', '[doc](#d=D1)']]);
    const docs = [{ doc: 'D1', tree: target }, { doc: 'D2', tree: other }];
    expect(parseItemUrl('https://h/app/#d=D1&z=x')).toEqual({ doc: 'D1', zoom: 'x' });
    expect(parseItemUrl('https://example.com')).toBe(null);
    expect(findBacklinks(docs, 'D1', 'x')).toEqual([{ doc: 'D2', id: 'l1' }, { doc: 'D2', id: 'l2' }]);
    expect(findBacklinks(docs, 'D1', ROOT)).toEqual([{ doc: 'D2', id: 'l3' }]);
    expect(findBacklinks(docs, 'D1', 'self')).toEqual([]); // a link to itself is not a reference
    const counts = backlinkCounts(docs, 'D1');
    expect(counts.get('x')).toBe(2);
    expect(counts.get(ROOT)).toBe(1);
    expect(counts.has('self')).toBe(false);
  });
});

describe('search filter and checked items', () => {
  it('is:completed shows checked matches even when checked items are hidden', () => {
    const t = tree([['a', 'open'], ['b', 'done', { ck: true }]]);
    const v = { ...defaultView(), showChecked: false };
    const m = compileQuery('is:completed', 0);
    const res = searchTree(t, m, ROOT, { showChecked: false });
    expect(visibleChildren(t, ROOT, { ...v, filter: res.visible })).toEqual(['b']);
    const plain = searchTree(t, compileQuery('o', 0), ROOT, { showChecked: false });
    expect(visibleChildren(t, ROOT, { ...v, filter: plain.visible })).toEqual(['a']);
  });
});

describe('outdent in place', () => {
  it('following siblings become children of the unindented item', () => {
    const t = tree([['p', 'P']]);
    let after: string | null = null;
    for (const id of ['a', 'b', 'c']) {
      t.apply({ t: 'create', id, parent: 'p', after, props: { c: id }, ts: 1 });
      after = id;
    }
    t.apply({ t: 'create', id: 'b1', parent: 'b', after: null, props: { c: 'b1' }, ts: 1 });
    const v = defaultView();
    outdent(new Tx(t, 2), v, ['b'], NONE, true);
    expect(kids(t)).toEqual(['p', 'b']);
    expect(kids(t, 'p')).toEqual(['a']);
    expect(kids(t, 'b')).toEqual(['b1', 'c']);
    const t2 = tree([['p', 'P']]);
    for (const id of ['a', 'b']) t2.apply({ t: 'create', id, parent: 'p', after: id === 'a' ? null : 'a', props: { c: id }, ts: 1 });
    outdent(new Tx(t2, 2), v, ['a'], NONE);
    expect(kids(t2)).toEqual(['p', 'a']);
    expect(kids(t2, 'p')).toEqual(['b']);
  });
});
