import { describe, expect, it } from 'vitest';
import {
  extractDates, extractLinks, extractTags, inlineToPlain, mapDisplayOffset, normalizeUrl, parseInline,
  renderInline, replaceTag, safeUrl, tokenPathAt, walkTokens, type InlineToken,
} from '../src/inline.ts';

/** Compact structural view: [type, payload, children?] */
function shape(tokens: InlineToken[]): unknown[] {
  return tokens.map((t) => {
    switch (t.t) {
      case 'text': case 'escape': case 'latex': return [t.t, t.text];
      case 'code': return ['code', t.text];
      case 'codeblock': return ['codeblock', t.lang, t.text];
      case 'link': return ['link', t.text, t.url];
      case 'image': return ['image', t.text, t.url];
      case 'autolink': return ['autolink', t.url];
      case 'tag': return ['tag', t.tag];
      case 'date': return ['date', t.text];
      default: return [t.t, shape(t.children)];
    }
  });
}
const p = (s: string) => shape(parseInline(s));

/** Every token's ranges must be consistent with the source. */
function checkOffsets(src: string) {
  walkTokens(parseInline(src), (t) => {
    expect(t.start).toBeLessThanOrEqual(t.innerStart);
    expect(t.innerStart).toBeLessThanOrEqual(t.innerEnd);
    expect(t.innerEnd).toBeLessThanOrEqual(t.end);
    if (t.t === 'text') expect(src.slice(t.start, t.end)).toBe(t.text);
    if (t.t === 'escape') expect(src.slice(t.start, t.end)).toBe('\\' + t.text);
  });
  // top-level tokens tile the source exactly
  const top = parseInline(src);
  let pos = 0;
  for (const t of top) {
    expect(t.start).toBe(pos);
    pos = t.end;
  }
  expect(pos).toBe(src.length);
}

describe('parseInline: emphasis', () => {
  it('parses the four span kinds', () => {
    expect(p('**b**')).toEqual([['bold', [['text', 'b']]]]);
    expect(p('__i__')).toEqual([['italic', [['text', 'i']]]]);
    expect(p('~~s~~')).toEqual([['strike', [['text', 's']]]]);
    expect(p('==h==')).toEqual([['highlight', [['text', 'h']]]]);
  });

  it('nests different delimiters', () => {
    expect(p('**a __b__ c**')).toEqual([['bold', [['text', 'a '], ['italic', [['text', 'b']]], ['text', ' c']]]]);
    expect(p('==~~x~~==')).toEqual([['highlight', [['strike', [['text', 'x']]]]]]);
  });

  it('treats unclosed delimiters as literal text', () => {
    expect(p('a ** b')).toEqual([['text', 'a ** b']]);
    expect(p('**a __b** c__')).toEqual([['bold', [['text', 'a __b']]], ['text', ' c__']]);
    expect(p('****')).toEqual([['text', '****']]);
    expect(p('~~')).toEqual([['text', '~~']]);
  });

  it('records offsets and inner ranges', () => {
    const [b] = parseInline('x **bold** y').filter((t) => t.t === 'bold');
    expect(b).toMatchObject({ start: 2, end: 10, innerStart: 4, innerEnd: 8 });
    for (const s of ['x **bold** y', '**a __b__ c** `x\\`y` [l](u) #t !(2026-01-02)', 'a\\*b ==c==\n```js\nx\n```\nz']) {
      checkOffsets(s);
    }
  });
});

describe('parseInline: code, latex, escapes', () => {
  it('inline code is literal and only ` closes, with \\` escapes', () => {
    expect(p('`**not bold**`')).toEqual([['code', '**not bold**']]);
    expect(p('`a\\`b`')).toEqual([['code', 'a`b']]);
    expect(p('`a\\nb`')).toEqual([['code', 'a\\nb']]);
    expect(p('`open')).toEqual([['text', '`open']]);
    const [c] = parseInline('x `ab` y').filter((t) => t.t === 'code');
    expect(c).toMatchObject({ start: 2, end: 6, innerStart: 3, innerEnd: 5 });
  });

  it('code block with language, swallowing a newline after the fence', () => {
    const toks = parseInline('```js\nlet a = 1;\n```\nafter');
    expect(shape(toks)).toEqual([['codeblock', 'js', 'let a = 1;'], ['text', 'after']]);
    expect(toks[0]).toMatchObject({ start: 0, end: 21, innerStart: 6, innerEnd: 16 });
  });

  it('code block without a language line', () => {
    expect(p('```x = 1```')).toEqual([['codeblock', '', 'x = 1']]);
    expect(p('```Foo bar\nbaz```')).toEqual([['codeblock', '', 'Foo bar\nbaz']]);
    expect(p('```\ncode\n```')).toEqual([['codeblock', '', 'code']]);
    expect(p('``` unclosed')).toEqual([['text', '``` unclosed']]);
  });

  it('latex', () => {
    expect(p('$$E=mc^2$$ ok')).toEqual([['latex', 'E=mc^2'], ['text', ' ok']]);
    expect(p('$$ open')).toEqual([['text', '$$ open']]);
    expect(p('$$$$')).toEqual([['text', '$$$$']]);
  });

  it('escapes', () => {
    expect(p('\\*\\*a\\*\\*')).toEqual([['escape', '*'], ['escape', '*'], ['text', 'a'], ['escape', '*'], ['escape', '*']]);
    expect(p('\\#notatag')).toEqual([['escape', '#'], ['text', 'notatag']]);
    expect(p('a\\qb')).toEqual([['text', 'a\\qb']]);
    for (const c of '\\~$=`*_{}[]()#+-.!') expect(p('\\' + c)).toEqual([['escape', c]]);
  });
});

