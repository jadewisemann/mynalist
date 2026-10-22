// Search query language (in-document filter, flat and global search) and a
// fuzzy matcher for the file/item finders and the `[[` link picker.

import { ROOT, type NodeId, type TNode, type Tree } from './tree.ts';
import { COLOR_NAMES } from './item.ts';
import { extractDates } from './inline.ts';
import { addDays, dateRange, parseDateTime, startOfDay, toDate, type TimeInput } from './dates.ts';

export interface QueryOptions {
  /** the user is still typing: the last term (if not followed by a space) is a prefix match */
  typing?: boolean;
}

/** A compiled query: call it as a predicate, read flags off it. */
export type Matcher = ((tree: Tree, id: NodeId) => boolean) & {
  /** query contains a positive `is:completed`: checked items must be shown */
  forcesShowChecked: boolean;
  /** positive text terms, for highlighting matches in rendered text */
  terms: string[];
  /** no effective terms: everything matches */
  empty: boolean;
};

type Pred = (tree: Tree, n: TNode) => boolean;
type Scope = 'all' | 'title' | 'note';

interface RawTerm {
  neg: boolean;
  /** term text with quotes removed */
  v: string;
  /** the whole term was a quoted phrase */
  quoted: boolean;
  /** operator key (lowercase, without colon) if `key:value` */
  op?: string;
}

const OPERATORS = new Set([
  'is', 'has', 'color', 'heading', 'since', 'until', 'within', 'edited', 'created', 'parent', 'ancestor', 'time', 'in',
]);

// ---- tokenizer -----------------------------------------------------------------

function tokenize(q: string): RawTerm[] {
  const out: RawTerm[] = [];
  let i = 0;
  const n = q.length;
  const readQuoted = (): string => {
    // at opening quote
    const close = q.indexOf('"', i + 1);
    const v = close < 0 ? q.slice(i + 1) : q.slice(i + 1, close);
    i = close < 0 ? n : close + 1;
    return v;
  };
  const readWord = (): string => {
    const s = i;
    while (i < n && !/\s/.test(q[i]!)) i++;
    return q.slice(s, i);
  };
  const skipWs = () => {
    while (i < n && /\s/.test(q[i]!)) i++;
  };
  while (true) {
    skipWs();
    if (i >= n) break;
    let neg = false;
    if (q[i] === '-' && i + 1 < n && !/\s/.test(q[i + 1]!)) {
      neg = true;
      i++;
    }
    if (q[i] === '"') {
      out.push({ neg, v: readQuoted(), quoted: true });
      continue;
    }
    const m = /^([a-zA-Z]+):/.exec(q.slice(i));
    if (m && OPERATORS.has(m[1]!.toLowerCase())) {
      const op = m[1]!.toLowerCase();
      i += m[0].length;
      // "op: value" joins with the next word
      if (i >= n || /\s/.test(q[i]!)) skipWs();
      let v = '';
      if (q[i] === '"') v = readQuoted();
      else if (i < n) v = readWord();
      out.push({ neg, v, quoted: false, op });
      continue;
    }
    out.push({ neg, v: readWord(), quoted: false });
  }
  return out;
}

// ---- time values -----------------------------------------------------------------

export interface TimeRange {
  /** inclusive */
  start: number;
  /** exclusive */
  end: number;
  /** relative amount (`4d`, `-2w`, `3h`) */
  rel?: { n: number; unit: 'd' | 'w' | 'm' | 'h' };
}

function addMonthsMs(t: number, n: number): number {
  const d = new Date(t);
  const target = new Date(d.getFullYear(), d.getMonth() + n, 1);
  const dim = new Date(target.getFullYear(), target.getMonth() + 1, 0).getDate();
  return new Date(target.getFullYear(), target.getMonth(), Math.min(d.getDate(), dim)).getTime();
}

/**
 * Parse an operator time value: now, today, yesterday, tomorrow, `<n>d|w|m|h`
 * (negative = past; days/weeks/months snap to local midnight and cover that
 * whole day), `YYYY-MM-DD` (whole day), or any parseable date/time (instant).
 */
