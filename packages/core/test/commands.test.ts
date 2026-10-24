import { describe, expect, it } from 'vitest';
import {
  ROOT, Tree, Tx, backspaceAtStart, defaultView, deleteItems, enter, indent, moveDown, moveUp, outdent,
  pasteItems, toggleChecked, toggleWrap, wrapLink, type Op, type Portable,
} from '../src/index.ts';

/** Build a tree from an indented outline: "a\n  b\n  c\nd" (ids = texts). */
function build(src: string): Tree {
  const t = new Tree();
  const stack: { id: string; ind: number }[] = [{ id: ROOT, ind: -1 }];
  const lastChild = new Map<string, string>();
  for (const line of src.split('\n').filter(Boolean)) {
    const ind = line.length - line.trimStart().length;
    const id = line.trim();
    while (stack[stack.length - 1]!.ind >= ind) stack.pop();
    const parent = stack[stack.length - 1]!.id;
    t.apply({ t: 'create', id, parent, after: lastChild.get(parent) ?? null, props: { c: id }, ts: 1 } as Op);
    lastChild.set(parent, id);
    stack.push({ id, ind });
  }
  return t;
}

/** Render tree as indented outline of contents. */
function show(t: Tree, id = ROOT, d = 0): string {
  return t.get(id)!.children.map((c) => '  '.repeat(d) + (t.get(c)!.props.c ?? '') + '\n' + show(t, c, d + 1)).join('');
}

const v = defaultView();
const sel = { k: 'none' } as const;

describe('enter', () => {
  it('splits mid-text: head becomes new item above, original keeps tail', () => {
    const t = build('hello\n  kid');
    const tx = new Tx(t);
    const r = enter(tx, v, 'hello', 2, 2)!;
    expect(show(t)).toBe('he\nllo\n  kid\n');
    expect(r.sel).toMatchObject({ id: 'hello', s: 0 });
  });
  it('at end of an expanded parent creates the first child', () => {
    const t = build('a\n  b');
    const r = enter(new Tx(t), v, 'a', 1, 1)!;
    expect(show(t)).toBe('a\n  \n  b\n');
    expect(t.get('a')!.children[0]).toBe((r.sel as { id: string }).id);
  });
  it('on an empty trailing child outdents', () => {
    const t = build('a\n  b');
    new Tx(t).set('b', { c: null });
    enter(new Tx(t), v, 'b', 0, 0);
    expect(t.get('b')!.parent).toBe(ROOT);
  });
});

describe('backspace / merge', () => {
  it('merges into previous visible item', () => {
    const t = build('a\n  x\nb');
    const r = backspaceAtStart(new Tx(t), v, 'b')!;
    expect(show(t)).toBe('a\n  xb\n');
    expect(r.sel).toMatchObject({ id: 'x', s: 1 });
  });
  it('survivor is the item with children', () => {
    const t = build('a\nb\n  c');
    const r = backspaceAtStart(new Tx(t), v, 'b')!;
    expect(show(t)).toBe('ab\n  c\n');
    expect(r.sel).toMatchObject({ id: 'b', s: 1 });
  });
  it('refuses when both have children', () => {
    const t = build('a\n  x\nb\n  y');
    expect(backspaceAtStart(new Tx(t), v, 'b')).toBeNull();
  });
});

describe('structure', () => {
  it('indent / outdent / move', () => {
    const t = build('a\nb\nc');
    indent(new Tx(t), v, ['b', 'c'], sel);
    expect(show(t)).toBe('a\n  b\n  c\n');
    outdent(new Tx(t), v, ['c'], sel);
    expect(show(t)).toBe('a\n  b\nc\n');
    moveUp(new Tx(t), v, ['c'], sel);
    expect(show(t)).toBe('c\na\n  b\n'); // swaps with previous sibling
    moveDown(new Tx(t), v, ['c'], sel);
    moveDown(new Tx(t), v, ['c'], sel);
    expect(show(t)).toBe('a\n  b\nc\n'); // last sibling: after parent... already top level → no-op
    moveUp(new Tx(t), v, ['c'], sel);
    moveDown(new Tx(t), v, ['b'], sel);
    expect(show(t)).toBe('c\na\nb\n'); // last child escapes below its parent
    moveUp(new Tx(t), v, ['b'], sel);
    moveUp(new Tx(t), v, ['b'], sel);
    expect(show(t)).toBe('b\nc\na\n');
  });
  it('delete and undo via inverses', () => {
    const t = build('a\nb\nc');
    const tx = new Tx(t);
    deleteItems(tx, v, ['b']);
    expect(show(t)).toBe('a\nc\n');
    tx.rollback();
    expect(show(t)).toBe('a\nb\nc\n');
  });
  it('check moves caret to next item', () => {
    const t = build('a\nb');
    const r = toggleChecked(new Tx(t), v, ['a'], { k: 'text', id: 'a', f: 'c', s: 1, e: 1 })!;
    expect(t.get('a')!.props.ck).toBe(true);
    expect(r.sel).toMatchObject({ id: 'b' });
  });
  it('check with stay (mobile toolbar) keeps the caret on the item', () => {
    const t = build('a\nb');
    const sel = { k: 'text', id: 'a', f: 'c', s: 1, e: 1 } as const;
    const r = toggleChecked(new Tx(t), v, ['a'], sel, { stay: true })!;
    expect(t.get('a')!.props.ck).toBe(true);
    expect(r.sel).toEqual(sel);
  });
});

