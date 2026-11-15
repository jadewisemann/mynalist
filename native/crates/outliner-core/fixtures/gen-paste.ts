// Records what the TS clipboard / import parsers (packages/core/src/paste.ts) do, for
// crates/outliner-core/tests/paste.rs: indented text, entities, the markup tokenizer, HTML → text, OPML and the
// clipboard priority. Run from the repo root:
//
//   node native/crates/outliner-core/fixtures/gen-paste.ts
//
// Not generated (TS quirks the port leaves out, see src/paste.rs): entity and attribute names of
// Object.prototype members (`&constructor;`, `toString="x"`), attribute names over 256 UTF-16 units.

import { writeFileSync } from 'node:fs';
import {
  decodeEntities, htmlToText, isOpml, normalizeNewlines, parseClipboard, parseOpml, parsePlainText, portableToOpml,
  tokenizeMarkup, exportOpml, ROOT,
} from '../../../../packages/core/src/index.ts';
import { int, pick, rand, randomTree, reseed, textOf } from './lib.ts';

reseed(0x9a57e);

// ---- plain text
const LINE = ['', ' ', '  ', '    ', '\t', '\t\t', '- ', '* ', '1. ', 'item', 'b', '한글', '😀', '　x', '  '];
const plain = Array.from({ length: 1500 }, () => {
  const lines = Array.from({ length: int(8) }, () => textOf(LINE, 3));
  const s = lines.join(pick(['\n', '\r\n', '\r'])) + (rand() < 0.3 ? '\n' : '');
  return [s, parsePlainText(s), normalizeNewlines(s)];
});

// ---- entities
const ENT = ['&', 'amp', ';', '&amp;', '&AMP;', '&Amp', '&lt', '&nbsp;', '&#65;', '&#x41;', '&#X41;', '&#x;', '&#0;', '&#55296;', '&#1114112;',
  '&#99999999999999999999;', '&#x1F600;', '&shy;', '&hellip', '&foo;', '&a1;', 'x', ' ', '한', '#', '&&', '&#', '&copy2'];
const entities = Array.from({ length: 1500 }, () => {
  const s = textOf(ENT, 6);
  return [s, decodeEntities(s)];
});

// ---- markup
const TAGS = ['p', 'div', 'b', 'strong', 'i', 'em', 'span', 'font', 'a', 'ul', 'ol', 'li', 'br', 'img', 'pre', 'h1', 'h3', 'td', 'th', 'tr', 'table',
  'script', 'style', 'textarea', 'xmp', 'head', 'title', 'svg', 'button', 'hr', 'o:p', 'x-y_z', 'B', 'LI', 'Span'];
const STYLES = ['font-weight:bold', 'font-weight: 700', 'font-weight:normal', 'font-weight:bolder', 'FONT-WEIGHT:600', 'font-weight:300',
  'font-style:italic', 'font-style: oblique', 'font-style:normal', 'mso-list:l0 level2 lfo1', 'mso-list:Ignore', 'mso-list: l1 level1 level3 lfo2',
  'color:red', ''];
function attr(): string {
  const name = pick(['href', 'HREF', 'src', 'alt', 'style', 'class', 'data-x', 'title', 'Style']);
  let v: string;
  if (name.toLowerCase() === 'style') v = pick(STYLES) + (rand() < 0.3 ? ';' + pick(STYLES) : '');
  else if (name.toLowerCase() === 'href' || name === 'src') v = pick(['https://x.org/a?b=1&amp;c', ' javascript:alert(1)', 'data:x', 'VBScript:x', '', 'img.png', 'https://same']);
  else if (name === 'class') v = pick(['MsoNormal', 'MsoListParagraph', 'x']);
  else v = pick(['x', 'a b', '&lt;b&gt;', '"', "'", '']);
  const q = pick(['"', "'", '', '"']);
  if (q === '' && /[\s>]/.test(v)) return `${name}=x`;
  if (q !== '' && v.includes(q)) v = v.split(q).join('');
  return rand() < 0.1 ? name : `${name}${pick(['=', ' = '])}${q}${v}${q}`;
}
function tag(): string {
  const r = rand();
  if (r < 0.1) return pick(['<!-- c -->', '<!--', '<![CDATA[<b>x</b>]]>', '<!DOCTYPE html>', '<?xml version="1.0"?>', '<', '< b>', '</>', '<3', '</ p>']);
  const name = pick(TAGS);
  if (r < 0.4) return `</${name}${rand() < 0.1 ? ' ' : ''}>`;
  const attrs = Array.from({ length: int(3) }, attr).map((a) => ' ' + a).join('');
  return `<${name}${attrs}${pick(['>', '>', '/>', ' />', ''])}`;
}
const TEXT = ['hello', ' ', '  ', '\n', '\t', 'a  b', '&amp;', '&nbsp;', ' ', '한글', '😀', 'https://same', '</script>', '</SCRIPT >', 'x'];
function html(): string {
  let s = rand() < 0.15 ? '<html xmlns:o="urn:schemas-microsoft-com:office:office">' : '';
  const n = 1 + int(14);
  for (let i = 0; i < n; i++) s += rand() < 0.5 ? tag() : pick(TEXT);
  return s;
}
const markup = Array.from({ length: 3000 }, () => {
  const s = html();
  return { s, tokens: tokenizeMarkup(s), text: htmlToText(s) };
});