export function parseQueryTime(x: string, now: TimeInput): TimeRange | null {
  const t = typeof now === 'number' ? now : now.getTime();
  const today = startOfDay(t);
  const day = (d: number): TimeRange => ({ start: d, end: addDays(d, 1) });
  const v = x.trim().toLowerCase();
  if (!v) return null;
  if (v === 'now') return { start: t, end: t + 1 };
  if (v === 'today') return day(today);
  if (v === 'yesterday') return day(addDays(today, -1));
  if (v === 'tomorrow') return day(addDays(today, 1));
  const rel = /^([+-]?\d+)([dwmh])$/.exec(v);
  if (rel) {
    const n = +rel[1]!;
    const unit = rel[2] as 'd' | 'w' | 'm' | 'h';
    if (unit === 'h') {
      const at = t + n * 3600000;
      return { start: at, end: at + 1, rel: { n, unit } };
    }
    const d = unit === 'd' ? addDays(today, n) : unit === 'w' ? addDays(today, 7 * n) : addMonthsMs(today, n);
    return { ...day(d), rel: { n, unit } };
  }
  const dt = parseDateTime(x.trim());
  if (dt) {
    const s = toDate(dt).getTime();
    return dt.hour === undefined ? day(s) : { start: s, end: s + 1 };
  }
  if (/\d/.test(v)) {
    const p = Date.parse(x);
    if (!Number.isNaN(p)) return { start: p, end: p + 1 };
  }
  return null;
}

/** Threshold for edited:/created: ("within the past X"). */
function pastThreshold(r: TimeRange, now: number): number {
  if (r.rel) {
    const n = -Math.abs(r.rel.n);
    const today = startOfDay(now);
    switch (r.rel.unit) {
      case 'h': return now + n * 3600000;
      case 'd': return addDays(today, n);
      case 'w': return addDays(today, 7 * n);
      case 'm': return addMonthsMs(today, n);
    }
  }
  return r.start;
}

// ---- matching helpers -----------------------------------------------------------

const WORD_CHAR = /[\p{L}\p{N}_-]/u;
const isWordChar = (c: string | undefined) => c !== undefined && WORD_CHAR.test(c);

/** Whole-word occurrence of `needle` in `hay` (boundaries = whitespace/punctuation). */
export function wholeWordMatch(hay: string, needle: string, caseSensitive: boolean, prefix = false): boolean {
  if (!needle) return true;
  const h = caseSensitive ? hay : hay.toLowerCase();
  const nd = caseSensitive ? needle : needle.toLowerCase();
  for (let i = h.indexOf(nd); i >= 0; i = h.indexOf(nd, i + 1)) {
    const before = i === 0 || !isWordChar(hay[i - 1]);
    const j = i + nd.length;
    const after = prefix || j >= hay.length || !isWordChar(hay[j]);
    if (before && after) return true;
  }
  return false;
}

const contentOf = (n: TNode | undefined) => (n?.props.c as string | undefined) ?? '';
const noteOf = (n: TNode | undefined) => (n?.props.n as string | undefined) ?? '';

function fields(n: TNode, scope: Scope): string[] {
  if (scope === 'title') return [contentOf(n)];
  if (scope === 'note') return [noteOf(n)];
  return [contentOf(n), noteOf(n)];
}

function itemDates(n: TNode): { start: number; end: number }[] {
  const out: { start: number; end: number }[] = [];
  for (const src of [contentOf(n), noteOf(n)]) {
    if (!src.includes('!(')) continue;
    for (const d of extractDates(src)) out.push(dateRange(d.spec));
  }
  return out;
}

/** Last instant covered by a date range (all-day ranges are exclusive at the end). */
const lastInstant = (d: { start: number; end: number }) => (d.end > d.start ? d.end - 1 : d.end);

function pickValues(v: string, values: readonly string[], prefix: boolean): string[] | null {
  const lv = v.toLowerCase();
  if (values.includes(lv)) return [lv];
  if (prefix && lv) {
    const c = values.filter((x) => x.startsWith(lv));
    if (c.length) return c;
  }
  return null;
}

// ---- compiler --------------------------------------------------------------------

