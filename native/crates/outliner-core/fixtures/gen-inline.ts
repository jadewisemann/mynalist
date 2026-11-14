// Records what the TS inline parser (packages/core/src/inline.ts) and date helpers (dates.ts) do, for
// crates/outliner-core/tests/inline.rs. Run from the repo root after changing packages/core:
//
//   node native/crates/outliner-core/fixtures/gen-inline.ts
//
// Inputs: every string literal of the web's own tests (inline.test.ts, dates.test.ts) plus seeded random markup.
// Date math depends on the local time zone, so the date cases are recorded in several zones (process.env.TZ);
// the Rust test replays each zone with the same TZ.

import { readFileSync, writeFileSync } from 'node:fs';
import {
  addDays, advanceRecurrence, currentDateToken, dateRange, dayDiff, describeRecurrence, findDates, formatDateBody,
  formatDateLabel, formatPattern, fromNow, isOverdue, localTz, nextOccurrence, parseDateBody, relativeDayLabel,
  startOfDay, toDate, withLocalTz, type DateSpec,
} from '../../../../packages/core/src/dates.ts';
import {
  escapeInline, extractDates, extractLinks, extractTags, inlineToPlain, matchRanges, normalizeUrl, parseInline,
  replaceTag, safeUrl, tokenPathAt, unescape,
} from '../../../../packages/core/src/inline.ts';

let seed = 0x1b873593;
function rand(): number {
  seed |= 0;
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
const int = (n: number) => Math.floor(rand() * n);
const pick = <T>(xs: readonly T[]): T => xs[int(xs.length)]!;

// ---- the corpus: string literals of the web's tests ------------------------------------------------

function literals(file: string): string[] {
  const src = readFileSync(new URL(`../../../../packages/core/test/${file}`, import.meta.url), 'utf8');
  const out: string[] = [];
  for (const m of src.matchAll(/'((?:[^'\\\n]|\\.)*)'/g)) {
    try {
      out.push(JSON.parse('"' + m[1]!.replace(/\\'/g, "'").replace(/"/g, '\\"') + '"'));
    } catch {
      // a literal JSON can't read (\x.. and the like): not needed
    }
  }
  return [...new Set(out)];
}
const corpus = [...literals('inline.test.ts'), ...literals('dates.test.ts')];

// ---- random markup -----------------------------------------------------------------------------------

/** Pieces that make every construct, open and closed, nested and broken. */
const PIECES = [
  'a', 'b', 'text', ' ', ' ', '  ', '\n', '한글', '😀', 'x y', '**', '__', '~~', '==', '`', '``', '```', '```js\n',
  '$$', '\\', '\\*', '\\`', '\\[', '[', ']', '(', ')', '](', '![', '!(', '#', '@', '#tag', '@me', '#12', '#a.b',
  'tag:', 'http://', 'https://x.org/a', 'mailto:', 'ftp://h', ':', '.', ',', '?', '!', ';', '/', "'", '{', '}',
  '!(2026-10-22)', '!(2026-10-22 10:00 - 2026-10-23 | ~2w135)', '!(bad)', '!(2026-02-30)', '[t](u)', '[](x)',
  '![alt](img.png)', '`c`', '$$x^2$$', 'javascript:alert(1)', 'data:image/png;base64,AA', '\t', '\u00a0', '\ufeff', '\r', '\u2028', '\u0085',
];

function markup(): string {
  let s = '';
  const n = 1 + int(14);
  for (let i = 0; i < n; i++) s += pick(PIECES);
  return s;
}

// ---- dates -----------------------------------------------------------------------------------------------

const pad = (n: number, w = 2) => String(n).padStart(w, '0');
const ZONES = ['UTC', 'America/New_York', 'Asia/Seoul', 'America/Santiago', 'Australia/Lord_Howe'];
/** Instants near DST changes (New York, Santiago, Lord Howe) and plain days. */
const NOWS = [
  Date.UTC(2026, 9, 22, 2, 0), Date.UTC(2026, 2, 8, 6, 30), Date.UTC(2026, 10, 1, 5, 30), Date.UTC(2026, 8, 6, 3, 30),
  Date.UTC(2026, 3, 5, 2, 30), Date.UTC(2026, 9, 3, 15, 15), Date.UTC(2024, 1, 29, 12, 0), Date.UTC(1999, 11, 31, 23, 59),
];

function randomNow(): number {
  const base = pick(NOWS);
  return rand() < 0.4 ? base : base + Math.floor((rand() - 0.5) * 40 * 86400000);
}

function endpoint(around: number): string {
  if (rand() < 0.05) return pick(['0050-03-04', '2026-02-29', '2024-02-29', '2026-13-01', '9999-12-31', '2026-1-01', 'x']);
  const d = new Date(around + Math.floor((rand() - 0.5) * (rand() < 0.7 ? 20 : 900) * 86400000));
  let s = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  if (rand() < 0.5) {
    const h = rand() < 0.3 ? pick([0, 1, 2, 3, 23, 24]) : int(24);
    s += `${pick([' ', 'T'])}${rand() < 0.2 ? h : pad(h)}:${pad(pick([0, 0, 5, 30, 59, 60]))}`;
    if (rand() < 0.25) s += pick(['Z', ' Z', '+09:00', ' -05:00', '+0530', ' +14:00', '+9:00']);
  }
  return s;
}

