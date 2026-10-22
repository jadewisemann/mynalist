// Inline markdown-ish syntax: tokenizer with source offsets + safe HTML renderer.
//
// Every token carries `start`/`end` (its full raw range) and
// `innerStart`/`innerEnd` (the range of its content, e.g. between delimiters),
// so a click on rendered text can be mapped back to a caret offset.

import {
  describeRecurrence, formatDateLabel, isOverdue, parseDateBody,
  type DateFormatOptions, type DateSpec, type TimeInput,
} from './dates.ts';

interface Span {
  start: number;
  end: number;
  innerStart: number;
  innerEnd: number;
}

export type EmphasisKind = 'bold' | 'italic' | 'strike' | 'highlight';

/** Literal text; maps 1:1 onto `src.slice(start, end)` (may contain `\n`). */
export interface TextToken extends Span { t: 'text'; text: string }
/** `\x` — renders as `text` (the escaped char); inner range is the char. */
export interface EscapeToken extends Span { t: 'escape'; text: string }
export interface EmphasisToken extends Span { t: EmphasisKind; children: InlineToken[] }
/** `` `code` `` — `text` has `` \` `` escapes resolved; `children` are text/escape parts. */
export interface CodeToken extends Span { t: 'code'; text: string; children: InlineToken[] }
/** ```` ```lang\n…``` ```` — `text` excludes one trailing newline before the fence. */
export interface CodeBlockToken extends Span { t: 'codeblock'; lang: string; text: string }
export interface LatexToken extends Span { t: 'latex'; text: string }
/** `[text](url)` — inner range = text; children = formatted text (no nested links/tags). */
export interface LinkToken extends Span {
  t: 'link'; text: string; url: string; urlStart: number; urlEnd: number; children: InlineToken[];
}
/** `![alt](url)` — `text` is the alt text with escapes resolved. */
export interface ImageToken extends Span { t: 'image'; text: string; url: string; urlStart: number; urlEnd: number }
export interface AutolinkToken extends Span { t: 'autolink'; url: string }
/** `#tag` / `@tag` — `tag` includes the prefix char. */
export interface TagToken extends Span { t: 'tag'; tag: string }
/** `!(…)` — `text` is the raw body between the parens. */
export interface DateToken extends Span { t: 'date'; text: string; date: DateSpec }

export type InlineToken =
  | TextToken | EscapeToken | EmphasisToken | CodeToken | CodeBlockToken | LatexToken
  | LinkToken | ImageToken | AutolinkToken | TagToken | DateToken;

const EMPHASIS: Record<string, EmphasisKind> = { '**': 'bold', '__': 'italic', '~~': 'strike', '==': 'highlight' };
const DELIMS: Record<EmphasisKind, string> = { bold: '**', italic: '__', strike: '~~', highlight: '==' };
export const ESCAPABLE = '\\~$=`*_{}[]()#+-.!';
const isEscapable = (c: string | undefined) => c !== undefined && c !== '' && ESCAPABLE.includes(c);
const isWs = (c: string | undefined) => c !== undefined && /\s/.test(c);
const SCHEME_RE = /[a-zA-Z][a-zA-Z0-9+.-]*:\/\//y;
const SCHEME_CHAR = /[a-zA-Z0-9+.-]/;
const TAG_STOP = "`',?!;/";

interface Ctx {
  src: string;
  /** links, images, autolinks, tags and dates allowed (false inside link text) */
  rich: boolean;
  /** shared lazily-computed `[` → matching `]` table */
  brackets: { map: Int32Array | null };
}

interface SeqResult {
  tokens: InlineToken[];
  pos: number;
  closedBy: string | null;
}

/** Tokenize inline source. Never throws; unclosed delimiters become text. */
export function parseInline(src: string): InlineToken[] {
  return parseSeq({ src, rich: true, brackets: { map: null } }, 0, src.length, []).tokens;
}