const IS_VALUES = ['completed', 'checked', 'checklist', 'heading'] as const;
const HAS_VALUES = ['checkbox', 'note', 'color', 'children', 'date'] as const;

function isCompleted(tree: Tree, n: TNode): boolean {
  let cur: TNode | undefined = n;
  while (cur) {
    if (cur.props.ck) return true;
    cur = cur.parent ? tree.get(cur.parent) : undefined;
  }
  return false;
}

const isPreds: Record<string, Pred> = {
  completed: isCompleted,
  checked: isCompleted,
  checklist: (_t, n) => !!n.props.cb,
  heading: (_t, n) => !!n.props.h,
};

const hasPreds: Record<string, Pred> = {
  checkbox: (_t, n) => !!n.props.cb,
  note: (_t, n) => noteOf(n).trim().length > 0,
  color: (_t, n) => !!n.props.cl,
  children: (_t, n) => n.children.length > 0,
  date: (_t, n) => itemDates(n).length > 0,
};

const anyOf = (preds: Pred[]): Pred => (t, n) => preds.some((p) => p(t, n));

interface Compiled {
  pred: Pred | null;
  /** term text to highlight */
  hl?: string;
  forcesChecked?: boolean;
}

function compileTerm(term: RawTerm, scope: Scope, prefix: boolean, now: number): Compiled {
  const op = term.op;
  if (!op) {
    const v = term.v;
    if (!v) return { pred: null };
    if (term.quoted) {
      return { pred: (_t, n) => fields(n, scope).some((f) => wholeWordMatch(f, v, true, prefix)), hl: v };
    }
    if (v[0] === '#' || v[0] === '@') {
      return { pred: (_t, n) => fields(n, scope).some((f) => wholeWordMatch(f, v, false, prefix)), hl: v };
    }
    const lv = v.toLowerCase();
    return { pred: (_t, n) => fields(n, scope).some((f) => f.toLowerCase().includes(lv)), hl: v };
  }

  const v = term.v.trim();
  const asText = (): Compiled => {
    if (prefix) return { pred: null }; // incomplete operator while typing: ignore
    const lv = `${op}:${term.v}`.toLowerCase();
    return { pred: (_t, n) => fields(n, scope).some((f) => f.toLowerCase().includes(lv)), hl: `${op}:${term.v}` };
  };

  switch (op) {
    case 'is': {
      const vals = pickValues(v, IS_VALUES, prefix);
      if (!vals) return asText();
      return {
        pred: anyOf(vals.map((x) => isPreds[x]!)),
        forcesChecked: vals.length === 1 && (vals[0] === 'completed' || vals[0] === 'checked'),
      };
    }
    case 'has': {
      const vals = pickValues(v, HAS_VALUES, prefix);
      if (!vals) return asText();
      return { pred: anyOf(vals.map((x) => hasPreds[x]!)) };
    }
    case 'time':
      return { pred: hasPreds.date! };
    case 'color': {
      const vals = pickValues(v, COLOR_NAMES, prefix);
      if (!vals) return asText();
      const nums = vals.map((x) => COLOR_NAMES.indexOf(x as (typeof COLOR_NAMES)[number]));
      return { pred: (_t, n) => nums.includes((n.props.cl as number | undefined) ?? 0) };
    }
    case 'heading': {
      if (!/^[0-3]$/.test(v)) return asText();
      const h = +v;
      return { pred: (_t, n) => ((n.props.h as number | undefined) ?? 0) === h };
    }
    case 'parent':
    case 'ancestor': {
      if (!v) return asText();
      const lv = v.toLowerCase();
      if (op === 'parent') {
        return { pred: (t, n) => !!n.parent && contentOf(t.get(n.parent)).toLowerCase().includes(lv) };
      }
      return {
        pred: (t, n) => {
          for (let p = n.parent ? t.get(n.parent) : undefined; p; p = p.parent ? t.get(p.parent) : undefined) {
            if (contentOf(p).toLowerCase().includes(lv)) return true;
          }
          return false;
        },
      };
    }
    case 'since':
    case 'until':
    case 'within':
    case 'edited':
    case 'created': {
      const r = parseQueryTime(v, now);
      if (!r) return asText();
      if (op === 'edited' || op === 'created') {
        const th = pastThreshold(r, now);
        return { pred: (_t, n) => (op === 'edited' ? n.edited : n.created) >= th };
      }
      if (op === 'since') return { pred: (_t, n) => itemDates(n).some((d) => lastInstant(d) >= r.start) };
      if (op === 'until') return { pred: (_t, n) => itemDates(n).some((d) => d.start < r.end) };
      // within
      let w: { start: number; end: number };
      if (v.toLowerCase() === 'today') w = { start: r.start, end: r.end };
      else if (r.end > now) w = { start: now, end: r.end };
      else w = { start: r.start, end: addDays(startOfDay(now), 1) };
      return { pred: (_t, n) => itemDates(n).some((d) => d.start < w.end && lastInstant(d) >= w.start) };
    }
  }
  return asText();
}

