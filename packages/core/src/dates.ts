// Date syntax `!(START[ - END][ | RECURRENCE])` — parsing, formatting,
// relative labels, overdue checks and recurrence advancement.
//
// All functions are pure: anything that depends on the current time takes
// `now` as a parameter. Calendar math happens in the runtime's local time zone
// unless an endpoint carries an explicit offset (`+09:00` / `Z`).

/** One endpoint. All-day when `hour` is undefined. `month` is 1-12. */
export interface DateTime {
  year: number;
  month: number;
  day: number;
  hour?: number;
  minute?: number;
  /** explicit UTC offset as written (`Z`, `+09:00`, `-0500`); only with a time */
  tz?: string;
}

export type RecurUnit = 'd' | 'w' | 'm' | 'y';

export interface Recurrence {
  /** interval, >= 1 */
  n: number;
  unit: RecurUnit;
  /** ISO weekdays 1 (Mon) .. 7 (Sun), sorted, only meaningful for `w` */
  weekdays: number[];
  /** `~` prefix: next occurrence is computed from the completion date */
  fromCompletion: boolean;
}

export interface DateSpec {
  start: DateTime;
  end?: DateTime;
  recur?: Recurrence;
}

export type TimeInput = number | Date;

const DAY = 86400000;
const toMs = (t: TimeInput): number => (typeof t === 'number' ? t : t.getTime());

// ---- parsing ---------------------------------------------------------------

const ENDPOINT_RE = /^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{1,2}):(\d{2})(?: ?(Z|[+-]\d{2}:?\d{2}))?)?$/;
const RECUR_RE = /^(~)?(\d*)([dwmy])([1-7]*)$/;

export function daysInMonth(year: number, month: number): number {
  return new Date(year, month, 0).getDate();
}

/** Parse `YYYY-MM-DD[ HH:mm[tz]]`. */
export function parseDateTime(s: string): DateTime | null {
  const m = ENDPOINT_RE.exec(s.trim());
  if (!m) return null;
  const year = +m[1]!;
  const month = +m[2]!;
  const day = +m[3]!;
  if (month < 1 || month > 12 || day < 1 || day > daysInMonth(year, month)) return null;
  const dt: DateTime = { year, month, day };
  if (m[4] !== undefined) {
    const hour = +m[4];
    const minute = +m[5]!;
    if (hour > 23 || minute > 59) return null;
    dt.hour = hour;
    dt.minute = minute;
    if (m[6]) dt.tz = m[6];
  }
  return dt;
}

/** Parse a recurrence like `1w135`, `~2d`, `m` (missing n = 1). */
export function parseRecurrence(s: string): Recurrence | null {
  const m = RECUR_RE.exec(s.trim());
  if (!m) return null;
  const n = m[2] ? +m[2] : 1;
  if (!(n >= 1)) return null;
  const unit = m[3] as RecurUnit;
  if (m[4] && unit !== 'w') return null;
  const weekdays = [...new Set([...m[4]!].map(Number))].sort((a, b) => a - b);
  return { n, unit, weekdays, fromCompletion: !!m[1] };
}

/** Parse the body of a date token (the text between `!(` and `)`). */
export function parseDateBody(body: string): DateSpec | null {
  const bar = body.indexOf('|');
  const main = bar < 0 ? body : body.slice(0, bar);
  let recur: Recurrence | undefined;
  if (bar >= 0) {
    const r = parseRecurrence(body.slice(bar + 1));
    if (!r) return null;
    recur = r;
  }
  const parts = main.trim().split(/\s+-\s+/);
  if (parts.length > 2) return null;
  const start = parseDateTime(parts[0]!);
  if (!start) return null;
  const spec: DateSpec = { start };
  if (parts.length === 2) {
    const end = parseDateTime(parts[1]!);
    if (!end) return null;
    spec.end = end;
  }
  if (recur) spec.recur = recur;
  return spec;
}

/** Parse a whole `!(…)` token; null if it isn't one. */
export function parseDateToken(src: string): DateSpec | null {
  const m = /^!\(([^()\n]*)\)$/.exec(src);
  return m ? parseDateBody(m[1]!) : null;
}

// ---- serialization -----------------------------------------------------------

const pad2 = (n: number) => String(n).padStart(2, '0');

