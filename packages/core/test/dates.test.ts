import { describe, expect, it } from 'vitest';
import {
  advanceRecurrence, currentDateToken, dateRange, describeRecurrence, findDates, formatDateBody, formatDateLabel,
  formatPattern, formatTime, fromNow, isOverdue, nextOccurrence, parseDateBody, parseDateToken, parseRecurrence,
  relativeDayLabel, type DateSpec,
} from '../src/dates.ts';

const at = (y: number, m: number, d: number, h = 0, mi = 0) => new Date(y, m - 1, d, h, mi).getTime();
const NOW = at(2026, 10, 22, 12, 0); // Thursday

describe('fromNow', () => {
  it('uses moment.js thresholds', () => {
    const m = 60000, h = 60 * m;
    expect(fromNow(30 * 1000)).toBe('in a few seconds');
    expect(fromNow(-60 * 1000)).toBe('a minute ago');
    expect(fromNow(29 * m)).toBe('in 29 minutes');
    expect(fromNow(-31 * m)).toBe('31 minutes ago');
    expect(fromNow(59 * m)).toBe('in an hour');
    expect(fromNow(89 * m)).toBe('in an hour');
    expect(fromNow(5 * h)).toBe('in 5 hours');
    expect(fromNow(21 * h)).toBe('in 21 hours');
    expect(fromNow(22 * h)).toBe('in a day');
    expect(fromNow(-3 * 24 * h)).toBe('3 days ago');
  });
});

describe('parse / format', () => {
  it('parses all-day, timed, ranges and recurrence', () => {
    expect(parseDateBody('2026-10-22')).toEqual({ start: { year: 2026, month: 10, day: 22 } });
    expect(parseDateBody('2026-10-22 09:05')).toEqual({ start: { year: 2026, month: 10, day: 22, hour: 9, minute: 5 } });
    expect(parseDateBody('2026-10-22 - 2026-10-25')).toEqual({
      start: { year: 2026, month: 10, day: 22 }, end: { year: 2026, month: 10, day: 25 },
    });
    expect(parseDateBody('2026-10-22 10:00 - 2026-10-22 11:30 | ~2w135')).toEqual({
      start: { year: 2026, month: 10, day: 22, hour: 10, minute: 0 },
      end: { year: 2026, month: 10, day: 22, hour: 11, minute: 30 },
      recur: { n: 2, unit: 'w', weekdays: [1, 3, 5], fromCompletion: true },
    });
    expect(parseDateBody('2026-10-22 10:00 +09:00')!.start.tz).toBe('+09:00');
  });

  it('recurrence defaults and validation', () => {
    expect(parseRecurrence('d')).toEqual({ n: 1, unit: 'd', weekdays: [], fromCompletion: false });
    expect(parseRecurrence('3m')).toMatchObject({ n: 3, unit: 'm' });
    expect(parseRecurrence('1w531')!.weekdays).toEqual([1, 3, 5]);
    expect(parseRecurrence('0d')).toBeNull();
    expect(parseRecurrence('1d12')).toBeNull();
    expect(parseRecurrence('1x')).toBeNull();
    expect(parseRecurrence('1w8')).toBeNull();
  });

  it('rejects invalid dates', () => {
    for (const s of ['2026-02-30', '2026-13-01', '2026-1-1', '2026-10-22 24:00', '2026-10-22 10:60', 'tomorrow', '2026-10-22 |', '2026-10-22 | 1q']) {
      expect(parseDateBody(s), s).toBeNull();
    }
    expect(parseDateBody('2024-02-29')).not.toBeNull();
    expect(parseDateToken('!(2026-10-22)')).not.toBeNull();
    expect(parseDateToken('(2026-10-22)')).toBeNull();
  });

  it('round-trips canonical bodies', () => {
    for (const s of ['2026-10-22', '2026-10-22 09:05', '2026-10-22 - 2026-10-25', '2026-10-22 10:00 - 2026-10-22 11:30 | ~2w135', '2026-01-31 | 1m', '2026-10-22 10:00 +09:00']) {
      expect(formatDateBody(parseDateBody(s)!)).toBe(s);
    }
    expect(formatDateBody(parseDateBody('2026-10-22|w')!)).toBe('2026-10-22 | 1w');
  });

  it('current date tokens', () => {
    expect(currentDateToken(at(2026, 3, 4, 7, 8))).toBe('!(2026-03-04)');
    expect(currentDateToken(at(2026, 3, 4, 7, 8), true)).toBe('!(2026-03-04 07:08)');
  });
});