/**
 * Compile a search query. Terms are ANDed; a bare `OR` starts a new
 * alternative group; `-term` negates; `"phrase"` is a case-sensitive
 * whole-word match; `#tag`/`@tag` match whole words; `in:title` / `in:note`
 * restrict the following terms. See docs/03 §5.2 for operators.
 */
export function compileQuery(q: string, now: TimeInput, opts: QueryOptions = {}): Matcher {
  const t = typeof now === 'number' ? now : now.getTime();
  const raw = tokenize(q);
  const lastIsPrefix = !!opts.typing && q.length > 0 && !/\s$/.test(q);
  const groups: Pred[][] = [[]];
  const terms: string[] = [];
  let forces = false;
  let scope: Scope = 'all';
  raw.forEach((term, idx) => {
    if (!term.op && !term.quoted && !term.neg && term.v === 'OR') {
      groups.push([]);
      return;
    }
    if (term.op === 'in') {
      const v = term.v.toLowerCase();
      if (v === 'title' || v === 'content') scope = 'title';
      else if (v === 'note' || v === 'notes') scope = 'note';
      else if (v === 'all' || v === 'any' || v === 'both') scope = 'all';
      return;
    }
    const c = compileTerm(term, scope, lastIsPrefix && idx === raw.length - 1, t);
    if (!c.pred) return;
    const p = c.pred;
    groups[groups.length - 1]!.push(term.neg ? (tr, n) => !p(tr, n) : p);
    if (!term.neg) {
      if (c.hl) terms.push(c.hl);
      if (c.forcesChecked) forces = true;
    }
  });
  const live = groups.filter((g) => g.length > 0);
  const fn = ((tree: Tree, id: NodeId) => {
    if (!live.length) return true;
    const n = tree.get(id);
    if (!n) return false;
    return live.some((g) => g.every((p) => p(tree, n)));
  }) as Matcher;
  fn.forcesShowChecked = forces;
  fn.terms = terms;
  fn.empty = live.length === 0;
  return fn;
}

/** True if the query contains a positive `is:completed`. */
export function forcesShowChecked(q: string): boolean {
  return compileQuery(q, 0).forcesShowChecked;
}

export interface SearchResult {
  /** matching items in document order */
  matches: NodeId[];
  /** matches plus their ancestors below `scope` (for View.filter) */
  visible: Set<NodeId>;
}

/**
 * Run a matcher over the subtree under `scope` (exclusive). With
 * `showChecked` false (and the query not forcing it), checked items and their
 * descendants are skipped.
 */
export function searchTree(tree: Tree, matcher: Matcher, scope: NodeId = ROOT, opts: { showChecked?: boolean } = {}): SearchResult {
  const matches: NodeId[] = [];
  const visible = new Set<NodeId>();
  const hideChecked = opts.showChecked === false && !matcher.forcesShowChecked;
  const walk = (id: NodeId, chain: NodeId[]) => {
    const n = tree.get(id);
    if (!n) return;
    for (const c of n.children) {
      const cn = tree.get(c);
      if (!cn || (hideChecked && cn.props.ck)) continue;
      if (matcher(tree, c)) {
        matches.push(c);
        visible.add(c);
        for (const a of chain) visible.add(a);
      }
      chain.push(c);
      walk(c, chain);
      chain.pop();
    }
  };
  walk(scope, []);
  return { matches, visible };
}