export function formatDateTime(dt: DateTime): string {
  let s = `${String(dt.year).padStart(4, '0')}-${pad2(dt.month)}-${pad2(dt.day)}`;
  if (dt.hour !== undefined) {
    s += ` ${pad2(dt.hour)}:${pad2(dt.minute ?? 0)}`;
    if (dt.tz) s += dt.tz === 'Z' ? 'Z' : ` ${dt.tz}`;
  }
  return s;
}

export function formatRecurrence(r: Recurrence): string {
  return `${r.fromCompletion ? '~' : ''}${r.n}${r.unit}${r.unit === 'w' ? r.weekdays.join('') : ''}`;
}

/** Canonical body text (without `!(` `)`). */
export function formatDateBody(spec: DateSpec): string {
  let s = formatDateTime(spec.start);
  if (spec.end) s += ` - ${formatDateTime(spec.end)}`;
  if (spec.recur) s += ` | ${formatRecurrence(spec.recur)}`;
  return s;
}

export function formatDateToken(spec: DateSpec): string {
  return `!(${formatDateBody(spec)})`;
}

/** `!(YYYY-MM-DD)` for today, or `!(YYYY-MM-DD HH:mm)` with the current time. */
export function currentDateToken(now: TimeInput, withTime = false): string {
  return formatDateToken({ start: fromDate(new Date(toMs(now)), withTime) });
}

// ---- conversion ----------------------------------------------------------------

function tzOffsetMinutes(tz: string): number {
  if (tz === 'Z') return 0;
  const m = /^([+-])(\d{2}):?(\d{2})$/.exec(tz);
  if (!m) return 0;
  return (m[1] === '-' ? -1 : 1) * (+m[2]! * 60 + +m[3]!);
}

/** Instant of an endpoint (all-day = local midnight starting that day). */
export function toDate(dt: DateTime): Date {
  if (dt.hour !== undefined && dt.tz) {
    const utc = Date.UTC(dt.year, dt.month - 1, dt.day, dt.hour, dt.minute ?? 0);
    return new Date(utc - tzOffsetMinutes(dt.tz) * 60000);
  }
  return new Date(dt.year, dt.month - 1, dt.day, dt.hour ?? 0, dt.minute ?? 0);
}

export function fromDate(d: Date, withTime: boolean): DateTime {
  const dt: DateTime = { year: d.getFullYear(), month: d.getMonth() + 1, day: d.getDate() };
  if (withTime) {
    dt.hour = d.getHours();
    dt.minute = d.getMinutes();
  }
  return dt;
}

/** Endpoint as seen in local time (resolves explicit offsets). */
function localView(dt: DateTime): DateTime {
  if (dt.hour === undefined || !dt.tz) return dt;
  return fromDate(toDate(dt), true);
}

/** Local midnight at the start of the day containing `t`. */
export function startOfDay(t: TimeInput): number {
  const d = new Date(toMs(t));
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** Add calendar days in local time (DST-safe). */
export function addDays(t: TimeInput, n: number): number {
  const d = new Date(toMs(t));
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + n, d.getHours(), d.getMinutes(), d.getSeconds(), d.getMilliseconds()).getTime();
}

/** Calendar-day difference b - a (local days). */
export function dayDiff(a: TimeInput, b: TimeInput): number {
  return Math.round((startOfDay(b) - startOfDay(a)) / DAY);
}

/**
 * Time span covered by a date, as [start, end) in ms. All-day endpoints
 * cover the whole local day; a timed date without an end is an instant.
 */
export function dateRange(spec: DateSpec): { start: number; end: number } {
  const start = toDate(spec.start).getTime();
  const last = spec.end ?? spec.start;
  let end: number;
  if (last.hour === undefined) end = addDays(toDate(last).getTime(), 1);
  else end = toDate(last).getTime();
  return { start, end: Math.max(start, end) };
}

/** True if the date (its end, for ranges) lies in the past. */
export function isOverdue(spec: DateSpec, now: TimeInput): boolean {
  const last = spec.end ?? spec.start;
  const { end } = dateRange(spec);
  return last.hour === undefined ? end <= toMs(now) : end < toMs(now);
}

// ---- display -------------------------------------------------------------------

export const MONTH_NAMES = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
export const WEEKDAY_NAMES = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];

function ordinal(n: number): string {
  const v = n % 100;
  if (v >= 11 && v <= 13) return `${n}th`;
  return n + (['th', 'st', 'nd', 'rd'][n % 10] ?? 'th');
}