describe('parseInline: links and images', () => {
  it('links with balanced brackets and parens', () => {
    expect(p('[text](http://x.com)')).toEqual([['link', 'text', 'http://x.com']]);
    expect(p('[a [b] c](u)')).toEqual([['link', 'a [b] c', 'u']]);
    expect(p('[w](http://en.wikipedia.org/wiki/A_(b))')).toEqual([['link', 'w', 'http://en.wikipedia.org/wiki/A_(b)']]);
    expect(p('[x] (y)')).toEqual([['text', '[x] (y)']]);
    expect(p('[x](y')).toEqual([['text', '[x](y']]);
    const [l] = parseInline('a [txt](url) b').filter((t) => t.t === 'link');
    expect(l).toMatchObject({ start: 2, end: 12, innerStart: 3, innerEnd: 6, urlStart: 8, urlEnd: 11 });
  });

  it('link text may be formatted but not contain links or tags', () => {
    const [l] = parseInline('[**b** #t](u)');
    expect(l!.t).toBe('link');
    expect(shape((l as { children: InlineToken[] }).children)).toEqual([['bold', [['text', 'b']]], ['text', ' #t']]);
  });

  it('images', () => {
    expect(p('![alt](pic.png)')).toEqual([['image', 'alt', 'pic.png']]);
    expect(p('![](pic.png)')).toEqual([['image', '', 'pic.png']]);
    expect(p('!notimage')).toEqual([['text', '!notimage']]);
  });

  it('autolinks end at whitespace or an unbalanced )', () => {
    expect(p('see https://x.com/a?b=1 now')).toEqual([['text', 'see '], ['autolink', 'https://x.com/a?b=1'], ['text', ' now']]);
    expect(p('(http://x.com/y)')).toEqual([['text', '('], ['autolink', 'http://x.com/y'], ['text', ')']]);
    expect(p('http://x.com/(a)b')).toEqual([['autolink', 'http://x.com/(a)b']]);
    expect(p('**http://x.com**')).toEqual([['bold', [['autolink', 'http://x.com']]]]);
    expect(p('http://')).toEqual([['text', 'http://']]);
  });
});

describe('parseInline: tags', () => {
  it('requires start or whitespace before and a non-space after', () => {
    expect(p('#tag')).toEqual([['tag', '#tag']]);
    expect(p('a @bob b')).toEqual([['text', 'a '], ['tag', '@bob'], ['text', ' b']]);
    expect(p('a#tag')).toEqual([['text', 'a#tag']]);
    expect(p('# heading')).toEqual([['text', '# heading']]);
    expect(p('#')).toEqual([['text', '#']]);
  });

  it('excludes all-digit tags', () => {
    expect(p('#123')).toEqual([['text', '#123']]);
    expect(p('#123a')).toEqual([['tag', '#123a']]);
    expect(p('#2026-10')).toEqual([['tag', '#2026-10']]);
  });

  it('ends at the listed terminators', () => {
    for (const end of [' ', '\n', '**', '__', '`', "'", ',', '?', '!', ';', '/', ')']) {
      expect(extractTags(`#tag${end}x`)).toEqual(['#tag']);
    }
    expect(extractTags('#tag: x')).toEqual(['#tag']);
    expect(extractTags('#tag.')).toEqual(['#tag']);
    expect(extractTags('#a:b')).toEqual(['#a:b']);
    expect(extractTags('#a.b')).toEqual(['#a.b']);
    expect(extractTags('#f(x) y')).toEqual(['#f(x)']);
    expect(extractTags('(see #tag)')).toEqual(['#tag']);
    expect(extractTags('#a-b_c')).toEqual(['#a-b_c']);
  });

  it('extractTags returns all tags in order, skipping code', () => {
    expect(extractTags('#a `#b` **#c** @d #a')).toEqual(['#a', '@d', '#a']);
    expect(extractTags('x ** #c** y')).toEqual(['#c']);
  });

  it('replaceTag renames and deletes case-insensitively', () => {
    expect(replaceTag('#Work and #work, #workshop', '#work', '#job')).toBe('#job and #job, #workshop');
    expect(replaceTag('a #x b', '#x', null)).toBe('a b');
    expect(replaceTag('#x', '#x', null)).toBe('');
  });
});