describe('labels', () => {
  const lbl = (s: string, o: Partial<Parameters<typeof formatDateLabel>[1]> = {}) =>
    formatDateLabel(parseDateBody(s)!, { now: NOW, ...o });

  it('relative day labels', () => {
    expect(lbl('2026-10-22')).toBe('Today');
    expect(lbl('2026-10-23')).toBe('Tomorrow');
    expect(lbl('2026-10-21')).toBe('Yesterday');
    expect(lbl('2026-10-25')).toBe('In 3 days');
    expect(lbl('2026-10-28')).toBe('In 6 days');
    expect(lbl('2026-10-18')).toBe('4 days ago');
    expect(lbl('2026-10-29')).toBe('Thu, October 29th 2026');
    expect(lbl('2022-01-03')).toBe('Mon, January 3rd 2022');
    expect(lbl('2026-10-01')).toBe('Thu, October 1st 2026');
    expect(lbl('2026-10-02')).toBe('Fri, October 2nd 2026');
    expect(lbl('2026-10-11')).toBe('Sun, October 11th 2026');
  });

  it('time formats', () => {
    expect(formatTime(15, 0)).toBe('3 pm');
    expect(formatTime(15, 30)).toBe('3:30 pm');
    expect(formatTime(0, 5)).toBe('12:05 am');
    expect(formatTime(12, 0)).toBe('12 pm');
    expect(formatTime(15, 30, false)).toBe('15:30');
    expect(formatTime(9, 5, false)).toBe('09:05');
    // a timed date today reads relative to now (moment fromNow), other days keep the clock time
    expect(lbl('2026-10-22 15:30')).toBe('in 4 hours');
    expect(lbl('2026-10-23 15:30')).toBe('Tomorrow, 3:30 pm');
    expect(lbl('2026-10-23 15:30', { hour12: false })).toBe('Tomorrow, 15:30');
  });

  it('ranges', () => {
    expect(lbl('2026-10-22 15:00 - 2026-10-22 17:00')).toBe('Today, 3 pm - 5 pm');
    expect(lbl('2026-10-22 - 2026-10-25')).toBe('Today - In 3 days');
    expect(lbl('2026-10-22 15:00 - 2026-10-23 09:00')).toBe('Today, 3 pm - Tomorrow, 9 am');
  });

  it('custom formats', () => {
    expect(lbl('2026-10-22 15:04', { format: 'YYYY/MM/DD HH:mm' })).toBe('2026/10/22 15:04');
    expect(lbl('2026-10-22', { formatNoTime: 'D MMM YY [w]' })).toBe('22 Oct 26 w');
    expect(lbl('2026-10-22', { formatNoTime: 'relative' })).toBe('Today');
    expect(formatPattern(new Date(at(2026, 10, 22, 9, 7)), 'dddd h:mm A, Do')).toBe('Thursday 9:07 AM, 22nd');
  });

  it('timezone offsets resolve to local time', () => {
    const spec = parseDateBody('2026-10-22 12:00 Z')!;
    const d = new Date(Date.UTC(2026, 9, 22, 12, 0));
    expect(dateRange(spec).start).toBe(d.getTime());
    expect(relativeDayLabel(spec.start, d.getTime())).toBe('Today');
  });

  it('describes recurrence', () => {
    expect(describeRecurrence(parseRecurrence('1d')!)).toBe('every day');
    expect(describeRecurrence(parseRecurrence('2w13')!)).toBe('every 2 weeks on Mon, Wed');
    expect(describeRecurrence(parseRecurrence('1w7')!)).toBe('every week on Sun');
    expect(describeRecurrence(parseRecurrence('~3m')!)).toBe('every 3 months (from completion)');
    expect(describeRecurrence(parseRecurrence('y')!)).toBe('every year');
  });
});

