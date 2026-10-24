import { describe, expect, it } from 'vitest';
import {
  decodeEntities, htmlToText, isOpml, parseClipboard, parseOpml, parsePlainText, tokenizeMarkup, type PortableItem,
} from '../src/paste.ts';

/** Compact view: "text" or ["text", ...children] */
function view(items: PortableItem[]): unknown[] {
  return items.map((i) => {
    const c = (i.props.c as string | undefined) ?? '';
    return i.children.length ? [c, ...view(i.children)] : c;
  });
}

describe('parsePlainText', () => {
  it('single line is spliced as-is', () => {
    expect(view(parsePlainText('hello'))).toEqual(['hello']);
    expect(view(parsePlainText('- not a bullet strip'))).toEqual(['- not a bullet strip']);
    expect(view(parsePlainText('   indented'))).toEqual(['indented']);
  });

  it('nests by indentation with a stack', () => {
    const text = 'a\n    b\n        c\n    d\ne';
    expect(view(parsePlainText(text))).toEqual([['a', ['b', 'c'], 'd'], 'e']);
  });

  it('converts tabs and CRLF, strips the common indent', () => {
    expect(view(parsePlainText('\ta\r\n\t\tb\r\n\tc'))).toEqual([['a', 'b'], 'c']);
    expect(view(parsePlainText('    a\n    b'))).toEqual(['a', 'b']);
  });

  it('pops to the nearest level that is <= the line indent', () => {
    // 0, 4, 2: the "2" line pops past 4 and becomes a sibling of the 0 line
    expect(view(parsePlainText('a\n    b\n  c\n   d'))).toEqual([['a', 'b'], ['c', 'd']]);
    // first line deeper than later lines
    expect(view(parsePlainText('    a\nb\n    c'))).toEqual(['a', ['b', 'c']]);
  });

  it('keeps blank lines as empty items (Dynalist)', () => {
    expect(view(parsePlainText('a\n\n\nb'))).toEqual(['a', '', '', 'b']);
    expect(view(parsePlainText('a\nb\n'))).toEqual(['a', 'b', '']);
    expect(view(parsePlainText('a\n'))).toEqual(['a', '']);
  });

  it('keeps list markers as text (Dynalist)', () => {
    expect(view(parsePlainText('- a\n    * b\n    • c\n+ d'))).toEqual([['- a', '* b', '• c'], '+ d']);
    expect(view(parsePlainText('**bold**\n*not bullet'))).toEqual(['**bold**', '*not bullet']);
  });

  it('produces portable items', () => {
    expect(parsePlainText('x\n  y')).toEqual([
      { props: { c: 'x' }, collapsed: false, children: [{ props: { c: 'y' }, collapsed: false, children: [] }] },
    ]);
  });
});

describe('tokenizeMarkup', () => {
  it('handles attributes, entities, comments and raw text', () => {
    const toks = tokenizeMarkup('<!-- c --><A HREF="x&amp;y" data-x=1 checked>t&lt;</a><script>if (a<b) {}</script><br/>');
    expect(toks).toEqual([
      { k: 'open', name: 'a', attrs: { href: 'x&y', 'data-x': '1', checked: '' }, selfClose: false },
      { k: 'text', v: 't<' },
      { k: 'close', name: 'a' },
      { k: 'open', name: 'script', attrs: {}, selfClose: false },
      { k: 'text', v: 'if (a<b) {}' },
      { k: 'close', name: 'script' },
      { k: 'open', name: 'br', attrs: {}, selfClose: true },
    ]);
  });

  it('is tolerant of junk', () => {
    expect(tokenizeMarkup('a < b <3 <<x')).toEqual([{ k: 'text', v: 'a < b <3 <' }, { k: 'open', name: 'x', attrs: {}, selfClose: false }]);
    expect(tokenizeMarkup('<p class="unterminated')).toHaveLength(1);
  });

  it('decodes entities', () => {
    expect(decodeEntities('&amp;&lt;&#65;&#x42;&nbsp;&bogus;&copy')).toBe('&<AB &bogus;©');
  });
});

