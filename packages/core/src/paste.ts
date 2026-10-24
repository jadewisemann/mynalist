// Clipboard / import parsing: indented plain text, HTML → indented markdown
// text, and OPML → portable item trees. Runs anywhere (no DOMParser): HTML and
// OPML go through a small tolerant tokenizer.

import type { Props } from './tree.ts';

/** Portable item tree (same shape as commands.Portable). First item = first line. */
export interface PortableItem {
  props: Props;
  collapsed?: boolean;
  children: PortableItem[];
}

// ---- plain text ------------------------------------------------------------------


/** CRLF/CR → LF. */
export function normalizeNewlines(s: string): string {
  return s.replace(/\r\n?/g, '\n');
}

const leadingSpaces = (l: string) => /^ */.exec(l)![0].length;

/**
 * Indented plain text → items (docs/02 §5.2): tabs become 4 spaces, the
 * common indent is stripped, deeper lines nest under the previous line,
 * shallower lines pop to a matching level, blank lines are skipped except the
 * last one. When more than one line has text, leading bullets (`- `, `* `,
 * `• `, …) are stripped. The first returned item is line 1 (meant to be spliced
 * into the item at the caret); a trailing newline yields a final empty item.
 */
export function parsePlainText(text: string): PortableItem[] {
  const lines = normalizeNewlines(text).replace(/\t/g, '    ').split('\n');
  const nonBlank = lines.filter((l) => l.trim() !== '');
  const min = nonBlank.length ? Math.min(...nonBlank.map(leadingSpaces)) : 0;
  // Like Dynalist, list markers ("- ", "* ", "1. ") stay part of the text and blank lines become
  // empty items; only indentation carries structure.
  const clean = (l: string) => (l.trim() === '' ? '' : l.replace(/^ +/, ''));
  const mk = (c: string): PortableItem => ({ props: c ? { c } : {}, collapsed: false, children: [] });

  const first = mk(clean(lines[0]!));
  const roots: PortableItem[] = [first];
  // stack entries: item, its indent, and the list it lives in
  type Entry = { item: PortableItem; indent: number; list: PortableItem[] };
  let stack: Entry[] = [{ item: first, indent: lines[0]!.trim() === '' ? 0 : leadingSpaces(lines[0]!) - min, list: roots }];
  for (let k = 1; k < lines.length; k++) {
    const line = lines[k]!;
    const top0 = stack[stack.length - 1]!;
    const indent = line.trim() === '' ? top0.indent : leadingSpaces(line) - min;
    const item = mk(clean(line));
    const top = stack[stack.length - 1]!;
    if (indent > top.indent) {
      top.item.children.push(item);
      stack.push({ item, indent, list: top.item.children });
      continue;
    }
    while (stack.length && stack[stack.length - 1]!.indent > indent) stack.pop();
    if (!stack.length) {
      roots.push(item);
      stack = [{ item, indent, list: roots }];
      continue;
    }
    const sib = stack[stack.length - 1]!;
    sib.list.push(item);
    stack[stack.length - 1] = { item, indent, list: sib.list };
  }
  return roots;
}

// ---- markup tokenizer ---------------------------------------------------------------

export type MarkupToken =
  | { k: 'open'; name: string; attrs: Record<string, string>; selfClose: boolean }
  | { k: 'close'; name: string }
  | { k: 'text'; v: string };

const ENTITIES: Record<string, string> = {
  amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: ' ', copy: '©', reg: '®', trade: '™',
  hellip: '…', mdash: '—', ndash: '–', lsquo: '‘', rsquo: '’', ldquo: '“', rdquo: '”', bull: '•',
  middot: '·', euro: '€', pound: '£', yen: '¥', cent: '¢', sect: '§', deg: '°', times: '×', divide: '÷',
  laquo: '«', raquo: '»', shy: '', zwj: '‍', zwnj: '‌', ensp: ' ', emsp: ' ', thinsp: ' ',
};