describe('parseInline: dates', () => {
  it('parses valid dates and leaves invalid ones as text', () => {
    expect(p('due !(2026-10-22) ok')).toEqual([['text', 'due '], ['date', '2026-10-22'], ['text', ' ok']]);
    expect(p('!(2026-10-22 14:30 - 2026-10-23 | 1w135)')).toEqual([['date', '2026-10-22 14:30 - 2026-10-23 | 1w135']]);
    expect(p('!(2026-13-01)')).toEqual([['text', '!(2026-13-01)']]);
    expect(p('wow!(nope)')).toEqual([['text', 'wow!(nope)']]);
  });

  it('extractDates is syntax-aware', () => {
    const d = extractDates('a !(2026-01-02) `!(2026-01-03)`');
    expect(d).toHaveLength(1);
    expect(d[0]).toMatchObject({ start: 2, end: 15, body: '2026-01-02' });
  });
});

describe('extractLinks / inlineToPlain', () => {
  it('collects links, images and autolinks', () => {
    expect(extractLinks('[a](u1) ![i](u2) http://u3 `[c](u4)`').map((l) => [l.kind, l.text, l.url])).toEqual([
      ['link', 'a', 'u1'], ['image', 'i', 'u2'], ['autolink', 'http://u3', 'http://u3'],
    ]);
  });

  it('plain text drops formatting', () => {
    expect(inlineToPlain('**b** __i__ \\* [t](u) [](u) ![](i) `c` #tag')).toBe('b i * t Link Image c #tag');
    expect(inlineToPlain('[](https://app/d/x#z=y)', (u) => (u.endsWith('#z=y') ? 'Target' : undefined))).toBe('Target');
  });
});

describe('URLs', () => {
  it('normalizes per spec', () => {
    expect(normalizeUrl('https://a.b')).toBe('https://a.b');
    expect(normalizeUrl('mailto:me@x.y')).toBe('mailto:me@x.y');
    expect(normalizeUrl('example.com/x')).toBe('http://example.com/x');
    expect(normalizeUrl('/abs/path')).toBe('file:///abs/path');
    expect(normalizeUrl('C:\\Users\\me')).toBe('file:///C:/Users/me');
  });

  it('rejects dangerous schemes', () => {
    expect(safeUrl('javascript:alert(1)')).toBeNull();
    expect(safeUrl(' JavaScript:alert(1)')).toBeNull();
    expect(safeUrl('java\tscript:alert(1)')).toBeNull();
    expect(safeUrl('vbscript:x')).toBeNull();
    expect(safeUrl('data:text/html,<b>')).toBeNull();
    expect(safeUrl('data:image/png;base64,AAA', { allowDataImage: true })).toBe('data:image/png;base64,AAA');
    expect(safeUrl('https://ok')).toBe('https://ok');
  });
});

