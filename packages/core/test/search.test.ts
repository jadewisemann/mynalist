import { describe, expect, it } from 'vitest';
import { ROOT, Tree, type Props } from '../src/tree.ts';
import { compileQuery, forcesShowChecked, fuzzyFilter, fuzzyMatch, fuzzyScore, parseQueryTime, searchTree, wholeWordMatch } from '../src/search.ts';

const at = (y: number, m: number, d: number, h = 0, mi = 0) => new Date(y, m - 1, d, h, mi).getTime();
const NOW = at(2026, 10, 22, 12, 0);

function build(spec: [string, string | null, Props, number?][]): Tree {
  const t = new Tree();
  const last = new Map<string, string | null>();
  for (const [id, parent, props, ts] of spec) {
    const p = parent ?? ROOT;
    t.apply({ t: 'create', id, parent: p, after: last.get(p) ?? null, props, ts: ts ?? NOW });
    last.set(p, id);
  }
  return t;
}

const tree = build([
  ['a', null, { c: 'Buy milk #shopping', n: 'from the Store' }],
  ['b', null, { c: 'Project plan', cb: true, cl: 1, h: 2 }],
  ['b1', 'b', { c: 'Draft the spec @alice', ck: true }],
  ['b1x', 'b1', { c: 'sub task milk' }],
  ['b2', 'b', { c: 'Review !(2026-10-25)', cl: 5 }],
  ['c', null, { c: 'Meeting !(2026-10-22 15:00) #shopping-list' }],
  ['d', null, { c: 'Old !(2026-10-01 - 2026-10-05)', n: 'note with "Exact Phrase" here' }, at(2026, 9, 1)],
  ['e', null, { c: 'code `#notag` and milkshake' }],
]);
// make `d` old for edited:/created:
tree.get('d')!.edited = at(2026, 9, 1);

const run = (q: string, opts = {}) => {
  const m = compileQuery(q, NOW, opts);
  return [...tree.nodes.keys()].filter((id) => id !== ROOT && m(tree, id));
};

describe('text terms', () => {
  it('ANDs case-insensitive substrings over content and note', () => {
    expect(run('milk')).toEqual(['a', 'b1x', 'e']);
    expect(run('MILK buy')).toEqual(['a']);
    expect(run('store')).toEqual(['a']);
  });

  it('OR groups and negation', () => {
    expect(run('plan OR review')).toEqual(['b', 'b2']);
    expect(run('milk -shake')).toEqual(['a', 'b1x']);
    expect(run('milk or plan')).toEqual([]); // lowercase "or" is a plain term
  });

  it('quoted phrases are case-sensitive whole words', () => {
    expect(run('"Exact Phrase"')).toEqual(['d']);
    expect(run('"exact phrase"')).toEqual([]);
    expect(run('"milk"')).toEqual(['a', 'b1x']);
    expect(run('-"milk"')).not.toContain('a');
  });

  it('a quote without a closing quote is a literal character', () => {
    expect(run('"milk')).toEqual([]);
    expect(run('"Exact')).toEqual(['d']);
    expect(compileQuery('"Exact Phrase', NOW).terms).toEqual(['"Exact', 'Phrase']);
  });

  it('tags match whole words', () => {
    expect(run('#shopping')).toEqual(['a']);
    expect(run('#shopping-list')).toEqual(['c']);
    expect(run('@alice')).toEqual(['b1']);
    expect(run('#shop')).toEqual([]);
    expect(run('#shop', { typing: true })).toEqual(['a', 'c']);
    expect(run('#shop ', { typing: true })).toEqual([]);
  });

  it('in:title / in:note scope following terms', () => {
    expect(run('in:note store')).toEqual(['a']);
    expect(run('in:title store')).toEqual([]);
    expect(run('in:title milk')).toEqual(['a', 'b1x', 'e']);
    expect(run('in:note milk')).toEqual([]);
  });
});

