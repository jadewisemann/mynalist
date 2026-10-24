import { describe, expect, it } from 'vitest';
import { ROOT, Tree, buildCalendar } from '../src/index.ts';

describe('calendar feed', () => {
  it('writes one event per date, all-day / timed / offset / recurring, skipping checked items', () => {
    const t = new Tree();
    const items: [string, string, Record<string, unknown>?][] = [
      ['a', 'Dentist !(2026-10-24 15:30) **bring card**', { n: 'room 2' }],
      ['b', 'Trip !(2026-11-01 - 2026-11-03)'],
      ['c', 'Standup !(2026-10-26 09:00 +09:00 | 1w135)'],
      ['d', 'Done !(2026-10-01)', { ck: true }],
      ['e', 'No date'],
    ];
    let after: string | null = null;
    for (const [id, c, extra] of items) {
      t.apply({ t: 'create', id, parent: ROOT, after, props: { c, ...extra }, ts: 1 });
      after = id;
    }
    const ics = buildCalendar([{ id: 'D', title: 'Plans', tree: t }], { stamp: Date.UTC(2026, 9, 24), itemUrl: (d, i) => `https://x/#d=${d}&z=${i}` });
    const events = ics.split('BEGIN:VEVENT').slice(1);
    expect(events.length).toBe(3);
    expect(events[0]).toContain('DTSTART:20261024T153000\r\n');
    expect(events[0]).toContain('SUMMARY:Dentist bring card');
    expect(events[0]).toContain('DESCRIPTION:Plans\\n\\nroom 2');
    expect(events[0]).toContain('URL:https://x/#d=D&z=a');
    expect(events[1]).toContain('DTSTART;VALUE=DATE:20261101');
    expect(events[1]).toContain('DTEND;VALUE=DATE:20261104');
    expect(events[2]).toContain('DTSTART:20261026T000000Z');
    expect(events[2]).toContain('RRULE:FREQ=WEEKLY;INTERVAL=1;BYDAY=MO,WE,FR');
    expect(ics.startsWith('BEGIN:VCALENDAR\r\n')).toBe(true);
  });
});

describe('withLocalTz', () => {
  it('adds the local offset to timed endpoints only', async () => {
    const { withLocalTz, localTz } = await import('../src/index.ts');
    const s = withLocalTz({ start: { year: 2026, month: 1, day: 2, hour: 9, minute: 0 }, end: { year: 2026, month: 1, day: 3 } });
    expect(s.start.tz).toBe(localTz(new Date(2026, 0, 2, 9, 0)));
    expect(s.end!.tz).toBeUndefined();
    expect(withLocalTz({ start: { year: 2026, month: 1, day: 2, hour: 9, tz: '+05:30' } }).start.tz).toBe('+05:30');
    expect(localTz(new Date())).toMatch(/^[+-]\d{2}:\d{2}$/);
  });
});