function dateBody(now: number): string {
  let s = endpoint(now);
  if (rand() < 0.3) s += pick([' - ', ' -  ', '\t-\t', '-', ' – ']) + endpoint(now);
  if (rand() < 0.4) {
    s += pick([' | ', '|', ' |']) + pick(['~', '', '']) + pick(['', '1', '2', '3', '10', '0', '007']) + pick(['d', 'w', 'm', 'y', 'q'])
      + (rand() < 0.4 ? pick(['1', '135', '7', '51', '8', '67']) : '');
  }
  if (rand() < 0.1) s = pick([' ', '  ']) + s + pick([' ', ' ']);
  return s;
}

const PATTERNS = [
  'YYYY-MM-DD', 'YY M D', 'MMMM Do, dddd', 'MMM ddd', 'HH:mm', 'H:mm A', 'hh h a', '[Week of] MMM D', '[unclosed',
  'relative', '', 'D/M/YYYY HH:mm',
];

function labelOptions(now: number) {
  return {
    now, hour12: rand() < 0.7,
    format: rand() < 0.6 ? undefined : pick(PATTERNS),
    formatNoTime: rand() < 0.6 ? undefined : pick(PATTERNS),
  };
}

function zoneCases(n: number) {
  const out: unknown[] = [];
  for (let i = 0; i < n; i++) {
    const now = randomNow();
    const body = i < corpus.length ? corpus[i]!.replace(/^!\((.*)\)$/s, '$1') : dateBody(now);
    const spec = parseDateBody(body);
    const c: Record<string, unknown> = { now, body, spec };
    if (spec) {
      const o = labelOptions(now);
      Object.assign(c, {
        canonical: formatDateBody(spec),
        start: toDate(spec.start).getTime(),
        range: dateRange(spec),
        overdue: isOverdue(spec, now),
        opts: o,
        label: formatDateLabel(spec, o),
        relDay: relativeDayLabel(spec.start, now),
        recur: spec.recur ? describeRecurrence(spec.recur) : null,
        next: nextOccurrence(spec, now),
        localized: formatDateBody(withLocalTz(spec)),
      });
    }
    const text = `${pick(['', 'pay ', '`!(2026-01-01|d)` ', '\\!(2026-01-01|d) '])}!(${body})${pick(['', ' x', ' !(2026-05-05 | w)'])}`;
    const adv = advanceRecurrence(text, now);
    const k = int(30) - 10;
    const delta = Math.floor((rand() - 0.5) * pick([60e3, 3600e3, 86400e3, 40 * 86400e3, 800 * 86400e3]));
    Object.assign(c, {
      text, found: findDates(text), advanced: adv,
      currentDay: currentDateToken(now), currentTime: currentDateToken(now, true),
      sod: startOfDay(now), plusDays: [k, addDays(now, k)], diff: dayDiff(now, now + delta), fromNow: [delta, fromNow(delta)],
      tz: localTz(new Date(now)), pattern: [PATTERNS[i % PATTERNS.length], formatPattern(new Date(now + delta), PATTERNS[i % PATTERNS.length]!)],
    });
    out.push(c);
  }
  return out;
}

// ---- inline cases ------------------------------------------------------------------------------------

const linkTitle = (url: string) => (url.length % 2 ? `Title of ${url}` : undefined);
const TERMS = ['a', 'TAG', 'x', '한', 'http', '*', 'ß', 'İ'];

function inlineCase(src: string) {
  const tags = extractTags(src);
  const from = tags.length && rand() < 0.8 ? pick(tags).toUpperCase() : '#tag';
  const units = src.length;
  const at = int(units + 1);
  const terms = [pick(TERMS), pick(TERMS)].slice(0, 1 + int(2));
  return {
    src,
    tokens: parseInline(src),
    plain: inlineToPlain(src),
    plainTitled: inlineToPlain(src, linkTitle),
    tags,
    links: extractLinks(src),
    dates: extractDates(src),
    unescaped: unescape(src),
    escaped: escapeInline(src),
    normalized: normalizeUrl(src),
    safe: safeUrl(src),
    safeImage: safeUrl(src, { allowDataImage: true }),
    rename: [from, replaceTag(src, from, '#new'), replaceTag(src, from, null)],
    path: [at, tokenPathAt(parseInline(src), at).map((t) => [t.start, t.end])],
    matches: [terms, matchRanges(src, terms)],
  };
}

const inline = [...corpus, ...Array.from({ length: 4000 }, markup)].map(inlineCase);

const zones: Record<string, unknown[]> = {};
for (const z of ZONES) {
  process.env.TZ = z;
  zones[z] = zoneCases(corpus.length + 600);
}

const out = new URL('./inline.json', import.meta.url);
writeFileSync(out, JSON.stringify({ inline, zones }));
console.log(`wrote ${inline.length} inline cases and ${ZONES.length} zones of date cases to ${out.pathname}`);