describe('operators', () => {
  it('is: / has: / color: / heading:', () => {
    expect(run('is:completed')).toEqual(['b1', 'b1x']);
    expect(run('is:checklist')).toEqual(['b']);
    expect(run('has:checkbox')).toEqual(['b']);
    expect(run('has:note')).toEqual(['a', 'd']);
    expect(run('has:color')).toEqual(['b', 'b2']);
    expect(run('color:blue')).toEqual(['b2']);
    expect(run('color:none milk')).toEqual(['a', 'b1x', 'e']);
    expect(run('is:heading')).toEqual(['b']);
    expect(run('heading:2')).toEqual(['b']);
    expect(run('heading:0 plan')).toEqual([]);
    expect(run('has:children')).toEqual(['b', 'b1']);
    expect(run('has:date')).toEqual(['b2', 'c', 'd']);
    expect(run('time:')).toEqual(['b2', 'c', 'd']);
    expect(run('-is:completed milk')).toEqual(['a', 'e']);
  });

  it('operator followed by a space joins the next word', () => {
    expect(run('color: blue')).toEqual(['b2']);
    expect(run('parent: plan')).toEqual(['b1', 'b2']);
  });

  it('parent: and ancestor:', () => {
    expect(run('parent:plan')).toEqual(['b1', 'b2']);
    expect(run('ancestor:plan')).toEqual(['b1', 'b1x', 'b2']);
    expect(run('ancestor:"project plan" milk')).toEqual(['b1x']);
  });

  it('since: / until: / within:', () => {
    expect(run('since:today')).toEqual(['b2', 'c']);
    expect(run('until:today')).toEqual(['c', 'd']);
    expect(run('until:2026-10-01')).toEqual(['d']);
    expect(run('since:2026-10-05')).toEqual(['b2', 'c', 'd']);
    expect(run('since:2026-10-06')).toEqual(['b2', 'c']);
    expect(run('within:today')).toEqual(['c']);
    expect(run('within:0d')).toEqual(['c']);
    expect(run('within:3d')).toEqual(['b2', 'c']);
    expect(run('within:2d')).toEqual(['c']);
    expect(run('within:-30d')).toEqual(['c', 'd']);
    expect(run('within:1w')).toEqual(['b2', 'c']);
  });

  it('edited: / created:', () => {
    expect(run('edited:4d')).not.toContain('d');
    expect(run('edited:60d')).toContain('d');
    expect(run('created:3w')).not.toContain('d');
    expect(run('created:2m')).toContain('d');
  });

  it('is:completed forces checked items to show', () => {
    expect(forcesShowChecked('foo is:completed')).toBe(true);
    expect(forcesShowChecked('-is:completed')).toBe(false);
    expect(forcesShowChecked('is:checklist')).toBe(false);
  });

  it('unknown operator values fall back to text; incomplete ones are ignored while typing', () => {
    expect(run('is:bogus')).toEqual([]);
    expect(run('milk is:', { typing: true })).toEqual(['a', 'b1x', 'e']);
    expect(run('color:bl', { typing: true })).toEqual(['b2']);
    expect(run('is:comp', { typing: true })).toEqual(['b1', 'b1x']);
    expect(run('foo:bar')).toEqual([]);
  });

  it('empty query matches everything', () => {
    const m = compileQuery('   ', NOW);
    expect(m.empty).toBe(true);
    expect(m(tree, 'a')).toBe(true);
  });

  it('reports highlight terms', () => {
    expect(compileQuery('milk -x "A b" #t is:completed', NOW).terms).toEqual(['milk', 'A b', '#t']);
  });
});

