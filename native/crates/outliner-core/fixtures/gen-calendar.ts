// Records what the TS calendar feed (packages/core/src/calendar.ts) writes, for
// crates/outliner-core/tests/calendar.rs. Run from the repo root:
//
//   node native/crates/outliner-core/fixtures/gen-calendar.ts

import { writeFileSync } from 'node:fs';
import { buildCalendar } from '../../../../packages/core/src/index.ts';
import { int, pick, rand, randomTree, reseed, ser, textOf } from './lib.ts';

reseed(0xca1e);

const pad = (n: number) => String(n).padStart(2, '0');
function endpoint(): string {
  if (rand() < 0.05) return pick(['0050-03-04 10:00 +01:00', '2024-02-29', '2026-12-31', '9999-12-31 23:59 Z', '2026-01-01 00:10 +14:00']);
  let s = `${2020 + int(10)}-${pad(1 + int(12))}-${pad(1 + int(28))}`;
  if (rand() < 0.5) {
    s += ` ${pad(int(24))}:${pad(pick([0, 5, 30, 59]))}`;
    if (rand() < 0.4) s += pick(['Z', ' Z', ' +09:00', ' -05:00', ' +0530', ' -0330', ' +14:00']);
  }
  return s;
}
function date(): string {
  let b = endpoint();
  if (rand() < 0.3) b += ' - ' + endpoint();
  if (rand() < 0.35) b += ' | ' + pick(['', '~']) + pick(['', '2', '10']) + pick(['d', 'w', 'm', 'y']) + (rand() < 0.4 ? pick(['1', '135', '7', '67']) : '');
  return `!(${b})`;
}
const WORDS = ['Pay rent', 'meet; bob', 'a, b', '**bold**', '[link](https://x.org)', '#tag', '`code`', '한글 일정', '😀', '  spaced  ', 'line\nbreak', 'cr\rhere', 'sep x', 'back\\slash'];
function text(): string {
  const parts: string[] = [];
  const n = 1 + int(4);
  for (let i = 0; i < n; i++) parts.push(rand() < 0.4 ? date() : pick(WORDS));
  if (rand() < 0.1) parts.push('x'.repeat(60 + int(100)));
  if (rand() < 0.05) parts.push('😀'.repeat(80));
  return parts.join(' ');
}

const cases = Array.from({ length: 150 }, () => {
  const docs = Array.from({ length: 1 + int(3) }, (_, k) => ({
    id: `doc${k}`, title: pick(['Plans', 'Work; home', '', '한글 문서', 'A, B']),
    tree: randomTree(3 + int(10), { texts: text, notes: () => pick(['', 'a note', 'multi\nline, note', '**b** note; x', textOf(['x', ' ', '\n'], 90)]) }),
  }));
  const name = pick([undefined, 'My cal', 'a;b,c', '']);
  const withUrl = rand() < 0.5;
  const stamp = Date.UTC(2026, int(12), 1 + int(28), int(24), int(60), int(60), int(1000));
  const ics = buildCalendar(docs, { name, stamp, itemUrl: withUrl ? (doc, id) => `https://app.example/#d=${doc}&z=${id}` : undefined });
  return { docs: docs.map((d) => ({ ...d, tree: ser(d.tree) })), name: name ?? null, withUrl, stamp, ics };
});

const out = new URL('./calendar.json', import.meta.url);
writeFileSync(out, JSON.stringify({ cases }));
console.log(`wrote ${cases.length} calendar cases to ${out.pathname}`);