function parseSeq(ctx: Ctx, from: number, end: number, closers: string[]): SeqResult {
  const { src } = ctx;
  const tokens: InlineToken[] = [];
  let textStart = from;
  let i = from;
  const flush = (upTo: number) => {
    if (upTo > textStart) {
      tokens.push({ t: 'text', text: src.slice(textStart, upTo), start: textStart, end: upTo, innerStart: textStart, innerEnd: upTo });
    }
  };
  const emit = (tok: InlineToken) => {
    flush(i);
    tokens.push(tok);
    i = tok.end;
    textStart = i;
  };

  while (i < end) {
    const c = src[i]!;
    // escape
    if (c === '\\' && i + 1 < end && isEscapable(src[i + 1])) {
      emit({ t: 'escape', text: src[i + 1]!, start: i, end: i + 2, innerStart: i + 1, innerEnd: i + 2 });
      continue;
    }
    // closer of an enclosing span
    const two = src.slice(i, i + 2);
    if (closers.includes(two)) {
      flush(i);
      return { tokens, pos: i, closedBy: two };
    }
    let tok: InlineToken | null = null;
    if (c === '`') tok = src.startsWith('```', i) ? codeBlock(src, i, end) : inlineCode(src, i, end);
    else if (two === '$$') tok = latex(src, i, end);
    else if (ctx.rich && two === '![') tok = image(ctx, i, end);
    else if (ctx.rich && c === '[') tok = link(ctx, i, end);
    else if (ctx.rich && two === '!(') tok = date(src, i, end);
    else if (ctx.rich && (c === '#' || c === '@') && (i === 0 || isWs(src[i - 1]))) tok = tag(src, i, end, closers);
    else if (ctx.rich && /[a-zA-Z]/.test(c) && (i === 0 || !SCHEME_CHAR.test(src[i - 1]!))) tok = autolink(src, i, end, closers);

    if (tok) {
      emit(tok);
      continue;
    }
    if (c === '`' || two === '$$' || (two === '![' && ctx.rich)) {
      // unclosed opener: literal; skip the whole delimiter so it can't re-open
      i += c === '`' ? (src.startsWith('```', i) ? 3 : src[i + 1] === '`' ? 2 : 1) : 2;
      continue;
    }
    const kind = EMPHASIS[two];
    if (kind) {
      // cheap pre-check: no later delimiter → can't close
      const again = src.indexOf(two, i + 2);
      const r = again >= 0 && again + 2 <= end ? parseSeq(ctx, i + 2, end, [...closers, two]) : null;
      if (r && r.closedBy === two && r.pos > i + 2) {
        emit({ t: kind, children: r.tokens, start: i, end: r.pos + 2, innerStart: i + 2, innerEnd: r.pos });
      } else {
        i += 2; // unclosed or empty: literal delimiter, resume after it
      }
      continue;
    }
    i++;
  }
  flush(i);
  return { tokens, pos: i, closedBy: null };
}

function inlineCode(src: string, i: number, end: number): CodeToken | null {
  const children: InlineToken[] = [];
  let text = '';
  let segStart = i + 1;
  let j = i + 1;
  while (j < end) {
    if (src[j] === '\\' && src[j + 1] === '`' && j + 1 < end) {
      if (j > segStart) children.push(textTok(src, segStart, j));
      children.push({ t: 'escape', text: '`', start: j, end: j + 2, innerStart: j + 1, innerEnd: j + 2 });
      text += src.slice(segStart, j) + '`';
      j += 2;
      segStart = j;
      continue;
    }
    if (src[j] === '`') {
      if (j === i + 1) return null; // empty
      if (j > segStart) children.push(textTok(src, segStart, j));
      text += src.slice(segStart, j);
      return { t: 'code', text, children, start: i, end: j + 1, innerStart: i + 1, innerEnd: j };
    }
    j++;
  }
  return null;
}

const textTok = (src: string, s: number, e: number): TextToken =>
  ({ t: 'text', text: src.slice(s, e), start: s, end: e, innerStart: s, innerEnd: e });

function codeBlock(src: string, i: number, end: number): CodeBlockToken | null {
  const firstClose = src.indexOf('```', i + 3);
  if (firstClose < 0 || firstClose + 3 > end) return null;
  let lang = '';
  let cs = i + 3;
  const nl = src.indexOf('\n', i + 3);
  if (nl >= 0 && nl < firstClose && /^[a-z]*$/.test(src.slice(i + 3, nl))) {
    lang = src.slice(i + 3, nl);
    cs = nl + 1;
  }
  const close = src.indexOf('```', cs);
  if (close < 0 || close + 3 > end) return null;
  let text = src.slice(cs, close);
  if (text.endsWith('\n')) text = text.slice(0, -1);
  let tokEnd = close + 3;
  if (src[tokEnd] === '\n' && tokEnd < end) tokEnd++;
  return { t: 'codeblock', lang, text, start: i, end: tokEnd, innerStart: cs, innerEnd: cs + text.length };
}

