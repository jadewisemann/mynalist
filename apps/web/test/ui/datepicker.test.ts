import { describe, expect, it } from 'vitest';
import { pageMonth } from '../../src/ui/datepicker.ts';

const day = (year: number, month: number, d: number) => ({ year, month, day: d });

describe('date picker Page Up / Down', () => {
  it('moves the end with the start when there is no range', () => {
    // a date that is not a range keeps start == end: after Page Up, Done must not take the old end as the start
    const s = { start: day(2024, 6, 15), end: day(2024, 6, 15), range: false, picking: 'start' as const };
    expect(pageMonth(s, -1)).toEqual({ start: day(2024, 5, 15), end: day(2024, 5, 15) });
    expect(pageMonth(s, 1)).toEqual({ start: day(2024, 7, 15), end: day(2024, 7, 15) });
    // the shorter month clamps both
    expect(pageMonth({ ...s, start: day(2024, 1, 31), end: day(2024, 1, 31) }, 1)).toEqual({ start: day(2024, 2, 29), end: day(2024, 2, 29) });
  });

  it('moves only the end being picked in a range', () => {
    const r = { start: day(2024, 6, 10), end: day(2024, 6, 20), range: true, picking: 'end' as const };
    expect(pageMonth(r, 1)).toEqual({ start: day(2024, 6, 10), end: day(2024, 7, 20) });
    expect(pageMonth({ ...r, picking: 'start' }, -1)).toEqual({ start: day(2024, 5, 10), end: day(2024, 6, 20) });
  });
});
