// Records what the TS search code (packages/core/src/search.ts) does, for crates/outliner-core/tests/search.rs:
// queries over random trees (matches per item, searchTree results, the matcher's flags), operator time values,
// whole-word matching and the fuzzy matcher. Run from the repo root after changing packages/core:
//
//   node native/crates/outliner-core/fixtures/gen-search.ts
//
// Date operators depend on the local time zone: the query cases are recorded in several zones (process.env.TZ)
// and the Rust test replays each zone with the same TZ.
//
// Operator times fall back to Date.parse; the Rust port reads ISO 8601 and the slash forms only (see
// src/search.rs), so the time values here stay within those (V8 reads "1" or "next 3" as dates too).

import { writeFileSync } from 'node:fs';
import {
  compileQuery, forcesShowChecked, fuzzyFilter, fuzzyMatch, parseQueryTime, searchTree, wholeWordMatch, ROOT,
} from '../../../../packages/core/src/index.ts';
import { int, pick, rand, randomTree, reseed, ser, textOf } from './lib.ts';

reseed(0x5eac4);

const pad = (n: number) => String(n).padStart(2, '0');
const ZONES = ['UTC', 'America/New_York', 'Asia/Seoul', 'Australia/Lord_Howe'];
// month ends too, so `1m` / `-1m` clamp the day
const NOWS = [Date.UTC(2026, 9, 22, 2, 0), Date.UTC(2026, 2, 8, 6, 30), Date.UTC(2026, 9, 4, 15, 45), Date.UTC(2024, 1, 29, 12, 0), Date.UTC(2026, 0, 31, 12, 0), Date.UTC(2026, 4, 31, 12, 0)];

function localDate(t: number, time: boolean): string {
  const d = new Date(t);
  let s = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  if (time) s += ` ${pad(d.getHours())}:${pad(pick([0, 15, 30, 59]))}`;
  return s;
}

const WORDS = ['apple', 'Apple', 'pie', 'apple-pie', 'plan', 'Plan', 'meeting', 'is', 'has', 'OR', 'or', 'color', '한글', '글', 'ß', 'İs', 'x_y', 'note', 'due', 'heading:2'];
const TAGS = ['#work', '#Work', '#work-item', '@bob', '@Bob', '#home', '#w'];

function itemText(now: number): string {
  const parts: string[] = [];
  const n = 1 + int(4);
  for (let i = 0; i < n; i++) {
    const r = rand();
    if (r < 0.5) parts.push(pick(WORDS));
    else if (r < 0.75) parts.push(pick(TAGS));
    else {
      const t = now + Math.floor((rand() - 0.5) * 30 * 86400000);
      let body = localDate(t, rand() < 0.4);
      if (rand() < 0.2) body += ` - ${localDate(t + int(5) * 86400000, false)}`;
      if (rand() < 0.15) body += ' | w';
      parts.push(`!(${body})`);
    }
  }
  return parts.join(pick([' ', ' ', ', ', '.']));
}

const TIMES = [
  'now', 'today', 'Today', 'yesterday', 'tomorrow', '3d', '-3d', '+2d', '0d', '1w', '-2w', '1m', '-1m', '-13m', '5h',
  '-12h', '2026-10-22', '2026-10-21', '2026-03-08', '2026-10-22 10:00', '2026-10-22T09:30', '2026/10/22',
  '2026/10/22 10:00', '2026/3/8 23:15:30', '10/22/2026', '2026-10', '2026', '2026-10-22T10:00:30.5Z',
  '2026-10-22T10:00:00+09:00', '2026-10-22T10:00Z', 'abc', '3x', 'x', 'soon', '2026-13-01', '2026/13/01',
];

function opTerm(now: number): string {
  const op = pick(['is', 'has', 'color', 'heading', 'since', 'until', 'within', 'edited', 'created', 'parent', 'ancestor', 'time', 'in', 'IS', 'Has']);
  let v: string;
  switch (op.toLowerCase()) {
    case 'is': v = pick(['completed', 'checked', 'checklist', 'heading', 'c', 'che', 'h', 'x', '']); break;
    case 'has': v = pick(['checkbox', 'note', 'color', 'children', 'date', 'c', 'ch', 'n', 'x']); break;
    case 'color': v = pick(['red', 'Blue', 'none', 'r', 'p', 'o', 'pink', '']); break;
    case 'heading': v = pick(['0', '1', '2', '3', '4', 'x']); break;
    case 'parent': case 'ancestor': v = pick(['apple', 'title', 'doc', 'plan', '', '한']); break;
    case 'in': v = pick(['title', 'content', 'note', 'notes', 'all', 'any', 'both', 'x']); break;
    case 'time': v = pick(['', 'x']); break;
    default: v = pick(TIMES);
  }
  if (v.includes(' ') || rand() < 0.1) v = `"${v}"`;
  return `${op}:${rand() < 0.15 ? ' ' : ''}${v}`;
}