export interface DateFormatOptions {
  now: TimeInput;
  /** 12h clock (default) vs 24h */
  hour12?: boolean;
  /** custom format for dates with a time: moment-style pattern or `relative` */
  format?: string;
  /** custom format for all-day dates: moment-style pattern or `relative` */
  formatNoTime?: string;
}

/** "3 pm", "3:30 pm" (12h) or "15:30" (24h). */
export function formatTime(hour: number, minute: number, hour12 = true): string {
  if (!hour12) return `${pad2(hour)}:${pad2(minute)}`;
  const h = hour % 12 || 12;
  const ap = hour < 12 ? 'am' : 'pm';
  return minute ? `${h}:${pad2(minute)} ${ap}` : `${h} ${ap}`;
}

/**
 * Format a Date with a moment-style pattern. Supported tokens: YYYY YY MMMM
 * MMM MM M Do DD D dddd ddd HH H hh h mm A a; `[text]` is literal.
 */
export function formatPattern(d: Date, pattern: string): string {
  return pattern.replace(/\[([^\]]*)]|YYYY|YY|MMMM|MMM|MM|M|Do|DD|D|dddd|ddd|HH|H|hh|h|mm|A|a/g, (tok, lit) => {
    if (lit !== undefined) return lit;
    const h = d.getHours();
    switch (tok) {
      case 'YYYY': return String(d.getFullYear()).padStart(4, '0');
      case 'YY': return pad2(d.getFullYear() % 100);
      case 'MMMM': return MONTH_NAMES[d.getMonth()]!;
      case 'MMM': return MONTH_NAMES[d.getMonth()]!.slice(0, 3);
      case 'MM': return pad2(d.getMonth() + 1);
      case 'M': return String(d.getMonth() + 1);
      case 'Do': return ordinal(d.getDate());
      case 'DD': return pad2(d.getDate());
      case 'D': return String(d.getDate());
      case 'dddd': return WEEKDAY_NAMES[d.getDay()]!;
      case 'ddd': return WEEKDAY_NAMES[d.getDay()]!.slice(0, 3);
      case 'HH': return pad2(h);
      case 'H': return String(h);
      case 'hh': return pad2(h % 12 || 12);
      case 'h': return String(h % 12 || 12);
      case 'mm': return pad2(d.getMinutes());
      case 'A': return h < 12 ? 'AM' : 'PM';
      case 'a': return h < 12 ? 'am' : 'pm';
    }
    return tok;
  });
}

/** "Today", "Tomorrow", "In 3 days", "2 days ago" (|diff| < 7), else "Mon, January 3rd 2022". */
export function relativeDayLabel(dt: DateTime, now: TimeInput): string {
  const local = localView(dt);
  const d = new Date(local.year, local.month - 1, local.day);
  const diff = dayDiff(now, d);
  if (diff === 0) return 'Today';
  if (diff === 1) return 'Tomorrow';
  if (diff === -1) return 'Yesterday';
  if (diff > 1 && diff < 7) return `In ${diff} days`;
  if (diff < -1 && diff > -7) return `${-diff} days ago`;
  return formatPattern(d, 'ddd, MMMM Do YYYY');
}

function endpointLabel(dt: DateTime, o: DateFormatOptions, showDay = true): string {
  const local = localView(dt);
  const timed = local.hour !== undefined;
  const custom = timed ? o.format : o.formatNoTime;
  if (custom && custom !== 'relative') return formatPattern(toDate(local), custom);
  const time = timed ? formatTime(local.hour!, local.minute ?? 0, o.hour12 !== false) : '';
  if (!showDay) return time;
  const day = relativeDayLabel(local, o.now);
  return time ? `${day} ${time}` : day;
}

const sameDay = (a: DateTime, b: DateTime) => a.year === b.year && a.month === b.month && a.day === b.day;

/** Human label for a date chip ("Today 3 pm", "Tomorrow - In 3 days", "Today 3 pm - 5 pm"). */
export function formatDateLabel(spec: DateSpec, o: DateFormatOptions): string {
  const start = endpointLabel(spec.start, o);
  if (!spec.end) return start;
  const a = localView(spec.start);
  const b = localView(spec.end);
  const customEnd = b.hour !== undefined ? o.format : o.formatNoTime;
  const timeOnly = sameDay(a, b) && a.hour !== undefined && b.hour !== undefined && !(customEnd && customEnd !== 'relative');
  return `${start} - ${endpointLabel(spec.end, o, !timeOnly)}`;
}

const UNIT_WORDS: Record<RecurUnit, [string, string]> = {
  d: ['day', 'days'], w: ['week', 'weeks'], m: ['month', 'months'], y: ['year', 'years'],
};