function latex(src: string, i: number, end: number): LatexToken | null {
  const close = src.indexOf('$$', i + 2);
  if (close < 0 || close + 2 > end || close === i + 2) return null;
  return { t: 'latex', text: src.slice(i + 2, close), start: i, end: close + 2, innerStart: i + 2, innerEnd: close };
}

/** Find the `]` matching the `[` at `open`, honoring escapes and nesting. */
function matchBracket(ctx: Ctx, open: number, end: number): number {
  let map = ctx.brackets.map;
  if (!map) {
    // one stack pass: equivalent to depth-counting forward from each `[`
    const { src } = ctx;
    map = new Int32Array(src.length).fill(-1);
    const stack: number[] = [];
    for (let j = 0; j < src.length; j++) {
      const c = src[j];
      if (c === '\\' && isEscapable(src[j + 1])) { j++; continue; }
      if (c === '[') stack.push(j);
      else if (c === ']' && stack.length) map[stack.pop()!] = j;
    }
    ctx.brackets.map = map;
  }
  const m = map[open]!;
  return m >= 0 && m < end ? m : -1;
}

/** Given `(` at `open`, find the balanced `)`; no newlines allowed. */
function matchParen(src: string, open: number, end: number): number {
  let depth = 0;
  for (let j = open; j < end; j++) {
    const c = src[j];
    if (c === '\n') return -1;
    if (c === '(') depth++;
    else if (c === ')' && --depth === 0) return j;
  }
  return -1;
}

function linkParts(ctx: Ctx, lb: number, end: number) {
  const { src } = ctx;
  const rb = matchBracket(ctx, lb, end);
  if (rb < 0 || src[rb + 1] !== '(') return null;
  const rp = matchParen(src, rb + 1, end);
  if (rp < 0) return null;
  return { rb, rp };
}

function link(ctx: Ctx, i: number, end: number): LinkToken | null {
  const p = linkParts(ctx, i, end);
  if (!p) return null;
  const children = parseSeq({ ...ctx, rich: false }, i + 1, p.rb, []).tokens;
  return {
    t: 'link', text: ctx.src.slice(i + 1, p.rb), url: ctx.src.slice(p.rb + 2, p.rp).trim(),
    urlStart: p.rb + 2, urlEnd: p.rp, children, start: i, end: p.rp + 1, innerStart: i + 1, innerEnd: p.rb,
  };
}

function image(ctx: Ctx, i: number, end: number): ImageToken | null {
  const { src } = ctx;
  const p = linkParts(ctx, i + 1, end);
  if (!p) return null;
  return {
    t: 'image', text: unescape(src.slice(i + 2, p.rb)), url: src.slice(p.rb + 2, p.rp).trim(),
    urlStart: p.rb + 2, urlEnd: p.rp, start: i, end: p.rp + 1, innerStart: i + 2, innerEnd: p.rb,
  };
}

function date(src: string, i: number, end: number): DateToken | null {
  let j = i + 2;
  while (j < end && src[j] !== ')' && src[j] !== '(' && src[j] !== '\n') j++;
  if (j >= end || src[j] !== ')') return null;
  const body = src.slice(i + 2, j);
  const spec = parseDateBody(body);
  if (!spec) return null;
  return { t: 'date', text: body, date: spec, start: i, end: j + 1, innerStart: i + 2, innerEnd: j };
}

/** Length of a tag body starting at `from` (after `#`/`@`), or -1. */
function scanTag(src: string, from: number, end: number, closers: string[]): number {
  let depth = 0;
  let j = from;
  while (j < end) {
    const c = src[j]!;
    if (isWs(c) || TAG_STOP.includes(c)) break;
    const two = src.slice(j, j + 2);
    if (two === '**' || two === '__' || closers.includes(two)) break;
    if (c === '(') depth++;
    else if (c === ')') {
      if (depth === 0) break;
      depth--;
    } else if ((c === ':' || c === '.') && (j + 1 >= end || isWs(src[j + 1]))) break;
    j++;
  }
  return j - from;
}