// structured HTML: nested lists, links and styles as browsers and Word put them on the clipboard
function block(depth: number): string {
  const inline = () => {
    const w = pick(['word', 'two words', '한글', 'x']);
    const r = rand();
    if (r < 0.15) return `<b>${w}</b>`;
    if (r < 0.25) return `<span style="${pick(STYLES)}">${w}</span>`;
    if (r < 0.35) return `<a href="${pick(['https://x.org', 'https://same', 'javascript:x'])}">${rand() < 0.3 ? 'https://same' : w}</a>`;
    if (r < 0.4) return `<i> ${w} </i>`;
    if (r < 0.45) return `<img src="p.png" alt="${w}">`;
    return w;
  };
  const items = Array.from({ length: 1 + int(3) }, () => {
    let s = Array.from({ length: 1 + int(3) }, inline).join(pick([' ', '', '<br>']));
    if (depth < 3 && rand() < 0.4) s += block(depth + 1);
    return `<li>${s}</li>`;
  });
  return `<${pick(['ul', 'ol'])}>${items.join(pick(['', '\n']))}</ul>`;
}
const structured = Array.from({ length: 600 }, () => {
  const s = rand() < 0.3 ? `<p class=MsoListParagraph style='mso-list:l0 level${1 + int(3)} lfo1'><span style='mso-list:Ignore'>·</span>Word item</p>` + block(0)
    : rand() < 0.2 ? `<pre>  code\n    indented\r\n&lt;x&gt; </pre>` + block(0) : block(0);
  return { s, tokens: tokenizeMarkup(s), text: htmlToText(s) };
});

// ---- OPML
const opmlTexts = ['plain', '<b>bold</b> &amp; x', 'a\nb', '**md**', '', 'https://same', '<a href="https://x.org">x</a>'];
function opml(): string {
  const r = rand();
  if (r < 0.4) {
    const t = randomTree(2 + int(8), { texts: () => pick(opmlTexts), notes: () => pick(opmlTexts) });
    return exportOpml(t, t.get(ROOT)!.children, { title: pick(['Doc', 'A &amp; B']) });
  }
  const flavor = pick(['', '<flavor>outliner</flavor>', '<flavor> Dynalist </flavor>', '<flavor>other</flavor>', '<flavor/>']);
  const outline = (d: number): string => {
    const at: string[] = [`text="${pick(['x', '&lt;b&gt;b&lt;/b&gt;', 'a&#10;b', ''])}"`];
    if (rand() < 0.3) at.push(`_note="${pick(['n', 'n&#10;two'])}"`);
    if (rand() < 0.2) at.push(`title="T"`);
    if (rand() < 0.2) at.push(`checkbox="${pick(['true', 'YES', '1', 'no', ' true '])}"`);
    if (rand() < 0.2) at.push(`complete="${pick(['true', 'false'])}"`, `_complete="1"`);
    if (rand() < 0.2) at.push(`heading="${pick(['1', '3', '4', '2.5', ' 2', 'x', '-1'])}"`);
    if (rand() < 0.2) at.push(`colorLabel="${pick(['1', '6', '7', '0'])}"`);
    if (rand() < 0.1) at.push(`_colorLabel="2"`);
    if (rand() < 0.2) at.push(`listStyle="${pick(['arabic', 'Numbered', 'decimal', 'bullet'])}"`);
    if (rand() < 0.2) at.push(`collapsed="true"`);
    const kids = d < 3 && rand() < 0.4 ? Array.from({ length: 1 + int(2) }, () => outline(d + 1)).join('') : '';
    return kids ? `<outline ${at.join(' ')}>${kids}</outline>` : `<outline ${at.join(' ')}${pick(['/>', '></outline>'])}`;
  };
  const body = Array.from({ length: int(4) }, () => outline(0)).join('\n');
  return `${pick(['', '<?xml version="1.0"?>\n', '  <!-- c -->\n', '<!-- a --> x --> '])}${pick(['<opml version="2.0">', '<OPML>', '<opmlx>'])}<head><title>${pick(['T', ' A &amp; B ', ''])}</title>${flavor}</head><body>${body}</body></opml>`;
}
const opmls = Array.from({ length: 800 }, () => {
  const s = opml();
  return { s, isOpml: isOpml(s), doc: parseOpml(s) };
});
const opmlProbes = ['<?xml?><opml>', '<?xml x="1" ?>  <opml\n>', '<?xml>?><opml>', '<!--a-->x--><opml>', '<!--a--><!--b-->\n<OPML version="1">', '  \n<opml>', '<opml', 'opml', '<?xml ?><!-- --><opmlx>', ''];

// ---- clipboard
const clipboard = Array.from({ length: 800 }, () => {
  const d: Record<string, string> = {};
  if (rand() < 0.3) d['text/opml'] = rand() < 0.7 ? opml() : 'not opml';
  if (rand() < 0.6) d['text/html'] = rand() < 0.5 ? block(0) : pick(['', '  ', '<p> </p>', html()]);
  if (rand() < 0.8) d['text/plain'] = rand() < 0.2 ? opml() : pick(['', 'a\n  b\nc', 'x\r\ny\n', textOf(LINE, 6)]);
  return { d, r: parseClipboard(d) };
});

const out = new URL('./paste.json', import.meta.url);
writeFileSync(out, JSON.stringify({ plain, entities, markup: [...markup, ...structured], opmls, opmlProbes: opmlProbes.map((s) => [s, isOpml(s)]), clipboard }));
console.log(`wrote paste cases to ${out.pathname}`);