// ---- fuzzy matching ----------------------------------------------------------------

export interface FuzzyMatch {
  /** higher is better (always <= 0) */
  score: number;
  /** matched [start, end) ranges in the target, merged and sorted */
  ranges: [number, number][];
}

function isBoundary(t: string, p: number): boolean {
  if (p === 0) return true;
  const prev = t[p - 1]!;
  const cur = t[p]!;
  if (!isWordChar(prev)) return true;
  return /\p{Ll}/u.test(prev) && /\p{Lu}/u.test(cur);
}

function scoreRanges(target: string, ranges: [number, number][], charMode: boolean): FuzzyMatch {
  const merged: [number, number][] = [];
  for (const r of ranges) {
    const last = merged[merged.length - 1];
    if (last && last[1] === r[0]) last[1] = r[1];
    else merged.push([r[0], r[1]]);
  }
  let misses = 0;
  let matched = 0;
  for (const [s, e] of merged) {
    if (!isBoundary(target, s)) misses++;
    matched += e - s;
  }
  const first = merged[0]?.[0] ?? 0;
  const lastEnd = merged[merged.length - 1]?.[1] ?? 0;
  const spread = lastEnd - first - matched;
  const penalty = merged.length * 10 + misses * 6 + spread * 1 + first * 0.5 + target.length * 0.1 + (charMode ? 25 : 0);
  return { score: -penalty, ranges: merged };
}

function tokenMatch(t: string, tokens: string[], preferBoundary: boolean, target: string): [number, number][] | null {
  const out: [number, number][] = [];
  let pos = 0;
  for (const tok of tokens) {
    let at = t.indexOf(tok, pos);
    if (at < 0) return null;
    if (preferBoundary) {
      for (let j = at; j >= 0; j = t.indexOf(tok, j + 1)) {
        if (isBoundary(target, j)) { at = j; break; }
      }
    }
    out.push([at, at + tok.length]);
    pos = at + tok.length;
  }
  return out;
}

/**
 * Fuzzy match for finders: first tries the query's words in order as
 * substrings, then falls back to matching individual characters in order.
 * Case-insensitive. Null if no match.
 */
export function fuzzyMatch(query: string, target: string): FuzzyMatch | null {
  const tokens = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!tokens.length) return { score: 0, ranges: [] };
  const t = target.toLowerCase();
  if (t.length !== target.length) return null;
  let best: FuzzyMatch | null = null;
  const consider = (r: [number, number][] | null, charMode: boolean) => {
    if (!r) return;
    const m = scoreRanges(target, r, charMode);
    if (!best || m.score > best.score) best = m;
  };
  consider(tokenMatch(t, tokens, true, target), false);
  consider(tokenMatch(t, tokens, false, target), false);
  if (best) return best;
  const chars = tokens.join('');
  for (let s = t.indexOf(chars[0]!); s >= 0; s = t.indexOf(chars[0]!, s + 1)) {
    const r: [number, number][] = [[s, s + 1]];
    let pos = s + 1;
    let ok = true;
    for (let k = 1; k < chars.length; k++) {
      const at = t.indexOf(chars[k]!, pos);
      if (at < 0) { ok = false; break; }
      r.push([at, at + 1]);
      pos = at + 1;
    }
    if (!ok) break;
    consider(r, true);
  }
  return best;
}

/** Fuzzy score (higher is better); -Infinity when there is no match. */
export function fuzzyScore(query: string, target: string): number {
  return fuzzyMatch(query, target)?.score ?? -Infinity;
}

/** Sort candidates by fuzzy score (best first), dropping non-matches. Stable for ties. */
export function fuzzyFilter<T>(query: string, items: T[], key: (item: T) => string): { item: T; match: FuzzyMatch }[] {
  const out: { item: T; match: FuzzyMatch; i: number }[] = [];
  items.forEach((item, i) => {
    const match = fuzzyMatch(query, key(item));
    if (match) out.push({ item, match, i });
  });
  out.sort((a, b) => b.match.score - a.match.score || a.i - b.i);
  return out.map(({ item, match }) => ({ item, match }));
}
