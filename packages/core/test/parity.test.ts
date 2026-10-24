// Behaviour rules found by differential testing against Dynalist (qa/), pinned at the core level.
import { describe, expect, it } from 'vitest';
import {
  DocSession, ROOT, Tree, Tx, backspaceAtStart, defaultView, deleteAtEnd, deleteItems, moveDown, moveUp,
  toggleChecked, type Op,
} from '../src/index.ts';

/** Build a tree from an indented outline: "a\n  b\n  c\nd" (ids = texts; "-" lines are empty items). */
function build(src: string, title = 'T'): Tree {
  const t = new Tree();
  t.apply({ t: 'set', id: ROOT, props: { c: title }, ts: 1 } as Op);
  const stack: { id: string; ind: number }[] = [{ id: ROOT, ind: -1 }];
  const lastChild = new Map<string, string>();
  for (const line of src.split('\n').filter(Boolean)) {
    const ind = line.length - line.trimStart().length;
    const id = line.trim();
    while (stack[stack.length - 1]!.ind >= ind) stack.pop();
    const parent = stack[stack.length - 1]!.id;
    t.apply({ t: 'create', id, parent, after: lastChild.get(parent) ?? null, props: id.startsWith('-') ? {} : { c: id }, ts: 1 } as Op);
    lastChild.set(parent, id);
    stack.push({ id, ind });
  }
  return t;
}

function show(t: Tree, id = ROOT, d = 0): string {
  return t.get(id)!.children.map((c) => '  '.repeat(d) + (t.get(c)!.props.c ?? '') + '\n' + show(t, c, d + 1)).join('');
}

const v = defaultView();
const at = (id: string, s: number) => ({ k: 'text', id, f: 'c', s, e: s }) as const;

describe('backspace at the start of the first item', () => {
  it('merges a plain first item into the title', () => {
    const t = build('a\nb');
    const r = backspaceAtStart(new Tx(t), v, 'a')!;
    expect(t.get(ROOT)!.props.c).toBe('Ta');
    expect(show(t)).toBe('b\n');
    expect(r.sel).toMatchObject({ id: ROOT, s: 1 });
  });
  it('does nothing when the first item has children', () => {
    const t = build('a\n  x\nb');
    expect(backspaceAtStart(new Tx(t), v, 'a')).toBeNull();
  });
});

describe('delete at the end of an item', () => {
  it('removes an empty item and leaves the next one in place', () => {
    const t = build('a\n  -e\nb');
    const r = deleteAtEnd(new Tx(t), v, '-e')!;
    expect(show(t)).toBe('a\nb\n');
    expect(r.sel).toMatchObject({ id: 'b', s: 0 });
  });
  it('the merged item takes the next item’s place (and keeps the first one’s marks)', () => {
    const t = build('top\n  mid\n    deep\nlast');
    new Tx(t).set('deep', { ck: true });
    const r = deleteAtEnd(new Tx(t), v, 'deep')!;
    expect(show(t)).toBe('top\n  mid\ndeeplast\n');
    expect(r.sel).toMatchObject({ id: 'last', s: 4 });
    expect(t.get('last')!.props.ck).toBeTruthy();
  });
  it('does nothing when the next item is an only child with children', () => {
    const t = build('y\n  y1\n    y2\nz');
    expect(deleteAtEnd(new Tx(t), v, 'y')).toBeNull();
    expect(show(t)).toBe('y\n  y1\n    y2\nz\n');
  });
});

describe('deleting selected items', () => {
  it('Backspace puts the caret at the end of the previous item, never in the title', () => {
    const t = build('a\nb\nc');
    expect(deleteItems(new Tx(t), v, ['b'], 'left')!.sel).toMatchObject({ id: 'a', s: 1 });
    const t2 = build('a\nb');
    expect(deleteItems(new Tx(t2), v, ['a'], 'left')!.sel).toMatchObject({ id: 'b', s: 0 });
  });
  it('Delete puts the caret at the start of the next item', () => {
    const t = build('a\nb\nc');
    expect(deleteItems(new Tx(t), v, ['b'], 'right')!.sel).toMatchObject({ id: 'c', s: 0 });
  });
});

describe('toggle checked', () => {
  it('moves the caret to the start of the next item that is not a descendant', () => {
    const t = build('a\n  a1\nb');
    const r = toggleChecked(new Tx(t), v, ['a'], at('a', 1))!;
    expect(t.get('a')!.props.ck).toBeTruthy();
    expect(r.sel).toMatchObject({ id: 'b', s: 0 });
  });
});

describe('moving items past the edge of their parent', () => {
  it('a first child moves up into the end of the parent’s previous sibling, even a childless one', () => {
    const t = build('p\nq\n  x\n  y');
    moveUp(new Tx(t), v, ['x'], at('x', 0));
    expect(show(t)).toBe('p\n  x\nq\n  y\n');
  });
  it('a last child moves down into the start of the parent’s next sibling', () => {
    const t = build('q\n  x\n  y\nr');
    moveDown(new Tx(t), v, ['y'], at('y', 0));
    expect(show(t)).toBe('q\n  x\nr\n  y\n');
  });
});

describe('undo selection for deletions', () => {
  it('remembers the selection after the first edit of a coalesced run', () => {
    const s = new DocSession();
    const set = (c: string): Op => ({ t: 'set', id: 'a', props: { c }, ts: 1 }) as Op;
    s.transact([{ t: 'create', id: 'a', parent: ROOT, after: null, props: { c: 'abc' }, ts: 1 } as Op], { before: 0, after: 0 });
    s.transact([set('ab')], { before: 'at 3', after: 'at 2' });
    s.transact([set('a')], { before: 'at 2', after: 'at 1' });
    expect(s.undo()).toBe('at 3');
    expect(s.undoneFirst).toBe('at 2');
    expect(s.live.get('a')!.props.c).toBe('abc');
  });
});