/** "every day", "every 2 weeks on Mon, Wed", "every month (from completion)". */
export function describeRecurrence(r: Recurrence): string {
  const [one, many] = UNIT_WORDS[r.unit];
  let s = r.n === 1 ? `every ${one}` : `every ${r.n} ${many}`;
  if (r.unit === 'w' && r.weekdays.length) {
    s += ' on ' + r.weekdays.map((w) => WEEKDAY_NAMES[w % 7]!.slice(0, 3)).join(', ');
  }
  if (r.fromCompletion) s += ' (from completion)';
  return s;
}

// ---- recurrence --------------------------------------------------------------------

function shiftDate(dt: DateTime, days: number): DateTime {
  const d = new Date(dt.year, dt.month - 1, dt.day + days);
  return { ...dt, year: d.getFullYear(), month: d.getMonth() + 1, day: d.getDate() };
}

function addMonthsClamped(dt: DateTime, months: number): DateTime {
  const idx = dt.year * 12 + (dt.month - 1) + months;
  const year = Math.floor(idx / 12);
  const month = (idx % 12) + 1;
  return { ...dt, year, month, day: Math.min(dt.day, daysInMonth(year, month)) };
}

/** ISO weekday 1..7 of a calendar date. */
function isoWeekday(dt: DateTime): number {
  return new Date(dt.year, dt.month - 1, dt.day).getDay() || 7;
}

const dayNumber = (dt: DateTime) => Math.round(Date.UTC(dt.year, dt.month - 1, dt.day) / DAY);

function nextStart(base: DateTime, r: Recurrence): DateTime {
  switch (r.unit) {
    case 'd': return shiftDate(base, r.n);
    case 'm': return addMonthsClamped(base, r.n);
    case 'y': return addMonthsClamped(base, 12 * r.n);
    case 'w': {
      if (!r.weekdays.length) return shiftDate(base, 7 * r.n);
      // Weeks are Monday-based; eligible weeks are multiples of n from the base week.
      const baseMonday = dayNumber(base) - (isoWeekday(base) - 1);
      for (let i = 1; i <= 7 * r.n + 7; i++) {
        const cand = shiftDate(base, i);
        const week = Math.floor((dayNumber(cand) - baseMonday) / 7);
        if (week % r.n === 0 && r.weekdays.includes(isoWeekday(cand))) return cand;
      }
      return shiftDate(base, 7 * r.n);
    }
  }
}

/**
 * The date after checking off a recurring item: start advanced by one
 * interval (from today when `fromCompletion`), range length preserved, all-day
 * stays all-day, times of day kept. Null if the spec has no recurrence.
 */
export function nextOccurrence(spec: DateSpec, now: TimeInput): DateSpec | null {
  const r = spec.recur;
  if (!r) return null;
  let base: DateTime = spec.start;
  if (r.fromCompletion) {
    const today = fromDate(new Date(toMs(now)), false);
    base = { ...spec.start, year: today.year, month: today.month, day: today.day };
  }
  const start = nextStart(base, r);
  const out: DateSpec = { start, recur: r };
  if (spec.end) out.end = shiftDate(spec.end, dayNumber(start) - dayNumber(spec.start));
  return out;
}

export interface FoundDate {
  /** offset of `!` */
  start: number;
  /** offset after `)` */
  end: number;
  spec: DateSpec;
}

/** All valid `!(…)` dates in raw text (regex scan; see inline.extractDates for a syntax-aware scan). */
export function findDates(src: string): FoundDate[] {
  const out: FoundDate[] = [];
  const re = /!\(([^()\n]*)\)/g;
  for (let m; (m = re.exec(src)); ) {
    if (m.index > 0 && src[m.index - 1] === '\\') continue;
    const spec = parseDateBody(m[1]!);
    if (spec) out.push({ start: m.index, end: m.index + m[0].length, spec });
  }
  return out;
}

/**
 * If the first date in `src` recurs, return the text with that date advanced
 * to its next occurrence (and the new spec); otherwise null.
 */
export function advanceRecurrence(src: string, now: TimeInput): { text: string; spec: DateSpec } | null {
  const first = findDates(src)[0];
  if (!first?.spec.recur) return null;
  const spec = nextOccurrence(first.spec, now)!;
  return { text: src.slice(0, first.start) + formatDateToken(spec) + src.slice(first.end), spec };
}