describe('renderInline', () => {
  const now = new Date(2026, 9, 22, 12, 0);

  it('escapes all user text', () => {
    const html = renderInline('<script>alert("x")</script> & \'q\'');
    expect(html).not.toContain('<script>');
    expect(html).toContain('&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt; &amp; &#39;q&#39;');
  });

  it('wraps text with source offsets', () => {
    expect(renderInline('ab **cd**')).toBe(
      '<span class="o-t" data-s="0" data-e="3">ab </span><strong class="o-bold" data-s="3" data-e="9"><span class="o-t" data-s="5" data-e="7">cd</span></strong>',
    );
  });

  it('renders newlines as <br> with per-line offsets', () => {
    expect(renderInline('a\nbc')).toBe('<span class="o-t" data-s="0" data-e="1">a</span><br><span class="o-t" data-s="2" data-e="4">bc</span>');
  });

  it('renders each construct with o- classes', () => {
    expect(renderInline('__i__')).toContain('<em class="o-italic"');
    expect(renderInline('~~s~~')).toContain('<del class="o-strike"');
    expect(renderInline('==h==')).toContain('<mark class="o-highlight"');
    expect(renderInline('`c`')).toContain('<code class="o-code" data-s="0" data-e="3"><span class="o-t" data-s="1" data-e="2">c</span></code>');
    expect(renderInline('```py\nx\n```')).toContain('<pre class="o-codeblock" data-lang="py"');
    expect(renderInline('$$x^2$$')).toContain('class="o-latex" data-tex="x^2"');
    expect(renderInline('$$x^2$$')).toContain('data-select-whole');
    expect(renderInline('#t')).toBe('<span class="o-tag" data-tag="#t" data-s="0" data-e="2" data-atomic>#t</span>');
    expect(renderInline('\\*')).toBe('<span class="o-esc" data-s="0" data-e="2">*</span>');
  });

  it('renders links safely', () => {
    expect(renderInline('[t](x.com)')).toBe(
      '<a class="o-link" href="http://x.com" target="_blank" rel="noopener noreferrer" data-s="0" data-e="10" data-atomic><span class="o-t" data-s="1" data-e="2">t</span></a>',
    );
    const bad = renderInline('[t](javascript:alert(1))');
    expect(bad).not.toContain('href');
    expect(bad).not.toContain('javascript');
    expect(renderInline('[](u)')).toContain('>Link</a>');
    expect(renderInline('javascript://%0aalert(1)')).not.toContain('<a');
    expect(renderInline('[x"><img src=x onerror=alert(1)>](http://a)')).not.toContain('<img');
    expect(renderInline('[t]("onmouseover="alert(1))')).not.toMatch(/href="[^"]*"onmouseover/);
  });

  it('internal links and live titles', () => {
    const html = renderInline('[](https://app/d/1#z=2)', {
      isInternal: (u) => u.startsWith('https://app/'),
      linkTitle: () => 'Title <b>',
    });
    expect(html).toContain('class="o-link o-link-internal"');
    expect(html).not.toContain('target=');
    expect(html).toContain('>Title &lt;b&gt;</a>');
  });

  it('images as chips or inline', () => {
    expect(renderInline('![](p.png)')).toContain('class="o-image"');
    expect(renderInline('![](p.png)')).toContain('>Image</a>');
    expect(renderInline('![a](p.png)', { inlineImages: true })).toContain('<img class="o-img" src="http://p.png" alt="a"');
    expect(renderInline('![a](javascript:x)', { inlineImages: true })).not.toContain('<img');
  });

  it('dates: relative label, overdue and recurring classes', () => {
    expect(renderInline('!(2026-10-22)', { now })).toContain('>Today</span>');
    expect(renderInline('!(2026-10-20)', { now, markOverdue: true })).toContain('class="o-date o-overdue"');
    expect(renderInline('!(2026-10-20)', { now })).not.toContain('o-overdue');
    const rec = renderInline('!(2026-10-23 15:00 | 1w)', { now });
    expect(rec).toContain('o-recurring');
    expect(rec).toContain('title="every week"');
    expect(rec).toContain('>Tomorrow 3 pm<');
    expect(renderInline('!(2026-10-23 15:00)', { now, hour12: false })).toContain('>Tomorrow 15:00<');
  });

  it('highlights search terms inside text', () => {
    expect(renderInline('Foo bar foo', { highlight: ['foo'] })).toBe(
      '<span class="o-t" data-s="0" data-e="11"><mark class="o-match">Foo</mark> bar <mark class="o-match">foo</mark></span>',
    );
    expect(renderInline('a<b', { highlight: ['<'] })).toContain('<mark class="o-match">&lt;</mark>');
  });

  it('clean mode emits plain HTML without attributes', () => {
    expect(renderInline('**b** __i__ [l](http://x) #t !(2026-01-01) `c`', { clean: true })).toBe(
      '<strong>b</strong> <em>i</em> <a href="http://x">l</a> #t !(2026-01-01) <code>c</code>',
    );
  });
});

describe('caret mapping helpers', () => {
  it('mapDisplayOffset maps 1:1 runs and clamps atomic ones', () => {
    expect(mapDisplayOffset(5, 9, 4, 2)).toBe(7);
    expect(mapDisplayOffset(5, 7, 1, 0)).toBe(5); // escape, before char
    expect(mapDisplayOffset(5, 7, 1, 1)).toBe(7); // escape, after char
    expect(mapDisplayOffset(0, 13, 5, 3)).toBe(13);
  });

  it('tokenPathAt finds the innermost token', () => {
    const path = tokenPathAt(parseInline('a **b __c__**'), 8);
    expect(path.map((t) => t.t)).toEqual(['bold', 'italic', 'text']);
  });
});