describe('parseQueryTime', () => {
  it('handles keywords, relative and absolute values', () => {
    expect(parseQueryTime('today', NOW)).toEqual({ start: at(2026, 10, 22), end: at(2026, 10, 23) });
    expect(parseQueryTime('yesterday', NOW)!.start).toBe(at(2026, 10, 21));
    expect(parseQueryTime('3d', NOW)).toMatchObject({ start: at(2026, 10, 25), end: at(2026, 10, 26) });
    expect(parseQueryTime('-1w', NOW)!.start).toBe(at(2026, 10, 15));
    expect(parseQueryTime('1m', NOW)!.start).toBe(at(2026, 11, 22));
    expect(parseQueryTime('2h', NOW)!.start).toBe(NOW + 2 * 3600000);
    expect(parseQueryTime('now', NOW)!.start).toBe(NOW);
    expect(parseQueryTime('2026-01-02', NOW)).toEqual({ start: at(2026, 1, 2), end: at(2026, 1, 3) });
    expect(parseQueryTime('2026-01-02T10:30', NOW)!.start).toBe(at(2026, 1, 2, 10, 30));
    expect(parseQueryTime('soon', NOW)).toBeNull();
  });
});

describe('searchTree', () => {
  it('returns matches and the ancestor chain', () => {
    const r = searchTree(tree, compileQuery('milk', NOW));
    expect(r.matches).toEqual(['a', 'b1x', 'e']);
    expect([...r.visible].sort()).toEqual(['a', 'b', 'b1', 'b1x', 'e']);
  });

  it('skips checked subtrees unless forced', () => {
    expect(searchTree(tree, compileQuery('milk', NOW), ROOT, { showChecked: false }).matches).toEqual(['a', 'e']);
    expect(searchTree(tree, compileQuery('milk is:completed', NOW), ROOT, { showChecked: false }).matches).toEqual(['b1x']);
  });

  it('scopes to a zoomed subtree', () => {
    expect(searchTree(tree, compileQuery('milk', NOW), 'b').matches).toEqual(['b1x']);
  });
});

describe('wholeWordMatch', () => {
  it('respects boundaries', () => {
    expect(wholeWordMatch('a #tag.', '#tag', false)).toBe(true);
    expect(wholeWordMatch('a #tags', '#tag', false)).toBe(false);
    expect(wholeWordMatch('a #tags', '#tag', false, true)).toBe(true);
    expect(wholeWordMatch('x#tag', '#tag', false)).toBe(false);
    expect(wholeWordMatch('Hello World!', 'World', true)).toBe(true);
    expect(wholeWordMatch('Hello World!', 'world', true)).toBe(false);
  });
});

describe('fuzzy', () => {
  it('matches word tokens in order before characters', () => {
    const m = fuzzyMatch('pro pla', 'Project plan')!;
    expect(m.ranges).toEqual([[0, 3], [8, 11]]);
    expect(fuzzyMatch('pp', 'Project plan')!.ranges).toEqual([[0, 1], [8, 9]]);
    expect(fuzzyMatch('xyz', 'Project plan')).toBeNull();
    expect(fuzzyMatch('prj pln', 'Project plan')!.ranges).toEqual([[0, 2], [3, 4], [8, 10], [11, 12]]); // char fallback
    expect(fuzzyMatch('pla pro', 'Project plan')).toBeNull();
  });

  it('ranks tighter, earlier, boundary matches higher', () => {
    expect(fuzzyScore('plan', 'plan')).toBeGreaterThan(fuzzyScore('plan', 'my plan'));
    expect(fuzzyScore('plan', 'my plan')).toBeGreaterThan(fuzzyScore('plan', 'explanation'));
    expect(fuzzyScore('doc', 'Docs')).toBeGreaterThan(fuzzyScore('doc', 'd-o-c'));
    expect(fuzzyScore('abc', 'nothing')).toBe(-Infinity);
    expect(fuzzyScore('', 'anything')).toBe(0);
  });

  it('prefers word-boundary occurrences', () => {
    expect(fuzzyMatch('note', 'denote Notes')!.ranges).toEqual([[7, 11]]);
    expect(fuzzyMatch('ws', 'WorkSpace')!.ranges).toEqual([[0, 1], [4, 5]]);
  });

  it('fuzzyFilter sorts by score and drops misses', () => {
    const out = fuzzyFilter('inbox', ['Archive', 'My inbox items', 'Inbox', 'in a box'], (s) => s).map((r) => r.item);
    expect(out).toEqual(['Inbox', 'My inbox items', 'in a box']);
  });
});
