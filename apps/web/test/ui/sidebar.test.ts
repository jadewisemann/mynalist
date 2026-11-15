import { describe, expect, it } from 'vitest';
import { dropIndex } from '../../src/ui/sidebar.ts';

// what Workspace.move / moveBookmark do with the index: take the row out, then insert at the index
const move = (list: string[], id: string, index: number) => {
  const rest = list.filter((x) => x !== id);
  rest.splice(Math.max(0, Math.min(index, rest.length)), 0, id);
  return rest;
};
const drop = (list: string[], id: string, target: string, where: 'before' | 'after') => {
  const i = list.indexOf(target);
  return move(list, id, dropIndex(list, id, where === 'before' ? i : i + 1));
};

describe('sidebar drop index', () => {
  const rows = ['a', 'b', 'c', 'd'];
  it('puts a row dragged down where it was dropped', () => {
    expect(drop(rows, 'a', 'c', 'after')).toEqual(['b', 'c', 'a', 'd']);
    expect(drop(rows, 'a', 'c', 'before')).toEqual(['b', 'a', 'c', 'd']);
    expect(drop(rows, 'b', 'd', 'after')).toEqual(['a', 'c', 'd', 'b']);
  });
  it('puts a row dragged up where it was dropped', () => {
    expect(drop(rows, 'd', 'b', 'before')).toEqual(['a', 'd', 'b', 'c']);
    expect(drop(rows, 'c', 'a', 'after')).toEqual(['a', 'c', 'b', 'd']);
  });
  it('keeps the index of a row coming from another list', () => {
    expect(dropIndex(rows, 'x', 2)).toBe(2);
  });
});