describe('toggleChecked with recurring dates', () => {
  const now = new Date(2026, 9, 22, 12).getTime();
  const rec = (c: string) => {
    const t = new Tree();
    t.apply({ t: 'create', id: 'a', parent: ROOT, after: null, props: { c }, ts: 1 } as Op);
    t.apply({ t: 'create', id: 'b', parent: ROOT, after: 'a', props: { c: 'b' }, ts: 1 } as Op);
    return t;
  };
  it('new_instance: checked copy above, original advanced and unchecked', () => {
    const t = rec('pay !(2026-10-01 | 1m)');
    const r = toggleChecked(new Tx(t), v, ['a'], { k: 'text', id: 'a', f: 'c', s: 3, e: 3 }, { recurring: 'new_instance', now })!;
    const kids = t.get(ROOT)!.children;
    expect(kids.length).toBe(3);
    expect(kids[1]).toBe('a');
    expect(t.get(kids[0]!)!.props).toMatchObject({ c: 'pay !(2026-10-01 | 1m)', ck: true });
    expect(t.get('a')!.props.c).toBe('pay !(2026-11-01 | 1m)');
    expect(t.get('a')!.props.ck).toBeUndefined();
    expect(r.sel).toMatchObject({ id: 'a', s: 3 });
  });
  it('update: only advances (from completion uses now)', () => {
    const t = rec('x !(2026-01-05 | ~1w)');
    toggleChecked(new Tx(t), v, ['a'], sel, { recurring: 'update', now });
    expect(t.get(ROOT)!.children.length).toBe(2);
    expect(t.get('a')!.props.c).toBe('x !(2026-10-29 | ~1w)');
    expect(t.get('a')!.props.ck).toBeUndefined();
  });
  it('non-recurring dates and no options check normally', () => {
    const t = rec('x !(2026-01-05)');
    toggleChecked(new Tx(t), v, ['a'], sel, { recurring: 'new_instance', now });
    expect(t.get('a')!.props.ck).toBe(true);
    const t2 = rec('x !(2026-01-05 | 1d)');
    toggleChecked(new Tx(t2), v, ['a'], sel);
    expect(t2.get('a')!.props).toMatchObject({ c: 'x !(2026-01-05 | 1d)', ck: true });
  });
});

describe('paste', () => {
  it('splices first line and appends tail to last', () => {
    const t = build('ab');
    const items: Portable[] = [
      { props: { c: 'X' }, children: [{ props: { c: 'Y' }, children: [] }] },
      { props: { c: 'Z' }, children: [] },
    ];
    const r = pasteItems(new Tx(t), v, 'ab', 1, 1, items)!;
    expect(show(t)).toBe('aX\n  Y\nZb\n');
    expect(r.sel).toMatchObject({ s: 1 });
  });
});

describe('text formatting', () => {
  it('toggles wrap', () => {
    expect(toggleWrap('a word b', 2, 6, '**')).toEqual({ text: 'a **word** b', s: 4, e: 8 });
    expect(toggleWrap('a **word** b', 4, 8, '**')).toEqual({ text: 'a word b', s: 2, e: 6 });
    expect(toggleWrap('ab', 1, 1, '__')).toEqual({ text: 'a____b', s: 3, e: 3 });
    expect(wrapLink('go here', 3, 7).text).toBe('go [here]()');
  });
});