describe('htmlToText', () => {
  it('converts nested lists to tab indentation', () => {
    const html = '<ul><li>a<ul><li>b</li><li>c<ol><li>d</li></ol></li></ul></li><li>e</li></ul>';
    expect(htmlToText(html)).toBe('\ta\n\t\tb\n\t\tc\n\t\t\td\n\te');
    expect(view(parsePlainText(htmlToText(html)))).toEqual([['a', 'b', ['c', 'd']], 'e']);
  });

  it('blocks and br make new lines; whitespace collapses', () => {
    expect(htmlToText('<p>  one\n  two </p><div>three<br>four</div>')).toBe('one two\nthree\nfour');
    expect(htmlToText('<h1>Title</h1><ul><li>x</li></ul>')).toBe('Title\n\tx');
  });

  it('inline formatting, links and images', () => {
    expect(htmlToText('<b>bold</b> <strong>s</strong> <i>it</i> <em>e</em>')).toBe('**bold** **s** __it__ __e__');
    expect(htmlToText('<a href="http://x.com">site</a> <a href="http://y.com">http://y.com</a>')).toBe('[site](http://x.com) http://y.com');
    expect(htmlToText('<img src="p.png" alt="pic">')).toBe('![pic](p.png)');
    expect(htmlToText('<b> spaced </b>end')).toBe(' **spaced** end'.trim());
    expect(htmlToText('<b><a href="u">x</a></b>')).toBe('**[x](u)**');
    expect(htmlToText('<a href="u"><b>x</b></a>')).toBe('[**x**](u)');
    expect(htmlToText('<a href="javascript:alert(1)">x</a>')).toBe('x');
  });

  it('respects CSS font weight (Google Docs wrapper)', () => {
    expect(htmlToText('<b style="font-weight:normal;" id="docs-internal-guid-1"><span style="font-weight:700">B</span> n</b>')).toBe('**B** n');
    expect(htmlToText('<span style="font-style:italic">i</span>')).toBe('__i__');
  });

  it('formatting spanning a line break is closed and reopened', () => {
    expect(htmlToText('<b>a<br>b</b>')).toBe('**a**\n**b**');
  });

  it('drops scripts, styles and head', () => {
    expect(htmlToText('<html><head><title>T</title><style>p{}</style></head><body><script>x()</script><p>ok</p></body></html>')).toBe('ok');
  });

  it('preserves whitespace in pre', () => {
    expect(htmlToText('<pre>a\n  b</pre>')).toBe('a\n  b');
  });

  it('uses mso-list levels for MS Word paragraphs', () => {
    const html = `<html xmlns:w="urn:schemas-microsoft-com:office:word"><body>
<p class=MsoNormal>Intro</p>
<p class=MsoListParagraphCxSpFirst style='text-indent:-.25in;mso-list:l0 level1 lfo1'><![if !supportLists]><span style='font-family:Symbol;mso-list:Ignore'>·<span>&nbsp;&nbsp;</span></span><![endif]>One</p>
<p class=MsoListParagraphCxSpLast style='mso-list:l0 level2 lfo1'><span style='mso-list:Ignore'>o<span>&nbsp;</span></span>Two</p>
</body></html>`;
    expect(htmlToText(html)).toBe('Intro\n\tOne\n\t\tTwo');
  });

  it('table cells are space separated', () => {
    expect(htmlToText('<table><tr><td>a</td><td>b</td></tr><tr><td>c</td></tr></table>')).toBe('a b\nc');
  });
});

describe('OPML', () => {
  const native = `<?xml version="1.0" encoding="utf-8"?>
<opml version="2.0">
  <head><title>My &amp; Doc</title><flavor>outliner</flavor></head>
  <body>
    <outline text="**a** &lt;b&gt;" _note="line1&#10;line2" checkbox="true" complete="true" heading="2" colorLabel="3" listStyle="arabic" collapsed="true">
      <outline text="child" />
    </outline>
    <outline text="b"/>
  </body>
</opml>`;

  it('detects OPML', () => {
    expect(isOpml(native)).toBe(true);
    expect(isOpml('  <opml version="1.0"><body/></opml>')).toBe(true);
    expect(isOpml('<html>')).toBe(false);
    expect(isOpml('opml')).toBe(false);
  });

  it('parses native attributes losslessly', () => {
    const doc = parseOpml(native)!;
    expect(doc.title).toBe('My & Doc');
    expect(doc.flavor).toBe('outliner');
    expect(doc.items).toEqual([
      {
        props: { c: '**a** <b>', n: 'line1\nline2', cb: true, ck: true, h: 2, cl: 3, num: true },
        collapsed: true,
        children: [{ props: { c: 'child' }, collapsed: false, children: [] }],
      },
      { props: { c: 'b' }, collapsed: false, children: [] },
    ]);
  });

  it('honors aliases and converts HTML in foreign OPML', () => {
    const xml = `<opml version="2.0"><head><title>W</title></head><body>
      <outline text="&lt;b&gt;Bold&lt;/b&gt; &lt;a href=&quot;http://x&quot;&gt;l&lt;/a&gt;" note="n" checklist="true" _complete="true"/>
    </body></opml>`;
    expect(parseOpml(xml)!.items[0]!.props).toEqual({ c: '**Bold** [l](http://x)', n: 'n', cb: true, ck: true });
  });

  it('returns null for non-OPML', () => {
    expect(parseOpml('<html><body>x</body></html>')).toBeNull();
  });
});

describe('parseClipboard', () => {
  it('prefers OPML, then HTML, then plain text', () => {
    const opml = '<opml version="2.0"><head><flavor>outliner</flavor></head><body><outline text="o"/></body></opml>';
    expect(parseClipboard({ 'text/opml': opml, 'text/html': '<p>h</p>', 'text/plain': 'p' })).toEqual({
      items: [{ props: { c: 'o' }, collapsed: false, children: [] }], structured: true,
    });
    expect(parseClipboard({ 'text/plain': opml })!.structured).toBe(true);
    expect(view(parseClipboard({ 'text/html': '<ul><li>h<ul><li>i</li></ul></li></ul>', 'text/plain': 'p' })!.items)).toEqual([['h', 'i']]);
    expect(view(parseClipboard({ 'text/plain': 'a\n  b' })!.items)).toEqual([['a', 'b']]);
    expect(parseClipboard({})).toBeNull();
  });
});
