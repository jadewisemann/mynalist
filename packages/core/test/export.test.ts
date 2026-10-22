import { describe, expect, it } from 'vitest';
import { ROOT, Tree, type Props } from '../src/tree.ts';
import {
  exportHtml, exportOpml, exportText, opmlToPortable, portableFromTree, portableToHtml, portableToOpml, portableToText,
} from '../src/export.ts';
import { parsePlainText } from '../src/paste.ts';

function build(spec: [string, string | null, Props][]): Tree {
  const t = new Tree({ c: 'Doc' });
  const last = new Map<string, string | null>();
  for (const [id, parent, props] of spec) {
    const p = parent ?? ROOT;
    t.apply({ t: 'create', id, parent: p, after: last.get(p) ?? null, props, ts: 1 });
    last.set(p, id);
  }
  return t;
}

const tree = build([
  ['a', null, { c: 'Alpha **bold**', n: 'note 1\nnote 2' }],
  ['a1', 'a', { c: 'child', num: true }],
  ['a1x', 'a1', { c: 'first' }],
  ['a1y', 'a1', { c: 'second' }],
  ['b', null, { c: 'Done', ck: true, cb: true }],
  ['b1', 'b', { c: 'under done' }],
  ['c', null, { c: 'Head <x> & "q"', h: 1, cl: 4 }],
  ['c1', 'c', { c: 'hidden by collapse' }],
]);
tree.apply({ t: 'collapse', id: 'c', v: true });
const top = tree.root().children;

describe('plain text', () => {
  it('spaces (default) with numbering and aligned notes', () => {
    expect(exportText(tree, top)).toBe([
      'Alpha **bold**',
      'note 1',
      'note 2',
      '    child',
      '        1. first',
      '        2. second',
      'Done',
      '    under done',
      'Head <x> & "q"',
      '    hidden by collapse',
    ].join('\n'));
  });

  it('dashes / asterisks / none', () => {
    expect(exportText(tree, ['a'], { indent: 'dashes' })).toBe(
      '- Alpha **bold**\n  note 1\n  note 2\n    - child\n        1. first\n        2. second',
    );
    expect(exportText(tree, ['a1'], { indent: 'asterisks' })).toBe('* child\n    1. first\n    2. second');
    expect(exportText(tree, ['a'], { indent: 'none', includeNotes: false })).toBe('Alpha **bold**\nchild\n1. first\n2. second');
  });

  it('numbers top-level items when their parent is numbered', () => {
    expect(exportText(tree, ['a1x', 'a1y'])).toBe('1. first\n2. second');
  });

  it('multi-line content continues aligned', () => {
    expect(portableToText([{ props: { c: 'x\ny', n: 'n' }, children: [] }], { indent: 'dashes' })).toBe('- x\n  y\n  n');
  });

  it('visible only: skips collapsed children, checked items and hidden notes', () => {
    expect(exportText(tree, top, { visibleOnly: true, showChecked: false, notes: 'first' })).toBe([
      'Alpha **bold**',
      'note 1',
      '    child',
      '        1. first',
      '        2. second',
      'Head <x> & "q"',
    ].join('\n'));
    expect(exportText(tree, ['a'], { visibleOnly: true, notes: 'hide', include: (id) => id !== 'a1y' })).toBe(
      'Alpha **bold**\n    child\n        1. first',
    );
  });

  it('round-trips through the plain-text paste parser', () => {
    const text = exportText(tree, top, { indent: 'dashes', includeNotes: false });
    const back = parsePlainText(text);
    expect(back.map((i) => i.props.c)).toEqual(['Alpha **bold**', 'Done', 'Head <x> & "q"']);
    expect(back[0]!.children[0]!.children.map((i) => i.props.c)).toEqual(['1. first', '2. second']);
  });
});

describe('HTML', () => {
  it('nested lists, headings, del, notes, escaping', () => {
    expect(exportHtml(tree, top)).toBe(
      '<ul>' +
        '<li>Alpha <strong>bold</strong>' +
        '<br>note 1<br>note 2' +
        '<ul><li>child<ol><li>first</li><li>second</li></ol></li></ul></li>' +
        '<li><del>Done</del><ul><li>under done</li></ul></li>' +
        '<li><h1>Head &lt;x&gt; &amp; &quot;q&quot;</h1><ul><li>hidden by collapse</li></ul></li>' +
      '</ul>',
    );
  });

  it('visible only and full document', () => {
    const html = exportHtml(tree, top, { visibleOnly: true, showChecked: false, document: true, title: 'T & U' });
    expect(html).toContain('<title>T &amp; U</title>');
    expect(html).not.toContain('Done');
    expect(html).not.toContain('hidden by collapse');
    expect(html.startsWith('<!DOCTYPE html>')).toBe(true);
  });

  it('never emits raw user HTML or unsafe links', () => {
    const html = portableToHtml([{ props: { c: '<img src=x onerror=alert(1)> [x](javascript:alert(1))' }, children: [] }]);
    expect(html).not.toContain('<img');
    expect(html).not.toContain('javascript:');
  });
});

describe('OPML', () => {
  it('writes OPML 2.0 with our head and attributes', () => {
    const xml = exportOpml(tree, top, { title: 'Doc', source: 'https://example/d/1', ownerName: 'Me', ownerEmail: 'me@x.y' });
    expect(xml).toContain('<opml version="2.0">');
    expect(xml).toContain('<title>Doc</title>');
    expect(xml).toContain('<flavor>outliner</flavor>');
    expect(xml).toContain('<source>https://example/d/1</source>');
    expect(xml).toContain('<ownerName>Me</ownerName>');
    expect(xml).toContain('<ownerEmail>me@x.y</ownerEmail>');
    expect(xml).toContain('<outline text="Alpha **bold**" _note="note 1&#10;note 2">');
    expect(xml).toContain('<outline text="child" listStyle="arabic">');
    expect(xml).toContain('<outline text="Done" checkbox="true" complete="true">');
    expect(xml).toContain('<outline text="Head &lt;x&gt; &amp; &quot;q&quot;" heading="1" colorLabel="4" collapsed="true">');
  });

  it('round-trips losslessly through opmlToPortable', () => {
    const items = portableFromTree(tree, top);
    const doc = opmlToPortable(portableToOpml(items, { title: 'Doc' }))!;
    expect(doc.title).toBe('Doc');
    expect(doc.items).toEqual(items);
  });

  it('visible only drops collapsed children and the collapsed flag', () => {
    const xml = exportOpml(tree, ['c'], { visibleOnly: true });
    expect(xml).toContain('<outline text="Head &lt;x&gt; &amp; &quot;q&quot;" heading="1" colorLabel="4" />');
  });

  it('escapes special characters in attributes', () => {
    const xml = portableToOpml([{ props: { c: 'a\tb', n: 'x\r\ny' }, children: [] }]);
    expect(xml).toContain('text="a&#9;b" _note="x&#13;&#10;y"');
    expect(opmlToPortable(xml)!.items[0]!.props).toEqual({ c: 'a\tb', n: 'x\r\ny' });
  });

  it('returns null for garbage', () => {
    expect(opmlToPortable('not xml')).toBeNull();
  });
});