export function decodeEntities(s: string): string {
  if (!s.includes('&')) return s;
  return s.replace(/&(#x[0-9a-fA-F]+|#\d+|[a-zA-Z][a-zA-Z0-9]*);?/g, (m, e: string) => {
    if (e[0] === '#') {
      const cp = e[1] === 'x' || e[1] === 'X' ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10);
      if (!Number.isFinite(cp) || cp < 0 || cp > 0x10ffff || (cp >= 0xd800 && cp <= 0xdfff)) return '�';
      return cp === 0 ? '�' : String.fromCodePoint(cp);
    }
    const v = ENTITIES[e] ?? ENTITIES[e.toLowerCase()];
    return v !== undefined && (m.endsWith(';') || e in ENTITIES) ? v : m;
  });
}

const RAW_TEXT = new Set(['script', 'style', 'textarea', 'xmp']);

/** Tolerant HTML/XML tokenizer. Tag and attribute names are lowercased; values decoded. */
export function tokenizeMarkup(src: string): MarkupToken[] {
  const out: MarkupToken[] = [];
  const n = src.length;
  let i = 0;
  let textStart = 0;
  const flushText = (to: number) => {
    if (to > textStart) out.push({ k: 'text', v: decodeEntities(src.slice(textStart, to)) });
  };
  while (i < n) {
    if (src[i] !== '<') { i++; continue; }
    const next = src[i + 1] ?? '';
    if (src.startsWith('<!--', i)) {
      flushText(i);
      const e = src.indexOf('-->', i + 4);
      i = e < 0 ? n : e + 3;
      textStart = i;
      continue;
    }
    if (src.startsWith('<![CDATA[', i)) {
      flushText(i);
      const e = src.indexOf(']]>', i + 9);
      out.push({ k: 'text', v: src.slice(i + 9, e < 0 ? n : e) });
      i = e < 0 ? n : e + 3;
      textStart = i;
      continue;
    }
    if (next === '!' || next === '?') {
      flushText(i);
      const e = src.indexOf('>', i + 2);
      i = e < 0 ? n : e + 1;
      textStart = i;
      continue;
    }
    const m = /^<(\/?)([a-zA-Z][a-zA-Z0-9:_-]*)/.exec(src.slice(i, i + 64));
    if (!m) { i++; continue; }
    flushText(i);
    const closing = m[1] === '/';
    const name = m[2]!.toLowerCase();
    let j = i + m[0].length;
    const attrs: Record<string, string> = {};
    // attributes
    while (j < n) {
      while (j < n && /\s/.test(src[j]!)) j++;
      if (j >= n || src[j] === '>' || src.startsWith('/>', j)) break;
      const am = /^[^\s=>/]+/.exec(src.slice(j, j + 256));
      if (!am) { j++; continue; }
      const an = am[0].toLowerCase();
      j += am[0].length;
      while (j < n && /\s/.test(src[j]!)) j++;
      let val = '';
      if (src[j] === '=') {
        j++;
        while (j < n && /\s/.test(src[j]!)) j++;
        const q = src[j];
        if (q === '"' || q === "'") {
          const e = src.indexOf(q, j + 1);
          val = src.slice(j + 1, e < 0 ? n : e);
          j = e < 0 ? n : e + 1;
        } else {
          const vm = /^[^\s>]*/.exec(src.slice(j))!;
          val = vm[0];
          j += val.length;
        }
      }
      if (!(an in attrs)) attrs[an] = decodeEntities(val);
    }
    const selfClose = src.startsWith('/>', j);
    i = selfClose ? j + 2 : Math.min(n, j + 1);
    textStart = i;
    if (closing) {
      out.push({ k: 'close', name });
      continue;
    }
    out.push({ k: 'open', name, attrs, selfClose });
    if (RAW_TEXT.has(name) && !selfClose) {
      const re = new RegExp(`</${name}\\s*>`, 'ig');
      re.lastIndex = i;
      const em = re.exec(src);
      const end = em ? em.index : n;
      if (end > i) out.push({ k: 'text', v: name === 'textarea' ? decodeEntities(src.slice(i, end)) : src.slice(i, end) });
      out.push({ k: 'close', name });
      i = em ? em.index + em[0].length : n;
      textStart = i;
    }
  }
  flushText(n);
  return out;
}

// ---- HTML → indented text ------------------------------------------------------------