describe('ranges / overdue', () => {
  it('all-day dates cover whole days', () => {
    expect(dateRange(parseDateBody('2026-10-22')!)).toEqual({ start: at(2026, 10, 22), end: at(2026, 10, 23) });
    expect(dateRange(parseDateBody('2026-10-22 - 2026-10-24')!)).toEqual({ start: at(2026, 10, 22), end: at(2026, 10, 25) });
    expect(dateRange(parseDateBody('2026-10-22 10:00')!)).toEqual({ start: at(2026, 10, 22, 10), end: at(2026, 10, 22, 10) });
  });

  it('overdue', () => {
    const od = (s: string) => isOverdue(parseDateBody(s)!, NOW);
    expect(od('2026-10-21')).toBe(true);
    expect(od('2026-10-22')).toBe(false);
    expect(od('2026-10-22 11:59')).toBe(true);
    expect(od('2026-10-22 12:01')).toBe(false);
    expect(od('2026-10-20 - 2026-10-23')).toBe(false);
    expect(od('2026-10-20 10:00 - 2026-10-22 11:00')).toBe(true);
  });
});

describe('recurrence', () => {
  const next = (s: string, now = NOW) => formatDateBody(nextOccurrence(parseDateBody(s)!, now)!);

  it('advances by unit', () => {
    expect(next('2026-10-22 | 1d')).toBe('2026-10-23 | 1d');
    expect(next('2026-10-22 | 3d')).toBe('2026-10-25 | 3d');
    expect(next('2026-10-22 | 2w')).toBe('2026-11-05 | 2w');
    expect(next('2026-10-22 | 1m')).toBe('2026-11-22 | 1m');
    expect(next('2026-01-31 | 1m')).toBe('2026-02-28 | 1m');
    expect(next('2026-12-15 | 1m')).toBe('2027-01-15 | 1m');
    expect(next('2024-02-29 | 1y')).toBe('2025-02-28 | 1y');
    expect(nextOccurrence(parseDateBody('2026-10-22')!, NOW)).toBeNull();
  });

  it('weekly on weekdays', () => {
    // 2026-10-22 is a Thursday (4)
    expect(next('2026-10-22 | 1w135')).toBe('2026-10-23 | 1w135'); // Fri
    expect(next('2026-10-23 | 1w135')).toBe('2026-10-26 | 1w135'); // Mon
    expect(next('2026-10-23 | 2w135')).toBe('2026-11-02 | 2w135'); // skip a week
    expect(next('2026-10-19 | 2w15')).toBe('2026-10-23 | 2w15'); // same week
    expect(next('2026-10-25 | 1w7')).toBe('2026-11-01 | 1w7'); // Sun → Sun
  });

  it('preserves spans and time of day', () => {
    expect(next('2026-10-22 - 2026-10-24 | 1w')).toBe('2026-10-29 - 2026-10-31 | 1w');
    expect(next('2026-10-22 10:00 - 2026-10-22 11:30 | 1d')).toBe('2026-10-23 10:00 - 2026-10-23 11:30 | 1d');
    expect(next('2026-10-22 23:00 - 2026-10-23 01:00 | 1d')).toBe('2026-10-23 23:00 - 2026-10-24 01:00 | 1d');
  });

  it('from completion uses today', () => {
    expect(next('2026-10-01 | ~1w')).toBe('2026-10-29 | ~1w');
    expect(next('2026-10-01 09:00 | ~1d')).toBe('2026-10-23 09:00 | ~1d');
    expect(next('2026-10-01 | 1w')).toBe('2026-10-08 | 1w');
  });

  it('advanceRecurrence rewrites the first date only if it recurs', () => {
    expect(advanceRecurrence('Pay !(2026-10-22 | 1m) and !(2026-11-01)', NOW)!.text).toBe('Pay !(2026-11-22 | 1m) and !(2026-11-01)');
    expect(advanceRecurrence('a !(2026-10-22) b !(2026-10-22 | 1d)', NOW)).toBeNull();
    expect(advanceRecurrence('nothing', NOW)).toBeNull();
  });

  it('findDates skips escaped and invalid tokens', () => {
    expect(findDates('\\!(2026-01-01) !(bad) !(2026-01-02)').map((d) => d.start)).toEqual([22]);
  });

  it('keeps the spec immutable', () => {
    const spec: DateSpec = parseDateBody('2026-10-22 | 1d')!;
    nextOccurrence(spec, NOW);
    expect(formatDateBody(spec)).toBe('2026-10-22 | 1d');
  });
});