function query(now: number): string {
  const parts: string[] = [];
  const n = int(5);
  for (let i = 0; i < n; i++) {
    const r = rand();
    let t: string;
    if (r < 0.35) t = pick(WORDS);
    else if (r < 0.5) t = pick(TAGS);
    else if (r < 0.6) t = `"${pick(WORDS)}${rand() < 0.3 ? ' ' + pick(WORDS) : ''}"`;
    else if (r < 0.65) t = 'OR';
    else if (r < 0.7) t = pick(['"unclosed', '-', '"', 'a"b']);
    else t = opTerm(now);
    if (rand() < 0.15) t = '-' + t;
    parts.push(t);
  }
  // a scope switch before a plain word, so the scopes split items
  if (rand() < 0.25) parts.unshift(`in:${pick(['title', 'content', 'note', 'notes', 'all', 'both'])}`, pick(WORDS));
  return parts.join(pick([' ', ' ', '  ', '\t'])) + (rand() < 0.3 ? ' ' : '');
}

function zoneCases(count: number) {
  const out: unknown[] = [];
  for (let k = 0; k < count; k++) {
    const now = pick(NOWS) + (rand() < 0.5 ? 0 : Math.floor((rand() - 0.5) * 6 * 86400000));
    const tree = randomTree(4 + int(14), { texts: () => itemText(now), notes: () => pick(['', ' ', 'apple note', '#work\nmore', itemText(now)]) });
    // edits around now so edited:/created: thresholds split items
    for (const n of tree.nodes.values()) {
      n.created = now - int(40) * 86400000 - int(86400000);
      n.edited = n.created + int(30) * 86400000;
    }
    const ids = [...tree.nodes.keys(), 'missing'];
    const queries = Array.from({ length: 6 }, () => query(now));
    const cases = queries.map((q) => {
      const typing = rand() < 0.4;
      const m = compileQuery(q, now, { typing });
      const showChecked = pick([undefined, true, false]);
      const withKids = [...tree.nodes.values()].filter((n) => n.children.length && tree.isAttached(n.id));
      const scope = rand() < 0.7 || !withKids.length ? ROOT : pick(withKids).id;
      const res = searchTree(tree, m, scope, showChecked === undefined ? {} : { showChecked });
      return {
        q, typing, terms: m.terms, forces: m.forcesShowChecked, empty: m.empty, forcesOnly: forcesShowChecked(q),
        hits: ids.filter((id) => m(tree, id)),
        scope, showChecked: showChecked ?? null, matches: res.matches, visible: [...res.visible].sort(),
      };
    });
    const times = TIMES.map((x) => [x, parseQueryTime(x, now)]);
    out.push({ now, tree: ser(tree), cases, times });
  }
  return out;
}

const zones: Record<string, unknown[]> = {};
for (const z of ZONES) {
  process.env.TZ = z;
  zones[z] = zoneCases(60);
}

// ---- whole words and fuzzy ------------------------------------------------------------

const PIECES = ['a', 'b', 'A', 'B', ' ', '-', '_', '.', ',', '#', '@', 'ab', 'ß', 'SS', 'İ', 'i', 'é', 'É', '한', '😀', '1', '٣', 'Σ', 'ς', 'σ', 'ǅ', '́', 'ª'];
const words = Array.from({ length: 3000 }, () => {
  const hay = textOf(PIECES, 10);
  const needle = rand() < 0.5 && hay ? hay.slice(int(hay.length), int(hay.length + 1) + 3).replace(/[\ud800-\udbff]$/, '').replace(/^[\udc00-\udfff]/, '') : textOf(PIECES, 3);
  const cs = rand() < 0.5;
  const prefix = rand() < 0.3;
  return { hay, needle, cs, prefix, hit: wholeWordMatch(hay, needle, cs, prefix) };
});

const TARGETS = [
  'Project Plan', 'projectPlan.md', 'my-notes', 'Inbox', '한글 문서', 'İstanbul trip', 'straße', 'readme', 'README',
  'Weekly Review 2026', '😀 emoji doc', 'a_b_c', 'CamelCaseThing', 'xyz', '', 'plan', 'Planning the plan', 'ǅemal', 'ªb',
  'notes/2026/plan', 'The Quick Brown Fox',
];
const QPIECES = ['p', 'pl', 'plan', 'Plan', ' ', 'proj', 'x', 'c', 'ct', 'n', 'r', 'e', '한', '글', 'i', 'İ', 'ß', 'ss', '😀', 'th', 'q b', '2026', '/', 'z'];
const fuzzy = Array.from({ length: 1500 }, () => {
  const q = textOf(QPIECES, 3);
  const target = rand() < 0.2 ? textOf([...QPIECES, 'A', 'B', '-'], 6) : pick(TARGETS);
  return { q, target, m: fuzzyMatch(q, target) };
});
const filters = Array.from({ length: 200 }, () => {
  const q = textOf(QPIECES, 2);
  const items = TARGETS.filter(() => rand() < 0.6);
  return { q, items, out: fuzzyFilter(q, items, (x) => x).map((r) => [r.item, r.match]) };
});

const out = new URL('./search.json', import.meta.url);
writeFileSync(out, JSON.stringify({ zones, words, fuzzy, filters }));
console.log(`wrote search cases (${ZONES.length} zones, ${words.length} word, ${fuzzy.length} fuzzy, ${filters.length} filter cases) to ${out.pathname}`);