const BLOCKS = new Set([
  'p', 'div', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'li', 'ul', 'ol', 'blockquote', 'pre', 'table', 'thead', 'tbody',
  'tfoot', 'tr', 'section', 'article', 'header', 'footer', 'nav', 'aside', 'dl', 'dt', 'dd', 'hr', 'figure',
  'figcaption', 'address', 'main', 'form', 'fieldset', 'details', 'summary', 'center', 'caption', 'body', 'html',
]);
const SKIP = new Set(['script', 'style', 'head', 'title', 'template', 'noscript', 'xml', 'iframe', 'object', 'svg', 'math', 'textarea', 'select', 'button']);
const VOID = new Set(['br', 'img', 'hr', 'meta', 'link', 'input', 'col', 'area', 'base', 'wbr', 'source', 'embed', 'param', 'track']);

interface Frame {
  name: string;
  markers: Marker[];
  link?: { href: string; pos: number; line: number };
  pre?: boolean;
  list?: boolean;
}

interface Marker {
  m: string;
  /** position of the opening marker in the current line, -1 = not emitted on this line yet */
  pos: number;
}

function styleMarkers(name: string, style: string): string[] {
  const s = style.toLowerCase().replace(/\s+/g, '');
  const weight = /font-weight:(bold|bolder|[6-9]00|normal|lighter|[1-5]00)/.exec(s)?.[1];
  const italic = /font-style:(italic|oblique|normal)/.exec(s)?.[1];
  const out: string[] = [];
  const boldTag = name === 'b' || name === 'strong';
  const italicTag = name === 'i' || name === 'em';
  const isBold = weight ? !/^(normal|lighter|[1-5]00)$/.test(weight) : boldTag;
  const isItalic = italic ? italic !== 'normal' : italicTag;
  if (isBold) out.push('**');
  if (isItalic) out.push('__');
  return out;
}

/**
 * Convert clipboard HTML to indented markdown text (docs/02 §5.2): one tab
 * per list nesting level, block elements and `<br>` → new lines, b/strong →
 * `**`, i/em → `__`, links → `[text](href)` (bare URL if text == href),
 * images → `![alt](src)`, scripts/styles dropped, whitespace collapsed except
 * in `<pre>`. MS Word list paragraphs use `mso-list … levelN` as depth.
 */