function tag(src: string, i: number, end: number, closers: string[]): TagToken | null {
  const len = scanTag(src, i + 1, end, closers);
  if (len <= 0) return null;
  const body = src.slice(i + 1, i + 1 + len);
  if (/^\d+$/.test(body)) return null;
  return { t: 'tag', tag: src.slice(i, i + 1 + len), start: i, end: i + 1 + len, innerStart: i, innerEnd: i + 1 + len };
}

function autolink(src: string, i: number, end: number, closers: string[]): AutolinkToken | null {
  SCHEME_RE.lastIndex = i;
  const m = SCHEME_RE.exec(src);
  if (!m) return null;
  let j = i + m[0].length;
  const bodyStart = j;
  let depth = 0;
  while (j < end) {
    const c = src[j]!;
    if (isWs(c)) break;
    if (closers.includes(src.slice(j, j + 2))) break;
    if (c === '(') depth++;
    else if (c === ')') {
      if (depth === 0) break;
      depth--;
    }
    j++;
  }
  if (j === bodyStart) return null;
  return { t: 'autolink', url: src.slice(i, j), start: i, end: j, innerStart: i, innerEnd: j };
}

/** Resolve `\x` escapes (escapable chars only). */
export function unescape(s: string): string {
  return s.replace(/\\(.)/g, (m, c: string) => (isEscapable(c) ? c : m));
}

/** Backslash-escape inline syntax characters so `s` renders literally. */
export function escapeInline(s: string): string {
  return s.replace(/[\\`*_~=$[\]()#!]/g, '\\$&');
}

// ---- traversal helpers -----------------------------------------------------------

/** Depth-first walk over tokens (including children of spans, code and links). */
export function walkTokens(tokens: InlineToken[], fn: (t: InlineToken) => void): void {
  for (const t of tokens) {
    fn(t);
    if ('children' in t) walkTokens(t.children, fn);
  }
}

export interface FoundTag {
  tag: string;
  start: number;
  end: number;
}

export function findTags(src: string): FoundTag[] {
  const out: FoundTag[] = [];
  walkTokens(parseInline(src), (t) => {
    if (t.t === 'tag') out.push({ tag: t.tag, start: t.start, end: t.end });
  });
  return out;
}

/** All tags (with `#`/`@` prefix, as written) in source order, duplicates kept. */
export function extractTags(src: string): string[] {
  return findTags(src).map((t) => t.tag);
}

/**
 * Rename (or delete, with `to = null`) every occurrence of a tag,
 * case-insensitively. Deleting also removes one adjacent space.
 */
export function replaceTag(src: string, from: string, to: string | null): string {
  const want = from.toLowerCase();
  let out = src;
  for (const f of findTags(src).reverse()) {
    if (f.tag.toLowerCase() !== want) continue;
    let s = f.start;
    let e = f.end;
    if (to === null) {
      if (out[e] === ' ') e++;
      else if (s > 0 && out[s - 1] === ' ') s--;
    }
    out = out.slice(0, s) + (to ?? '') + out.slice(e);
  }
  return out;
}

export interface FoundLink {
  kind: 'link' | 'image' | 'autolink';
  /** link text / image alt (raw source for links); the URL itself for autolinks */
  text: string;
  url: string;
  start: number;
  end: number;
}

export function extractLinks(src: string): FoundLink[] {
  const out: FoundLink[] = [];
  walkTokens(parseInline(src), (t) => {
    if (t.t === 'link' || t.t === 'image') out.push({ kind: t.t, text: t.text, url: t.url, start: t.start, end: t.end });
    else if (t.t === 'autolink') out.push({ kind: 'autolink', text: t.url, url: t.url, start: t.start, end: t.end });
  });
  return out;
}

export interface FoundInlineDate {
  spec: DateSpec;
  body: string;
  start: number;
  end: number;
}

/** Dates in source order (syntax-aware: dates inside code are ignored). */
export function extractDates(src: string): FoundInlineDate[] {
  const out: FoundInlineDate[] = [];
  walkTokens(parseInline(src), (t) => {
    if (t.t === 'date') out.push({ spec: t.date, body: t.text, start: t.start, end: t.end });
  });
  return out;
}

/** Text as displayed, without formatting (breadcrumbs, window titles, finders). */
export function inlineToPlain(src: string, linkTitle?: (url: string) => string | undefined): string {
  return plainOf(parseInline(src), linkTitle);
}

function plainOf(tokens: InlineToken[], linkTitle?: (url: string) => string | undefined): string {
  let s = '';
  for (const t of tokens) {
    switch (t.t) {
      case 'text': case 'escape': case 'code': case 'codeblock': case 'latex': s += t.text; break;
      case 'bold': case 'italic': case 'strike': case 'highlight': s += plainOf(t.children, linkTitle); break;
      case 'link': {
        const own = plainOf(t.children);
        const title = (!t.text || t.text === t.url) ? linkTitle?.(t.url) : undefined;
        s += title ?? (own || 'Link');
        break;
      }
      case 'image': s += t.text || 'Image'; break;
      case 'autolink': s += t.url; break;
      case 'tag': s += t.tag; break;
      case 'date': s += `!(${t.text})`; break;
    }
  }
  return s;
}

// ---- URLs --------------------------------------------------------------------------

/**
 * Spec normalization: keep anything with a `scheme:`; `C:\…` or `/abs/path`
 * → `file://…`; otherwise prefix `http://`.
 */
export function normalizeUrl(raw: string): string {
  const url = raw.trim();
  if (!url) return '';
  if (/^[a-zA-Z]:[\\/]/.test(url)) return 'file:///' + url.replace(/\\/g, '/');
  if (url.startsWith('/')) return 'file://' + url;
  if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(url)) return url;
  if (url.startsWith('#')) return url;
  return 'http://' + url;
}

const BAD_SCHEMES = new Set(['javascript', 'vbscript', 'data', 'livescript', 'mocha']);

/** Normalized URL, or null if it uses a dangerous scheme (javascript:, data:, …). */
export function safeUrl(raw: string, opts: { allowDataImage?: boolean } = {}): string | null {
  const url = normalizeUrl(raw.replace(/[\t\n\r]/g, ''));
  if (!url) return null;
  // browsers ignore control chars/whitespace inside the scheme
  const probe = url.replace(/[\u0000-\u0020\u007f-\u00a0\u1680\u180e\u2000-\u2029\u205f\u3000\ufeff]/g, '').toLowerCase();
  const m = /^([a-z][a-z0-9+.-]*):/.exec(probe);
  if (m && BAD_SCHEMES.has(m[1]!)) {
    if (opts.allowDataImage && /^data:image\/(png|gif|jpe?g|webp|bmp);/.test(probe)) return url;
    return null;
  }
  return url;
}

// ---- HTML rendering ---------------------------------------------------------------

export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);
}

