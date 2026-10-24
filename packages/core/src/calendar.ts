// iCalendar (RFC 5545) feed of every dated item: a read-only calendar subscription (in place of a
// calendar integration). Times without an offset are written as floating local times, like the app shows them.

import { extractDates, inlineToPlain } from './inline.ts';
import { ROOT, type NodeId, type Tree } from './tree.ts';
import type { DateSpec, DateTime } from './dates.ts';

const p2 = (n: number) => String(n).padStart(2, '0');

function icsDate(dt: DateTime): string {
  if (dt.hour === undefined) return `;VALUE=DATE:${dt.year}${p2(dt.month)}${p2(dt.day)}`;
  const local = `${dt.year}${p2(dt.month)}${p2(dt.day)}T${p2(dt.hour)}${p2(dt.minute ?? 0)}00`;
  if (!dt.tz) return `:${local}`;
  // explicit offset: convert to UTC
  const m = /^([+-])(\d{2}):?(\d{2})$/.exec(dt.tz);
  const off = dt.tz === 'Z' || !m ? 0 : (m[1] === '-' ? -1 : 1) * (Number(m[2]) * 60 + Number(m[3]));
  const d = new Date(Date.UTC(dt.year, dt.month - 1, dt.day, dt.hour, (dt.minute ?? 0) - off));
  return `:${d.getUTCFullYear()}${p2(d.getUTCMonth() + 1)}${p2(d.getUTCDate())}T${p2(d.getUTCHours())}${p2(d.getUTCMinutes())}00Z`;
}

/** The day after (all-day DTEND is exclusive). */
function nextDay(dt: DateTime): DateTime {
  const d = new Date(Date.UTC(dt.year, dt.month - 1, dt.day + 1));
  return { year: d.getUTCFullYear(), month: d.getUTCMonth() + 1, day: d.getUTCDate() };
}

function rrule(spec: DateSpec): string | null {
  const r = spec.recur;
  if (!r) return null;
  const freq = { d: 'DAILY', w: 'WEEKLY', m: 'MONTHLY', y: 'YEARLY' }[r.unit];
  let s = `RRULE:FREQ=${freq};INTERVAL=${r.n}`;
  if (r.unit === 'w' && r.weekdays.length) s += `;BYDAY=${r.weekdays.map((d) => ['MO', 'TU', 'WE', 'TH', 'FR', 'SA', 'SU'][d - 1]).join(',')}`;
  return s;
}

const esc = (s: string) => s.replace(/[\;,]/g, (c) => `\\${c}`).replace(/\r?\n/g, '\\n');

/** Fold content lines at 75 octets (approximately: by characters). */
const fold = (line: string) => line.match(/.{1,74}/gu)?.join('\r\n ') ?? line;

export interface CalendarDoc {
  id: string;
  title: string;
  tree: Tree;
}

export function buildCalendar(docs: CalendarDoc[], opts: { name?: string; itemUrl?: (doc: string, id: NodeId) => string; stamp?: number } = {}): string {
  const stamp = new Date(opts.stamp ?? Date.now()).toISOString().replace(/[-:]/g, '').replace(/\.\d+/, '');
  const lines = ['BEGIN:VCALENDAR', 'VERSION:2.0', 'PRODID:-//Outliner//Calendar feed//EN', 'CALSCALE:GREGORIAN', `X-WR-CALNAME:${esc(opts.name ?? 'Outliner')}`];
  for (const doc of docs) {
    const t = doc.tree;
    const walk = (id: NodeId) => {
      for (const c of t.get(id)?.children ?? []) {
        const n = t.get(c)!;
        walk(c);
        const src = typeof n.props.c === 'string' ? n.props.c : '';
        if (n.props.ck || !src.includes('!(')) continue;
        const dates = extractDates(src);
        // the summary is the item without its date chips
        let text = src;
        for (const d of [...dates].reverse()) text = text.slice(0, d.start) + text.slice(d.end);
        const summary = inlineToPlain(text).replace(/\s+/g, ' ').trim() || '(untitled)';
        dates.forEach((d, i) => {
          const s = d.spec;
          const allDay = s.start.hour === undefined;
          const end = s.end ?? (allDay ? s.start : undefined);
          lines.push('BEGIN:VEVENT', `UID:${doc.id}-${c}-${i}@outliner`, `DTSTAMP:${stamp}`, `DTSTART${icsDate(s.start)}`);
          if (end) lines.push(`DTEND${icsDate(allDay && end.hour === undefined ? nextDay(end) : end)}`);
          lines.push(fold(`SUMMARY:${esc(summary)}`));
          const note = typeof n.props.n === 'string' ? inlineToPlain(n.props.n) : '';
          const where = [doc.title, ...t.ancestors(c).filter((a) => a !== ROOT).map((a) => inlineToPlain(String(t.get(a)?.props.c ?? '')))].join(' › ');
          lines.push(fold(`DESCRIPTION:${esc([where, note].filter(Boolean).join('\n\n'))}`));
          if (opts.itemUrl) lines.push(fold(`URL:${opts.itemUrl(doc.id, c)}`));
          const rr = rrule(s);
          if (rr) lines.push(rr);
          lines.push('END:VEVENT');
        });
      }
    };
    walk(ROOT);
  }
  lines.push('END:VCALENDAR');
  return lines.join('\r\n') + '\r\n';
}