export function htmlToText(html: string): string {
  const tokens = tokenizeMarkup(html);
  const isWord = /urn:schemas-microsoft-com:office|class=["']?Mso|mso-/i.test(html);
  const lines: string[] = [];
  let cur = '';
  let curDepth: number | null = null;
  let lineNo = 0;
  let listDepth = 0;
  let preDepth = 0;
  let pendingDepth: number | null = null;
  const stack: Frame[] = [];
  let skip: { name: string; depth: number } | null = null;

  const openMarkers = () => stack.flatMap((f) => f.markers);

  const closeMarker = (mk: Marker) => {
    if (mk.pos < 0) return;
    const inner = cur.slice(mk.pos + mk.m.length);
    if (inner.trim() === '') {
      cur = cur.slice(0, mk.pos) + inner;
    } else {
      const lead = /^\s*/.exec(inner)![0];
      const trail = /\s*$/.exec(inner)![0];
      cur = cur.slice(0, mk.pos) + lead + mk.m + inner.trim() + mk.m + trail;
    }
    mk.pos = -1;
  };

  const flush = () => {
    const ms = openMarkers();
    for (let k = ms.length - 1; k >= 0; k--) closeMarker(ms[k]!);
    const text = preDepth ? cur.replace(/\s+$/, '') : cur.trim();
    if (text) lines.push('\t'.repeat(Math.max(0, curDepth ?? 0)) + text);
    cur = '';
    curDepth = null;
    lineNo++;
  };

  const append = (s: string) => {
    if (!s) return;
    if (!preDepth) {
      s = s.replace(/[\s ]+/g, ' ');
      if (cur === '' || /\s$/.test(cur)) s = s.replace(/^ /, '');
      if (!s) return;
    }
    if (curDepth === null) {
      if (s.trim() === '' && !preDepth) return;
      curDepth = pendingDepth ?? listDepth;
    }
    if (s.trim() !== '') {
      // emit pending markers (outermost first) and pin link starts after outer markers
      for (const f of stack) {
        for (const mk of f.markers) {
          if (mk.pos < 0) {
            mk.pos = cur.length;
            cur += mk.m;
          }
        }
        if (f.link && f.link.pos < 0) {
          f.link.pos = cur.length;
          f.link.line = lineNo;
        }
      }
    }
    cur += s;
  };

  for (const tok of tokens) {
    if (skip) {
      if (tok.k === 'open' && tok.name === skip.name && !tok.selfClose) skip.depth++;
      else if (tok.k === 'close' && tok.name === skip.name && --skip.depth === 0) skip = null;
      continue;
    }
    if (tok.k === 'text') {
      if (preDepth) {
        const parts = tok.v.replace(/\r\n?/g, '\n').split('\n');
        parts.forEach((p, k) => {
          if (k > 0) flush();
          append(p.replace(/ /g, ' '));
        });
      } else {
        append(tok.v);
      }
      continue;
    }
    if (tok.k === 'open') {
      const { name, attrs } = tok;
      const style = attrs.style ?? '';
      if (SKIP.has(name) || (name === 'span' && /mso-list:\s*ignore/i.test(style))) {
        if (!tok.selfClose && !VOID.has(name)) skip = { name, depth: 1 };
        continue;
      }
      if (name === 'br') { flush(); continue; }
      if (name === 'img') {
        const src = attrs.src ?? '';
        if (src) append(`![${attrs.alt ?? ''}](${src})`);
        continue;
      }
      if (name === 'td' || name === 'th') {
        if (cur.trim()) append(' ');
      }
      if (BLOCKS.has(name)) {
        flush();
        pendingDepth = null;
        if (isWord) {
          const lvl = /mso-list:[^;"']*level(\d+)/i.exec(style);
          if (lvl) pendingDepth = +lvl[1]!;
          else if (/^h[1-6]$/.test(name)) pendingDepth = +name[1]! - 1;
        }
      }
      if (VOID.has(name) || tok.selfClose) continue;
      const frame: Frame = { name, markers: [] };
      if (name === 'ul' || name === 'ol') {
        listDepth++;
        frame.list = true;
      }
      if (name === 'pre') {
        preDepth++;
        frame.pre = true;
      }
      if (name === 'a') {
        const href = (attrs.href ?? '').trim();
        frame.link = { href: /^\s*(javascript|vbscript|data):/i.test(href) ? '' : href, pos: -1, line: lineNo };
      }
      if (['b', 'strong', 'i', 'em', 'span', 'font'].includes(name)) {
        for (const m of styleMarkers(name, style)) {
          if (!openMarkers().some((x) => x.m === m)) frame.markers.push({ m, pos: -1 });
        }
      }
      stack.push(frame);
      continue;
    }
    // close
    const idx = stack.map((f) => f.name).lastIndexOf(tok.name);
    if (idx < 0) {
      if (BLOCKS.has(tok.name)) flush();
      continue;
    }
    while (stack.length > idx) {
      const f = stack[stack.length - 1]!;
      for (let k = f.markers.length - 1; k >= 0; k--) closeMarker(f.markers[k]!);
      if (f.link && f.link.pos >= 0 && f.link.line === lineNo && f.link.href) {
        const text = cur.slice(f.link.pos);
        const t = text.trim();
        if (t) {
          const lead = /^\s*/.exec(text)![0];
          const trail = /\s*$/.exec(text)![0];
          const md = t === f.link.href ? f.link.href : `[${t}](${f.link.href})`;
          cur = cur.slice(0, f.link.pos) + lead + md + trail;
        }
      }
      if (BLOCKS.has(f.name)) {
        flush();
        pendingDepth = null;
      }
      if (f.list) listDepth--;
      if (f.pre) preDepth--;
      stack.pop();
    }
  }
  flush();
  return lines.join('\n');
}

// ---- OPML ------------------------------------------------------------------------------

export interface OpmlDocument {
  title: string;
  /** `<flavor>` from the head, if any */
  flavor: string;
  items: PortableItem[];
}

/** Flavors whose `text` attributes are raw inline markdown (no HTML conversion). */
export const NATIVE_FLAVORS = ['outliner', 'dynalist'];

/** True if the string looks like an OPML document. */
export function isOpml(text: string): boolean {
  const t = text.trimStart().slice(0, 512);
  return /^(<\?xml[^>]*\?>\s*)?(<!--[\s\S]*?-->\s*)*<opml[\s>]/i.test(t);
}

/** Inline HTML (b, i, a…) from foreign OPML → markdown, newlines kept. */
function inlineHtmlToMarkdown(s: string): string {
  if (!/<[a-zA-Z/]/.test(s)) return s;
  return htmlToText(s.replace(/\r?\n/g, '<br>'));
}

const truthy = (v: string | undefined) => v !== undefined && /^(true|1|yes)$/i.test(v.trim());

/** Parse OPML (any flavor) into portable items. Null if it isn't OPML. */
export function parseOpml(xml: string): OpmlDocument | null {
  const tokens = tokenizeMarkup(xml);
  if (!tokens.some((t) => t.k === 'open' && t.name === 'opml')) return null;
  let title = '';
  let flavor = '';
  let field: 'title' | 'flavor' | null = null;
  let inHead = false;
  // flavor may come after the outlines in malformed files: collect raw attrs, convert at the end
  type Raw = { attrs: Record<string, string>; children: Raw[] };
  const roots: Raw[] = [];
  const stack: Raw[] = [];
  for (const t of tokens) {
    if (t.k === 'open') {
      if (t.name === 'head') inHead = true;
      else if (inHead && (t.name === 'title' || t.name === 'flavor')) field = t.selfClose ? null : t.name;
      else if (t.name === 'outline') {
        const raw: Raw = { attrs: t.attrs, children: [] };
        if (stack.length) stack[stack.length - 1]!.children.push(raw);
        else roots.push(raw);
        if (!t.selfClose) stack.push(raw);
      }
    } else if (t.k === 'close') {
      if (t.name === 'head') inHead = false;
      else if (t.name === field) field = null;
      else if (t.name === 'outline') stack.pop();
    } else if (field === 'title') title += t.v;
    else if (field === 'flavor') flavor += t.v;
  }
  flavor = flavor.trim();
  const native = NATIVE_FLAVORS.includes(flavor.toLowerCase());
  const conv = (raw: Raw): PortableItem => {
    const a = raw.attrs;
    const props: Props = {};
    let c = a.text ?? a.title ?? '';
    let n = a._note ?? a.note ?? '';
    if (!native) {
      c = inlineHtmlToMarkdown(c).replace(/\n/g, ' ');
      n = inlineHtmlToMarkdown(n);
    }
    if (c) props.c = c;
    if (n) props.n = n;
    if (truthy(a.checkbox) || truthy(a.checklist)) props.cb = true;
    if (truthy(a.complete) || truthy(a._complete)) props.ck = true;
    const h = parseInt(a.heading ?? '', 10);
    if (h >= 1 && h <= 3) props.h = h;
    const cl = parseInt(a.colorlabel ?? a._colorlabel ?? '', 10);
    if (cl >= 1 && cl <= 6) props.cl = cl;
    if (/^(arabic|numbered|decimal)$/i.test(a.liststyle ?? '')) props.num = true;
    return { props, collapsed: truthy(a.collapsed) || truthy(a._collapsed), children: raw.children.map(conv) };
  };
  return { title: title.trim(), flavor, items: roots.map(conv) };
}

// ---- clipboard ---------------------------------------------------------------------------

export interface ClipboardData {
  'text/plain'?: string;
  'text/html'?: string;
  'text/opml'?: string;
}

export interface ParsedPaste {
  items: PortableItem[];
  /** came from OPML: lossless, restore collapsed flags; else derived from text */
  structured: boolean;
}

/**
 * Clipboard → items, by priority (docs/02 §5.2): OPML flavor or plain text
 * that is OPML; then HTML (converted to indented text); then plain text.
 * Returns null when there is no usable text. Literal-text contexts (notes,
 * zoom title) should use `normalizeNewlines(plain)` instead.
 */
export function parseClipboard(data: ClipboardData): ParsedPaste | null {
  const plain = data['text/plain'];
  for (const src of [data['text/opml'], plain && isOpml(plain) ? plain : undefined]) {
    if (!src) continue;
    const doc = parseOpml(src);
    if (doc && doc.items.length) return { items: doc.items, structured: true };
  }
  const html = data['text/html'];
  if (html && html.trim()) {
    const text = htmlToText(html);
    if (text.trim()) return { items: parsePlainText(text), structured: false };
  }
  if (plain) return { items: parsePlainText(plain), structured: false };
  return null;
}