export interface RenderOptions {
  /** current time for relative date labels / overdue (default Date.now()) */
  now?: TimeInput;
  /** 12h clock (default true) */
  hour12?: boolean;
  /** custom date formats (see dates.formatDateLabel) */
  dateFormat?: string;
  dateFormatNoTime?: string;
  /** add `o-overdue` to past dates (pass highlight_overdue && !item.checked) */
  markOverdue?: boolean;
  /** render `![]()` as `<img>` instead of a chip */
  inlineImages?: boolean;
  /** case-insensitive substrings to wrap in `<mark class="o-match">` (search) */
  highlight?: string[];
  /** true for app-internal URLs (rendered with `o-link-internal`, no target) */
  isInternal?: (url: string) => boolean;
  /** live title for links whose text is empty or equals the URL */
  linkTitle?: (url: string) => string | undefined;
  /**
   * Clean HTML for export/clipboard: plain tags (b, i, del, mark, code, a…),
   * no classes or data attributes; tags and dates as literal text.
   */
  clean?: boolean;
}

/**
 * Render inline source to an HTML string. All user text is escaped; URLs go
 * through `safeUrl`.
 *
 * Offset attributes (non-clean mode): every element carries `data-s`/`data-e`
 * (raw source range). Text is always wrapped in an element whose
 * `data-s`..`data-e` maps 1:1 onto its textContent, except escapes and atomic
 * tokens — use `mapDisplayOffset` for those. Atomic tokens (links, images,
 * tags, dates, latex) also get `data-atomic` (click acts instead of placing a
 * caret); latex additionally gets `data-select-whole`.
 */
export function renderInline(src: string, opts: RenderOptions = {}): string {
  return renderTokens(parseInline(src), opts);
}

export function renderTokens(tokens: InlineToken[], opts: RenderOptions = {}): string {
  const r = new Renderer(opts);
  return r.tokens(tokens);
}

/**
 * Map a character offset inside a rendered element back to a source offset.
 * `s`/`e` are the element's data-s/data-e, `displayLen` its textContent length.
 */
export function mapDisplayOffset(s: number, e: number, displayLen: number, offset: number): number {
  if (e - s === displayLen) return s + Math.max(0, Math.min(offset, displayLen));
  if (offset <= 0) return s;
  return e;
}

/** Innermost token containing source `offset` (start <= offset < end), with its ancestors. */
export function tokenPathAt(tokens: InlineToken[], offset: number): InlineToken[] {
  const path: InlineToken[] = [];
  let list: InlineToken[] | null = tokens;
  while (list) {
    const hit: InlineToken | undefined = list.find((t) => t.start <= offset && offset < t.end);
    if (!hit) break;
    path.push(hit);
    list = 'children' in hit ? hit.children : null;
  }
  return path;
}

class Renderer {
  private o: RenderOptions;
  private hl: RegExp | null;

  constructor(o: RenderOptions) {
    this.o = o;
    const terms = (o.highlight ?? []).filter((t) => t.length > 0);
    this.hl = terms.length
      ? new RegExp(terms.sort((a, b) => b.length - a.length).map((t) => t.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|'), 'gi')
      : null;
  }

  private attrs(t: Span, extra = ''): string {
    return this.o.clean ? '' : ` data-s="${t.start}" data-e="${t.end}"${extra}`;
  }

  private cls(c: string): string {
    return this.o.clean ? '' : ` class="${c}"`;
  }

  tokens(list: InlineToken[]): string {
    let out = '';
    for (const t of list) out += this.token(t);
    return out;
  }

  /** Plain text run starting at source offset `s` (1:1 mapping), split at newlines. */
  private text(text: string, s: number, cls = 'o-t'): string {
    let out = '';
    let off = 0;
    for (const line of text.split('\n')) {
      if (off > 0) out += '<br>';
      if (line) {
        const body = this.mark(line);
        out += this.o.clean ? body : `<span class="${cls}" data-s="${s + off}" data-e="${s + off + line.length}">${body}</span>`;
      }
      off += line.length + 1;
    }
    return out;
  }

  private mark(s: string): string {
    if (!this.hl) return escapeHtml(s);
    let out = '';
    let last = 0;
    this.hl.lastIndex = 0;
    for (let m; (m = this.hl.exec(s)); ) {
      if (!m[0]) { this.hl.lastIndex++; continue; }
      out += escapeHtml(s.slice(last, m.index)) + `<mark class="o-match">${escapeHtml(m[0])}</mark>`;
      last = m.index + m[0].length;
    }
    return out + escapeHtml(s.slice(last));
  }

  private token(t: InlineToken): string {
    const o = this.o;
    switch (t.t) {
      case 'text':
        return this.text(t.text, t.start);
      case 'escape':
        return o.clean ? escapeHtml(t.text) : `<span class="o-esc"${this.attrs(t)}>${escapeHtml(t.text)}</span>`;
      case 'bold': case 'italic': case 'strike': case 'highlight': {
        const tag = { bold: 'strong', italic: 'em', strike: 'del', highlight: 'mark' }[t.t];
        const c = { bold: 'o-bold', italic: 'o-italic', strike: 'o-strike', highlight: 'o-highlight' }[t.t];
        return `<${tag}${this.cls(c)}${this.attrs(t)}>${this.tokens(t.children)}</${tag}>`;
      }
      case 'code': {
        let inner = '';
        for (const c of t.children) {
          inner += c.t === 'escape'
            ? (o.clean ? '`' : `<span class="o-esc"${this.attrs(c)}>\`</span>`)
            : this.text((c as TextToken).text, c.start);
        }
        return `<code${this.cls('o-code')}${this.attrs(t)}>${inner}</code>`;
      }
      case 'codeblock': {
        const lang = t.lang ? ` data-lang="${escapeHtml(t.lang)}"` : '';
        const body = o.clean ? escapeHtml(t.text)
          : t.text ? `<span class="o-t" data-s="${t.innerStart}" data-e="${t.innerEnd}">${this.mark(t.text)}</span>` : '';
        const langCls = o.clean && t.lang ? ` class="language-${escapeHtml(t.lang)}"` : '';
        return `<pre${this.cls('o-codeblock')}${lang}${this.attrs(t)}><code${langCls}>${body}</code></pre>`;
      }
      case 'latex':
        if (o.clean) return escapeHtml(`$$${t.text}$$`);
        return `<span class="o-latex" data-tex="${escapeHtml(t.text)}"${this.attrs(t, ' data-atomic data-select-whole')}>${escapeHtml(t.text)}</span>`;
      case 'link': {
        const href = safeUrl(t.url);
        const internal = !!(href && o.isInternal?.(href));
        const title = (!t.text || t.text === t.url) ? o.linkTitle?.(t.url) : undefined;
        const body = title !== undefined ? escapeHtml(title) : t.children.length ? this.tokens(t.children) : 'Link';
        const hrefAttr = href ? ` href="${escapeHtml(href)}"` : '';
        if (o.clean) return `<a${hrefAttr}>${body}</a>`;
        const target = internal || !href ? '' : ' target="_blank" rel="noopener noreferrer"';
        const cls = internal ? 'o-link o-link-internal' : 'o-link';
        return `<a class="${cls}"${hrefAttr}${target}${this.attrs(t, ' data-atomic')}>${body}</a>`;
      }
      case 'image': {
        const src = safeUrl(t.url, { allowDataImage: true });
        const alt = escapeHtml(t.text);
        if (o.clean) return src ? `<img src="${escapeHtml(src)}" alt="${alt}">` : alt;
        if (o.inlineImages && src) {
          return `<img class="o-img" src="${escapeHtml(src)}" alt="${alt}"${this.attrs(t, ' data-atomic')}>`;
        }
        const hrefAttr = src ? ` href="${escapeHtml(src)}" target="_blank" rel="noopener noreferrer" data-src="${escapeHtml(src)}"` : '';
        return `<a class="o-image"${hrefAttr}${this.attrs(t, ' data-atomic')}>${alt || 'Image'}</a>`;
      }
      case 'autolink': {
        const href = safeUrl(t.url);
        if (!href) return this.text(t.url, t.start);
        if (o.clean) return `<a href="${escapeHtml(href)}">${escapeHtml(t.url)}</a>`;
        const internal = !!o.isInternal?.(href);
        const title = o.linkTitle?.(t.url);
        const cls = internal ? 'o-link o-autolink o-link-internal' : 'o-link o-autolink';
        const target = internal ? '' : ' target="_blank" rel="noopener noreferrer"';
        return `<a class="${cls}" href="${escapeHtml(href)}"${target}${this.attrs(t, ' data-atomic')}>${this.mark(title ?? t.url)}</a>`;
      }
      case 'tag':
        if (o.clean) return escapeHtml(t.tag);
        return `<span class="o-tag" data-tag="${escapeHtml(t.tag)}"${this.attrs(t, ' data-atomic')}>${this.mark(t.tag)}</span>`;
      case 'date': {
        if (o.clean) return escapeHtml(`!(${t.text})`);
        const now = o.now ?? Date.now();
        const fo: DateFormatOptions = { now, hour12: o.hour12 !== false, format: o.dateFormat, formatNoTime: o.dateFormatNoTime };
        let cls = 'o-date';
        if (o.markOverdue && isOverdue(t.date, now)) cls += ' o-overdue';
        let title = '';
        if (t.date.recur) {
          cls += ' o-recurring';
          title = ` title="${escapeHtml(describeRecurrence(t.date.recur))}"`;
        }
        return `<span class="${cls}" data-date="${escapeHtml(t.text)}"${title}${this.attrs(t, ' data-atomic')}>${escapeHtml(formatDateLabel(t.date, fo))}</span>`;
      }
    }
  }
}

/** Delimiter string for an emphasis kind (`bold` → `**`). */
export function emphasisDelimiter(kind: EmphasisKind): string {
  return DELIMS[kind];
}
